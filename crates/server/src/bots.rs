//! Bot brains. A bot only produces a `Cmd`, exactly like a human client,
//! and goes through the same validation and simulation path.

use crate::Game;
use ferrite_shared::proto::pickup;
use ferrite_shared::proto::{btn, btn2, Cmd};
use ferrite_shared::rng::Rng;
use ferrite_shared::sim;
use ferrite_shared::{eligible, mode, vec2, vec3, wrap_angle, Vec2, Vec3, FLAG_POINT, NUM_POINTS};

pub struct Bot {
    rng: Rng,
    skill: f32,
    defender: bool,
    /// Falls back when badly hurt instead of fighting to the death.
    cautious: bool,
    lane: usize,
    goal: usize,
    goal_timer: u32,
    target: Option<usize>,
    last_seen: u32,
    last_seen_pos: Vec3,
    acquire_at: u32,
    track_since: u32,
    yaw: f32,
    pitch: f32,
    err: Vec2,
    err_timer: u32,
    strafe: f32,
    strafe_timer: u32,
    burst: i32,
    phase: u32,
    stuck_ref: Vec3,
    stuck_timer: u32,
    unstuck: u32,
    unstuck_dir: i8,
    wander: Vec3,
    wander_timer: u32,
    alert: Option<usize>,
    alert_until: u32,
    seq: u32,
    /// Where a lost target was last seen, and until when to go and look.
    hunt_pos: Vec3,
    hunt_until: u32,
    /// Ticks spent waiting for the squad before an assault.
    regroup: u32,
    /// Ticks spent pushing against something without moving.
    slow: u32,
}

impl Bot {
    pub fn new(seed: u64, skill: f32, idx: usize) -> Bot {
        let mut rng = Rng::new(seed);
        Bot {
            defender: rng.chance(0.3),
            cautious: rng.chance(0.55),
            lane: (idx / 2) % 3,
            rng,
            skill: skill.max(0.2),
            goal: 2,
            goal_timer: 0,
            target: None,
            last_seen: 0,
            last_seen_pos: Vec3::ZERO,
            acquire_at: 0,
            track_since: 0,
            yaw: 0.0,
            pitch: 0.0,
            err: Vec2::ZERO,
            err_timer: 0,
            strafe: 1.0,
            strafe_timer: 0,
            burst: 0,
            phase: idx as u32 * 3,
            stuck_ref: Vec3::ZERO,
            stuck_timer: 0,
            unstuck: 0,
            unstuck_dir: 1,
            wander: Vec3::ZERO,
            wander_timer: 0,
            alert: None,
            alert_until: 0,
            seq: 0,
            hunt_pos: Vec3::ZERO,
            hunt_until: 0,
            regroup: 0,
            slow: 0,
        }
    }

    pub fn alert(&mut self, attacker: usize, tick: u32) {
        self.alert = Some(attacker);
        self.alert_until = tick + 120;
    }

    /// A teammate nearby was hit: look that way unless already busy.
    pub fn alert_soft(&mut self, attacker: usize, tick: u32) {
        if self.alert.is_none() && self.target.is_none() {
            self.alert = Some(attacker);
            self.alert_until = tick + 80;
        }
    }

    pub fn on_spawn(&mut self, yaw: f32) {
        self.hunt_until = 0;
        self.regroup = 0;
        self.slow = 0;
        self.yaw = yaw;
        self.pitch = 0.0;
        self.target = None;
        self.goal_timer = 0;
        self.wander_timer = 0;
        self.unstuck = 0;
        self.lane = self.rng.below(3) as usize;
    }
}

fn choose_goal(g: &Game, team: u8, pos: Vec3, b: &mut Bot) -> usize {
    match g.mode {
        // No objective: mostly the middle, sometimes their side or ours.
        mode::TDM => {
            let (theirs, ours) = if team == 0 { (NUM_POINTS - 2, 1) } else { (1, NUM_POINTS - 2) };
            let r = b.rng.f32();
            return if r < 0.55 {
                NUM_POINTS / 2
            } else if r < 0.85 {
                theirs
            } else {
                ours
            };
        }
        mode::KOTH => return NUM_POINTS / 2,
        // A few stay by their own flag; the rest go for the other.
        mode::CTF => {
            let home = b.defender && b.rng.chance(0.45);
            return FLAG_POINT[if home { team as usize % 2 } else { 1 - team as usize % 2 }];
        }
        _ => {}
    }
    let owners = g.owners();
    // Most attackers follow the team's push order so assaults arrive massed.
    let order = g.orders[team as usize];
    if !b.defender && g.points[order].owner != team && eligible(&owners, team, order) && b.rng.chance(0.8) {
        return order;
    }
    let mut cands: Vec<(usize, f32)> = Vec::new();
    for k in 0..NUM_POINTS {
        let pt = &g.points[k];
        let mut w = 0.0;
        if pt.owner != team && eligible(&owners, team, k) {
            w = if b.defender { 1.2 } else { 3.0 };
        } else if pt.owner == team {
            let threatened = pt.progress.abs() < 0.98 || pt.n[1 - team as usize] > 0;
            let front = eligible(&owners, 1 - team, k);
            if threatened {
                w = if b.defender { 5.0 } else { 2.5 };
            } else if front {
                w = if b.defender { 2.5 } else { 0.4 };
            }
        }
        if w > 0.0 {
            let d = (g.world.points[k] - pos).length();
            cands.push((k, w / (1.0 + d / 60.0)));
        }
    }
    let total: f32 = cands.iter().map(|c| c.1).sum();
    if total <= 0.0 {
        return if team == 0 { NUM_POINTS - 1 } else { 0 };
    }
    let mut r = b.rng.f32() * total;
    for (k, w) in &cands {
        r -= w;
        if r <= 0.0 {
            return *k;
        }
    }
    cands[0].0
}

pub fn think(g: &Game, i: usize, b: &mut Bot) -> Cmd {
    let me = &g.players[i];
    b.seq += 1;
    let t = g.tick;
    let mut cmd = Cmd { seq: b.seq, interp: t as f64, yaw: b.yaw, pitch: b.pitch, ..Default::default() };
    if !me.alive {
        b.target = None;
        return cmd;
    }
    let pos = me.st.pos;
    let eye = pos + vec3(0.0, sim::eye_h(me.st.crouch), 0.0);
    // How the bot fights follows the family of its primary: 0 rifle,
    // 1 shotgun, 2 long rifle.
    let kit = sim::weapon_def(me.st.prim).arch;
    let wd = sim::weapon_def(sim::wid(&me.st));
    let side = me.st.slot == 1;
    if b.alert.is_some() && t > b.alert_until {
        b.alert = None;
    }

    // --- Perception (staggered across bots, ~7 Hz) ---
    if (t + b.phase).is_multiple_of(8) {
        let fwd = sim::view_dir(b.yaw, b.pitch);
        let maxd = if kit == 2 { 140.0 } else { 90.0 };
        let mut best: Option<(usize, f32, bool)> = None;
        for (j, p) in g.players.iter().enumerate() {
            if !p.alive || p.team == me.team {
                continue;
            }
            let th = if p.st.crouch { 0.6 } else { 1.0 };
            let to = p.st.pos + vec3(0.0, th, 0.0) - eye;
            let d = to.length();
            if d > maxd {
                continue;
            }
            // Sight is a ~120 degree cone. Outside it a bot only knows what it
            // can hear: gunfire, sprinting close by, or someone on top of it.
            // Walking or crouching up behind a bot is silent.
            let in_view = to.dot(fwd) / d.max(0.01) > 0.5;
            let hs = vec2(p.st.vel.x, p.st.vel.z).length();
            let sprinting = !p.st.crouch && hs > sim::WALK * 1.2;
            let heard = d < 1.8 || (sprinting && d < 9.0) || (t < p.firing_until && d < 30.0);
            let aware = in_view || heard || b.alert == Some(j) || b.target == Some(j) || t < p.marked_until;
            let chest = p.st.pos + vec3(0.0, th + 0.3, 0.0);
            if !aware || !g.world.los(eye, chest, g.dmask) {
                continue;
            }
            // Leaves hide a soldier as well as a wall does, unless they give
            // themselves away with a muzzle flash where the bot is looking.
            let flash = in_view && t < p.firing_until;
            if !flash && d > 2.5 && g.world.foliage(eye, chest) {
                continue;
            }
            let mut score = d;
            // Whoever has our flag comes first.
            if g.mode == mode::CTF && g.flags[me.team as usize % 2].state == 1 && g.flags[me.team as usize % 2].carrier == j {
                score -= 25.0;
            }
            if b.target == Some(j) {
                score -= 12.0;
            }
            if b.alert == Some(j) {
                score -= 8.0;
            }
            if best.is_none_or(|(_, s, _)| score < s) {
                best = Some((j, score, in_view));
            }
        }
        match best {
            Some((j, score, in_view)) => {
                if b.target != Some(j) {
                    b.target = Some(j);
                    // Reaction time: slower at range, much slower when the
                    // threat is outside the bot's view and it has to work
                    // out where it is.
                    let mut react = 20.0 + b.rng.range(0.0, 22.0) + score.max(0.0) * 0.2;
                    if !in_view {
                        react += b.rng.range(30.0, 55.0);
                    }
                    b.acquire_at = t + (react / b.skill) as u32;
                    b.track_since = t;
                    b.err_timer = 0;
                }
                b.last_seen = t;
                b.last_seen_pos = g.players[j].st.pos;
            }
            None => {
                if b.target.is_some() && t - b.last_seen > 50 {
                    b.target = None;
                    // Attackers go and look where the enemy disappeared.
                    if !b.defender && kit != 2 && (b.last_seen_pos - pos).length() < 32.0 {
                        b.hunt_pos = b.last_seen_pos;
                        b.hunt_until = t + 170;
                    }
                }
            }
        }
    }
    if let Some(j) = b.target {
        if !g.players[j].alive {
            b.target = None;
        }
    }

    // --- Objective ---
    if b.goal_timer == 0 {
        b.regroup = 0;
        b.goal = choose_goal(g, me.team, pos, b);
        b.goal_timer = 180 + b.rng.below(240);
    } else {
        b.goal_timer -= 1;
    }
    // Carrying their flag: nothing matters but getting it home.
    let (mine, theirs) = (me.team as usize % 2, 1 - me.team as usize % 2);
    let carrying = g.mode == mode::CTF && g.flags[theirs].state == 1 && g.flags[theirs].carrier == i;
    if carrying {
        b.goal = FLAG_POINT[mine];
    }
    let gpos = g.world.points[b.goal];
    let dgoal = vec2(gpos.x - pos.x, gpos.z - pos.z).length();
    let mut mv = Vec2::ZERO;
    if dgoal > 7.0 {
        // Underground, the flow field (a surface map) is turned along the tunnel.
        mv = g.world.tunnel_steer(pos, g.nav.dir(b.goal, b.lane, pos.x, pos.z));
    } else {
        let to = vec2(b.wander.x - pos.x, b.wander.z - pos.z);
        if b.wander_timer == 0 || to.length() < 1.0 {
            let a = b.rng.range(0.0, std::f32::consts::TAU);
            let r = b.rng.range(1.0, 7.5);
            let (x, z) = (gpos.x + a.cos() * r, gpos.z + a.sin() * r);
            if !g.nav.is_blocked(x, z) {
                b.wander = vec3(x, 0.0, z);
                b.wander_timer = 100 + b.rng.below(260);
            }
        } else {
            b.wander_timer -= 1;
            if to.length() > 0.8 {
                mv = to.normalize();
            }
        }
    }

    // --- Things worth walking straight to ---
    let mut detour: Option<Vec3> = None;
    if g.mode == mode::CTF {
        let near = |p: Vec3, r: f32| (p - pos).length() < r;
        let (own, enemy) = (&g.flags[mine], &g.flags[theirs]);
        if carrying && dgoal < 14.0 {
            detour = Some(gpos); // the last steps onto our own stand
        } else if own.state == 2 && near(own.pos, 45.0) {
            detour = Some(own.pos); // ours is lying there: take it back
        } else if (enemy.state == 2 && near(enemy.pos, 45.0)) || (enemy.state == 0 && near(enemy.pos, 14.0) && !b.defender) {
            detour = Some(enemy.pos); // theirs, dropped or close by at home
        } else if own.state == 1 && near(own.pos, 40.0) {
            detour = Some(own.pos); // run the carrier down
        }
    }
    if detour.is_none() && b.target.is_none() {
        let max = sim::kit(me.st.kit).hp;
        detour = g
            .pickups
            .iter()
            .filter(|p| match p.kind {
                pickup::HEALTH => me.hp < max,
                pickup::ARMOR => me.armor < 100.0,
                _ => true,
            })
            .map(|p| p.pos)
            .filter(|p| (*p - pos).length() < 14.0 && (p.y - pos.y).abs() < 2.0)
            .min_by(|a, c| (*a - pos).length().total_cmp(&(*c - pos).length()));
    }
    if let Some(d) = detour {
        let to = vec2(d.x - pos.x, d.z - pos.z);
        if to.length() > 0.3 && !g.world.danger(d, 0.3) {
            mv = to.normalize();
        }
    }
    let route = mv;

    // --- Combat ---
    let mut want_yaw = b.yaw;
    let mut want_pitch = 0.0;
    let mut fire = false;
    let mut ads = false;
    let mut engaged = false;
    let mut crouch = false;
    let mut melee = false;
    if let Some(j) = b.target {
        let tp = &g.players[j];
        let visible = t - b.last_seen < 12;
        let aim_pt = if visible { tp.st.pos } else { b.last_seen_pos } + vec3(0.0, if tp.st.crouch { 0.6 } else { 1.0 }, 0.0);
        let to = aim_pt - eye;
        let d = to.length().max(0.01);
        if b.err_timer == 0 {
            // Aim error: large on acquisition, settles while tracking,
            // worse against fast targets and while the bot itself moves.
            let tracking = (t - b.track_since) as f32 / 60.0;
            let settle = (1.0 - tracking / 2.5).max(0.45);
            let tv = tp.st.vel.length();
            let sv = me.st.vel.length();
            let sigma = 0.075 / b.skill * settle * (1.0 + tv / 6.0) * (1.0 + sv / 10.0);
            let a = b.rng.range(0.0, std::f32::consts::TAU);
            let r = b.rng.f32().sqrt() * sigma;
            b.err = vec2(a.cos() * r, a.sin() * r * 0.6);
            b.err_timer = 8 + b.rng.below(10);
        } else {
            b.err_timer -= 1;
        }
        want_yaw = to.x.atan2(to.z) + b.err.x;
        want_pitch = (to.y / d).clamp(-1.0, 1.0).asin() + b.err.y;
        if visible {
            engaged = true;
            let toh = vec2(to.x, to.z).normalize_or_zero();
            // Fight at the range the weapon is good at.
            let ideal = (sim::weapon_def(me.st.prim).falloff.0 * 0.7).clamp(8.0, 60.0);
            if b.strafe_timer == 0 {
                b.strafe = if b.rng.chance(0.18) { 0.0 } else { b.rng.sign() };
                b.strafe_timer = 25 + b.rng.below(50);
            } else {
                b.strafe_timer -= 1;
            }
            let right = vec2(-b.yaw.cos(), b.yaw.sin());
            if kit == 1 {
                if d > 6.0 && d < 30.0 {
                    mv = toh + right * b.strafe * 0.5;
                } else if d <= 6.0 {
                    mv = right * b.strafe;
                }
            } else if !b.defender && dgoal > 7.0 && d > 14.0 {
                // Attackers keep pressing toward the objective under fire.
                mv += right * b.strafe * 0.6;
            } else if d < ideal * 1.3 || dgoal <= 7.0 {
                mv = right * b.strafe;
                if kit == 2 && d > 20.0 {
                    mv = Vec2::ZERO;
                }
                if d < ideal * 0.4 {
                    mv -= toh * 0.8;
                }
            }
            // Caught reloading: back away and keep moving.
            if me.st.reload > 0 && kit != 2 {
                mv = right * (if b.strafe == 0.0 { 1.0 } else { b.strafe }) - toh * 0.7;
            }
            ads = kit != 1 && d > 28.0;
            // Close enough to touch: hit them.
            melee = d < 1.9 && t >= b.acquire_at;
            // Holding still in a firefight: drop to a knee for a steadier shot.
            crouch = mv.length_squared() < 0.01 && d > 15.0;
            if t >= b.acquire_at {
                if kit == 0 {
                    if b.burst > 0 {
                        fire = true;
                        b.burst -= 1;
                        if b.burst == 0 {
                            b.burst = -(6 + b.rng.below(14) as i32);
                        }
                    } else if b.burst < 0 {
                        b.burst += 1;
                    } else {
                        b.burst = 14 + b.rng.below(30) as i32;
                    }
                } else {
                    fire = true;
                }
            }
        }
    } else if let Some(a) = b.alert {
        let to = g.players[a].st.pos - pos;
        want_yaw = to.x.atan2(to.z);
    } else if t < b.hunt_until {
        // Check the spot the last target vanished at.
        let to = vec2(b.hunt_pos.x - pos.x, b.hunt_pos.z - pos.z);
        if to.length() < 3.0 {
            b.hunt_until = 0;
        } else {
            mv = to.normalize();
            want_yaw = mv.x.atan2(mv.y);
        }
    } else if mv.length_squared() > 0.01 {
        want_yaw = mv.x.atan2(mv.y);
    }

    // The carrier does not stop to trade shots.
    if carrying && route.length_squared() > 0.01 {
        mv = route;
    }

    // --- Squad sense ---
    let enemy = 1 - me.team as usize;
    let assaulting = matches!(g.mode, mode::CONQUEST | mode::KOTH)
        && !engaged
        && b.target.is_none()
        && t >= b.hunt_until
        && g.points[b.goal].owner != me.team;
    let mut holding = false;
    if assaulting && kit == 2 && (7.0..26.0).contains(&dgoal) && g.points[b.goal].n[enemy] > 0 {
        // Long rifles cover the assault from outside the point.
        holding = true;
    } else if assaulting && !b.defender && (15.0..34.0).contains(&dgoal) && b.regroup < 150 {
        // Do not walk onto a point alone if friends are close behind.
        let (mut near, mut coming) = (0, 0);
        for p in g.players.iter().filter(|p| p.alive && p.team == me.team && p.id != me.id) {
            let d = (p.st.pos - pos).length();
            if d < 14.0 {
                near += 1;
            } else if d < 45.0 && (p.st.pos - gpos).length() > dgoal {
                coming += 1;
            }
        }
        if near < 2 && coming > 0 {
            b.regroup += 1;
            holding = true;
        }
    }
    if dgoal > 44.0 {
        b.regroup = 0;
    }
    if holding {
        mv = Vec2::ZERO;
        crouch = true;
        want_yaw = (gpos.x - pos.x).atan2(gpos.z - pos.z);
    }

    // --- Self-preservation ---
    let mut fleeing = false;
    for gr in &g.grenades {
        if gr.team == me.team && gr.owner != i {
            continue; // friendly frags are harmless
        }
        let away = vec2(pos.x - gr.pos.x, pos.z - gr.pos.z);
        if gr.fuse < 95 && away.length() < 8.5 && (gr.pos.y - pos.y).abs() < 4.0 {
            mv = away.normalize_or_zero();
            if mv == Vec2::ZERO {
                mv = vec2(b.strafe.max(0.5), 0.0);
            }
            fleeing = true;
            crouch = false;
            fire = false;
            if b.target.is_none() {
                want_yaw = mv.x.atan2(mv.y);
            }
            break;
        }
    }
    if !fleeing && engaged && b.cautious && me.hp < sim::kit(me.st.kit).hp * 0.35 {
        // Badly hurt: give ground toward friendly territory, still shooting.
        let back = g.world.tunnel_steer(pos, g.nav.dir(5 + me.team as usize, b.lane, pos.x, pos.z));
        mv = back + vec2(-b.yaw.cos(), b.yaw.sin()) * b.strafe * 0.5;
        crouch = false;
    }

    // Never walk, strafe or back-pedal into lava or flood water. The flow
    // field already takes the bridges over the casting channels; the
    // runner's lava and the flood come and go, so a bot that meets them
    // waits on the edge (or, caught in them, keeps going to get out).
    if mv.length_squared() > 0.01 && !g.world.danger(pos, 0.0) {
        let ahead = vec2(pos.x, pos.z) + mv.normalize() * 1.4;
        let feet = vec3(ahead.x, pos.y.max(g.world.height(ahead.x, ahead.y)), ahead.y);
        if g.world.danger(feet, 0.3) && (engaged || !g.world.lava_fixed(ahead.x, ahead.y, 0.3)) {
            mv = Vec2::ZERO;
        }
    }

    // --- Turn toward the desired view ---
    let dy = wrap_angle(want_yaw - b.yaw);
    let dp = want_pitch - b.pitch;
    let rate = if b.target.is_some() { 0.17 } else { 0.12 };
    b.yaw = wrap_angle(b.yaw + (dy * rate).clamp(-0.11, 0.11));
    b.pitch = (b.pitch + (dp * rate).clamp(-0.1, 0.1)).clamp(-1.4, 1.4);
    if fire && (dy.abs() > 0.07 || dp.abs() > 0.07) {
        fire = false;
    }
    cmd.yaw = b.yaw;
    cmd.pitch = b.pitch;

    // --- Movement relative to view ---
    let (sy, cy) = b.yaw.sin_cos();
    let f = mv.dot(vec2(sy, cy));
    let r = mv.dot(vec2(-cy, sy));
    cmd.mz = if f > 0.35 {
        1
    } else if f < -0.35 {
        -1
    } else {
        0
    };
    cmd.mx = if r > 0.35 {
        1
    } else if r < -0.35 {
        -1
    } else {
        0
    };
    if ((!engaged && dgoal > 14.0) || fleeing || carrying) && cmd.mz > 0 {
        if carrying {
            fire = false; // run
        }
        cmd.buttons |= btn::SPRINT;
    }
    if wd.semi && !t.is_multiple_of(3) {
        fire = false; // semi-auto: pull the trigger, don't hold it
    }
    if fire {
        cmd.buttons |= btn::FIRE;
    }
    if melee && dy.abs() < 0.5 {
        cmd.more |= btn2::MELEE;
    }
    // Dry in a firefight: draw the sidearm instead of reloading in the open.
    if (engaged && !side && me.st.ammo == 0 && me.st.ammo_alt > 0) || (!engaged && side) {
        cmd.buttons |= btn::SWAP;
    }
    if ads {
        cmd.buttons |= btn::ADS;
    }
    if crouch {
        cmd.buttons |= btn::CROUCH;
    }
    if !engaged && me.st.ammo < wd.mag / 2 {
        cmd.buttons |= btn::RELOAD;
    }
    if engaged && me.st.grenades > 0 && me.st.gren_cd == 0 {
        if let Some(j) = b.target {
            let d = (g.players[j].st.pos - pos).length();
            // More tempting the more enemies are bunched up; never onto friends.
            let tp = g.players[j].st.pos;
            let around = |team: u8, r: f32| g.players.iter().filter(|p| p.alive && p.team == team && (p.st.pos - tp).length() < r).count();
            let bunch = around(1 - me.team, 6.0) as f32;
            let safe = around(me.team, 7.5) == 0;
            if safe && (12.0..34.0).contains(&d) && b.rng.chance(0.003 * bunch * bunch) {
                cmd.buttons = (cmd.buttons | btn::GRENADE) & !btn::FIRE;
                cmd.pitch = (b.pitch + 0.16 + d * 0.006).min(1.2);
            }
        }
    }

    // --- Obstacles: hop or climb what is in the way before backing off ---
    let hs = vec2(me.st.vel.x, me.st.vel.z).length();
    if cmd.mz > 0 && !engaged && me.st.on_ground && hs < 1.2 {
        b.slow += 1;
    } else if hs > 2.0 || cmd.mz <= 0 {
        b.slow = 0;
    }
    if b.slow > 8 {
        cmd.buttons |= btn::JUMP;
        cmd.buttons &= !btn::CROUCH;
    }

    // --- Unstick ---
    b.stuck_timer += 1;
    if b.stuck_timer >= 40 {
        let moved = vec2(pos.x - b.stuck_ref.x, pos.z - b.stuck_ref.z).length();
        if moved < 0.5 && mv.length_squared() > 0.01 && !engaged {
            b.unstuck = 34;
            b.unstuck_dir = if b.rng.chance(0.5) { 1 } else { -1 };
        }
        b.stuck_ref = pos;
        b.stuck_timer = 0;
    }
    if b.unstuck > 0 {
        b.unstuck -= 1;
        cmd.buttons |= btn::JUMP;
        cmd.buttons &= !(btn::SPRINT | btn::CROUCH);
        cmd.mx = b.unstuck_dir;
        cmd.mz = if b.unstuck > 20 { -1 } else { 1 };
    }
    cmd
}
