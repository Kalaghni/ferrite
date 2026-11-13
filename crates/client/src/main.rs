//! FERRITE client. Opens on a home screen to join a server or host a
//! match on the embedded server; `--connect` and `--host` skip it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod fx;
mod gfx;
mod hud;
mod menu;
mod net;
mod scene;

use audio::{Audio, Sfx};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use ferrite_shared::proto::*;
use ferrite_shared::sim::{self, PState};
use ferrite_shared::{DT, NEUTRAL, POINT_NAMES, TEAM_NAMES};
use macroquad::prelude as mq;
use net::Net;
use scene::PlayerView;
use std::collections::VecDeque;
use std::net::ToSocketAddrs;

pub struct Args {
    connect: Option<String>,
    team: u8,
    host: bool,
    menu_shot: Option<String>,
    pick_team: bool,
    name: String,
    per_team: usize,
    port: u16,
    /// Map to host (index into the shared map table).
    map: Option<u8>,
    seed: u64,
    tickets: i32,
    skill: f32,
    spectate: bool,
    auto: bool,
    shot: Option<String>,
    shot_at: f64,
    quit_at: Option<f64>,
    volume: f32,
    sight: Option<u8>,
    kit: u8,
    /// Side of the sun shadow map in texels; 0 turns shadows off.
    shadows: u32,
    /// Verification aid: freeze the first-person reload animation here.
    vm_reload: Option<f32>,
}

fn parse_args() -> Args {
    let mut a = Args {
        connect: None,
        team: 2,
        host: false,
        menu_shot: None,
        pick_team: false,
        name: std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "Player".into()),
        per_team: 12,
        port: DEFAULT_PORT,
        map: None,
        seed: 0xFE221,
        tickets: 250,
        skill: 1.0,
        spectate: false,
        auto: false,
        shot: None,
        shot_at: 6.0,
        quit_at: None,
        volume: 0.7,
        sight: None,
        kit: 0,
        shadows: 4096,
        vm_reload: None,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned().unwrap_or_default();
        let mut used = 2;
        match args[i].as_str() {
            "--connect" => a.connect = Some(val),
            "--name" => a.name = val,
            "--bots" => a.per_team = val.parse().expect("--bots <players per team>"),
            "--port" => a.port = val.parse().expect("--port <u16>"),
            "--seed" => a.seed = val.parse().expect("--seed <u64>"),
            "--map" => match ferrite_shared::map::map_by_name(&val) {
                Some(m) => a.map = Some(m),
                None => {
                    let names: Vec<String> = ferrite_shared::map::MAPS.iter().map(|m| m.name.to_lowercase()).collect();
                    eprintln!("unknown map '{val}'. Maps: {}", names.join(", "));
                    std::process::exit(2);
                }
            },
            "--tickets" => a.tickets = val.parse().expect("--tickets <n>"),
            "--skill" => a.skill = val.parse().expect("--skill <f32>"),
            "--volume" => a.volume = val.parse().expect("--volume <0..1>"),
            "--sight" => a.sight = Some(val.parse().expect("--sight <0..5>")),
            "--kit" => a.kit = val.parse::<u8>().expect("--kit <1..3>").clamp(1, 3) - 1,
            "--shot" => a.shot = Some(val),
            "--shot-at" => a.shot_at = val.parse().expect("--shot-at <secs>"),
            "--quit-at" => a.quit_at = Some(val.parse().expect("--quit-at <secs>")),
            "--menu-shot" => a.menu_shot = Some(val),
            "--team" => {
                a.team = match val.to_ascii_lowercase().as_str() {
                    "ember" | "0" => 0,
                    "cobalt" | "1" => 1,
                    _ => 2,
                }
            }
            "--pick-team" => {
                a.pick_team = true;
                used = 1;
            }
            "--host" => {
                a.host = true;
                used = 1;
            }
            "--vm-reload" => a.vm_reload = val.parse().ok(),
            "--shadows" => {
                a.shadows = match val.as_str() {
                    "off" | "0" => 0,
                    "low" => 2048,
                    _ => 4096,
                }
            }
            "--spectate" => {
                a.spectate = true;
                used = 1;
            }
            "--auto" => {
                a.auto = true;
                used = 1;
            }
            "--fullscreen" => used = 1,
            "-h" | "--help" => {
                println!("ferrite [--connect host[:port]] [--host] [--team ember|cobalt|auto] [--name N] [--map NAME] [--bots 12] [--port 27500] [--seed N] [--tickets 250] [--skill 1.0]\n        [--volume 0.7] [--shadows high|low|off] [--fullscreen] [--spectate] [--auto] [--shot file.png --shot-at SECS] [--quit-at SECS]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other} (try --help)");
                std::process::exit(2);
            }
        }
        i += used;
    }
    a
}

fn window_conf() -> mq::Conf {
    mq::Conf {
        window_title: "FERRITE".to_string(),
        window_width: 1280,
        window_height: 720,
        fullscreen: std::env::args().any(|a| a == "--fullscreen"),
        sample_count: 4,
        high_dpi: false,
        ..Default::default()
    }
}

pub struct KillLine {
    pub killer: String,
    pub kteam: u8,
    pub victim: String,
    pub vteam: u8,
    pub weapon: &'static str,
    pub head: bool,
    pub mine: bool,
    pub at: f64,
}

#[derive(Default)]
pub struct Stats {
    pub frames: u64,
    pub frame_sum: f64,
    pub frame_max: f64,
    pub corrections: u64,
    pub replays: u64,
    pub max_err: f32,
    pub cmds: u64,
    pub my_kills: u32,
    pub my_deaths: u32,
}

pub struct App {
    pub world: World,
    pub net: Net,
    pub my_id: u8,
    pub my_team: u8,
    pub spectator: bool,
    pub auto: bool,
    pub st: PState,
    prev_pos: Vec3,
    pub alive: bool,
    pub hp: u8,
    pub yaw: f32,
    pub pitch: f32,
    seq: u32,
    pending: VecDeque<Cmd>,
    smooth: Vec3,
    spawn_count: u8,
    pub respawn_in: f32,
    pub killer: u8,
    pub kit_choice: u8,
    step_y: f32,
    pub eye: f32,
    jump_latch: bool,
    gren_latch: bool,
    swap_latch: bool,
    reload_latch: bool,
    pub captured: bool,
    pub sens: f32,
    last_mouse: (f32, f32),
    pub ads: f32,
    pub fov: f32,
    recoil: f32,
    /// Recoil climb and drift added to the aim; settles back off the trigger.
    pub recoil_p: f32,
    pub recoil_y: f32,
    last_fire: f64,
    shot_seq: u32,
    land: f32,
    mantling: bool,
    mantle_vis: f32,
    slide_vis: f32,
    air_vis: f32,
    strafe_vis: f32,
    view_lit: f32,
    /// Rounds snapping past: darkens the edges of the screen for a moment.
    pub suppress: f32,
    whizz_at: f64,
    trail_acc: f32,
    bob: f32,
    flash: f32,
    sprint_vis: f32,
    stride: f32,
    shake: f32,
    pub hitmarker: f32,
    pub hit_head: bool,
    pub hit_kill: f32,
    pub dmg_flash: f32,
    pub dmg_dirs: Vec<(Vec3, f64)>,
    pub feed: Vec<KillLine>,
    pub banner: Option<(String, [f32; 3], f64)>,
    pub sub_banner: Option<(String, f64)>,
    pub dmask: u64,
    pub fx: fx::Fx,
    pub audio: Audio,
    walk: [f32; 64],
    // Smoothed animation state for other players.
    rvel: [(f32, f32); 64],
    rcrouch: [f32; 64],
    rair: [f32; 64],
    rslide: [f32; 64],
    rreload: [f32; 64],
    rfire: [f32; 64],
    rhurt: [f32; 64],
    rhp: [u8; 64],
    rstep: [f32; 64],
    last_view: [PlayerView; 64],
    /// The fallen: pose at death and when it happened.
    corpses: Vec<(PlayerView, f64)>,
    pub last_pos: [Vec3; 64],
    last_seen: [f64; 64],
    last_shot_snd: [f64; 64],
    pub views: Vec<PlayerView>,
    pub spec_target: u8,
    spec_since: f64,
    death_pos: Vec3,
    pub show_net: bool,
    /// The in-match team picker is open.
    pub team_pick: bool,
    /// Chosen sight per weapon id.
    pub sights: [u8; 5],
    pub stats: Stats,
    pub hosting: Option<String>,
    pub start: f64,
}

impl App {
    pub fn name(&self, id: u8) -> String {
        self.net.roster.iter().find(|r| r.id == id).map_or_else(|| "?".to_string(), |r| r.name.clone())
    }
    pub fn team(&self, id: u8) -> u8 {
        self.net.roster.iter().find(|r| r.id == id).map_or((id % 2) as u8, |r| r.team)
    }
    pub fn kit_of(&self, id: u8) -> u8 {
        self.net.roster.iter().find(|r| r.id == id).map_or(0, |r| r.kit)
    }
    pub fn render_pos(&self, alpha: f32) -> Vec3 {
        self.prev_pos + (self.st.pos - self.prev_pos) * alpha + self.smooth
    }
    pub fn sight(&self) -> u8 {
        self.sights[sim::wid(&self.st) as usize % 5]
    }
    pub fn hspeed(&self) -> f32 {
        (self.st.vel.x * self.st.vel.x + self.st.vel.z * self.st.vel.z).sqrt()
    }
    /// Where the weapon actually points: the view plus recoil.
    pub fn aim(&self) -> (f32, f32) {
        (self.yaw + self.recoil_y, (self.pitch + self.recoil_p).clamp(-1.5, 1.5))
    }
    fn own_view(&self) -> PlayerView {
        PlayerView {
            id: self.my_id,
            pos: self.st.pos,
            yaw: self.yaw,
            pitch: self.pitch,
            flags: if self.st.slot == 1 { flag::SIDEARM } else { 0 },
            team: self.my_team,
            kit: self.st.kit,
            crouch: if self.st.crouch { 1.0 } else { 0.0 },
            ..Default::default()
        }
    }

    fn handle_fx(&mut self, f: &Fx, listener: Vec3, now: f64) {
        match *f {
            Fx::Shot { who, from, to, hit } => {
                let (from, to) = (Vec3::from(from), Vec3::from(to));
                let mine = who == self.my_id && !self.spectator;
                let team = self.team(who);
                if !mine {
                    let len = (to - from).length();
                    let d = (to - from).normalize_or_zero();
                    let col = if team == 0 { [1.0, 0.72, 0.38] } else { [0.5, 0.8, 1.0] };
                    self.fx.tracer(from + d * 0.8 - vec3(0.0, 0.14, 0.0), to, col);
                    let i = who as usize % 64;
                    self.rfire[i] = 1.0;
                    // A round that passes close by cracks in your ear.
                    if team != self.my_team && !self.spectator && self.alive && len > 8.0 {
                        let t = (listener - from).dot(d).clamp(0.0, len);
                        let near = from + d * t;
                        if t > 6.0 && (near - listener).length() < 2.4 {
                            self.suppress = (self.suppress + 0.3).min(1.0);
                            if now - self.whizz_at > 0.09 {
                                self.whizz_at = now;
                                self.audio.play_at(Sfx::Whizz, near, listener, 0.9, 6.0);
                            }
                        }
                    }
                    if now - self.last_shot_snd[i] > 0.03 {
                        self.last_shot_snd[i] = now;
                        self.fx.muzzle(from + d * 0.9 - vec3(0.0, 0.14, 0.0));
                        let side = self.views.iter().any(|v| v.id == who && v.flags & flag::SIDEARM != 0);
                        let s = audio::shot_sfx(if side { sim::SIDEARM_ID } else { self.kit_of(who) });
                        self.audio.play_at(s, from, listener, 0.55, 28.0);
                    }
                    if hit == 1 {
                        let n = self.world.raycast(from, d, len + 0.5, self.dmask).map_or(-d, |h| h.normal);
                        self.fx.impact(to, n, [0.55, 0.5, 0.42]);
                    }
                }
                if hit == 2 {
                    self.fx.hit_burst(to, [1.0, 0.95, 0.7]);
                }
            }
            Fx::Hit { attacker, victim, head, kill, .. } => {
                if attacker == self.my_id && victim != self.my_id {
                    self.hitmarker = 0.22;
                    self.hit_head = head;
                    if kill {
                        self.hit_kill = 0.6;
                        self.audio.play(Sfx::Kill, 0.8);
                    } else {
                        self.audio.play(if head { Sfx::Head } else { Sfx::Hit }, 0.6);
                    }
                }
                if victim == self.my_id {
                    self.dmg_flash = 1.0;
                    self.audio.play(Sfx::Hurt, 0.7);
                    if attacker != self.my_id {
                        self.dmg_dirs.push((self.last_pos[attacker as usize % 64], now));
                    }
                }
            }
            Fx::Boom { pos } => {
                let p = Vec3::from(pos);
                self.fx.explosion(p, &self.world);
                self.audio.play_at(Sfx::Boom, p, listener, 1.0, 40.0);
                let d = (p - listener).length();
                self.shake = self.shake.max((1.0 - d / 30.0).max(0.0));
            }
            Fx::CrateBreak { pos } => {
                let p = Vec3::from(pos);
                self.fx.crate_break(p);
                self.audio.play_at(Sfx::Crate, p, listener, 0.7, 22.0);
            }
            Fx::Throw { who } => {
                let p = self.last_pos[who as usize % 64];
                self.audio.play_at(Sfx::Throw, p, listener, 0.5, 14.0);
            }
        }
    }

    fn handle_feed(&mut self, f: &Feed, now: f64) {
        match *f {
            Feed::Kill { killer, victim, weapon, head, assist } => {
                let vteam = self.team(victim);
                let me = victim == self.my_id && !self.spectator;
                let pos = if me { self.st.pos } else { self.last_pos[victim as usize % 64] };
                if me || now - self.last_seen[victim as usize % 64] < 1.5 {
                    self.fx.shatter(pos, scene::team_col(vteam));
                    // Leave a body where they fell.
                    let mut v = if me { self.own_view() } else { self.last_view[victim as usize % 64] };
                    v.pos = pos;
                    v.team = vteam;
                    self.corpses.push((v, now));
                    if self.corpses.len() > 18 {
                        self.corpses.remove(0);
                    }
                }
                if killer == self.my_id && victim != self.my_id {
                    self.stats.my_kills += 1;
                    let how = if head { "HEADSHOT" } else { "ELIMINATED" };
                    self.sub_banner = Some((format!("{how}  {}", self.name(victim)), now));
                } else if assist == self.my_id && !self.spectator {
                    self.sub_banner = Some((format!("ASSIST  {}  +50", self.name(victim)), now));
                }
                if victim == self.my_id {
                    self.stats.my_deaths += 1;
                }
                let weapon = match weapon {
                    0 => "Carbine",
                    1 => "Scattergun",
                    2 => "Longrifle",
                    4 => "Sidearm",
                    _ => "Frag",
                };
                self.feed.push(KillLine {
                    killer: self.name(killer),
                    kteam: self.team(killer),
                    victim: self.name(victim),
                    vteam,
                    weapon,
                    head,
                    mine: killer == self.my_id || victim == self.my_id,
                    at: now,
                });
                if self.feed.len() > 6 {
                    self.feed.remove(0);
                }
            }
            Feed::Capture { point, team } => {
                let ours = team == self.my_team && !self.spectator;
                let msg = format!("{} CAPTURED {}", TEAM_NAMES[team as usize % 2], POINT_NAMES[point as usize % 5]);
                self.banner = Some((msg, scene::team_col(team), now));
                self.audio.play(if ours || self.spectator { Sfx::CapUp } else { Sfx::CapDown }, 0.7);
                if ours {
                    self.sub_banner = Some(("SURGE - move fast, reload fast".into(), now));
                }
            }
            Feed::Neutralized { point, by } => {
                let msg = format!("{} NEUTRALISED {}", TEAM_NAMES[by as usize % 2], POINT_NAMES[point as usize % 5]);
                self.banner = Some((msg, scene::NEUTRAL_COL, now));
            }
            Feed::MatchEnd { winner } => {
                self.banner = Some((format!("{} WINS THE FIELD", TEAM_NAMES[winner as usize % 2]), scene::team_col(winner), now + 8.0));
                let ours = winner == self.my_team;
                self.audio.play(if ours { Sfx::CapUp } else { Sfx::CapDown }, 0.9);
            }
            Feed::MatchStart => {
                self.banner = Some(("NEW ROUND - TAKE THE LINE".into(), scene::NEUTRAL_COL, now));
                self.feed.clear();
            }
        }
    }

    /// Server reconciliation: adopt the authoritative state, then replay
    /// every command the server has not yet processed.
    fn reconcile(&mut self, s: &Snapshot) {
        let Some(me) = s.me else { return };
        if me.spawn_count != self.spawn_count {
            self.spawn_count = me.spawn_count;
            self.yaw = me.spawn_yaw;
            self.pitch = 0.0;
            self.smooth = Vec3::ZERO;
            self.prev_pos = me.st.pos;
            self.st = me.st;
            self.step_y = 0.0;
            self.audio.play(Sfx::Spawn, 0.5);
        }
        let was_alive = self.alive;
        self.alive = me.alive;
        self.hp = me.hp;
        self.respawn_in = me.respawn_in;
        self.killer = me.killer;
        while self.pending.front().map_or(false, |c| c.seq <= s.ack_seq) {
            self.pending.pop_front();
        }
        if !me.alive {
            if was_alive {
                self.death_pos = self.st.pos;
            }
            return;
        }
        let before = self.st.pos;
        let mut st = me.st;
        for c in &self.pending {
            sim::step(&mut st, c, &self.world, self.dmask);
        }
        self.stats.replays += 1;
        let err = before - st.pos;
        let e = err.length();
        if was_alive && e > 0.01 {
            self.stats.corrections += 1;
            self.stats.max_err = self.stats.max_err.max(e);
        }
        if was_alive && e < 3.0 {
            self.smooth += err;
        } else {
            self.smooth = Vec3::ZERO;
        }
        self.prev_pos += st.pos - before;
        self.st = st;
    }

    fn build_cmd(&mut self, now: f64) -> Cmd {
        self.seq += 1;
        let (yaw, pitch) = self.aim();
        let mut c = Cmd { seq: self.seq, yaw, pitch, interp: self.net.render_tick(now), ..Default::default() };
        if self.auto {
            // Scripted input for unattended end-to-end runs.
            let t = now - self.start;
            c.mz = 1;
            c.mx = if (t * 0.7).sin() > 0.6 { 1 } else { 0 };
            if t % 3.0 < 1.3 {
                c.buttons |= btn::FIRE;
            } else if t % 3.0 > 2.0 {
                c.buttons |= btn::SPRINT;
            }
            if t % 2.5 < 0.05 {
                c.buttons |= btn::JUMP;
            }
            if t % 9.0 > 8.9 {
                c.buttons |= btn::GRENADE;
            }
            if t % 7.0 > 5.5 {
                c.buttons |= btn::ADS;
            }
            if t % 5.0 > 3.4 {
                c.buttons |= btn::CROUCH;
            }
            if t % 6.0 < 0.02 {
                c.buttons |= btn::SWAP;
            }
        } else if self.captured {
            let k = |a, b| mq::is_key_down(a) || mq::is_key_down(b);
            c.mz = k(mq::KeyCode::W, mq::KeyCode::Up) as i8 - k(mq::KeyCode::S, mq::KeyCode::Down) as i8;
            c.mx = k(mq::KeyCode::D, mq::KeyCode::Right) as i8 - k(mq::KeyCode::A, mq::KeyCode::Left) as i8;
            if mq::is_key_down(mq::KeyCode::Space) || self.jump_latch {
                c.buttons |= btn::JUMP;
            }
            if mq::is_key_down(mq::KeyCode::LeftShift) {
                c.buttons |= btn::SPRINT;
            }
            if mq::is_mouse_button_down(mq::MouseButton::Left) {
                c.buttons |= btn::FIRE;
            }
            if mq::is_mouse_button_down(mq::MouseButton::Right) {
                c.buttons |= btn::ADS;
            }
            if k(mq::KeyCode::LeftControl, mq::KeyCode::C) {
                c.buttons |= btn::CROUCH;
            }
            if self.reload_latch {
                c.buttons |= btn::RELOAD;
            }
            if self.gren_latch {
                c.buttons |= btn::GRENADE;
            }
            if self.swap_latch {
                c.buttons |= btn::SWAP;
            }
        }
        self.jump_latch = false;
        self.reload_latch = false;
        self.gren_latch = false;
        self.swap_latch = false;
        c
    }

    /// One fixed 60 Hz client tick: sample input, predict, send.
    fn tick(&mut self, now: f64) {
        let c = self.build_cmd(now);
        if self.alive && !self.spectator {
            self.prev_pos = self.st.pos;
            let y0 = self.st.pos.y;
            let was_ground = self.st.on_ground;
            let out = sim::step(&mut self.st, &c, &self.world, self.dmask);
            // Stair steps teleport the feet up; ease the camera instead.
            let dy = self.st.pos.y - y0;
            if was_ground && self.st.on_ground && dy > 0.12 {
                self.step_y -= dy;
            }
            let kit = sim::wid(&self.st);
            let wd = sim::weapon_def(kit);
            let ads = c.buttons & btn::ADS != 0;
            if out.fired {
                let origin = self.st.pos + vec3(0.0, sim::eye_h(self.st.crouch), 0.0);
                let right = sim::right_of(c.yaw);
                let fwd = sim::view_dir(c.yaw, c.pitch);
                let muzzle = origin + fwd * 0.7 + right * 0.17 * (1.0 - self.ads) - vec3(0.0, 0.11, 0.0);
                let col = if self.my_team == 0 { [1.0, 0.72, 0.38] } else { [0.5, 0.8, 1.0] };
                for d in sim::shot_dirs(kit, c.seq, c.yaw, c.pitch, ads, self.hspeed(), !self.st.on_ground, self.st.crouch, out.heat) {
                    let hit = self.world.raycast(origin, d, 300.0, self.dmask);
                    let end = origin + d * hit.map_or(300.0, |h| h.t);
                    self.fx.tracer(muzzle, end, col);
                    if let Some(h) = hit {
                        self.fx.impact(end, h.normal, [0.55, 0.5, 0.42]);
                    }
                }
                // Recoil climbs and wanders while you hold the trigger, then
                // settles back. Crouching and aiming both steady it.
                let r = (self.seq as f32 * 0.618).fract();
                let r2 = (self.seq as f32 * 0.381).fract();
                let steady = if self.st.crouch { 0.8 } else { 1.0 } * (1.0 - 0.25 * self.ads);
                self.recoil_p = (self.recoil_p + wd.recoil * (0.8 + 0.5 * r) * steady).min(0.17);
                self.recoil_y = (self.recoil_y + wd.recoil * 0.6 * (r2 - 0.5) * steady).clamp(-0.07, 0.07);
                self.last_fire = now;
                self.shot_seq = self.shot_seq.wrapping_add(1);
                self.recoil = 1.0;
                self.flash = 1.0;
                self.audio.play(audio::shot_sfx(kit), 0.75);
                self.fx.light(muzzle, 7.0, [1.0, 0.75, 0.4], 1.4, 0.06);
                if kit != 2 {
                    let port = origin + fwd * 0.5 + right * 0.2 - vec3(0.0, 0.12, 0.0);
                    self.fx.shell(port, right * 2.4 + vec3(0.0, 1.9, 0.0) + self.st.vel, kit == 1);
                }
            }
            self.mantling = out.mantle;
            if out.slid {
                self.audio.play(Sfx::Slide, 0.6);
            }
            if self.st.slide > 0 && self.st.on_ground && self.seq % 4 == 0 {
                self.fx.dust(self.st.pos, 0.8);
            }
            if out.swapped {
                self.audio.play(Sfx::Step, 0.5);
            }
            if out.reload_started {
                self.audio.play(Sfx::Reload, 0.6);
            }
            if out.threw {
                self.audio.play(Sfx::Throw, 0.6);
            }
            if out.landed {
                self.audio.play(Sfx::Step, 0.7);
                self.step_y -= 0.12;
                self.land = 1.0;
                self.fx.dust(self.st.pos, 1.0);
            }
            if self.st.on_ground {
                self.stride += self.hspeed() * DT;
                if self.stride > 2.7 && !self.st.crouch {
                    self.stride = 0.0;
                    self.audio.play(Sfx::Step, 0.28);
                }
            }
        }
        self.pending.push_back(c);
        while self.pending.len() > 120 {
            self.pending.pop_front();
        }
        self.stats.cmds += 1;
        // Each command rides in three consecutive packets.
        let n = self.pending.len();
        let recent: Vec<Cmd> = self.pending.iter().skip(n.saturating_sub(3)).copied().collect();
        self.net.send_cmds(&recent, self.kit_choice, now);
    }
}

/// Keep a third-person camera out of walls and terrain.
fn cam_pull(w: &World, from: Vec3, to: Vec3, dmask: u64) -> Vec3 {
    let d = to - from;
    let len = d.length();
    if len < 0.01 {
        return to;
    }
    let dir = d / len;
    match w.raycast(from, dir, len + 0.4, dmask) {
        Some(h) => from + dir * (h.t - 0.4).max(0.2),
        None => to,
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let args = parse_args();
    // Scripted and command-line launches go straight into a match.
    let direct = args.connect.is_some() || args.host || args.auto || args.spectate || args.shot.is_some() || args.quit_at.is_some();
    if direct {
        let choice = match &args.connect {
            Some(a) => menu::Choice::Join(a.clone()),
            None => menu::Choice::Host { port: args.port, per_team: args.per_team, map: args.map.unwrap_or(0) },
        };
        let mut sens = 0.0022;
        let (_, msg) = play(&args, &args.name, args.team, choice, &mut sens).await;
        if let Some(m) = msg {
            eprintln!("{m}");
            for _ in 0..240 {
                mq::clear_background(mq::Color::new(0.09, 0.08, 0.08, 1.0));
                mq::draw_text(&m, 40.0, 60.0, 28.0, mq::ORANGE);
                mq::next_frame().await;
            }
        }
        return;
    }
    let mut menu = menu::Menu::load(&args.name, args.port, args.per_team);
    if let Some(m) = args.map {
        menu.map = m;
    }
    menu.shot = args.menu_shot.clone();
    loop {
        let choice = menu.run().await;
        let joining = match &choice {
            menu::Choice::Quit => break,
            menu::Choice::Join(a) => Some(a.clone()),
            menu::Choice::Host { .. } => None,
        };
        let name = menu.name();
        let (joined, msg) = play(&args, &name, menu.team, choice, &mut menu.sens).await;
        menu.persist();
        if let (true, Some(a)) = (joined, joining) {
            menu.remember(&a);
        }
        menu.error = msg;
    }
}

/// Run one match. Returns whether we got into the match, and a message for
/// the home screen if it ended for a reason other than the player leaving.
async fn play(args: &Args, name: &str, team: u8, choice: menu::Choice, sens: &mut f32) -> (bool, Option<String>) {
    let mut server = None;
    let addr = match choice {
        menu::Choice::Join(a) => {
            let a = if a.contains(':') { a } else { format!("{a}:{DEFAULT_PORT}") };
            match a.to_socket_addrs().ok().and_then(|mut i| i.next()) {
                Some(x) => x,
                None => return (false, Some(format!("Cannot resolve {a}"))),
            }
        }
        menu::Choice::Host { port, per_team, map } => {
            let h = match ferrite_server::spawn(ferrite_server::Config {
                port,
                per_team,
                map,
                seed: args.seed,
                tickets: args.tickets,
                bot_skill: args.skill,
                log: false,
            }) {
                Ok(h) => h,
                Err(e) => return (false, Some(format!("Could not start a server: {e}"))),
            };
            let port = h.addr.port();
            server = Some(h);
            format!("127.0.0.1:{port}").parse().unwrap()
        }
        menu::Choice::Quit => return (false, None),
    };
    mq::clear_background(mq::Color::new(0.09, 0.08, 0.08, 1.0));
    mq::draw_text(&format!("FERRITE - connecting to {addr}..."), 40.0, 60.0, 32.0, mq::WHITE);
    mq::next_frame().await;

    let (net, welcome) = match Net::connect(addr, name, args.spectate, team) {
        Ok(x) => x,
        Err(e) => return (false, Some(format!("Could not join {addr}: {e}"))),
    };
    let world = World::with_map(welcome.seed, welcome.map);
    let bounds = (
        vec3(-ferrite_shared::map::HALF_X - 8.0, -12.0, -ferrite_shared::map::HALF_Z - 8.0),
        vec3(ferrite_shared::map::HALF_X + 8.0, 34.0, ferrite_shared::map::HALF_Z + 8.0),
    );
    let mut gpu = gfx::Gpu::new(scene::build_world(&world), scene::build_sky(), args.shadows, scene::SUN.normalize(), bounds);
    let map_name = ferrite_shared::map::map_def(world.map).name;
    let audio = Audio::new(args.volume).await;
    let start = mq::get_time();
    let hosting = server.as_ref().map(|h: &ferrite_server::ServerHandle| format!("hosting on UDP {}", h.addr.port()));
    let mut app = App {
        world,
        net,
        my_id: welcome.id,
        my_team: welcome.team,
        spectator: args.spectate,
        auto: args.auto,
        st: PState::spawn(Vec3::ZERO, 0),
        prev_pos: Vec3::ZERO,
        alive: false,
        hp: 0,
        yaw: 0.0,
        pitch: 0.0,
        seq: 0,
        pending: VecDeque::new(),
        smooth: Vec3::ZERO,
        spawn_count: 0,
        respawn_in: 0.0,
        killer: SPECTATOR_ID,
        kit_choice: args.kit,
        step_y: 0.0,
        eye: sim::EYE,
        jump_latch: false,
        gren_latch: false,
        swap_latch: false,
        reload_latch: false,
        captured: false,
        sens: sens.clamp(0.0004, 0.01),
        last_mouse: (0.0, 0.0),
        ads: 0.0,
        fov: 66.0,
        recoil: 0.0,
        recoil_p: 0.0,
        recoil_y: 0.0,
        last_fire: 0.0,
        shot_seq: 0,
        land: 0.0,
        mantling: false,
        mantle_vis: 0.0,
        slide_vis: 0.0,
        air_vis: 0.0,
        strafe_vis: 0.0,
        view_lit: 1.0,
        suppress: 0.0,
        whizz_at: 0.0,
        trail_acc: 0.0,
        bob: 0.0,
        flash: 0.0,
        sprint_vis: 0.0,
        stride: 0.0,
        shake: 0.0,
        hitmarker: 0.0,
        hit_head: false,
        hit_kill: 0.0,
        dmg_flash: 0.0,
        dmg_dirs: Vec::new(),
        feed: Vec::new(),
        banner: None,
        sub_banner: None,
        dmask: 0,
        fx: fx::Fx::new(),
        audio,
        walk: [0.0; 64],
        rvel: [(0.0, 0.0); 64],
        rcrouch: [0.0; 64],
        rair: [0.0; 64],
        rslide: [0.0; 64],
        rreload: [0.0; 64],
        rfire: [0.0; 64],
        rhurt: [0.0; 64],
        rhp: [0; 64],
        rstep: [0.0; 64],
        last_view: [PlayerView::default(); 64],
        corpses: Vec::new(),
        last_pos: [Vec3::ZERO; 64],
        last_seen: [-10.0; 64],
        last_shot_snd: [0.0; 64],
        views: Vec::new(),
        spec_target: 0,
        spec_since: 0.0,
        death_pos: Vec3::ZERO,
        show_net: false,
        team_pick: false,
        sights: match args.sight {
            Some(s) => [s.min(5); 5],
            None => [scene::IRON, scene::IRON, scene::SCOPE4, 0, scene::IRON],
        },
        stats: Stats::default(),
        hosting,
        start,
    };
    app.team_pick = args.pick_team;
    app.banner = Some((map_name.to_string(), scene::NEUTRAL_COL, start + 0.5));
    let (mut opaque, mut alpha, mut add) = (gfx::Mesh::default(), gfx::Mesh::default(), gfx::Mesh::default());
    let (mut vm_opaque, mut vm_add) = (gfx::Mesh::default(), gfx::Mesh::default());
    let mut acc = 0.0f32;
    let mut last = mq::get_time();
    let mut listener = Vec3::ZERO;
    let mut shot_done = false;
    let mut peak_tris = 0usize;
    let mut grabbed = false;
    let mut sway = (0.0f32, 0.0f32);
    let mut ended: Option<String> = None;

    loop {
        let now = mq::get_time();
        let frame = (now - last).min(0.1);
        last = now;
        let dt = frame as f32;
        app.stats.frames += 1;
        app.stats.frame_sum += frame;
        if app.stats.frames > 30 {
            app.stats.frame_max = app.stats.frame_max.max(frame);
        }
        app.audio.begin_frame();

        // ---- input: mouse capture, look, edge-triggered keys ----
        // Team picker: M opens it; click a side, or M / Esc to cancel.
        let mut picked = false;
        if app.team_pick {
            if mq::is_key_pressed(mq::KeyCode::M) || mq::is_key_pressed(mq::KeyCode::Escape) {
                app.team_pick = false;
                picked = true; // swallow this Esc
            } else if mq::is_mouse_button_pressed(mq::MouseButton::Left) {
                let (mx, my) = mq::mouse_position();
                let rects = hud::team_pick_rects(mq::screen_width(), mq::screen_height());
                for (t, &(x, y, w, h)) in rects.iter().enumerate() {
                    if mx >= x && mx < x + w && my >= y && my < y + h {
                        if t as u8 != app.my_team {
                            app.net.send_team(t as u8);
                        }
                        app.team_pick = false;
                    }
                }
                picked = true; // this click chose a team, it does not deploy
            }
        } else if mq::is_key_pressed(mq::KeyCode::M) && !app.spectator && !app.auto {
            app.team_pick = true;
            app.captured = false;
        }
        if picked || app.team_pick {
        } else if mq::is_key_pressed(mq::KeyCode::Escape) && app.captured {
            app.captured = false;
        } else if mq::is_mouse_button_pressed(mq::MouseButton::Left) && !app.captured && !app.auto && !app.spectator {
            app.captured = true;
        }
        // The mouse is only held while you are alive: dead, it is visible and
        // free to leave the window, and it is taken back when you deploy.
        let want_grab = app.captured && app.alive;
        if want_grab != grabbed {
            grabbed = want_grab;
            mq::set_cursor_grab(grabbed);
            mq::show_mouse(!grabbed);
            app.last_mouse = mq::mouse_position();
        }
        if !app.team_pick && !picked && (!app.captured || app.spectator) && (mq::is_key_pressed(mq::KeyCode::Q) || (app.spectator && mq::is_key_pressed(mq::KeyCode::Escape))) {
            break;
        }
        let m = mq::mouse_position();
        let (mdx, mdy) = ((m.0 - app.last_mouse.0).clamp(-400.0, 400.0), (m.1 - app.last_mouse.1).clamp(-400.0, 400.0));
        app.last_mouse = m;
        if grabbed {
            let s = app.sens * (app.fov / 66.0);
            app.yaw = ferrite_shared::wrap_angle(app.yaw - mdx * s);
            app.pitch = (app.pitch - mdy * s).clamp(-1.5, 1.5);
            sway.0 = (sway.0 - mdx * 0.00004).clamp(-0.02, 0.02);
            sway.1 = (sway.1 + mdy * 0.00004).clamp(-0.02, 0.02);
            app.jump_latch |= mq::is_key_pressed(mq::KeyCode::Space);
            app.reload_latch |= mq::is_key_pressed(mq::KeyCode::R);
            app.gren_latch |= mq::is_key_pressed(mq::KeyCode::G);
            app.swap_latch |= mq::is_key_pressed(mq::KeyCode::Q) || mq::is_key_pressed(mq::KeyCode::F) || mq::mouse_wheel().1 != 0.0;
        }
        if app.spectator && mq::is_mouse_button_pressed(mq::MouseButton::Left) {
            app.spec_since = -100.0;
        }
        sway.0 *= 1.0 - (10.0 * dt).min(1.0);
        sway.1 *= 1.0 - (10.0 * dt).min(1.0);
        for (k, key) in [mq::KeyCode::Key1, mq::KeyCode::Key2, mq::KeyCode::Key3].into_iter().enumerate() {
            if mq::is_key_pressed(key) {
                app.kit_choice = k as u8;
            }
        }
        if mq::is_key_pressed(mq::KeyCode::T) && app.alive && !app.spectator {
            // Cycle the sight on the weapon in hand.
            let w = sim::wid(&app.st);
            let opts = scene::sights_for(w);
            let cur = opts.iter().position(|&s| s == app.sights[w as usize % 5]).unwrap_or(0);
            let next = opts[(cur + 1) % opts.len()];
            app.sights[w as usize % 5] = next;
            app.sub_banner = Some((format!("{}  -  {}", sim::weapon_def(w).name, scene::SIGHT_NAMES[next as usize]), now));
            app.audio.play(Sfx::Step, 0.5);
        }
        if mq::is_key_pressed(mq::KeyCode::F3) {
            app.show_net = !app.show_net;
        }
        if mq::is_key_pressed(mq::KeyCode::Minus) {
            app.sens = (app.sens * 0.9).max(0.0004);
        }
        if mq::is_key_pressed(mq::KeyCode::Equal) {
            app.sens = (app.sens * 1.1).min(0.01);
        }

        // ---- network in ----
        app.net.poll(now);
        let fresh = std::mem::take(&mut app.net.fresh);
        for s in &fresh {
            app.dmask = s.dmask;
            for f in &s.fx {
                app.handle_fx(f, listener, now);
            }
            for (id, f) in &s.feed {
                if *id == app.net.ack_feed + 1 {
                    app.net.ack_feed = *id;
                    app.handle_feed(f, now);
                }
            }
        }
        if let Some(s) = fresh.last() {
            app.reconcile(s);
        }
        // The roster is the authority on which side we are on.
        if !app.spectator {
            if let Some(t) = app.net.roster.iter().find(|r| r.id == app.my_id).map(|r| r.team) {
                if t != app.my_team && t < 2 {
                    app.my_team = t;
                    app.banner = Some((format!("YOU JOINED {}", TEAM_NAMES[t as usize]), if t == 0 { [1.0, 0.72, 0.38] } else { [0.5, 0.8, 1.0] }, now));
                }
            }
        }

        // ---- fixed-rate prediction ----
        acc += dt;
        let mut n = 0;
        while acc >= DT {
            acc -= DT;
            n += 1;
            if n > 6 {
                acc = 0.0;
                break;
            }
            app.tick(now);
        }
        let a = (acc / DT).clamp(0.0, 1.0);

        // ---- view state ----
        let kit = sim::wid(&app.st);
        let wd = sim::weapon_def(kit);
        let playing = app.alive && !app.spectator;
        let ads_held = playing && if app.auto { (now - app.start) % 7.0 > 5.5 } else { app.captured && mq::is_mouse_button_down(mq::MouseButton::Right) };
        let reloading = app.st.reload > 0;
        let ads_target = if ads_held && !reloading { 1.0 } else { 0.0 };
        app.ads += (ads_target - app.ads) * (14.0 * dt).min(1.0);
        let sprinting = playing && app.hspeed() > sim::WALK * 1.2;
        app.sprint_vis += (if sprinting { 1.0 } else { 0.0 } - app.sprint_vis) * (9.0 * dt).min(1.0);
        let sight = app.sight();
        let ease = |cur: &mut f32, on: bool, rate: f32| *cur += (if on { 1.0 } else { 0.0 } - *cur) * (rate * dt).min(1.0);
        ease(&mut app.slide_vis, playing && app.st.slide > 0, 12.0);
        ease(&mut app.air_vis, playing && !app.st.on_ground, 8.0);
        let climbing = playing && app.mantling;
        ease(&mut app.mantle_vis, climbing, 10.0);
        let side_v = if playing { app.st.vel.dot(sim::right_of(app.yaw)) / sim::WALK } else { 0.0 };
        app.strafe_vis += (side_v.clamp(-1.0, 1.0) - app.strafe_vis) * (8.0 * dt).min(1.0);
        app.fov = 66.0 + (scene::sight_fov(kit, sight) - 66.0) * app.ads + 5.0 * app.sprint_vis + 5.0 * app.slide_vis * (1.0 - app.ads);
        // Recoil settles once you are off the trigger.
        let settle = if now - app.last_fire > 0.11 { 7.5 } else { 0.6 };
        app.recoil_p *= 1.0 - (settle * dt).min(1.0);
        app.recoil_y *= 1.0 - (settle * dt).min(1.0);
        app.land = (app.land - dt * 4.0).max(0.0);
        app.suppress = (app.suppress - dt * 0.7).max(0.0);
        app.recoil *= 1.0 - (11.0 * dt).min(1.0);
        app.flash = (app.flash - dt * 16.0).max(0.0);
        app.hitmarker = (app.hitmarker - dt).max(0.0);
        app.hit_kill = (app.hit_kill - dt).max(0.0);
        app.dmg_flash = (app.dmg_flash - dt * 1.6).max(0.0);
        app.shake = (app.shake - dt * 2.5).max(0.0);
        app.smooth *= 1.0 - (12.0 * dt).min(1.0);
        app.step_y *= 1.0 - (14.0 * dt).min(1.0);
        app.eye += (sim::eye_h(app.st.crouch) - app.eye) * (16.0 * dt).min(1.0);
        app.dmg_dirs.retain(|d| now - d.1 < 1.6);
        if playing && app.st.on_ground {
            app.bob += app.hspeed() * dt * 1.35;
        }

        // ---- remote players ----
        let rt = app.net.render_tick(now);
        app.views.clear();
        let roster: Vec<(u8, u8, u8)> = app.net.roster.iter().map(|r| (r.id, r.team, r.kit)).collect();
        for (id, team, kit) in roster {
            if id == app.my_id && !app.spectator {
                continue;
            }
            let Some(r) = app.net.remote(id, rt) else { continue };
            let i = id as usize % 64;
            let d = r.pos - app.last_pos[i];
            let step = (d.x * d.x + d.z * d.z).sqrt();
            let fresh_pos = now - app.last_seen[i] < 0.5 && step < 3.0;
            let grounded = r.flags & flag::GROUND != 0;
            let on = |b: bool| if b { 1.0 } else { 0.0 };
            let (crouch_t, air_t) = (on(r.flags & flag::CROUCH != 0), on(!grounded));
            let slide_t = on(r.flags2 & flag2::SLIDE != 0);
            let reload_t = on(r.flags2 & (flag2::RELOAD | flag2::DRAW) != 0);
            if fresh_pos && dt > 0.0 {
                // Velocity in the soldier's own frame drives the stride and lean.
                let (f, rt_dir) = (sim::view_dir(r.yaw, 0.0), sim::right_of(r.yaw));
                let v = d / dt;
                let k = (9.0 * dt).min(1.0);
                app.rvel[i].0 += (v.x * f.x + v.z * f.z - app.rvel[i].0) * k;
                app.rvel[i].1 += (v.x * rt_dir.x + v.z * rt_dir.z - app.rvel[i].1) * k;
                app.walk[i] += step;
                let k = |rate: f32| (rate * dt).min(1.0);
                app.rcrouch[i] += (crouch_t - app.rcrouch[i]) * k(12.0);
                app.rair[i] += (air_t - app.rair[i]) * k(10.0);
                app.rslide[i] += (slide_t - app.rslide[i]) * k(12.0);
                app.rreload[i] += (reload_t - app.rreload[i]) * k(9.0);
                if r.hp < app.rhp[i] {
                    app.rhurt[i] = 1.0;
                }
            } else {
                // Just came into view (or respawned): snap, do not blend.
                app.rvel[i] = (0.0, 0.0);
                app.rcrouch[i] = crouch_t;
                app.rair[i] = air_t;
                app.rslide[i] = slide_t;
                app.rreload[i] = reload_t;
                app.rhurt[i] = 0.0;
                app.rstep[i] = 0.0;
            }
            app.rhp[i] = r.hp;
            app.rfire[i] *= 1.0 - (14.0 * dt).min(1.0);
            app.rhurt[i] = (app.rhurt[i] - dt * 5.0).max(0.0);
            let speed = (app.rvel[i].0 * app.rvel[i].0 + app.rvel[i].1 * app.rvel[i].1).sqrt();
            // Footfalls: you can hear someone running up on you.
            if fresh_pos && grounded && speed > 4.2 && app.rcrouch[i] < 0.5 && app.rslide[i] < 0.3 {
                app.rstep[i] += step;
                if app.rstep[i] > 2.6 {
                    app.rstep[i] = 0.0;
                    // Only close ones: enemies carry further than friends.
                    let enemy = team != app.my_team || app.spectator;
                    let near = (r.pos - listener).length();
                    if near < if enemy { 14.0 } else { 6.0 } {
                        app.audio.play_at(Sfx::Step, r.pos, listener, if enemy { 0.45 } else { 0.16 }, 6.0);
                    }
                    if near < 40.0 {
                        app.fx.dust(r.pos, 0.5);
                    }
                }
            }
            app.last_pos[i] = r.pos;
            app.last_seen[i] = now;
            let view = PlayerView {
                id,
                pos: r.pos,
                yaw: r.yaw,
                pitch: r.pitch,
                flags: r.flags,
                flags2: r.flags2,
                hp: r.hp,
                team,
                kit,
                phase: app.walk[i],
                moving: (speed / 3.0).min(1.0),
                crouch: app.rcrouch[i],
                air: app.rair[i],
                slide: app.rslide[i],
                reload: app.rreload[i],
                fire: app.rfire[i],
                hurt: app.rhurt[i],
                vel: app.rvel[i],
                fall: 0.0,
                sink: 0.0,
            };
            app.last_view[i] = view;
            app.views.push(view);
        }

        // ---- camera ----
        let aspect = mq::screen_width() / mq::screen_height().max(1.0);
        let shake = vec3((now as f32 * 61.0).sin(), (now as f32 * 47.0).sin(), 0.0) * app.shake * 0.12;
        let (eye, dir) = if app.spectator {
            if now - app.spec_since > 10.0 || !app.views.iter().any(|v| v.id == app.spec_target) {
                let firing: Vec<u8> = app.views.iter().filter(|v| v.flags & flag::FIRING != 0).map(|v| v.id).collect();
                let pool: Vec<u8> = if firing.is_empty() { app.views.iter().map(|v| v.id).collect() } else { firing };
                if !pool.is_empty() {
                    let next = pool.iter().position(|&i| i > app.spec_target).unwrap_or(0);
                    app.spec_target = pool[next];
                    app.spec_since = now;
                }
            }
            match app.views.iter().find(|v| v.id == app.spec_target) {
                Some(v) => {
                    let f = sim::view_dir(v.yaw, 0.0);
                    let e = v.pos + vec3(0.0, 2.5, 0.0) - f * 4.6 + sim::right_of(v.yaw) * 0.9;
                    let e = cam_pull(&app.world, v.pos + vec3(0.0, 1.7, 0.0), e, app.dmask);
                    (e, sim::view_dir(v.yaw, v.pitch * 0.6 - 0.14))
                }
                None => (vec3(0.0, 45.0, -70.0), vec3(0.0, -0.5, 0.86)),
            }
        } else if app.alive {
            let p = app.render_pos(a);
            let kick = app.recoil * wd.recoil * 0.6;
            let (yaw, pitch) = app.aim();
            (p + vec3(0.0, app.eye + app.step_y, 0.0) + shake, sim::view_dir(yaw, pitch + kick))
        } else {
            // Death cam: pull back and look toward whoever got you.
            let target = if app.killer != SPECTATOR_ID { app.last_pos[app.killer as usize % 64] } else { app.death_pos + sim::view_dir(app.yaw, 0.0) };
            let mut h = target - app.death_pos;
            h.y = 0.0;
            let h = if h.length() > 0.5 { h.normalize() } else { sim::view_dir(app.yaw, 0.0) };
            let e = app.death_pos + vec3(0.0, 4.2, 0.0) - h * 6.5;
            let e = cam_pull(&app.world, app.death_pos + vec3(0.0, 1.7, 0.0), e, app.dmask);
            let look = (app.death_pos + h * 6.0 + vec3(0.0, 1.0, 0.0) - e).normalize();
            (e, look)
        };
        listener = eye;
        let proj = Mat4::perspective_rh_gl(app.fov.to_radians(), aspect, 0.08, 900.0);
        let cam_right = dir.cross(Vec3::Y).normalize_or_zero();
        // The view tips a little into a strafe and more into a slide.
        let tip = if playing { (app.strafe_vis * 0.012 + app.slide_vis * 0.045) * (1.0 - app.ads) } else { 0.0 };
        let up_dir = (Vec3::Y + cam_right * tip).normalize();
        let view = Mat4::look_to_rh(eye, dir, up_dir);
        let vp = proj * view;
        let sky_vp = proj * Mat4::look_to_rh(Vec3::ZERO, dir, up_dir);
        let cam_up = cam_right.cross(dir);
        app.audio.right = cam_right;
        // Is the player standing in sunlight? The weapon in hand dims in shade.
        let sun_dir = scene::SUN.normalize();
        let lit = !gpu.shadows() || app.world.los(eye, eye + sun_dir * 160.0, app.dmask);
        app.view_lit += (if lit { 1.0 } else { 0.0 } - app.view_lit) * (6.0 * dt).min(1.0);

        // ---- build dynamic geometry ----
        app.fx.update(dt, &app.world);
        opaque.clear();
        alpha.clear();
        add.clear();
        vm_opaque.clear();
        vm_add.clear();
        let time = now as f32;
        let blob = !gpu.shadows();
        for v in &app.views {
            scene::soldier(&mut opaque, &mut alpha, &app.world, v, time, blob);
        }
        app.corpses.retain(|c| now - c.1 < 7.0);
        for (v, at) in &app.corpses {
            let age = (now - at) as f32;
            let mut v = *v;
            v.fall = (age / 0.5).min(1.0);
            v.sink = ((age - 5.5) / 1.5).clamp(0.0, 1.0) * 0.7;
            v.hurt = (1.0 - age * 4.0).max(0.0);
            v.moving = 0.0;
            v.fire = 0.0;
            v.air = 0.0;
            v.flags &= !(flag::SHIELD | flag::SURGE);
            scene::soldier(&mut opaque, &mut alpha, &app.world, &v, time, false);
        }
        app.trail_acc += dt;
        let trail = app.trail_acc > 0.035;
        if trail {
            app.trail_acc = 0.0;
        }
        scene::crates(&mut opaque, &app.world, app.dmask);
        let nades: Vec<Vec3> = app.net.latest().map_or(Vec::new(), |s| s.grenades.iter().map(|g| Vec3::from(g.pos)).collect());
        if trail {
            for p in &nades {
                app.fx.trail(*p);
            }
        }
        if let Some(s) = app.net.latest() {
            let owners: Vec<(u8, f32, bool)> =
                s.points.iter().map(|p| (p.owner, p.progress as f32 / 100.0, p.n[0] > 0 && p.n[1] > 0)).collect();
            scene::beacons(&mut opaque, &mut alpha, &mut add, &app.world, &owners, time);
            for g in &s.grenades {
                let p = Vec3::from(g.pos);
                let m = Mat4::from_translation(p) * Mat4::from_rotation_y(time * 9.0) * Mat4::from_rotation_x(time * 7.0);
                opaque.obox(&m, Vec3::ZERO, Vec3::splat(0.09), gfx::rgb(0.14, 0.16, 0.12));
                let c = scene::team_col(g.team);
                let s = 0.2 + 0.1 * (time * 20.0).sin();
                add.quad(p - cam_right * s - cam_up * s, p + cam_right * s - cam_up * s, p + cam_right * s + cam_up * s, p - cam_right * s + cam_up * s, [c[0], c[1], c[2], 0.8]);
            }
        }
        app.fx.build(&mut opaque, &mut alpha, &mut add, eye, cam_right, cam_up);
        if playing {
            scene::viewmodel(
                &mut vm_opaque,
                &mut vm_add,
                &scene::ViewModel {
                    sight,
                    kit,
                    team: app.my_team,
                    ads: app.ads,
                    recoil: app.recoil,
                    bob: app.bob,
                    bob_amt: (app.hspeed() / sim::WALK).min(1.5),
                    reload: if let Some(r) = args.vm_reload {
                        r
                    } else if reloading {
                        let total = if app.st.surge { wd.reload / 2 } else { wd.reload }.max(1);
                        (1.0 - app.st.reload as f32 / total as f32).clamp(0.0, 1.0)
                    } else {
                        -1.0
                    },
                    draw: app.st.swap as f32 / if app.st.slot == 1 { 16.0 } else { 26.0 },
                    cycle: if wd.interval > 20 && app.st.ammo > 0 { app.st.cooldown as f32 / wd.interval as f32 } else { 0.0 },
                    sway,
                    flash: app.flash,
                    sprint: app.sprint_vis,
                    land: app.land,
                    strafe: app.strafe_vis,
                    slide: app.slide_vis,
                    air: app.air_vis,
                    mantle: app.mantle_vis,
                    time,
                    seq: app.shot_seq,
                },
            );
        }
        let scoped = playing && sight >= scene::SCOPE4 && app.ads > 0.85;
        let empty = gfx::Mesh::default();
        gpu.render(&gfx::Frame {
            vp,
            sky_vp,
            cam: eye,
            sun: scene::SUN.normalize(),
            fog: scene::FOG,
            lights: app.fx.lights(eye),
            opaque: &opaque,
            alpha: &alpha,
            add: &add,
            view_vp: Mat4::perspective_rh_gl(52f32.to_radians(), aspect, 0.01, 10.0),
            view_sun: view.transform_vector3(scene::SUN.normalize()),
            view_lit: app.view_lit,
            view_opaque: if scoped { &empty } else { &vm_opaque },
            view_add: if scoped { &empty } else { &vm_add },
        });

        let frame_tris = gpu.tris + (opaque.i.len() + alpha.i.len() + add.i.len()) / 3;
        peak_tris = peak_tris.max(frame_tris);
        hud::draw(&app, &vp, now, scoped, frame_tris);

        let t = now - app.start;
        if let Some(path) = &args.shot {
            if !shot_done && t >= args.shot_at {
                shot_done = true;
                let img = mq::get_screen_data();
                img.export_png(path);
            }
        }
        if let Some(why) = &app.net.rejected {
            ended = Some(format!("Disconnected: {why}"));
            break;
        }
        if app.net.snaps_recv > 0 && now - app.net.last_recv > 6.0 {
            ended = Some(format!("Lost connection to {addr}"));
            break;
        }
        if args.quit_at.map_or(false, |q| t >= q) {
            break;
        }
        // Without vsync (some drivers, or a hidden window) do not spin flat out.
        let spent = mq::get_time() - now;
        if spent < 0.002 {
            std::thread::sleep(std::time::Duration::from_secs_f64(0.002 - spent));
        }
        mq::next_frame().await
    }
    *sens = app.sens;

    app.net.bye();
    let s = &app.stats;
    println!(
        "session: {:.1} s | {} frames, avg {:.2} ms, worst {:.2} ms | ping {:.1} ms | {} snapshots, {:.1} kB/s down | {} cmds, {} replays, {} corrections >1 cm (max {:.3} m) | K/D {}/{} | audio {}",
        mq::get_time() - app.start,
        s.frames,
        s.frame_sum * 1000.0 / s.frames.max(1) as f64,
        s.frame_max * 1000.0,
        app.net.ping_ms,
        app.net.snaps_recv,
        app.net.bytes_in as f64 / 1000.0 / (mq::get_time() - app.start).max(0.1),
        s.cmds,
        s.replays,
        s.corrections,
        s.max_err,
        s.my_kills,
        s.my_deaths,
        if app.audio.ok { "ok" } else { "unavailable" },
    );
    println!("triangles: {} static world, {} peak per frame", gpu.tris, peak_tris);
    if let Some(mut h) = server {
        h.stop();
    }
    gpu.destroy();
    mq::set_cursor_grab(false);
    mq::show_mouse(true);
    let _ = NEUTRAL;
    (true, ended)
}
