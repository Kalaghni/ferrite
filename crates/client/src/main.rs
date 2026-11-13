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
    host: bool,
    menu_shot: Option<String>,
    name: String,
    per_team: usize,
    port: u16,
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
}

fn parse_args() -> Args {
    let mut a = Args {
        connect: None,
        host: false,
        menu_shot: None,
        name: std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "Player".into()),
        per_team: 12,
        port: DEFAULT_PORT,
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
            "--tickets" => a.tickets = val.parse().expect("--tickets <n>"),
            "--skill" => a.skill = val.parse().expect("--skill <f32>"),
            "--volume" => a.volume = val.parse().expect("--volume <0..1>"),
            "--sight" => a.sight = Some(val.parse().expect("--sight <0..5>")),
            "--kit" => a.kit = val.parse::<u8>().expect("--kit <1..3>").clamp(1, 3) - 1,
            "--shot" => a.shot = Some(val),
            "--shot-at" => a.shot_at = val.parse().expect("--shot-at <secs>"),
            "--quit-at" => a.quit_at = Some(val.parse().expect("--quit-at <secs>")),
            "--menu-shot" => a.menu_shot = Some(val),
            "--host" => {
                a.host = true;
                used = 1;
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
                println!("ferrite [--connect host[:port]] [--host] [--name N] [--bots 12] [--port 27500] [--seed N] [--tickets 250] [--skill 1.0]\n        [--volume 0.7] [--fullscreen] [--spectate] [--auto] [--shot file.png --shot-at SECS] [--quit-at SECS]");
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
    pub last_pos: [Vec3; 64],
    last_seen: [f64; 64],
    last_shot_snd: [f64; 64],
    pub views: Vec<PlayerView>,
    pub spec_target: u8,
    spec_since: f64,
    death_pos: Vec3,
    pub show_net: bool,
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

    fn handle_fx(&mut self, f: &Fx, listener: Vec3, now: f64) {
        match *f {
            Fx::Shot { who, from, to, hit } => {
                let (from, to) = (Vec3::from(from), Vec3::from(to));
                let mine = who == self.my_id && !self.spectator;
                let team = self.team(who);
                if !mine {
                    let d = (to - from).normalize_or_zero();
                    let col = if team == 0 { [1.0, 0.72, 0.38] } else { [0.5, 0.8, 1.0] };
                    self.fx.tracer(from + d * 0.8 - vec3(0.0, 0.14, 0.0), to, col);
                    let i = who as usize % 64;
                    if now - self.last_shot_snd[i] > 0.03 {
                        self.last_shot_snd[i] = now;
                        self.fx.muzzle(from + d * 0.9 - vec3(0.0, 0.14, 0.0));
                        let side = self.views.iter().any(|v| v.id == who && v.flags & flag::SIDEARM != 0);
                        let s = audio::shot_sfx(if side { sim::SIDEARM_ID } else { self.kit_of(who) });
                        self.audio.play_at(s, from, listener, 0.55, 28.0);
                    }
                    if hit == 1 {
                        self.fx.impact(to, [0.55, 0.5, 0.42]);
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
                self.fx.explosion(p);
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
            Feed::Kill { killer, victim, weapon, head } => {
                let vteam = self.team(victim);
                let pos = if victim == self.my_id && !self.spectator { self.st.pos } else { self.last_pos[victim as usize % 64] };
                if victim == self.my_id || now - self.last_seen[victim as usize % 64] < 1.5 {
                    self.fx.shatter(pos, scene::team_col(vteam));
                }
                if killer == self.my_id && victim != self.my_id {
                    self.stats.my_kills += 1;
                    self.sub_banner = Some((format!("ELIMINATED  {}", self.name(victim)), now));
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
        let mut c = Cmd { seq: self.seq, yaw: self.yaw, pitch: self.pitch, interp: self.net.render_tick(now), ..Default::default() };
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
                for d in sim::shot_dirs(kit, c.seq, c.yaw, c.pitch, ads, self.hspeed(), !self.st.on_ground, self.st.crouch) {
                    let hit = self.world.raycast(origin, d, 300.0, self.dmask);
                    let end = origin + d * hit.map_or(300.0, |h| h.t);
                    self.fx.tracer(muzzle, end, col);
                    if hit.is_some() {
                        self.fx.impact(end, [0.55, 0.5, 0.42]);
                    }
                }
                let r = (self.seq as f32 * 0.618).fract();
                self.pitch = (self.pitch + wd.recoil * (0.7 + 0.5 * r)).min(1.5);
                self.yaw += wd.recoil * 0.5 * (r - 0.5);
                self.recoil = 1.0;
                self.flash = 1.0;
                self.audio.play(audio::shot_sfx(kit), 0.75);
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
            None => menu::Choice::Host { port: args.port, per_team: args.per_team },
        };
        let (_, msg) = play(&args, &args.name, choice).await;
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
    menu.shot = args.menu_shot.clone();
    loop {
        let choice = menu.run().await;
        let joining = match &choice {
            menu::Choice::Quit => break,
            menu::Choice::Join(a) => Some(a.clone()),
            menu::Choice::Host { .. } => None,
        };
        let name = menu.name();
        let (joined, msg) = play(&args, &name, choice).await;
        if let (true, Some(a)) = (joined, joining) {
            menu.remember(&a);
        }
        menu.error = msg;
    }
}

/// Run one match. Returns whether we got into the match, and a message for
/// the home screen if it ended for a reason other than the player leaving.
async fn play(args: &Args, name: &str, choice: menu::Choice) -> (bool, Option<String>) {
    let mut server = None;
    let addr = match choice {
        menu::Choice::Join(a) => {
            let a = if a.contains(':') { a } else { format!("{a}:{DEFAULT_PORT}") };
            match a.to_socket_addrs().ok().and_then(|mut i| i.next()) {
                Some(x) => x,
                None => return (false, Some(format!("Cannot resolve {a}"))),
            }
        }
        menu::Choice::Host { port, per_team } => {
            let h = match ferrite_server::spawn(ferrite_server::Config {
                port,
                per_team,
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

    let (net, welcome) = match Net::connect(addr, name, args.spectate) {
        Ok(x) => x,
        Err(e) => return (false, Some(format!("Could not join {addr}: {e}"))),
    };
    let world = World::new(welcome.seed);
    let mut gpu = gfx::Gpu::new(scene::build_world(&world), scene::build_sky());
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
        sens: 0.0022,
        last_mouse: (0.0, 0.0),
        ads: 0.0,
        fov: 66.0,
        recoil: 0.0,
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
        last_pos: [Vec3::ZERO; 64],
        last_seen: [-10.0; 64],
        last_shot_snd: [0.0; 64],
        views: Vec::new(),
        spec_target: 0,
        spec_since: 0.0,
        death_pos: Vec3::ZERO,
        show_net: false,
        sights: match args.sight {
            Some(s) => [s.min(5); 5],
            None => [scene::IRON, scene::IRON, scene::SCOPE4, 0, scene::IRON],
        },
        stats: Stats::default(),
        hosting,
        start,
    };
    let (mut opaque, mut alpha, mut add) = (gfx::Mesh::default(), gfx::Mesh::default(), gfx::Mesh::default());
    let (mut vm_opaque, mut vm_add) = (gfx::Mesh::default(), gfx::Mesh::default());
    let mut acc = 0.0f32;
    let mut last = mq::get_time();
    let mut listener = Vec3::ZERO;
    let mut shot_done = false;
    let mut peak_tris = 0usize;
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
        if mq::is_key_pressed(mq::KeyCode::Escape) && app.captured {
            app.captured = false;
            mq::set_cursor_grab(false);
            mq::show_mouse(true);
        } else if mq::is_mouse_button_pressed(mq::MouseButton::Left) && !app.captured && !app.auto && !app.spectator {
            app.captured = true;
            mq::set_cursor_grab(true);
            mq::show_mouse(false);
            app.last_mouse = mq::mouse_position();
        }
        if (!app.captured || app.spectator) && (mq::is_key_pressed(mq::KeyCode::Q) || (app.spectator && mq::is_key_pressed(mq::KeyCode::Escape))) {
            break;
        }
        let m = mq::mouse_position();
        let (mdx, mdy) = ((m.0 - app.last_mouse.0).clamp(-400.0, 400.0), (m.1 - app.last_mouse.1).clamp(-400.0, 400.0));
        app.last_mouse = m;
        if app.captured {
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
        app.fov = 66.0 + (scene::sight_fov(kit, sight) - 66.0) * app.ads + 5.0 * app.sprint_vis;
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
            let speed = if fresh_pos && dt > 0.0 { step / dt } else { 0.0 };
            if fresh_pos {
                app.walk[i] += step;
            }
            app.last_pos[i] = r.pos;
            app.last_seen[i] = now;
            app.views.push(PlayerView {
                id,
                pos: r.pos,
                yaw: r.yaw,
                pitch: r.pitch,
                flags: r.flags,
                hp: r.hp,
                team,
                kit,
                phase: app.walk[i],
                moving: (speed / 3.0).min(1.0),
            });
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
            (p + vec3(0.0, app.eye + app.step_y, 0.0) + shake, sim::view_dir(app.yaw, app.pitch + kick))
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
        let view = Mat4::look_to_rh(eye, dir, Vec3::Y);
        let vp = proj * view;
        let sky_vp = proj * Mat4::look_to_rh(Vec3::ZERO, dir, Vec3::Y);
        let cam_right = dir.cross(Vec3::Y).normalize_or_zero();
        let cam_up = cam_right.cross(dir);

        // ---- build dynamic geometry ----
        app.fx.update(dt, &app.world);
        opaque.clear();
        alpha.clear();
        add.clear();
        vm_opaque.clear();
        vm_add.clear();
        let time = now as f32;
        for v in &app.views {
            scene::soldier(&mut opaque, &mut alpha, &app.world, v, time);
        }
        scene::crates(&mut opaque, &app.world, app.dmask);
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
                    reload: if reloading { 1.0 - app.st.reload as f32 / wd.reload as f32 } else { (app.st.swap as f32 / 26.0).min(1.0) * 0.6 },
                    sway,
                    flash: app.flash,
                    sprint: app.sprint_vis,
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
            opaque: &opaque,
            alpha: &alpha,
            add: &add,
            view_vp: Mat4::perspective_rh_gl(52f32.to_radians(), aspect, 0.01, 10.0),
            view_sun: view.transform_vector3(scene::SUN.normalize()),
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
        mq::next_frame().await
    }

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
