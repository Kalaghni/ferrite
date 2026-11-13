//! Authoritative FERRITE server: game rules, bots and the UDP front end.
//! `Game` is pure simulation (no sockets) so it can be tested headlessly;
//! `net` wraps it in a 60 Hz loop. The client embeds this crate to host.

pub mod bots;
pub mod net;

use bots::Bot;
use ferrite_shared::map::{Nav, World};
use ferrite_shared::proto::*;
use ferrite_shared::rng::Rng;
use ferrite_shared::sim::{self, PState, GRENADE_WEAPON};
use ferrite_shared::{eligible, vec3, Vec3, DT, NEUTRAL, NUM_POINTS, POINT_RADIUS, TICK_HZ};

pub use net::{run, spawn, ServerHandle};

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub per_team: usize,
    pub seed: u64,
    pub tickets: i32,
    pub bot_skill: f32,
    pub log: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { port: DEFAULT_PORT, per_team: 12, seed: 0xFE221, tickets: 250, bot_skill: 1.0, log: false }
    }
}

const HIST: usize = 64;
const MAX_REWIND: f64 = 30.0;
const CAP_RATE: f32 = 0.1;
const BOT_NAMES: [&str; 32] = [
    "Anvil", "Brick", "Cinder", "Dross", "Echo", "Flint", "Gasket", "Hex", "Ingot", "Jolt", "Kiln", "Latch",
    "Mica", "Nickel", "Onyx", "Piston", "Quartz", "Rivet", "Slag", "Tongs", "Umber", "Vise", "Weld", "Xylo",
    "Yoke", "Zinc", "Bolt", "Crank", "Dynamo", "Ember", "Forge", "Girder",
];

pub struct Player {
    pub id: u8,
    pub name: String,
    pub team: u8,
    pub next_kit: u8,
    pub st: PState,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: f32,
    pub alive: bool,
    pub respawn_at: u32,
    pub last_dmg_tick: u32,
    pub last_seq: u32,
    pub last_cmd_tick: u32,
    pub credits: f32,
    pub ads: bool,
    pub kills: u16,
    pub deaths: u16,
    pub score: u32,
    pub bot: Option<Bot>,
    pub marked_until: u32,
    pub surge_until: u32,
    pub firing_until: u32,
    pub protect_until: u32,
    pub spawn_count: u8,
    pub spawn_yaw: f32,
    pub killer: u8,
    hist: [Vec3; HIST],
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub owner: u8,
    /// -1 = fully team 0, +1 = fully team 1.
    pub progress: f32,
    pub n: [u8; 2],
}

pub struct Grenade {
    pub id: u16,
    pub pos: Vec3,
    pub vel: Vec3,
    pub fuse: i32,
    pub owner: usize,
    pub team: u8,
}

#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub kills: u32,
    pub shots: u32,
    pub captures: u32,
    pub rounds: u32,
    pub dropped_cmds: u32,
}

pub struct Game {
    pub cfg: Config,
    pub world: World,
    pub nav: Nav,
    pub tick: u32,
    pub players: Vec<Player>,
    pub points: [Point; NUM_POINTS],
    pub tickets: [i32; 2],
    pub dmask: u64,
    crate_hp: Vec<f32>,
    pub grenades: Vec<Grenade>,
    next_gren: u16,
    pub fx: Vec<(u32, Fx)>,
    pub feed: Vec<Feed>,
    pub phase: u8,
    pub winner: u8,
    pub phase_until: u32,
    pub stats: Stats,
    /// Each team's current push target (bots mostly follow it).
    pub orders: [usize; 2],
    rng: Rng,
}

fn initial_points() -> [Point; NUM_POINTS] {
    let mut p = [Point { owner: NEUTRAL, progress: 0.0, n: [0; 2] }; NUM_POINTS];
    p[0] = Point { owner: 0, progress: -1.0, n: [0; 2] };
    p[NUM_POINTS - 1] = Point { owner: 1, progress: 1.0, n: [0; 2] };
    p
}

impl Game {
    pub fn new(cfg: Config) -> Game {
        let world = World::new(cfg.seed);
        let nav = Nav::build(&world);
        let mut g = Game {
            world,
            nav,
            tick: 0,
            players: Vec::new(),
            points: initial_points(),
            tickets: [cfg.tickets; 2],
            dmask: 0,
            crate_hp: Vec::new(),
            grenades: Vec::new(),
            next_gren: 0,
            fx: Vec::new(),
            feed: Vec::new(),
            phase: 0,
            winner: NEUTRAL,
            phase_until: 0,
            stats: Stats::default(),
            orders: [1, 3],
            rng: Rng::new(cfg.seed ^ 0x5EED),
            cfg,
        };
        g.crate_hp = vec![110.0; g.world.n_crates];
        let n = (g.cfg.per_team * 2).min(64);
        for i in 0..n {
            let team = (i % 2) as u8;
            let mut p = Player {
                id: i as u8,
                name: String::new(),
                team,
                next_kit: 0,
                st: PState::spawn(Vec3::ZERO, 0),
                yaw: 0.0,
                pitch: 0.0,
                hp: 0.0,
                alive: false,
                respawn_at: 0,
                last_dmg_tick: 0,
                last_seq: 0,
                last_cmd_tick: 0,
                credits: 0.0,
                ads: false,
                kills: 0,
                deaths: 0,
                score: 0,
                bot: None,
                marked_until: 0,
                surge_until: 0,
                firing_until: 0,
                protect_until: 0,
                spawn_count: 0,
                spawn_yaw: 0.0,
                killer: SPECTATOR_ID,
                hist: [Vec3::ZERO; HIST],
            };
            g.make_bot(&mut p);
            g.players.push(p);
        }
        for i in 0..n {
            g.spawn_player(i);
        }
        g
    }

    fn make_bot(&mut self, p: &mut Player) {
        let i = p.id as usize;
        p.name = BOT_NAMES[i % BOT_NAMES.len()].to_string();
        p.next_kit = [0, 0, 1, 2][(i / 2) % 4];
        let skill = self.cfg.bot_skill * self.rng.range(0.7, 1.15);
        p.bot = Some(Bot::new(self.cfg.seed ^ (i as u64 * 977 + 13), skill, i));
        p.kills = 0;
        p.deaths = 0;
        p.score = 0;
        p.last_seq = 0;
    }

    /// A human takes over a bot slot on the team with fewer humans.
    pub fn join(&mut self, name: &str) -> Option<usize> {
        let humans = |t: u8, g: &Game| g.players.iter().filter(|p| p.team == t && p.bot.is_none()).count();
        let team = if humans(1, self) < humans(0, self) { 1 } else { 0 };
        let slot = self
            .players
            .iter()
            .position(|p| p.team == team && p.bot.is_some())
            .or_else(|| self.players.iter().position(|p| p.bot.is_some()))?;
        let clean: String = name.chars().filter(|c| c.is_ascii_graphic()).take(14).collect();
        let t = self.tick;
        let p = &mut self.players[slot];
        p.bot = None;
        p.name = if clean.is_empty() { format!("Player{}", slot) } else { clean };
        p.kills = 0;
        p.deaths = 0;
        p.score = 0;
        p.last_seq = 0;
        p.last_cmd_tick = t;
        p.next_kit = 0;
        p.alive = false;
        p.respawn_at = t + 20;
        Some(slot)
    }

    pub fn leave(&mut self, i: usize) {
        if i >= self.players.len() || self.players[i].bot.is_some() {
            return;
        }
        let mut p = std::mem::replace(&mut self.players[i], dummy());
        self.make_bot(&mut p);
        self.players[i] = p;
    }

    pub fn owners(&self) -> [u8; NUM_POINTS] {
        let mut o = [NEUTRAL; NUM_POINTS];
        for (i, p) in self.points.iter().enumerate() {
            o[i] = p.owner;
        }
        o
    }

    pub fn owned(&self, team: u8) -> i32 {
        self.points.iter().filter(|p| p.owner == team).count() as i32
    }

    fn push_fx(&mut self, fx: Fx) {
        self.fx.push((self.tick, fx));
    }

    fn pos_at(&self, j: usize, tick_f: f64) -> Vec3 {
        let p = &self.players[j];
        let now = self.tick as f64;
        if tick_f >= now {
            return p.st.pos;
        }
        let t = tick_f.max(now - MAX_REWIND).max(0.0);
        let i0 = t.floor() as usize;
        let f = (t - i0 as f64) as f32;
        let a = p.hist[i0 % HIST];
        let b = if i0 + 1 >= self.tick as usize { p.st.pos } else { p.hist[(i0 + 1) % HIST] };
        a + (b - a) * f
    }

    /// Run one client command. Every input is validated here: sequence
    /// must advance, the per-tick budget bounds how much simulated time a
    /// client can claim, and angles/axes are clamped. Position is never
    /// accepted from the network.
    pub fn apply_cmd(&mut self, i: usize, mut cmd: Cmd) {
        let t = self.tick;
        {
            let p = &mut self.players[i];
            if cmd.seq <= p.last_seq {
                return;
            }
            p.last_seq = cmd.seq;
            p.last_cmd_tick = t;
            if p.credits < 1.0 {
                self.stats.dropped_cmds += 1;
                return;
            }
            p.credits -= 1.0;
            if !cmd.yaw.is_finite() || !cmd.pitch.is_finite() || !cmd.interp.is_finite() {
                return;
            }
            cmd.pitch = cmd.pitch.clamp(-1.55, 1.55);
            cmd.mx = cmd.mx.clamp(-1, 1);
            cmd.mz = cmd.mz.clamp(-1, 1);
            p.yaw = cmd.yaw;
            p.pitch = cmd.pitch;
            p.ads = cmd.buttons & btn::ADS != 0;
            if !p.alive {
                return;
            }
        }
        let out = sim::step(&mut self.players[i].st, &cmd, &self.world, self.dmask);
        if out.fired {
            self.fire(i, &cmd);
        }
        if out.threw {
            let p = &self.players[i];
            let dir = sim::view_dir(cmd.yaw, cmd.pitch);
            let g = Grenade {
                id: self.next_gren,
                pos: p.st.pos + vec3(0.0, sim::eye_h(p.st.crouch), 0.0) + dir * 0.5,
                vel: dir * 19.0 + vec3(0.0, 4.0, 0.0) + p.st.vel * 0.5,
                fuse: 132,
                owner: i,
                team: p.team,
            };
            self.next_gren = self.next_gren.wrapping_add(1);
            self.grenades.push(g);
            self.push_fx(Fx::Throw { who: i as u8 });
        }
    }

    fn fire(&mut self, i: usize, cmd: &Cmd) {
        let t = self.tick;
        let (team, kit, origin, hspeed, airborne, human, crouch) = {
            let p = &self.players[i];
            let v = p.st.vel;
            (p.team, sim::wid(&p.st), p.st.pos + vec3(0.0, sim::eye_h(p.st.crouch), 0.0), (v.x * v.x + v.z * v.z).sqrt(), !p.st.on_ground, p.bot.is_none(), p.st.crouch)
        };
        let wd = sim::weapon_def(kit);
        let ads = cmd.buttons & btn::ADS != 0;
        // Lag compensation: judge the shot against where targets were on
        // the shooter's screen, bounded to half a second of history.
        let rewind = if human { cmd.interp.clamp(t as f64 - MAX_REWIND, t as f64) } else { t as f64 };
        let targets: Vec<(usize, Vec3, bool)> = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.alive && p.team != team && p.protect_until <= t)
            .map(|(j, p)| (j, self.pos_at(j, rewind), p.st.crouch))
            .collect();
        self.players[i].firing_until = t + 90;
        self.players[i].protect_until = 0;
        self.stats.shots += 1;
        for d in sim::shot_dirs(kit, cmd.seq, cmd.yaw, cmd.pitch, ads, hspeed, airborne, crouch) {
            let wh = self.world.raycast(origin, d, 300.0, self.dmask);
            let mut best_t = wh.map_or(300.0, |h| h.t);
            let mut hit_player = None;
            for &(j, pos, tc) in &targets {
                if let Some((tt, head)) = sim::ray_player(origin, d, pos, tc) {
                    if tt < best_t {
                        best_t = tt;
                        hit_player = Some((j, head));
                    }
                }
            }
            let end = origin + d * best_t;
            let mut kind = 0;
            if let Some((j, head)) = hit_player {
                kind = 2;
                let mut dmg = wd.damage * sim::falloff(kit, best_t);
                if head {
                    dmg *= wd.head_mult;
                }
                if kit == 2 {
                    self.players[j].marked_until = t + 6 * TICK_HZ;
                }
                self.damage(j, Some(i), dmg, head, kit);
            } else if let Some(h) = wh {
                kind = 1;
                if let Some(bi) = h.block {
                    self.hit_block(bi, wd.damage);
                }
            }
            self.push_fx(Fx::Shot { who: i as u8, from: origin.to_array(), to: end.to_array(), hit: kind });
        }
    }

    fn hit_block(&mut self, bi: usize, dmg: f32) {
        if let Some(d) = self.world.blocks[bi].dest {
            let d = d as usize;
            if self.dmask & (1 << d) != 0 {
                return;
            }
            self.crate_hp[d] -= dmg;
            if self.crate_hp[d] <= 0.0 {
                self.dmask |= 1 << d;
                let b = &self.world.blocks[bi];
                let c = (b.min + b.max) * 0.5;
                self.push_fx(Fx::CrateBreak { pos: [c.x, b.max.y - 0.5, c.z] });
            }
        }
    }

    pub fn damage(&mut self, j: usize, attacker: Option<usize>, dmg: f32, head: bool, weapon: u8) {
        let t = self.tick;
        if !self.players[j].alive {
            return;
        }
        let p = &mut self.players[j];
        p.hp -= dmg;
        p.last_dmg_tick = t;
        let kill = p.hp <= 0.0;
        if let (Some(b), Some(a)) = (p.bot.as_mut(), attacker) {
            b.alert(a, t);
        }
        if let Some(a) = attacker {
            self.push_fx(Fx::Hit { attacker: a as u8, victim: j as u8, dmg: dmg.min(255.0) as u8, head, kill });
        }
        if kill {
            self.kill(j, attacker, weapon, head);
        }
    }

    fn kill(&mut self, j: usize, attacker: Option<usize>, weapon: u8, head: bool) {
        let t = self.tick;
        let team = self.players[j].team;
        // Rubber band: the more ground a team holds, the slower it reinforces.
        // Reinforcements arrive in waves so pushes land together.
        let period = ((5.0 + 1.5 * self.owned(team) as f32) * TICK_HZ as f32) as u32;
        let phase = if team == 1 { period / 2 } else { 0 };
        let earliest = t + 2 * TICK_HZ + period - 1;
        let wave = (earliest - phase.min(earliest)) / period * period + phase;
        let p = &mut self.players[j];
        p.alive = false;
        p.hp = 0.0;
        p.deaths += 1;
        p.respawn_at = wave.max(t + 2 * TICK_HZ);
        p.killer = attacker.map_or(SPECTATOR_ID, |a| a as u8);
        let mut killer_id = j as u8;
        if let Some(a) = attacker {
            if a != j {
                self.players[a].kills += 1;
                self.players[a].score += 100;
                killer_id = a as u8;
            }
        }
        self.stats.kills += 1;
        if self.phase == 0 {
            self.tickets[team as usize] -= 1;
        }
        self.feed.push(Feed::Kill { killer: killer_id, victim: j as u8, weapon, head });
    }

    fn spawn_player(&mut self, i: usize) {
        let t = self.tick;
        let team = self.players[i].team;
        let mut anchor = self.world.bases[team as usize];
        let mut at_base = true;
        let order: Vec<usize> = if team == 0 { (0..NUM_POINTS).rev().collect() } else { (0..NUM_POINTS).collect() };
        for k in order {
            if self.points[k].owner != team {
                continue;
            }
            let c = self.world.points[k];
            let threatened = self.players.iter().any(|p| p.alive && p.team != team && (p.st.pos - c).length() < 16.0);
            if !threatened && self.points[k].progress.abs() > 0.6 {
                anchor = c;
                at_base = false;
                break;
            }
        }
        let mut pos = anchor;
        for _ in 0..30 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let r = if at_base { self.rng.range(0.0, 8.0) } else { self.rng.range(3.0, 8.5) };
            let (x, z) = (anchor.x + a.cos() * r, anchor.z + a.sin() * r);
            if !self.nav.is_blocked(x, z) {
                pos = vec3(x, self.world.height(x, z) + 0.05, z);
                break;
            }
        }
        let p = &mut self.players[i];
        p.st = PState::spawn(pos, p.next_kit);
        p.hp = sim::kit(p.next_kit).hp;
        p.alive = true;
        p.protect_until = t + 2 * TICK_HZ;
        p.marked_until = 0;
        p.spawn_count = p.spawn_count.wrapping_add(1);
        p.spawn_yaw = if team == 0 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 };
        p.yaw = p.spawn_yaw;
        p.pitch = 0.0;
        p.hist = [pos; HIST];
        if let Some(b) = p.bot.as_mut() {
            b.on_spawn(p.spawn_yaw);
        }
    }

    fn explode(&mut self, pos: Vec3, owner: usize, team: u8) {
        self.push_fx(Fx::Boom { pos: pos.to_array() });
        for bi in 0..self.world.blocks.len() {
            let b = &self.world.blocks[bi];
            if b.dest.is_some() && ((b.min + b.max) * 0.5 - pos).length() < 5.5 {
                self.hit_block(bi, 1000.0);
            }
        }
        let src = pos + vec3(0.0, 0.3, 0.0);
        for j in 0..self.players.len() {
            let p = &self.players[j];
            if !p.alive || (p.team == team && j != owner) || p.protect_until > self.tick {
                continue;
            }
            let chest = p.st.pos + vec3(0.0, if p.st.crouch { 0.55 } else { 0.9 }, 0.0);
            let d = (chest - src).length();
            if d > 8.0 || !self.world.los(src, chest, self.dmask) {
                continue;
            }
            let mut dmg = 140.0 * (1.0 - d / 8.0);
            if j == owner {
                dmg *= 0.5;
            }
            self.damage(j, Some(owner), dmg, false, GRENADE_WEAPON);
        }
    }

    fn update_grenades(&mut self) {
        let mut boom = Vec::new();
        for g in self.grenades.iter_mut() {
            g.vel.y -= 20.0 * DT;
            let step = g.vel * DT;
            let len = step.length();
            if len > 1e-5 {
                let d = step / len;
                match self.world.raycast(g.pos, d, len + 0.06, self.dmask) {
                    Some(h) => {
                        g.pos += d * (h.t - 0.06).max(0.0);
                        g.vel = (g.vel - 2.0 * g.vel.dot(h.normal) * h.normal) * 0.42;
                        if g.vel.length() < 1.2 {
                            g.vel = Vec3::ZERO;
                        }
                    }
                    None => g.pos += step,
                }
            }
            g.fuse -= 1;
            if g.fuse <= 0 {
                boom.push((g.pos, g.owner, g.team));
            }
        }
        self.grenades.retain(|g| g.fuse > 0);
        for (pos, owner, team) in boom {
            self.explode(pos, owner, team);
        }
    }

    fn update_points(&mut self) {
        let t = self.tick;
        for k in 0..NUM_POINTS {
            let c = self.world.points[k];
            let mut n = [0u8; 2];
            for p in self.players.iter().filter(|p| p.alive) {
                let d = p.st.pos - c;
                if d.x * d.x + d.z * d.z < POINT_RADIUS * POINT_RADIUS && d.y.abs() < 7.0 {
                    n[p.team as usize] += 1;
                }
            }
            let owners = self.owners();
            let pt = &mut self.points[k];
            pt.n = n;
            let diff = n[1] as i32 - n[0] as i32;
            if diff != 0 {
                let team = if diff > 0 { 1u8 } else { 0 };
                let dir = if team == 1 { 1.0 } else { -1.0 };
                if pt.owner == team || eligible(&owners, team, k) {
                    let rate = CAP_RATE * (diff.abs().min(3) as f32);
                    pt.progress = (pt.progress + dir * rate * DT).clamp(-1.0, 1.0);
                }
            } else if n[0] == 0 {
                let target = match pt.owner {
                    0 => -1.0,
                    1 => 1.0,
                    _ => 0.0,
                };
                let step = 0.04 * DT;
                pt.progress += (target - pt.progress).clamp(-step, step);
            }
            let sign = match pt.owner {
                0 => -1.0,
                1 => 1.0,
                _ => 0.0,
            };
            if pt.owner != NEUTRAL && pt.progress * sign <= 0.0 {
                let by = 1 - pt.owner;
                pt.owner = NEUTRAL;
                self.feed.push(Feed::Neutralized { point: k as u8, by });
            }
            let pt = &mut self.points[k];
            if pt.owner == NEUTRAL && pt.progress.abs() >= 1.0 {
                let team = if pt.progress > 0.0 { 1u8 } else { 0 };
                pt.owner = team;
                self.stats.captures += 1;
                self.feed.push(Feed::Capture { point: k as u8, team });
                // Surge: the capturing team gets a short burst of momentum.
                for p in self.players.iter_mut().filter(|p| p.team == team && p.alive) {
                    p.surge_until = t + 8 * TICK_HZ;
                    let d = p.st.pos - c;
                    if d.x * d.x + d.z * d.z < POINT_RADIUS * POINT_RADIUS {
                        p.score += 200;
                    }
                }
            }
        }
    }

    /// Pick one point per team to mass on: a capturable point, preferring
    /// the one the enemy is defending most thinly.
    fn update_orders(&mut self) {
        let owners = self.owners();
        for team in 0..2u8 {
            let mut best: Option<(usize, f32)> = None;
            for k in 0..NUM_POINTS {
                if self.points[k].owner == team || !eligible(&owners, team, k) {
                    continue;
                }
                let c = self.world.points[k];
                let defenders = self.players.iter().filter(|p| p.alive && p.team != team && (p.st.pos - c).length() < 22.0).count();
                let score = defenders as f32 + self.rng.range(0.0, 2.5);
                if best.map_or(true, |(_, s)| score < s) {
                    best = Some((k, score));
                }
            }
            if let Some((k, _)) = best {
                self.orders[team as usize] = k;
            }
        }
    }

    fn reset_round(&mut self) {
        self.points = initial_points();
        self.tickets = [self.cfg.tickets; 2];
        self.dmask = 0;
        self.crate_hp = vec![110.0; self.world.n_crates];
        self.grenades.clear();
        self.phase = 0;
        self.winner = NEUTRAL;
        for p in self.players.iter_mut() {
            p.kills = 0;
            p.deaths = 0;
            p.score = 0;
            p.surge_until = 0;
        }
        for i in 0..self.players.len() {
            self.spawn_player(i);
        }
        self.feed.push(Feed::MatchStart);
    }

    /// Advance the world by one 60 Hz tick.
    pub fn tick(&mut self) {
        self.tick += 1;
        let t = self.tick;
        for p in self.players.iter_mut() {
            p.credits = (p.credits + 1.0).min(12.0);
            p.st.surge = t < p.surge_until;
        }
        for i in 0..self.players.len() {
            if let Some(mut bot) = self.players[i].bot.take() {
                let cmd = bots::think(self, i, &mut bot);
                self.players[i].bot = Some(bot);
                self.apply_cmd(i, cmd);
            } else if self.players[i].alive && t.saturating_sub(self.players[i].last_cmd_tick) > 12 {
                // Silent client: keep physics honest (gravity, no input).
                let p = &mut self.players[i];
                let c = Cmd { yaw: p.yaw, pitch: p.pitch, ..Default::default() };
                sim::step(&mut p.st, &c, &self.world, self.dmask);
            }
        }
        for i in 0..self.players.len() {
            let p = &mut self.players[i];
            p.hist[t as usize % HIST] = p.st.pos;
            if p.alive {
                let max = sim::kit(p.st.kit).hp;
                if p.hp < max && t - p.last_dmg_tick > 5 * TICK_HZ {
                    p.hp = (p.hp + 18.0 * DT).min(max);
                }
            } else if t >= p.respawn_at {
                self.spawn_player(i);
            }
        }
        self.update_grenades();
        self.update_points();
        if t % (12 * TICK_HZ) == 1 {
            self.update_orders();
        }
        if self.phase == 0 {
            if t % (4 * TICK_HZ) == 0 {
                let diff = self.owned(0) - self.owned(1);
                let mult = if diff.abs() == NUM_POINTS as i32 { 3 } else { 1 };
                if diff > 0 {
                    self.tickets[1] -= diff * mult;
                } else {
                    self.tickets[0] += diff * mult;
                }
            }
            if self.tickets[0] <= 0 || self.tickets[1] <= 0 {
                self.winner = if self.tickets[0] > self.tickets[1] {
                    0
                } else if self.tickets[1] > self.tickets[0] {
                    1
                } else if self.owned(0) >= self.owned(1) {
                    0
                } else {
                    1
                };
                self.tickets[0] = self.tickets[0].max(0);
                self.tickets[1] = self.tickets[1].max(0);
                self.phase = 1;
                self.phase_until = t + 12 * TICK_HZ;
                self.stats.rounds += 1;
                self.feed.push(Feed::MatchEnd { winner: self.winner });
            }
        } else if t >= self.phase_until {
            self.reset_round();
        }
        let cutoff = t.saturating_sub(20);
        self.fx.retain(|(ft, _)| *ft >= cutoff);
    }

    pub fn snapshot(&self, me: Option<usize>, ack_feed: u32, fx_after: u32) -> Snapshot {
        let t = self.tick;
        let players = self
            .players
            .iter()
            .filter(|p| p.alive)
            .map(|p| {
                let mut flags = 0;
                if t < p.firing_until {
                    flags |= flag::FIRING;
                }
                if p.st.surge {
                    flags |= flag::SURGE;
                }
                if t < p.marked_until {
                    flags |= flag::MARKED;
                }
                if p.ads {
                    flags |= flag::ADS;
                }
                if p.st.on_ground {
                    flags |= flag::GROUND;
                }
                if t < p.protect_until {
                    flags |= flag::SHIELD;
                }
                if p.st.crouch {
                    flags |= flag::CROUCH;
                }
                if p.st.slot == 1 {
                    flags |= flag::SIDEARM;
                }
                NetPlayer {
                    id: p.id,
                    pos: p.st.pos.to_array(),
                    yaw: NetPlayer::enc_yaw(p.yaw),
                    pitch: NetPlayer::enc_pitch(p.pitch),
                    flags,
                    hp: p.hp.clamp(0.0, 255.0).ceil() as u8,
                }
            })
            .collect();
        let mut points = [PointNet::default(); 5];
        for (i, p) in self.points.iter().enumerate() {
            points[i] = PointNet { owner: p.owner, progress: (p.progress * 100.0) as i8, n: p.n };
        }
        let mut fx: Vec<Fx> = Vec::new();
        for (ft, f) in &self.fx {
            if *ft <= fx_after {
                continue;
            }
            // Hit confirmations are private to the two players involved.
            if let Fx::Hit { attacker, victim, .. } = f {
                let m = me.map_or(SPECTATOR_ID, |m| m as u8);
                if *attacker != m && *victim != m {
                    continue;
                }
            }
            if fx.len() < 56 {
                fx.push(*f);
            }
        }
        let feed = self
            .feed
            .iter()
            .enumerate()
            .skip(ack_feed as usize)
            .take(10)
            .map(|(i, f)| (i as u32 + 1, *f))
            .collect();
        let (ack_seq, me_state) = match me {
            Some(i) => {
                let p = &self.players[i];
                (
                    p.last_seq,
                    Some(SelfState {
                        st: p.st,
                        hp: p.hp.clamp(0.0, 255.0).ceil() as u8,
                        alive: p.alive,
                        respawn_in: p.respawn_at.saturating_sub(t) as f32 / TICK_HZ as f32,
                        spawn_count: p.spawn_count,
                        spawn_yaw: p.spawn_yaw,
                        killer: p.killer,
                    }),
                )
            }
            None => (0, None),
        };
        Snapshot {
            tick: t,
            ack_seq,
            me: me_state,
            players,
            points,
            tickets: [self.tickets[0].max(0) as u16, self.tickets[1].max(0) as u16],
            dmask: self.dmask,
            grenades: self.grenades.iter().map(|g| NetGrenade { id: g.id, pos: g.pos.to_array(), team: g.team }).collect(),
            fx,
            feed,
            phase: self.phase,
            winner: self.winner,
            phase_secs: self.phase_until.saturating_sub(t) as f32 / TICK_HZ as f32,
        }
    }

    pub fn roster(&self) -> Vec<RosterEntry> {
        self.players
            .iter()
            .map(|p| RosterEntry {
                id: p.id,
                name: p.name.clone(),
                team: p.team,
                kit: if p.alive { p.st.kit } else { p.next_kit },
                kills: p.kills,
                deaths: p.deaths,
                score: p.score,
                bot: p.bot.is_some(),
            })
            .collect()
    }
}

fn dummy() -> Player {
    Player {
        id: 0,
        name: String::new(),
        team: 0,
        next_kit: 0,
        st: PState::spawn(Vec3::ZERO, 0),
        yaw: 0.0,
        pitch: 0.0,
        hp: 0.0,
        alive: false,
        respawn_at: 0,
        last_dmg_tick: 0,
        last_seq: 0,
        last_cmd_tick: 0,
        credits: 0.0,
        ads: false,
        kills: 0,
        deaths: 0,
        score: 0,
        bot: None,
        marked_until: 0,
        surge_until: 0,
        firing_until: 0,
        protect_until: 0,
        spawn_count: 0,
        spawn_yaw: 0.0,
        killer: SPECTATOR_ID,
        hist: [Vec3::ZERO; HIST],
    }
}
