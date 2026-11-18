//! FERRITE client. Opens on a home screen to join a server or host a
//! match on the embedded server; `--connect` and `--host` skip it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod bindings;
mod cine;
mod controller;
mod fx;
mod gfx;
mod hud;
mod ident;
mod menu;
mod net;
mod scene;
mod touch;
mod ui;
mod update;

use audio::{Audio, Music, Scene, Sfx};
use bindings::{Action, Bindings};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use ferrite_shared::proto::*;
use ferrite_shared::sim::{self, PState};
use ferrite_shared::{mode, DT, NEUTRAL, POINT_NAMES, TEAM_NAMES};
use macroquad::prelude as mq;
use net::Net;
use scene::PlayerView;
use std::collections::VecDeque;
use std::net::ToSocketAddrs;

/// True while the chat box has the keyboard: the game's own keys are
/// ignored until it closes.
static TYPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the window is fullscreen right now. F11 flips it anywhere, in
/// the launcher or in a match; the settings row does too.
static FULLSCREEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_fullscreen() -> bool {
    FULLSCREEN.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn set_fullscreen(on: bool) {
    FULLSCREEN.store(on, std::sync::atomic::Ordering::Relaxed);
    mq::set_fullscreen(on);
}
/// F11 this frame: flip fullscreen. True when it was pressed.
pub fn f11_fullscreen() -> bool {
    let hit = mq::is_key_pressed(mq::KeyCode::F11);
    if hit {
        set_fullscreen(!is_fullscreen());
    }
    hit
}

fn typing() -> bool {
    TYPING.load(std::sync::atomic::Ordering::Relaxed)
}
fn pressed(k: mq::KeyCode) -> bool {
    !typing() && mq::is_key_pressed(k)
}

pub struct Args {
    connect: Option<String>,
    team: u8,
    host: bool,
    menu_shot: Option<String>,
    pick_team: bool,
    name: String,
    per_team: usize,
    /// Hosting: most bots in the match (None = fill every slot).
    max_bots: Option<usize>,
    skin: u8,
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
    /// Hosting: stay on one map instead of rotating.
    no_rotate: bool,
    /// Hosting: lobby length in seconds (0 = none). Scripted runs skip it.
    lobby: Option<u32>,
    /// Weapon ids for a direct launch; the class decides what is allowed.
    prim: u8,
    side: u8,
    /// Verification aid: keep the loadout panel on screen.
    show_loadout: bool,
    /// Verification aid: keep the scoreboard on screen.
    show_scores: bool,
    /// Side of the sun shadow map in texels; 0 turns shadows off.
    shadows: u32,
    /// Verification aid: freeze the first-person reload animation here.
    vm_reload: Option<f32>,
    no_music: bool,
    /// Verification aid: take the picture while following our killer.
    shot_dead: bool,
    /// Verification aid: launcher page to open on.
    menu_page: u8,
    /// Verification aids: freeze the first-person melee strike here; say
    /// this in chat a moment into a scripted run; wear this built-in icon;
    /// take the picture while the killcam is playing.
    vm_melee: Option<f32>,
    /// Verification aid: show this weapon finish in first person and in
    /// the armory preview. Only this screen sees it; it is never saved.
    wskin: Option<u8>,
    say: Option<String>,
    icon: Option<u8>,
    shot_killcam: bool,
    /// Verification aid: the scripted player heads for the front line.
    seek: bool,
    /// Do not look for a newer release; and, for trying the updater out,
    /// a version to take this build for.
    no_update: bool,
    pretend_version: Option<String>,
    /// Hosting: game mode, and the score and clock that end a round
    /// outside conquest (0 = the mode's own).
    mode: Option<u8>,
    score: u32,
    time: u32,
    /// On-screen stick and buttons, with the mouse standing in for a finger.
    touch: bool,
    /// Scripted cinematic shot: eye and target, field of view, focus
    /// distance (None = autofocus), aperture (None = no depth of field).
    cine: Option<(Vec3, Vec3)>,
    cine_fov: f32,
    cine_focus: Option<f32>,
    cine_aperture: Option<f32>,
    /// Posing aid for staged shots: a file of soldiers placed by hand.
    cine_cast: Option<String>,
    /// `--shot` (and the screenshot key) at this multiple of the screen size.
    shot_scale: u32,
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
        max_bots: None,
        skin: 0,
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
        no_rotate: false,
        lobby: None,
        prim: 255,
        side: 255,
        show_loadout: false,
        show_scores: false,
        shadows: 4096,
        vm_reload: None,
        no_music: false,
        shot_dead: false,
        menu_page: 0,
        vm_melee: None,
        wskin: None,
        say: None,
        icon: None,
        shot_killcam: false,
        seek: false,
        no_update: false,
        pretend_version: None,
        mode: None,
        score: 0,
        time: 0,
        touch: false,
        cine: None,
        cine_fov: cine::FOV,
        cine_focus: None,
        cine_aperture: None,
        cine_cast: None,
        shot_scale: 1,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned().unwrap_or_default();
        let mut used = 2;
        match args[i].as_str() {
            "--connect" => a.connect = Some(val),
            "--name" => a.name = val,
            "--max-bots" => a.max_bots = val.parse().ok(),
            "--skin" => a.skin = val.parse::<u8>().unwrap_or(0),
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
            "--mode" => match ferrite_shared::mode_by_name(&val) {
                Some(m) => a.mode = Some(m),
                None => {
                    let names: Vec<&str> = ferrite_shared::MODES.iter().map(|m| m.key).collect();
                    eprintln!("unknown mode '{val}'. Modes: {}", names.join(", "));
                    std::process::exit(2);
                }
            },
            "--score" => a.score = val.parse().expect("--score <n>"),
            "--time" => a.time = val.parse().expect("--time <seconds>"),
            "--touch" => {
                a.touch = true;
                used = 1;
            }
            "--skill" => a.skill = val.parse().expect("--skill <f32>"),
            "--difficulty" => {
                a.skill = ferrite_server::difficulty_by_name(&val).unwrap_or_else(|| {
                    eprintln!("unknown difficulty '{val}'. One of: recruit, regular, veteran, elite.");
                    std::process::exit(2);
                })
            }
            "--menu-page" => a.menu_page = val.parse::<u8>().unwrap_or(0), // tens digit: settings tab
            "--audio-devices" => {
                // List the sound outputs; an id can go in ferrite-client.txt.
                for (id, name) in quad_snd::output_devices() {
                    println!(
                        "{name}
    audio_device={id}"
                    );
                }
                std::process::exit(0);
            }
            "--shot-dead" => {
                a.shot_dead = true;
                used = 1;
            }
            "--no-music" => {
                a.no_music = true;
                used = 1;
            }
            "--volume" => a.volume = val.parse().expect("--volume <0..1>"),
            "--sight" => a.sight = Some(val.parse().expect("--sight <0..5>")),
            "--kit" => a.kit = val.parse::<u8>().expect("--kit <1..3>").clamp(1, 3) - 1,
            "--shot" => a.shot = Some(val),
            "--shot-at" => a.shot_at = val.parse().expect("--shot-at <secs>"),
            "--quit-at" => a.quit_at = Some(val.parse().expect("--quit-at <secs>")),
            "--menu-shot" => a.menu_shot = Some(val),
            "--cine" => {
                a.cine = Some(cine::parse_pose(&val).unwrap_or_else(|| {
                    eprintln!("--cine wants six numbers: eyeX,eyeY,eyeZ,targetX,targetY,targetZ");
                    std::process::exit(2);
                }))
            }
            "--cine-fov" => a.cine_fov = val.parse().expect("--cine-fov <degrees>"),
            "--cine-focus" => a.cine_focus = Some(val.parse().expect("--cine-focus <metres>")),
            "--cine-aperture" => a.cine_aperture = Some(val.parse().expect("--cine-aperture <0..4>")),
            "--cine-cast" => a.cine_cast = Some(val),
            "--shot-scale" => a.shot_scale = val.parse::<u32>().expect("--shot-scale <1..8>").clamp(1, 8),
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
            "--prim" => a.prim = val.parse().expect("--prim <weapon id>"),
            "--side" => a.side = val.parse().expect("--side <weapon id>"),
            "--show-scores" => {
                a.show_scores = true;
                used = 1;
            }
            "--show-loadout" => {
                a.show_loadout = true;
                used = 1;
            }
            "--lobby" => a.lobby = Some(val.parse().expect("--lobby <seconds, 0 for none>")),
            "--no-rotate" => {
                a.no_rotate = true;
                used = 1;
            }
            "--vm-reload" => a.vm_reload = val.parse().ok(),
            "--vm-melee" => a.vm_melee = val.parse().ok(),
            "--wskin" => a.wskin = val.parse().ok(),
            "--say" => a.say = Some(val),
            "--icon" => a.icon = val.parse().ok(),
            "--shot-killcam" => {
                a.shot_killcam = true;
                used = 1;
            }
            "--seek" => {
                a.seek = true;
                used = 1;
            }
            "--no-update" => {
                a.no_update = true;
                used = 1;
            }
            "--pretend-version" => a.pretend_version = Some(val),
            "--update" | "--rollback" => used = 1, // handled before the window opens
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
                println!("ferrite [--connect host[:port]] [--host] [--team ember|cobalt|auto] [--name N] [--map NAME] [--mode conquest|tdm|ctf|stronghold] [--score N] [--time SECS] [--touch] [--no-rotate] [--lobby 30] [--bots 12] [--max-bots N] [--port 27500] [--seed N] [--tickets 250] [--difficulty recruit|regular|veteran|elite]\n        [--volume 0.7] [--no-music] [--shadows high|low|off] [--fullscreen] [--version] [--update] [--no-update] [--spectate] [--auto] [--shot file.png --shot-at SECS] [--shot-scale N] [--quit-at SECS] [--rollback]\n        [--cine eyeX,eyeY,eyeZ,targetX,targetY,targetZ] [--cine-fov DEG] [--cine-focus METRES] [--cine-aperture 0.5] [--cine-cast FILE]");
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
    // `ferrite --update`: update from the command line, no window at all.
    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().any(|a| a == "--version" || a == "-V") {
        println!("FERRITE {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }
    if argv.iter().any(|a| a == "--update") {
        let pretend = argv.iter().position(|a| a == "--pretend-version").and_then(|i| argv.get(i + 1));
        std::process::exit(update::run_cli(pretend.map(|s| s.as_str())));
    }
    // `ferrite --rollback`: put back the version the last update replaced.
    if argv.iter().any(|a| a == "--rollback") {
        std::process::exit(update::run_rollback_cli());
    }
    // Tidy up after an update, or undo one that never got going (this may
    // start the restored version and end here).
    update::on_start();
    mq::Conf {
        window_title: "FERRITE".to_string(),
        window_width: 1280,
        window_height: 720,
        fullscreen: std::env::args().any(|a| a == "--fullscreen"),
        sample_count: 4,
        high_dpi: false,
        // The FERRITE mark, for the title bar and the taskbar.
        icon: Some(macroquad::miniquad::conf::Icon {
            small: *include_bytes!("../assets/icon-16.rgba"),
            medium: *include_bytes!("../assets/icon-32.rgba"),
            big: *include_bytes!("../assets/icon-64.rgba"),
        }),
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
    /// A person killed or was killed (not one bot shooting another).
    pub human: bool,
    pub kicon: Option<mq::Texture2D>,
    pub vicon: Option<mq::Texture2D>,
    pub at: f64,
}

/// A line in the chat log.
pub struct ChatMsg {
    pub team: u8,
    /// Said to everyone, not just the speaker's side.
    pub all: bool,
    /// A notice from the server rather than something a player said.
    pub system: bool,
    pub name: String,
    pub text: String,
    pub icon: Option<mq::Texture2D>,
    pub at: f64,
}

/// How far back the killcam starts when there is time for all of it, in seconds.
const KILLCAM_BACK: f64 = 3.2;

/// The killcam: the last seconds before our death, replayed through the
/// eyes of whoever did it, out of the snapshots we already have.
pub struct KillCam {
    pub killer: u8,
    pub weapon: u8,
    /// Server tick and local time of the death.
    death_tick: u32,
    death_now: f64,
    /// Local time the replay began; the tick-times it runs between.
    start_now: f64,
    from: f64,
    to: f64,
    /// Tick-time being shown, and up to where effects have been replayed.
    pub rt: f64,
    fx_tick: f64,
    /// The killer's aim, eased, for the sight picture.
    ads: f32,
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
    /// Primary chosen for each class, and the sidearm.
    pub prims: [u8; 3],
    pub side_choice: u8,
    /// Look worn: index into `sim::SKINS`.
    pub skin_choice: u8,
    /// The loadout panel is open while alive (it always shows while dead).
    pub loadout_open: bool,
    /// The server is in its lobby; our map vote (255 = none) and ready flag.
    pub lobby: bool,
    pub vote: u8,
    pub ready: bool,
    demo_panel: bool,
    pub demo_scores: bool,
    /// Scripted runs: draw the sidearm so it can be looked at.
    demo_side: bool,
    step_y: f32,
    pub eye: f32,
    jump_latch: bool,
    fire_latch: bool,
    gren_latch: bool,
    swap_latch: bool,
    reload_latch: bool,
    pub captured: bool,
    /// A UI action consumed this render frame; fixed ticks must stay neutral.
    input_blocked: bool,
    pub sens: f32,
    pub bindings: Bindings,
    last_mouse: (f32, f32),
    pub ads: f32,
    pub fov: f32,
    /// Settings: field of view from the hip, mouse and aim behaviour.
    base_fov: f32,
    invert_y: bool,
    toggle_aim: bool,
    /// Aiming down the sight (held, or toggled).
    aim_on: bool,
    aim_toggle_on: bool,
    recoil: f32,
    /// Recoil climb and drift added to the aim; settles back off the trigger.
    pub recoil_p: f32,
    pub recoil_y: f32,
    last_fire: f64,
    shot_seq: u32,
    land: f32,
    mantling: bool,
    mantle_vis: f32,
    /// Seconds Use has been held at a shade crank (Canopy), for the HUD.
    crank_hold: f32,
    /// The shade banks' positions last heard from the server, so a bank
    /// starting to move can be heard (None until the first snapshot).
    shade_heard: Option<[u8; 2]>,
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
    rreload_phase: [f32; 64],
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
    /// Seconds since we went down, and the camera that follows whoever
    /// did it (kept where it was if they fall too).
    dead_for: f32,
    kill_cam: Option<(Vec3, Vec3)>,
    pub spec_killer: bool,
    /// The track that can be heard, for the lobby and the pause overlay.
    pub now_playing: Option<String>,
    pub music_in_match: bool,
    pub lobby_bg: mq::Texture2D,
    pub art: ui::Art,
    /// Pause overlay: its settings page is open; whether we have been in
    /// control yet this match; the music volume it shows and changes.
    pub pause_settings: bool,
    pub deployed_once: bool,
    pub music_vol: f32,
    pub show_net: bool,
    /// The in-match team picker is open.
    pub team_pick: bool,
    /// Chosen sight per weapon id.
    pub sights: [u8; 20],
    pub stats: Stats,
    pub hosting: Option<String>,
    pub start: f64,
    /// Chat: the log, and the line being typed.
    pub chat: Vec<ChatMsg>,
    pub chat_open: bool,
    pub chat_team: bool,
    pub chat_buf: String,
    /// Icons of the players here: id -> (hash, picture).
    pub icons: std::collections::HashMap<u8, (u32, mq::Texture2D)>,
    icon_ask: f64,
    /// Finish chosen for each weapon id.
    pub wskins: [u8; 20],
    /// `--wskin`: a finish shown on our own weapon only, for review.
    review_wskin: Option<u8>,
    melee_latch: bool,
    /// Scripted runs: walk to the nearest point we do not hold.
    seek: bool,
    rmelee: [f32; 64],
    /// A death waiting for its killcam (tick, time, killer, weapon), and
    /// the killcam once it is playing.
    kc_armed: Option<(u32, f64, u8, u8)>,
    pub kc: Option<KillCam>,
    /// Lobby: the game mode voted for (255 = none).
    pub mode_vote: u8,
    /// Credits, and what they have bought.
    pub profile: menu::Profile,
    /// What the last finished round paid: (credits, won, when).
    pub award: Option<(u32, bool, f64)>,
    /// Armour worn, kills since the last death, seconds of overcharge left.
    pub armor: u8,
    pub streak: u8,
    pub power: u8,
    /// On-screen touch controls are in use.
    pub touch: bool,
    /// The cinematic camera, and where the game camera was last frame (it
    /// starts from there).
    pub cine: cine::Cine,
    last_cam: (Vec3, Vec3),
    /// Soldiers placed by hand for a staged shot, and their clock.
    cast: Vec<cine::Extra>,
    cast_clock: f32,
}

impl App {
    /// Keyboard and mouse bindings; keys alone while touch controls are
    /// on, when a finger on the screen is not a mouse button.
    fn bind_down(&self, action: Action) -> bool {
        if self.touch {
            self.bindings.down_keys(action)
        } else {
            self.bindings.down(action)
        }
    }
    fn bind_pressed(&self, action: Action) -> bool {
        if self.touch {
            self.bindings.pressed_keys(action)
        } else {
            self.bindings.pressed(action)
        }
    }
    pub fn action_down(&self, action: Action) -> bool {
        !typing() && (self.bind_down(action) || controller::down(action) || touch::down(action))
    }
    pub fn action_pressed(&self, action: Action) -> bool {
        !typing() && (self.bind_pressed(action) || controller::pressed(action) || touch::pressed(action))
    }
    /// Game mode of the round in progress.
    pub fn mode(&self) -> u8 {
        self.net.latest().map_or(mode::CONQUEST, |s| s.mode)
    }
    /// A line in the chat log from the game itself.
    fn notice(&mut self, text: String, now: f64) {
        self.chat.push(ChatMsg { team: NEUTRAL, all: true, system: true, name: String::new(), text, icon: None, at: now });
    }
    pub fn icon(&self, id: u8) -> Option<&mq::Texture2D> {
        self.icons.get(&id).map(|i| &i.1)
    }
    pub fn is_bot(&self, id: u8) -> bool {
        self.net.roster.iter().find(|r| r.id == id).is_none_or(|r| r.bot)
    }
    /// Finish worn by a player's primary, or their sidearm.
    pub fn wskin_of(&self, id: u8, side: bool) -> u8 {
        if id == self.my_id {
            let wid = if side { self.st.side } else { self.st.prim };
            let worn = self.review_wskin.unwrap_or(self.wskins[wid as usize % self.wskins.len()]);
            return sim::fix_weapon_skin(wid, worn);
        }
        self.net.roster.iter().find(|r| r.id == id).map_or(0, |r| r.wskin[side as usize])
    }
    /// Finishes asked for at the next deploy: primary, sidearm.
    pub fn wskin_choice(&self) -> [u8; 2] {
        let (_, prim, side) = self.loadout();
        [self.wskins[prim as usize % 20], self.wskins[side as usize % 20]]
    }
    pub fn name(&self, id: u8) -> String {
        self.net.roster.iter().find(|r| r.id == id).map_or_else(|| "?".to_string(), |r| r.name.clone())
    }
    pub fn team(&self, id: u8) -> u8 {
        self.net.roster.iter().find(|r| r.id == id).map_or(id % 2, |r| r.team)
    }
    /// Weapon a player is firing: their primary, or their sidearm.
    pub fn wid_of(&self, id: u8, side: bool) -> u8 {
        self.net.roster.iter().find(|r| r.id == id).map_or(0, |r| if side { r.side } else { r.prim })
    }
    /// Loadout asked for at the next deploy: class, primary, sidearm.
    pub fn loadout(&self) -> (u8, u8, u8) {
        (self.kit_choice, self.prims[self.kit_choice as usize % 3], self.side_choice)
    }
    pub fn skin_of(&self, id: u8) -> u8 {
        self.net.roster.iter().find(|r| r.id == id).map_or(0, |r| r.skin)
    }
    /// The pause overlay is up: the mouse is free and the menu takes clicks.
    pub fn paused(&self) -> bool {
        !self.captured && !self.auto && !self.spectator && !self.lobby && !self.team_pick
    }
    /// The loadout panel is on screen and takes clicks.
    pub fn loadout_shown(&self) -> bool {
        self.demo_panel
            || (!self.spectator
                && !self.auto
                && !self.team_pick
                && (self.lobby || (self.captured && ((!self.alive && self.kc.is_none()) || self.loadout_open))))
    }
    /// Choose row `row` of column `col` (0 class, 1 primary, 2 sidearm);
    /// column 3 moves on to the next look, 4 and 5 to the next finish for
    /// the primary and the sidearm.
    pub fn pick_loadout(&mut self, col: u8, row: u8) {
        let prims = sim::kit(self.kit_choice).prims;
        match col {
            4 | 5 => {
                let (_, prim, side) = self.loadout();
                let wid = if col == 4 { prim } else { side };
                // Only the finishes this player owns.
                let fits: Vec<u8> = sim::weapon_skins_for(wid).into_iter().filter(|&s| self.profile.owns_finish(s)).collect();
                let at = fits.iter().position(|&s| s == self.wskins[wid as usize % 20]).unwrap_or(0);
                self.wskins[wid as usize % 20] = fits[(at + 1) % fits.len()];
            }
            0 => self.kit_choice = row.min(2),
            1 => self.prims[self.kit_choice as usize % 3] = prims[row as usize % prims.len()],
            3 => {
                let n = sim::SKINS.len() as u8;
                let mut next = (self.skin_choice + 1) % n;
                while !self.profile.owns_look(next) && next != self.skin_choice {
                    next = (next + 1) % n;
                }
                self.skin_choice = next;
            }
            _ => self.side_choice = sim::SIDEARMS[row as usize % sim::SIDEARMS.len()],
        }
        self.audio.play(Sfx::Step, 0.5);
    }
    pub fn render_pos(&self, alpha: f32) -> Vec3 {
        self.prev_pos + (self.st.pos - self.prev_pos) * alpha + self.smooth
    }
    pub fn sight(&self) -> u8 {
        self.sights[sim::wid(&self.st) as usize % 20]
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
            gun: sim::weapon_def(self.st.prim).arch,
            wid: sim::wid(&self.st),
            wskin: self.wskin_of(self.my_id, self.st.slot == 1),
            reload_phase: if self.st.reload > 0 {
                let wd = sim::weapon_def(sim::wid(&self.st));
                let total = if self.st.surge { wd.reload / 2 } else { wd.reload }.max(1);
                (1.0 - self.st.reload as f32 / total as f32).clamp(0.0, 1.0)
            } else {
                -1.0
            },
            cycle: 0.0,
            skin: self.skin_choice,
            crouch: if self.st.crouch { 1.0 } else { 0.0 },
            ..Default::default()
        }
    }

    /// `replay`: the effect is being shown again by the killcam, so it is
    /// only looked at and listened to - our own shots included.
    fn handle_fx(&mut self, f: &Fx, listener: Vec3, now: f64, replay: bool) {
        if replay && !matches!(f, Fx::Shot { .. } | Fx::Boom { .. } | Fx::Swing { .. }) {
            return;
        }
        match *f {
            Fx::Shot { who, from, to, hit } => {
                let (from, to) = (Vec3::from(from), Vec3::from(to));
                let mine = who == self.my_id && !self.spectator && !replay;
                let team = self.team(who);
                if !mine {
                    let len = (to - from).length();
                    let d = (to - from).normalize_or_zero();
                    let col = if team == 0 { [1.0, 0.72, 0.38] } else { [0.5, 0.8, 1.0] };
                    self.fx.tracer(from + d * 0.8 - vec3(0.0, 0.14, 0.0), to, col);
                    let i = who as usize % 64;
                    self.rfire[i] = 1.0;
                    // A round that passes close by cracks in your ear.
                    if team != self.my_team && !self.spectator && self.alive && len > 8.0 && !replay {
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
                        // kc_eye: the killcam is looking down this very barrel.
                        let kc_eye = replay && self.kc.as_ref().is_some_and(|k| k.killer == who);
                        if !kc_eye {
                            self.fx.muzzle(from + d * 0.9 - vec3(0.0, 0.14, 0.0));
                        }
                        let side = self.views.iter().any(|v| v.id == who && v.flags & flag::SIDEARM != 0);
                        let s = audio::shot_sfx(self.wid_of(who, side));
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
            Fx::MapBreak { kind, pos } => {
                let p = Vec3::from(pos);
                self.fx.map_break(p, kind);
                self.audio.play_at(Sfx::Crate, p, listener, 0.85, 32.0);
                self.audio.play_at(Sfx::Boom, p, listener, 0.45, 38.0);
                self.shake = self.shake.max((1.0 - p.distance(listener) / 22.0).max(0.0) * 0.45);
            }
            Fx::Throw { who } => {
                let p = self.last_pos[who as usize % 64];
                self.audio.play_at(Sfx::Throw, p, listener, 0.5, 14.0);
            }
            Fx::Pickup { who, kind, pos } => {
                let p = Vec3::from(pos);
                let k = kind as usize % 5;
                self.fx.hit_burst(p + vec3(0.0, 0.6, 0.0), scene::PICKUP_COL[k]);
                if who == self.my_id && !self.spectator {
                    self.audio.play(Sfx::CapUp, 0.5);
                    self.sub_banner = Some((format!("{}  -  {}", pickup::NAMES[k], pickup::BLURBS[k]), now));
                } else {
                    self.audio.play_at(Sfx::Reload, p, listener, 0.5, 14.0);
                }
            }
            Fx::Swing { who, hit } => {
                // Our own swing was heard when we made it.
                if who != self.my_id || self.spectator || replay {
                    let p = self.last_pos[who as usize % 64];
                    self.audio.play_at(Sfx::Throw, p, listener, 0.55, 12.0);
                    if hit {
                        self.audio.play_at(Sfx::Crate, p, listener, 0.5, 12.0);
                    }
                }
            }
        }
    }

    /// `tick`: the snapshot the entry came in, as near the moment as known.
    fn handle_feed(&mut self, f: &Feed, tick: u32, now: f64) {
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
                    // Killed by someone else: their view of it is replayed.
                    if !self.spectator && killer != self.my_id {
                        self.kc_armed = Some((tick, now, killer, weapon));
                        self.kc = None;
                    }
                }
                let weapon = sim::weapon_name(weapon);
                self.feed.push(KillLine {
                    killer: self.name(killer),
                    kteam: self.team(killer),
                    victim: self.name(victim),
                    vteam,
                    weapon,
                    head,
                    mine: (killer == self.my_id || victim == self.my_id) && !self.spectator,
                    human: !self.is_bot(killer) || !self.is_bot(victim),
                    kicon: self.icon(killer).cloned(),
                    vicon: self.icon(victim).cloned(),
                    at: now,
                });
                // Room for a few lines: bots shooting bots go first.
                if self.feed.len() > 7 {
                    let drop = self.feed.iter().position(|k| !k.human).unwrap_or(0);
                    self.feed.remove(drop);
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
                let map = self.net.latest().map_or(self.world.map, |s| s.map);
                let brief = ferrite_shared::mode_def(self.mode()).brief;
                self.banner = Some((format!("{} - {brief}", ferrite_shared::map::map_def(map).name), scene::NEUTRAL_COL, now));
                self.feed.clear();
                self.award = None;
            }
            Feed::Flag { flag, by, what } => {
                let ours = flag == self.my_team && !self.spectator;
                let who = if by == SPECTATOR_ID { String::new() } else { self.name(by) };
                let side = TEAM_NAMES[flag as usize % 2];
                let me = by == self.my_id && !self.spectator;
                let (msg, good) = match what {
                    flag_ev::TAKEN if me => ("YOU HAVE THEIR FLAG - TAKE IT HOME".to_string(), true),
                    flag_ev::TAKEN => (format!("{who} TOOK THE {side} FLAG"), !ours),
                    flag_ev::DROPPED => (format!("{side} FLAG DROPPED"), ours),
                    flag_ev::RETURNED => (format!("{side} FLAG RETURNED"), ours),
                    _ => (format!("{who} CAPTURED THE {side} FLAG"), !ours),
                };
                self.banner = Some((msg, scene::team_col(flag), now));
                self.audio.play(if good || self.spectator { Sfx::CapUp } else { Sfx::CapDown }, 0.7);
            }
            Feed::Streak { who, n } => {
                let title = match n {
                    3 => "ON A ROLL",
                    5 => "RAMPAGE",
                    7 => "UNSTOPPABLE",
                    _ => "JUGGERNAUT",
                };
                if who == self.my_id && !self.spectator {
                    self.sub_banner = Some((format!("{n} KILL STREAK  -  {title}  -  your reward is where they fell"), now));
                    self.audio.play(Sfx::CapUp, 0.6);
                }
                self.notice(format!("{} is on a {n} kill streak: {title}", self.name(who)), now);
            }
            Feed::Shutdown { by, victim, n } => {
                if by == self.my_id && !self.spectator {
                    self.sub_banner = Some((format!("SHUTDOWN  {}  +100", self.name(victim)), now));
                }
                self.notice(format!("{} ended {}'s {n} kill streak", self.name(by), self.name(victim)), now);
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
        self.armor = me.armor;
        self.streak = me.streak;
        self.power = me.power;
        while self.pending.front().is_some_and(|c| c.seq <= s.ack_seq) {
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
        if self.auto && self.seek && self.alive {
            // Face the first point along the line that is not ours yet.
            let order: Vec<usize> = if self.my_team == 0 { (0..5).collect() } else { (0..5).rev().collect() };
            let goal = match self.mode() {
                mode::CONQUEST => self.net.latest().and_then(|s| order.into_iter().find(|&k| s.points[k].owner != self.my_team)),
                mode::CTF => Some(ferrite_shared::FLAG_POINT[1 - self.my_team as usize % 2]),
                _ => Some(2),
            };
            if let Some(k) = goal {
                let to = self.world.points[k] - self.st.pos;
                if to.x * to.x + to.z * to.z > 9.0 {
                    self.yaw = to.x.atan2(to.z);
                }
            }
        }
        let (yaw, pitch) = self.aim();
        let mut c = Cmd { seq: self.seq, yaw, pitch, interp: self.net.render_tick(now), ..Default::default() };
        if self.auto {
            // Scripted input for unattended end-to-end runs.
            let t = now - self.start;
            c.mz = 1;
            if self.demo_side && self.st.slot == 0 && t > 2.0 {
                c.buttons |= btn::SWAP;
            }
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
            if t % 4.0 > 3.97 {
                c.more |= btn2::MELEE;
            }
        } else if self.captured && !self.input_blocked && !self.loadout_open && !self.team_pick && !self.chat_open && !self.lobby {
            let keyboard = |action| !typing() && self.bind_down(action);
            c.mz = keyboard(Action::Forward) as i8 - keyboard(Action::Backward) as i8;
            c.mx = keyboard(Action::StrafeRight) as i8 - keyboard(Action::StrafeLeft) as i8;
            let (right, forward) = controller::movement();
            let (tr, tf) = touch::movement();
            let (right, forward) = (right + tr, forward + tf);
            if right != 0.0 || forward != 0.0 {
                c.set_analog_movement((c.mx as f32 + right).clamp(-1.0, 1.0), (c.mz as f32 + forward).clamp(-1.0, 1.0));
            }
            if self.action_down(Action::Jump) || self.jump_latch {
                c.buttons |= btn::JUMP;
            }
            if self.action_down(Action::Sprint) {
                c.buttons |= btn::SPRINT;
            }
            if self.action_down(Action::Fire) || self.fire_latch {
                c.buttons |= btn::FIRE;
            }
            if self.aim_on {
                c.buttons |= btn::ADS;
            }
            if self.action_down(Action::Crouch) {
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
            if self.melee_latch {
                c.more |= btn2::MELEE;
            }
            if self.action_down(Action::Use) {
                c.more |= btn2::USE;
            }
        }
        self.fire_latch = false;
        self.melee_latch = false;
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
                if !matches!(kit, 2 | 15) {
                    let port = origin + fwd * 0.5 + right * 0.2 - vec3(0.0, 0.12, 0.0);
                    self.fx.shell(port, right * 2.4 + vec3(0.0, 1.9, 0.0) + self.st.vel, wd.arch == 1);
                }
            }
            self.mantling = out.mantle;
            if out.slid {
                self.audio.play(Sfx::Slide, 0.6);
            }
            if self.st.slide > 0 && self.st.on_ground && self.seq.is_multiple_of(4) {
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
            if out.melee {
                self.audio.play(Sfx::Throw, 0.7);
            }
            if out.landed {
                self.audio.play(Sfx::Step, 0.7);
                self.step_y -= 0.12;
                self.land = 1.0;
                self.fx.dust(self.st.pos, 1.0);
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
        self.net.send_cmds(&recent, self.loadout(), self.skin_choice, self.wskin_choice(), now);
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
    mq::prevent_quit();
    let _controller_shutdown = controller::ShutdownGuard;
    let args = parse_args();
    // Scripted and command-line launches go straight into a match.
    let direct = args.connect.is_some() || args.host || args.auto || args.spectate || args.shot.is_some() || args.quit_at.is_some();
    let art = ui::init();
    menu::Menu::apply_saved_audio_device();
    let scripted = args.auto || args.spectate || args.shot.is_some() || args.quit_at.is_some();
    if direct {
        let choice = match &args.connect {
            Some(a) => menu::Choice::Join(a.clone()),
            None => menu::Choice::Host {
                port: args.port,
                per_team: args.per_team,
                max_bots: args.max_bots.unwrap_or(64),
                map: args.map.unwrap_or(0),
                rotate: !args.no_rotate,
                lobby: args.lobby.unwrap_or(if scripted { 0 } else { 30 }),
                skill: args.skill,
                mode: args.mode.unwrap_or(0),
                mode_rotate: false,
            },
        };
        let mut loadout = [args.kit, 0, 1, 2, args.side, args.skin];
        loadout[1 + args.kit as usize % 3] = args.prim;
        let mut music = if scripted || args.no_music || args.volume <= 0.0 { Music::silent() } else { Music::new(0.5) };
        let mut volume = args.volume;
        // Who we are comes from the settings file even on a direct launch.
        // A scripted run reads it but leaves the file alone.
        let mut saved = menu::Menu::load(&args.name, args.port, args.per_team);
        if !scripted && !args.no_music {
            music.on = saved.music_on;
            music.vol = saved.music_vol;
            music.in_match = saved.music_in_match;
        }
        let mut prefs = saved.prefs.clone();
        if std::env::args().any(|a| a == "--shadows") {
            prefs.shadows = args.shadows;
        }
        let mut sens = saved.sens;
        if !scripted {
            saved.persist();
        }
        let icon = match args.icon {
            Some(k) => ident::preset(k, k),
            None => saved.icon_rgba(),
        };
        let mut wskins = saved.wskins;
        let mut sights = saved.sights;
        let mut profile = saved.profile.clone();
        let (_, msg) = play(
            &args,
            &args.name,
            args.team,
            choice,
            &mut sens,
            &mut loadout,
            &mut volume,
            &mut music,
            &art,
            &prefs,
            saved.uuid,
            &icon,
            &mut wskins,
            &mut sights,
            &mut profile,
        )
        .await;
        update::confirm_start(); // it ran: a fresh update works
        if !scripted {
            saved.profile = profile;
            saved.prefs.fullscreen = is_fullscreen();
            saved.wskins = wskins;
            saved.sights = sights;
            saved.persist();
        }
        if let Some(m) = msg {
            eprintln!("{m}");
            for _ in 0..240 {
                if mq::is_quit_requested() {
                    break;
                }
                mq::clear_background(mq::Color::new(0.09, 0.08, 0.08, 1.0));
                mq::draw_text(&m, 40.0, 60.0, 28.0, mq::ORANGE);
                mq::next_frame().await;
            }
        }
        return;
    }
    let mut menu = menu::Menu::load(&args.name, args.port, args.per_team);
    if args.menu_shot.is_none() {
        menu.persist(); // a first launch has just been given its id
    }
    if let Some(k) = args.icon {
        menu.icon_kind = k + 1;
        menu.icon_colour = k;
    }
    let look = menu.updates && !args.no_update && (args.menu_shot.is_none() || args.pretend_version.is_some());
    menu.updater = update::Updater::new(look, args.pretend_version.as_deref());
    if let Some(m) = args.map {
        menu.map = m;
    }
    menu.shot = args.menu_shot.clone();
    menu.seed = args.seed;
    if std::env::args().any(|a| a == "--shadows") {
        menu.prefs.shadows = args.shadows;
    }
    if std::env::args().any(|a| a == "--fullscreen") {
        menu.prefs.fullscreen = true;
        FULLSCREEN.store(true, std::sync::atomic::Ordering::Relaxed); // the window opened that way
    } else if menu.prefs.fullscreen {
        set_fullscreen(true);
    }
    menu.go_to(args.menu_page % 10);
    menu.tab = (args.menu_page / 10).min(2);
    menu.shop_tab = (args.menu_page / 10).min(1);
    menu.ob_step = (args.menu_page / 10).min(3);
    if let Some(id) = args.wskin {
        menu.review_finish(id, args.prim);
    }
    // A first launch starts with the introduction.
    if args.menu_shot.is_none() && !menu.profile.onboarded {
        menu.go_to(5);
    }
    let mut music = if args.no_music { Music::silent() } else { Music::new(menu.music_vol) };
    loop {
        let choice = menu.run(&mut music, &art).await;
        let joining = match &choice {
            menu::Choice::Quit => break,
            menu::Choice::Join(a) => Some(a.clone()),
            menu::Choice::Host { .. } => None,
        };
        let name = menu.name();
        let prefs = menu.prefs.clone();
        let (uuid, icon) = (menu.uuid, menu.icon_rgba());
        let (joined, msg) = play(
            &args,
            &name,
            2,
            choice,
            &mut menu.sens,
            &mut menu.loadout,
            &mut menu.volume,
            &mut music,
            &art,
            &prefs,
            uuid,
            &icon,
            &mut menu.wskins,
            &mut menu.sights,
            &mut menu.profile,
        )
        .await;
        if music.available() {
            menu.music_vol = music.vol;
            menu.music_in_match = music.in_match;
        }
        menu.prefs.fullscreen = is_fullscreen(); // F11 in the match
        menu.persist();
        if mq::is_quit_requested() {
            break;
        }
        if let (true, Some(a)) = (joined, joining) {
            menu.remember(&a);
        }
        menu.error = msg;
    }
}

/// Run one match. Returns whether we got into the match, and a message for
/// the home screen if it ended for a reason other than the player leaving.
#[allow(clippy::too_many_arguments)]
async fn play(
    args: &Args,
    name: &str,
    team: u8,
    choice: menu::Choice,
    sens: &mut f32,
    loadout: &mut [u8; 6],
    volume: &mut f32,
    music: &mut Music,
    art: &ui::Art,
    prefs: &menu::Prefs,
    uuid: [u8; 16],
    icon: &[u8],
    wskins: &mut [u8; 20],
    sights: &mut [u8; 20],
    profile: &mut menu::Profile,
) -> (bool, Option<String>) {
    touch::set_enabled(args.touch || prefs.touch);
    let cast = match &args.cine_cast {
        Some(p) => match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| cine::parse_cast(&t)) {
            Ok(c) => c,
            Err(e) => return (false, Some(format!("--cine-cast {p}: {e}"))),
        },
        None => Vec::new(),
    };
    let mut server = None;
    let addr = match choice {
        menu::Choice::Join(a) => {
            let a = if a.contains(':') { a } else { format!("{a}:{DEFAULT_PORT}") };
            match a.to_socket_addrs().ok().and_then(|mut i| i.next()) {
                Some(x) => x,
                None => return (false, Some(format!("Cannot resolve {a}"))),
            }
        }
        menu::Choice::Host { port, per_team, max_bots, map, rotate, lobby, skill, mode, mode_rotate } => {
            let h = match ferrite_server::spawn(ferrite_server::Config {
                port,
                per_team,
                max_bots,
                map,
                rotation: if rotate { (0..ferrite_shared::map::MAPS.len() as u8).collect() } else { Vec::new() },
                lobby_secs: lobby,
                seed: args.seed,
                tickets: args.tickets,
                bot_skill: skill,
                log: false,
                name: format!("{}'s game", name.chars().take(16).collect::<String>()),
                discover: true,
                // Records of everyone who plays on games hosted from here.
                // Scripted runs leave the file alone.
                stats_file: if args.auto || args.shot.is_some() || args.quit_at.is_some() {
                    None
                } else {
                    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("ferrite-stats.txt")))
                },
                mode,
                mode_rotation: if mode_rotate { (0..ferrite_shared::MODES.len() as u8).collect() } else { Vec::new() },
                score_limit: args.score,
                time_limit: args.time,
                drops: true,
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
    {
        let (sw, sh) = (mq::screen_width(), mq::screen_height());
        mq::clear_background(ui::color::INK);
        ui::backdrop(&art.lobby, sw, sh);
        let (lx, ly, lw) = ui::LOBBY_LOGO;
        ui::logo(art, sw, sh, lx, ly, lw);
        mq::draw_rectangle(0.0, sh - 64.0, sw, 64.0, ui::color::GLASS);
        ui::text(&format!("CONNECTING TO {addr}"), (78.0 * ui::art_scale(sw, sh)).max(24.0), sh - 24.0, 24.0, ui::color::WHITE);
        mq::next_frame().await;
    }

    let (net, welcome) = match Net::connect(addr, name, args.spectate, team, uuid, icon) {
        Ok(x) => x,
        Err(e) => return (false, Some(format!("Could not join {addr}: {e}"))),
    };
    let seed = welcome.seed;
    let world = World::with_map(seed, welcome.map);
    let bounds = scene::shadow_bounds(world.map);
    let mut gpu = gfx::Gpu::new(
        scene::build_world(&world),
        scene::build_sky_for(world.map),
        prefs.shadows,
        scene::sun_for(world.map).normalize(),
        bounds,
    )
    .with_scenery(scene::build_scenery(&world));
    let map_name = ferrite_shared::map::map_def(world.map).name;
    let audio = Audio::new(*volume).await;
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
        kit_choice: loadout[0] % 3,
        prims: [0u8, 1, 2].map(|k| sim::fix_loadout(k, loadout[1 + k as usize], 0).1),
        side_choice: sim::fix_loadout(0, 0, loadout[4]).2,
        skin_choice: loadout[5] % sim::SKINS.len() as u8,
        loadout_open: false,
        lobby: false,
        vote: 255,
        ready: false,
        demo_panel: args.show_loadout,
        demo_scores: args.show_scores,
        demo_side: args.auto && args.side != 255,
        step_y: 0.0,
        eye: sim::EYE,
        jump_latch: false,
        fire_latch: false,
        gren_latch: false,
        swap_latch: false,
        reload_latch: false,
        captured: false,
        input_blocked: true,
        sens: sens.clamp(0.0004, 0.01),
        bindings: prefs.bindings.clone(),
        last_mouse: (0.0, 0.0),
        ads: 0.0,
        fov: prefs.fov,
        base_fov: prefs.fov,
        invert_y: prefs.invert_y,
        toggle_aim: prefs.toggle_aim,
        aim_on: false,
        aim_toggle_on: false,
        recoil: 0.0,
        recoil_p: 0.0,
        recoil_y: 0.0,
        last_fire: 0.0,
        shot_seq: 0,
        land: 0.0,
        mantling: false,
        mantle_vis: 0.0,
        crank_hold: 0.0,
        shade_heard: None,
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
        rreload_phase: [-1.0; 64],
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
        dead_for: 0.0,
        kill_cam: None,
        spec_killer: false,
        now_playing: None,
        music_in_match: music.in_match,
        lobby_bg: art.lobby.clone(),
        art: art.clone(),
        pause_settings: false,
        deployed_once: false,
        music_vol: music.vol,
        show_net: prefs.show_fps,
        team_pick: false,
        sights: std::array::from_fn(|w| match args.sight {
            Some(s) => s.min(5),
            None => sights[w],
        }),
        stats: Stats::default(),
        hosting,
        start,
        chat: Vec::new(),
        chat_open: false,
        chat_team: false,
        chat_buf: String::new(),
        icons: std::collections::HashMap::new(),
        icon_ask: 0.0,
        wskins: *wskins,
        review_wskin: args.wskin,
        melee_latch: false,
        seek: args.seek,
        rmelee: [0.0; 64],
        kc_armed: None,
        kc: None,
        mode_vote: 255,
        profile: profile.clone(),
        award: None,
        armor: 0,
        streak: 0,
        power: 0,
        touch: touch::enabled(),
        cine: cine::Cine::default(),
        last_cam: (Vec3::ZERO, Vec3::Z),
        cast,
        cast_clock: 0.0,
    };
    if let Some((eye, target)) = args.cine {
        app.cine.script(eye, target, args.cine_fov, args.cine_focus, args.cine_aperture);
    }
    app.cine.scale = args.shot_scale;
    // A look or a finish that is not ours (a hand-edited settings file)
    // goes back to standard issue.
    if !app.profile.owns_look(app.skin_choice) {
        app.skin_choice = 0;
    }
    for s in app.wskins.iter_mut() {
        if !app.profile.owns_finish(*s) {
            *s = 0;
        }
    }
    app.team_pick = args.pick_team;
    app.banner = Some((map_name.to_string(), scene::NEUTRAL_COL, start + 0.5));
    let (mut opaque, mut alpha, mut add) = (gfx::Mesh::default(), gfx::Mesh::default(), gfx::Mesh::default());
    let (mut vm_opaque, mut vm_add) = (gfx::Mesh::default(), gfx::Mesh::default());
    let mut over_mesh = gfx::Mesh::default();
    let mut acc = 0.0f32;
    let mut last = mq::get_time();
    let mut listener = Vec3::ZERO;
    let mut shot_done = false;
    let mut peak_tris = 0usize;
    // The visual clock as of the last frame, for monument sound cues.
    let mut cue_clock = f32::NAN;
    let (mut frames, began) = (0u32, mq::get_time());
    let mut grabbed = false;
    let mut lobby_sent = 0.0f64;
    let mut sway = (0.0f32, 0.0f32);
    let mut ended: Option<String> = None;
    let mut chat_repeat = 0.0f64;
    let mut said = false;

    loop {
        let now = mq::get_time();
        let frame = (now - last).min(0.1);
        last = now;
        let dt = frame as f32;
        let ui_input = !app.alive || !app.captured || app.loadout_open || app.team_pick || app.lobby || app.chat_open;
        controller::poll(&prefs.controller, dt, ui_input);
        // (a scripted run shows the controls whenever its soldier is up)
        touch::poll(if app.auto { app.alive } else { grabbed && !ui_input });
        if touch::enabled() != app.touch {
            // A first touch on a touch screen: hand the mouse back.
            app.touch = touch::enabled();
            mq::set_cursor_grab(false);
            mq::show_mouse(true);
        }
        app.stats.frames += 1;
        app.stats.frame_sum += frame;
        if app.stats.frames > 30 {
            app.stats.frame_max = app.stats.frame_max.max(frame);
        }
        app.audio.begin_frame();
        // The track belongs to the lobby; it fades out as the round starts.
        // The lobby has its track; the song rotation carries on through
        // the match if the player wants it there.
        music.vol = app.music_vol;
        music.in_match = app.music_in_match;
        let music_scene = if app.lobby {
            // The briefing names the winning next map while its predecessor
            // is still loaded; let that map choose the lobby score too.
            Scene::Lobby(app.net.latest().map_or(app.world.map, |snap| snap.next_map))
        } else {
            Scene::Match(app.world.map)
        };
        music.update(music_scene, dt).await;
        app.now_playing = music.now_playing().map(|t| t.to_string());

        // ---- chat: while the box is open it has the keyboard ----
        TYPING.store(app.chat_open, std::sync::atomic::Ordering::Relaxed);
        {
            use macroquad::input::{is_key_down as kd, is_key_pressed as kp};
            use mq::KeyCode as K;
            if app.chat_open {
                while let Some(c) = mq::get_char_pressed() {
                    // (some systems report arrow keys as private-use characters)
                    if !c.is_control() && !(0xE000..=0xF8FF).contains(&(c as u32)) && app.chat_buf.chars().count() < CHAT_MAX {
                        app.chat_buf.push(c);
                    }
                }
                if kp(K::Backspace) {
                    app.chat_buf.pop();
                    chat_repeat = now + 0.4;
                } else if kd(K::Backspace) && now > chat_repeat {
                    app.chat_buf.pop();
                    chat_repeat = now + 0.04;
                }
                if kp(K::Tab) {
                    app.chat_team = !app.chat_team;
                }
                if kp(K::Enter) || kp(K::KpEnter) {
                    let text = app.chat_buf.trim().to_string();
                    if !text.is_empty() {
                        app.net.say(app.chat_team, &text);
                    }
                    app.chat_open = false;
                    app.chat_buf.clear();
                } else if kp(K::Escape) || controller::menu_back() {
                    app.chat_open = false;
                    app.chat_buf.clear();
                }
            } else if !app.spectator
                && !app.auto
                && !app.team_pick
                && !app.cine.on
                && (app.bind_pressed(Action::ChatAll) || app.bind_pressed(Action::ChatTeam))
            {
                app.chat_open = true;
                app.chat_team = app.bind_pressed(Action::ChatTeam);
                app.chat_buf.clear();
                while mq::get_char_pressed().is_some() {}
                // The key that opened the box is not also a game key.
                TYPING.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if let (false, true, Some(text)) = (said, app.auto && now - start > 2.0, &args.say) {
            said = true;
            app.net.say(false, text);
        }

        // ---- input: mouse capture, look, edge-triggered keys ----
        // Team picker: M opens it; click a side, or M / Esc to cancel.
        let mut picked = false;
        let back_pressed = !typing()
            && (pressed(mq::KeyCode::Escape)
                || controller::pause_pressed()
                || touch::pause_pressed()
                || (ui_input && controller::menu_back()));
        // Cinematic camera: its key turns it on and off; Esc leaves it.
        let cine_key = app.action_pressed(Action::Cinematic);
        if app.cine.on && (cine_key || back_pressed) {
            app.cine.stop();
            picked = true; // swallow this Esc
        } else if cine_key && !app.team_pick && !app.lobby {
            app.cine.start(app.last_cam.0, app.last_cam.1, app.fov);
            app.captured = true;
            app.loadout_open = false;
        }
        if app.action_pressed(Action::HideHud) {
            app.cine.hide_hud = !app.cine.hide_hud;
        }
        let want_shot = app.action_pressed(Action::Screenshot);
        if app.team_pick {
            if app.action_pressed(Action::ChangeTeam) || back_pressed {
                app.team_pick = false;
                picked = true; // swallow this Esc
            } else if controller::left_pressed() {
                let (mx, my) = controller::mouse_position();
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
        } else if app.action_pressed(Action::ChangeTeam) && !app.spectator && !app.auto && !app.cine.on {
            app.team_pick = true;
            app.captured = false;
        }
        // Loadout panel: always up while dead, or opened with L. Click a
        // row, or use the number keys.
        if !app.alive {
            app.loadout_open = false;
        } else if app.loadout_open && back_pressed {
            app.loadout_open = false;
            picked = true; // swallow this Esc
        } else if app.action_pressed(Action::Loadout) && app.captured && !app.spectator && !app.auto && !app.team_pick && !app.cine.on {
            app.loadout_open = !app.loadout_open;
        }
        // Lobby: vote for a map, say when you are ready.
        let in_lobby = app.net.latest().is_some_and(|s| s.phase == 2);
        if in_lobby != app.lobby {
            app.lobby = in_lobby;
            app.vote = 255;
            app.mode_vote = 255;
            app.ready = false;
            app.loadout_open = false;
            if !in_lobby && !app.auto && !app.spectator {
                app.captured = true; // the match is starting: take the mouse
            }
        }
        if app.lobby && !app.spectator && !app.team_pick && !app.cine.on {
            let before = (app.vote, app.ready, app.mode_vote);
            if pressed(mq::KeyCode::R) {
                app.ready = !app.ready;
            }
            if !picked && controller::left_pressed() {
                let (mx, my) = controller::mouse_position();
                let inside = |r: (f32, f32, f32, f32)| mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3;
                let (cards, ready_btn, modes) = hud::lobby_rects(mq::screen_width(), mq::screen_height());
                for (k, r) in modes.into_iter().enumerate() {
                    if inside(r) {
                        app.mode_vote = if app.mode_vote == k as u8 { 255 } else { k as u8 };
                        picked = true;
                    }
                }
                for (k, r) in cards.into_iter().enumerate() {
                    if inside(r) {
                        app.vote = if app.vote == k as u8 { 255 } else { k as u8 };
                        picked = true;
                    }
                }
                if inside(ready_btn) {
                    app.ready = !app.ready;
                    picked = true;
                }
            }
            // Resent a few times a second: these ride on plain UDP.
            if before != (app.vote, app.ready, app.mode_vote) || now - lobby_sent > 0.3 {
                if before != (app.vote, app.ready, app.mode_vote) {
                    app.audio.play(Sfx::Step, 0.5);
                }
                lobby_sent = now;
                app.net.send_lobby(app.vote, app.ready, app.mode_vote);
            }
        }
        if app.loadout_shown() && !picked && controller::left_pressed() {
            let (mx, my) = controller::mouse_position();
            let (kit, prim, side) = app.loadout();
            for (c, r, x, y, w, h) in hud::loadout_rects(mq::screen_width(), mq::screen_height(), kit, prim, side) {
                if mx >= x && mx < x + w && my >= y && my < y + h {
                    app.pick_loadout(c, r);
                    picked = true;
                }
            }
        }
        let mut leave = false;
        if picked || app.team_pick {
        } else if back_pressed && app.captured {
            app.captured = false;
            app.pause_settings = false;
        } else if app.paused() {
            // Pause overlay: its buttons, or the keys it lists.
            let mut act = None;
            if back_pressed {
                act = Some(if app.pause_settings { 18 } else { 0 });
            } else if app.action_pressed(Action::Loadout) {
                act = Some(1);
            } else if controller::left_pressed() {
                let (mx, my) = controller::mouse_position();
                for (id, x, y, w, h) in hud::pause_rects(mq::screen_width(), mq::screen_height(), app.pause_settings) {
                    if mx >= x && mx < x + w && my >= y && my < y + h {
                        act = Some(id);
                    }
                }
            }
            let nudge = |v: f32, up: bool| ((v * 10.0).round() + if up { 1.0 } else { -1.0 }).clamp(0.0, 10.0) / 10.0;
            match act {
                Some(0) => app.captured = true,
                Some(1) => {
                    app.captured = true;
                    app.loadout_open = app.alive;
                }
                Some(2) => app.pause_settings = true,
                Some(3) => app.team_pick = true,
                Some(4) => leave = true,
                Some(10) => app.sens = (app.sens / 1.1).max(0.0004),
                Some(11) => app.sens = (app.sens * 1.1).min(0.01),
                Some(12) => app.audio.volume = nudge(app.audio.volume, false),
                Some(13) => app.audio.volume = nudge(app.audio.volume, true),
                Some(14) => app.music_vol = nudge(app.music_vol, false),
                Some(15) => app.music_vol = nudge(app.music_vol, true),
                Some(16) | Some(17) => app.music_in_match = !app.music_in_match,
                Some(18) => app.pause_settings = false,
                _ => {}
            }
            if act.is_some() {
                picked = true;
                app.audio.play(Sfx::Step, 0.5);
            }
        }
        if app.captured {
            app.deployed_once = true;
            app.pause_settings = false;
        }
        // The mouse is only held while you are alive: dead, it is visible and
        // free to leave the window, and it is taken back when you deploy.
        let want_grab = app.captured && (app.alive || app.cine.on) && !app.loadout_open;
        if want_grab != grabbed {
            grabbed = want_grab;
            if !app.touch {
                mq::set_cursor_grab(grabbed);
                mq::show_mouse(!grabbed);
            }
            app.last_mouse = mq::mouse_position();
        }
        if leave
            || !app.team_pick && !picked && (!app.captured || app.spectator) && (pressed(mq::KeyCode::Q) || (app.spectator && back_pressed))
        {
            break;
        }
        app.input_blocked = ui_input || picked || !grabbed || typing() || app.cine.on;
        if app.input_blocked {
            app.jump_latch = false;
            app.fire_latch = false;
            app.reload_latch = false;
            app.gren_latch = false;
            app.swap_latch = false;
            app.melee_latch = false;
        }
        let m = mq::mouse_position();
        let (mdx, mdy) = ((m.0 - app.last_mouse.0).clamp(-400.0, 400.0), (m.1 - app.last_mouse.1).clamp(-400.0, 400.0));
        app.last_mouse = m;
        // A finger dragging the view stands in for the mouse.
        let (mdx, mdy) = if app.touch {
            let t = touch::look_delta();
            (t.0 * 2.4, t.1 * 2.4)
        } else {
            (mdx, mdy)
        };
        if app.cine.on {
            // The free camera takes the mouse and the movement keys; the
            // soldier stands still.
            let key = |a| !typing() && app.bind_down(a);
            let held = |k| !typing() && mq::is_key_down(k);
            let axis = |a: bool, b: bool| a as i8 as f32 - b as i8 as f32;
            let c = cine::Controls {
                look: if grabbed && !picked { (mdx, if app.invert_y { -mdy } else { mdy }) } else { (0.0, 0.0) },
                sens: app.sens,
                mv: vec3(
                    axis(key(Action::StrafeRight), key(Action::StrafeLeft)),
                    axis(key(Action::Jump) || held(mq::KeyCode::E), key(Action::Crouch) || held(mq::KeyCode::Q)),
                    axis(key(Action::Forward), key(Action::Backward)),
                ),
                fast: key(Action::Sprint),
                slow: held(mq::KeyCode::LeftAlt) || held(mq::KeyCode::RightAlt),
            };
            if !typing() {
                app.cine.update(&c, dt, now);
            }
        } else if grabbed && !picked && !typing() {
            let s = app.sens * (app.fov / app.base_fov);
            let mdy = if app.invert_y { -mdy } else { mdy };
            app.yaw = ferrite_shared::wrap_angle(app.yaw - mdx * s);
            let pad_look = controller::look_delta();
            app.yaw = ferrite_shared::wrap_angle(app.yaw - pad_look.0 * (app.fov / app.base_fov));
            app.pitch = (app.pitch - mdy * s + pad_look.1 * (app.fov / app.base_fov)).clamp(-1.5, 1.5);
            sway.0 = (sway.0 - mdx * 0.00004).clamp(-0.02, 0.02);
            sway.1 = (sway.1 + mdy * 0.00004).clamp(-0.02, 0.02);
            app.jump_latch |= app.action_pressed(Action::Jump);
            app.fire_latch |= app.action_pressed(Action::Fire);
            app.reload_latch |= app.action_pressed(Action::Reload);
            app.gren_latch |= app.action_pressed(Action::Grenade);
            app.swap_latch |= app.action_pressed(Action::SwapWeapon);
            app.melee_latch |= app.action_pressed(Action::Melee);
        }
        if app.spectator && !app.cine.on && controller::left_pressed() {
            app.spec_since = -100.0;
        }
        sway.0 *= 1.0 - (10.0 * dt).min(1.0);
        sway.1 *= 1.0 - (10.0 * dt).min(1.0);
        // 1-3 class, 4 next primary, 5 next sidearm, 6 next look. Only while
        // the loadout panel is up (lobby, dead, or L): never mid-fight.
        let panel = app.loadout_shown() && !app.cine.on;
        if panel && !picked && app.action_pressed(Action::NextLook) && !app.spectator {
            app.pick_loadout(3, 0);
            app.sub_banner = Some((format!("LOOK: {}", sim::SKINS[app.skin_choice as usize % sim::SKINS.len()].0), now));
        }
        // 7 / 8 by default: next finish for the primary / sidearm, where there is one.
        for (col, action) in [(4u8, Action::PrimaryFinish), (5, Action::SidearmFinish)] {
            let (_, prim, side) = app.loadout();
            let wid = if col == 4 { prim } else { side };
            if panel && !picked && app.action_pressed(action) && !app.spectator && sim::weapon_skins_for(wid).len() > 1 {
                app.pick_loadout(col, 0);
                let fin = sim::weapon_skin(app.wskins[wid as usize % 20]).name;
                app.sub_banner = Some((format!("{} FINISH: {}", sim::weapon_def(wid).name.to_uppercase(), fin), now));
            }
        }
        for (k, action) in [Action::ClassVanguard, Action::ClassBreacher, Action::ClassMarksman, Action::NextPrimary, Action::NextSidearm]
            .into_iter()
            .enumerate()
        {
            if panel && !picked && app.action_pressed(action) && !app.spectator {
                let (kit, prim, side) = app.loadout();
                match k {
                    3 => {
                        let prims = sim::kit(kit).prims;
                        let at = prims.iter().position(|&p| p == prim).unwrap_or(0);
                        app.pick_loadout(1, (at as u8 + 1) % prims.len() as u8);
                    }
                    4 => {
                        let at = sim::SIDEARMS.iter().position(|&s| s == side).unwrap_or(0);
                        app.pick_loadout(2, (at as u8 + 1) % sim::SIDEARMS.len() as u8);
                    }
                    _ => app.pick_loadout(0, k as u8),
                }
                let (kit, prim, side) = app.loadout();
                app.sub_banner = Some((
                    format!("NEXT DEPLOY: {} - {} - {}", sim::kit(kit).name, sim::weapon_def(prim).name, sim::weapon_def(side).name),
                    now,
                ));
            }
        }
        if grabbed && !picked && app.action_pressed(Action::CycleSight) && app.alive && !app.spectator && !app.cine.on {
            // Cycle the sight on the weapon in hand.
            let w = sim::wid(&app.st);
            let opts = scene::sights_for(w);
            let cur = opts.iter().position(|&s| s == app.sights[w as usize % 20]).unwrap_or(0);
            let next = opts[(cur + 1) % opts.len()];
            app.sights[w as usize % 20] = next;
            app.sub_banner = Some((format!("{}  -  {}", sim::weapon_def(w).name, scene::SIGHT_NAMES[next as usize]), now));
            app.audio.play(Sfx::Step, 0.5);
        }
        f11_fullscreen();
        if app.action_pressed(Action::ToggleStats) {
            app.show_net = !app.show_net;
        }
        if app.action_pressed(Action::SensitivityDown) && !app.cine.on {
            app.sens = (app.sens * 0.9).max(0.0004);
        }
        if app.action_pressed(Action::SensitivityUp) && !app.cine.on {
            app.sens = (app.sens * 1.1).min(0.01);
        }

        // Mouse/keyboard aim can toggle; controller triggers always act as a hold.
        if app.input_blocked || !app.alive || !app.captured || app.loadout_open || app.team_pick || typing() {
            app.aim_on = false;
            app.aim_toggle_on = false;
        } else {
            if app.toggle_aim {
                if app.bind_pressed(Action::Aim) {
                    app.aim_toggle_on = !app.aim_toggle_on;
                }
            } else {
                app.aim_toggle_on = app.bind_down(Action::Aim);
            }
            app.aim_on = app.aim_toggle_on || controller::down(Action::Aim) || touch::down(Action::Aim);
        }

        // ---- network in ----
        app.net.poll(now);
        // The server moved to the next map in its rotation: rebuild the
        // world, its mesh and its shadow map.
        if let Some(m) = app.net.latest().map(|s| s.map) {
            if m != app.world.map {
                app.world = World::with_map(seed, m);
                let fresh = gfx::Gpu::new(
                    scene::build_world(&app.world),
                    scene::build_sky_for(app.world.map),
                    prefs.shadows,
                    scene::sun_for(app.world.map).normalize(),
                    scene::shadow_bounds(app.world.map),
                )
                .with_scenery(scene::build_scenery(&app.world));
                std::mem::replace(&mut gpu, fresh).destroy();
                app.fx = fx::Fx::new();
                app.corpses.clear();
                app.pending.clear();
                app.kc = None;
                app.kc_armed = None;
            }
        }
        let frozen = app.cine.on && app.cine.freeze;
        let fresh = std::mem::take(&mut app.net.fresh);
        for s in &fresh {
            app.fx.remove_destroyed_decals(&app.world, s.dmask & !app.dmask);
            app.dmask = s.dmask;
            for f in &s.fx {
                // While the killcam plays, what is seen and heard is the
                // past; the present is only kept track of.
                if (app.kc.is_none() || matches!(f, Fx::Hit { .. })) && !frozen {
                    app.handle_fx(f, listener, now, false);
                }
            }
            // Entries come in order. Normally each is the next one; after a
            // long stall the server may have dropped some, so take any newer.
            for (id, f) in &s.feed {
                if *id > app.net.ack_feed {
                    app.net.ack_feed = *id;
                    app.handle_feed(f, s.tick, now);
                }
            }
        }
        if let Some(s) = fresh.last() {
            app.reconcile(s);
        }
        // What the round just finished paid.
        for (credits, won) in std::mem::take(&mut app.net.awards) {
            if !app.spectator {
                app.profile.bank(credits, won);
                app.award = Some((credits, won, now));
                *profile = app.profile.clone();
            }
        }
        // Chat that has arrived, and the icons of whoever is here.
        for (id, hash, px) in std::mem::take(&mut app.net.icon_in) {
            if let Some(t) = ident::texture(&px) {
                app.icons.insert(id, (hash, t));
            }
        }
        let listed = app.net.icon_ids.clone();
        app.icons.retain(|id, v| listed.iter().any(|(i, h)| i == id && *h == v.0));
        if now - app.icon_ask > 0.15 {
            if let Some((id, _)) = listed.iter().find(|(i, h)| app.icons.get(i).map(|v| v.0) != Some(*h)) {
                app.net.want_icon(*id);
                app.icon_ask = now;
            }
        }
        for l in std::mem::take(&mut app.net.chat_in) {
            let system = l.from == SPECTATOR_ID;
            let icon = app.icon(l.from).cloned();
            app.chat.push(ChatMsg { team: l.team, all: l.all, system, name: l.name, text: l.text, icon, at: now });
            if !system {
                app.audio.play(Sfx::Step, 0.4);
            }
        }
        if app.chat.len() > 60 {
            app.chat.drain(..20);
        }
        // The roster is the authority on which side we are on.
        if !app.spectator {
            if let Some(t) = app.net.roster.iter().find(|r| r.id == app.my_id).map(|r| r.team) {
                if t != app.my_team && t < 2 {
                    app.my_team = t;
                    app.banner = Some((
                        format!("YOU JOINED {}", TEAM_NAMES[t as usize]),
                        if t == 0 { [1.0, 0.72, 0.38] } else { [0.5, 0.8, 1.0] },
                        now,
                    ));
                }
            }
        }

        if mq::is_quit_requested() {
            break;
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
        let ads_held = playing && if app.auto { (now - app.start) % 7.0 > 5.5 } else { app.aim_on };
        let reloading = app.st.reload > 0;
        let ads_target = if ads_held && !reloading { 1.0 } else { 0.0 };
        app.ads += (ads_target - app.ads) * (14.0 * dt).min(1.0);
        let sprinting = playing && app.hspeed() > sim::WALK * 1.2;
        app.sprint_vis += (if sprinting { 1.0 } else { 0.0 } - app.sprint_vis) * (9.0 * dt).min(1.0);
        let sight = app.sight();
        let ease = |cur: &mut f32, on: bool, rate: f32| *cur += (if on { 1.0 } else { 0.0 } - *cur) * (rate * dt).min(1.0);
        ease(&mut app.slide_vis, playing && app.st.slide > 0, 12.0);
        ease(&mut app.air_vis, playing && !app.st.on_ground, 8.0);
        // On a ladder the weapon drops out of the way, as for a mantle:
        // both hands are on the rungs and it cannot be fired.
        let climbing = playing && (app.mantling || app.st.climbing);
        ease(&mut app.mantle_vis, climbing, 10.0);
        let side_v = if playing { app.st.vel.dot(sim::right_of(app.yaw)) / sim::WALK } else { 0.0 };
        app.strafe_vis += (side_v.clamp(-1.0, 1.0) - app.strafe_vis) * (8.0 * dt).min(1.0);
        // Sights were tuned against a 66 degree hip view: keep their zoom.
        let zoomed = scene::sight_fov(kit, sight) * app.base_fov / 66.0;
        app.fov = app.base_fov + (zoomed - app.base_fov) * app.ads + 5.0 * app.sprint_vis + 5.0 * app.slide_vis * (1.0 - app.ads);
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
        // Lava and flood damage have no attacker, so no hit event arrives for
        // them: hold the damage overlay up, pulsing, while we stand in them.
        app.world.clock = app.net.world_time(now);
        // Canopy's shade banks roll toward where the server has them.
        if let Some(s) = app.net.latest() {
            let step = dt / ferrite_shared::map::canopy::SHADE_SECS;
            for k in 0..2 {
                let goal = s.shades[k] as f32 / 255.0;
                let p = &mut app.world.shades[k];
                *p += (goal - *p).clamp(-step, step);
            }
        }
        let at_crank = app.alive
            && !app.spectator
            && app.world.map == ferrite_shared::map::canopy::MAP_ID
            && ferrite_shared::map::canopy::crank_at(&app.world, app.st.pos, sim::view_dir(app.yaw, 0.0)).is_some();
        app.crank_hold = if at_crank && app.captured && app.bind_down(bindings::Action::Use) { app.crank_hold + dt } else { 0.0 };
        if app.alive && !app.spectator && app.world.hazard(app.st.pos).is_some() {
            app.dmg_flash = app.dmg_flash.max(0.75 + 0.25 * (now as f32 * 7.0).sin());
        }
        app.shake = (app.shake - dt * 2.5).max(0.0);
        app.smooth *= 1.0 - (12.0 * dt).min(1.0);
        app.step_y *= 1.0 - (14.0 * dt).min(1.0);
        app.eye += (sim::eye_h(app.st.crouch) - app.eye) * (16.0 * dt).min(1.0);
        app.dmg_dirs.retain(|d| now - d.1 < 1.6);
        if playing && app.st.on_ground {
            // One footfall per dip of the weapon bob. The sound is started a
            // little before the bottom of the dip so it lands on it.
            let beat = |b: f32| (b / std::f32::consts::PI + 0.7).floor();
            let before = beat(app.bob);
            app.bob += app.hspeed() * dt * 1.35;
            if beat(app.bob) != before && !app.st.crouch && app.st.slide == 0 {
                app.audio.play(Sfx::Step, 0.28);
            }
        }

        // ---- killcam ----
        // A moment after we fall, the seconds before it are played back as
        // the killer saw them. It ends with the redeploy, or on a click.
        if app.alive || app.lobby || app.spectator {
            app.kc = None;
            app.kc_armed = None;
        }
        if let Some((tick, at, killer, weapon)) = app.kc_armed {
            // The replay has to be over before we are back in: respawning
            // ends it. A short wait (TDM, an early conquest wave) leaves
            // less of a pause and fewer seconds of the past, so the replay
            // still reaches the kill instead of being cut off before it.
            const TAIL: f64 = 20.0; // ticks shown after the kill
            let since = now - at;
            let left = app.respawn_in as f64 - 0.2;
            if since > 0.8 || (since > 0.35 && left < KILLCAM_BACK + TAIL / 60.0) {
                app.kc_armed = None;
                // From where the killer is first on record, up to 3.2 s back.
                let back = KILLCAM_BACK.min(left - TAIL / 60.0);
                let from = tick as f64 - back * 60.0;
                let first =
                    app.net.snaps.iter().find(|s| s.tick as f64 >= from && s.players.iter().any(|p| p.id == killer)).map(|s| s.tick as f64);
                if let Some(first) = first.filter(|f| *f < tick as f64 - 40.0) {
                    app.kc = Some(KillCam {
                        killer,
                        weapon,
                        death_tick: tick,
                        death_now: at,
                        start_now: now,
                        from: first,
                        to: tick as f64 + TAIL,
                        rt: first,
                        fx_tick: first,
                        ads: 0.0,
                    });
                    app.last_seen = [-10.0; 64]; // everyone snaps to where they were
                }
            }
        }
        let skip = !app.auto
            && !picked
            && !app.cine.on
            && (app.action_pressed(Action::Jump) || app.action_pressed(Action::Fire) || (!typing() && controller::left_pressed()));
        let mut replayed: Vec<Fx> = Vec::new();
        if let Some(k) = app.kc.as_mut() {
            k.rt = k.from + (now - k.start_now) * 60.0;
            if k.rt > k.to || skip {
                app.kc = None;
                app.last_seen = [-10.0; 64];
            } else {
                // Shots and explosions from those seconds, as their moment comes round.
                for s in app.net.snaps.iter().filter(|s| (s.tick as f64) > k.fx_tick && (s.tick as f64) <= k.rt) {
                    replayed.extend(s.fx.iter().copied());
                }
                k.fx_tick = k.rt;
            }
        }
        for f in &replayed {
            app.handle_fx(f, listener, now, true);
        }
        let replay = app.kc.as_ref().map(|k| (k.rt, k.killer));

        // ---- remote players ----
        // (held as they are while the cinematic camera holds the action)
        let held_views = frozen.then(|| app.views.clone());
        let rt = replay.map_or(app.net.render_tick(now), |r| r.0);
        app.views.clear();
        let roster: Vec<(u8, u8, u8, u8, [u8; 2])> = app.net.roster.iter().map(|r| (r.id, r.team, r.kit, r.prim, r.wskin)).collect();
        for (id, team, kit, prim, wskin) in roster {
            // (in the killcam we are one of the soldiers on screen)
            if id == app.my_id && !app.spectator && replay.is_none() {
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
            let wid = app.wid_of(id, r.flags & flag::SIDEARM != 0);
            let wd = sim::weapon_def(wid);
            // The wire carries an action flag, so advance a local phase from
            // its rising edge. Drawing a weapon only lowers the arms.
            if r.flags2 & flag2::RELOAD != 0 {
                let ticks = if r.flags & flag::SURGE != 0 { wd.reload / 2 } else { wd.reload }.max(1);
                app.rreload_phase[i] =
                    if app.rreload_phase[i] < 0.0 || !fresh_pos { 0.0 } else { (app.rreload_phase[i] + dt * 60.0 / ticks as f32).min(1.0) };
            } else {
                app.rreload_phase[i] = -1.0;
            }
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
                app.rmelee[i] += (on(r.flags2 & flag2::MELEE != 0) - app.rmelee[i]) * k(16.0);
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
                app.rmelee[i] = 0.0;
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
                    // (not under the killcam's own feet: it would fill the view)
                    if near < 40.0 && replay.map(|r| r.1) != Some(id) {
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
                gun: sim::weapon_def(prim).arch,
                wid,
                reload_phase: app.rreload_phase[i],
                cycle: if app.rfire[i] > 0.0 && wd.interval > 20 {
                    (1.0 - (now - app.last_shot_snd[i]) as f32 * 60.0 / wd.interval as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                },
                skin: app.skin_of(id),
                wskin: wskin[(r.flags & flag::SIDEARM != 0) as usize],
                melee: app.rmelee[i],
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
                far: false,
            };
            app.last_view[i] = view;
            // The killcam looks through the killer's eyes: no body to draw.
            if replay.map(|r| r.1) != Some(id) {
                app.views.push(view);
            }
        }

        if let Some(v) = held_views {
            app.views = v;
        }
        // Staged soldiers (`--cine-cast`): the ones firing send a round,
        // with its flash and tracer, several times a second.
        if !frozen {
            let volley = (app.cast_clock * 7.0) as i64;
            app.cast_clock += dt;
            let volley = (app.cast_clock * 7.0) as i64 != volley;
            for (k, e) in app.cast.iter().enumerate().filter(|(_, e)| e.pose == cine::Pose::Fire) {
                let (from, d) = e.muzzle(&e.view(k, &app.world, app.cast_clock));
                if volley {
                    app.fx.muzzle(from);
                    let to = from + d * app.world.raycast(from, d, 220.0, app.dmask).map_or(220.0, |h| h.t);
                    app.fx.tracer(from, to, if e.team == 0 { [1.0, 0.72, 0.38] } else { [0.5, 0.8, 1.0] });
                }
            }
        }
        let cast_views: Vec<PlayerView> = app.cast.iter().enumerate().map(|(k, e)| e.view(k, &app.world, app.cast_clock)).collect();

        // ---- camera ----
        let aspect = mq::screen_width() / mq::screen_height().max(1.0);
        let shake = vec3((now as f32 * 61.0).sin(), (now as f32 * 47.0).sin(), 0.0) * app.shake * 0.12;
        // The killer's weapon, for the killcam: (weapon, sight, aim, state).
        let mut kc_vm: Option<(u8, u8, f32, net::Remote)> = None;
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
            app.dead_for = 0.0;
            app.kill_cam = None;
            app.spec_killer = false;
            let p = app.render_pos(a);
            let kick = app.recoil * wd.recoil * 0.6;
            let (yaw, pitch) = app.aim();
            (p + vec3(0.0, app.eye + app.step_y, 0.0) + shake, sim::view_dir(yaw, pitch + kick))
        } else if let Some((kv, kid)) = replay.and_then(|(rt, kid)| app.net.remote(kid, rt).map(|r| (r, kid))) {
            // Killcam: the killer's eyes, their weapon, their sight picture.
            app.dead_for += dt;
            let wid = app.wid_of(kid, kv.flags & flag::SIDEARM != 0);
            let sight = scene::sights_for(wid)[0];
            let zoomed = scene::sight_fov(wid, sight) * app.base_fov / 66.0;
            let aiming = if kv.flags & flag::ADS != 0 && kv.flags2 & flag2::RELOAD == 0 { 1.0 } else { 0.0 };
            if let Some(k) = app.kc.as_mut() {
                k.ads += (aiming - k.ads) * (14.0 * dt).min(1.0);
                app.fov = app.base_fov + (zoomed - app.base_fov) * k.ads;
                kc_vm = Some((wid, sight, k.ads, kv));
            }
            let crouch = app.rcrouch[kid as usize % 64];
            let h = sim::EYE + (sim::EYE_CROUCH - sim::EYE) * crouch;
            (kv.pos + vec3(0.0, h, 0.0), sim::view_dir(kv.yaw, kv.pitch))
        } else if let Some(v) = {
            // A moment on the spot where you fell, then follow whoever got you.
            app.dead_for += dt;
            let by_other = app.killer != SPECTATOR_ID && app.killer != app.my_id && app.dead_for > 1.2;
            app.views.iter().find(|v| by_other && v.id == app.killer).copied()
        } {
            // The lower part of the screen holds the loadout panel, so the
            // camera looks down a little to keep them in the upper part.
            let f = sim::view_dir(v.yaw, 0.0);
            let e = v.pos + vec3(0.0, 2.7, 0.0) - f * 4.8 + sim::right_of(v.yaw) * 0.9;
            let e = cam_pull(&app.world, v.pos + vec3(0.0, 1.7, 0.0), e, app.dmask);
            let d = sim::view_dir(v.yaw, v.pitch * 0.6 - 0.34);
            app.kill_cam = Some((e, d));
            app.spec_killer = true;
            (e, d)
        } else if let Some(c) = app.kill_cam {
            app.spec_killer = false;
            c
        } else {
            // Death cam: pull back and look toward whoever got you.
            let target = if app.killer != SPECTATOR_ID {
                app.last_pos[app.killer as usize % 64]
            } else {
                app.death_pos + sim::view_dir(app.yaw, 0.0)
            };
            let mut h = target - app.death_pos;
            h.y = 0.0;
            let h = if h.length() > 0.5 { h.normalize() } else { sim::view_dir(app.yaw, 0.0) };
            let e = app.death_pos + vec3(0.0, 4.2, 0.0) - h * 6.5;
            let e = cam_pull(&app.world, app.death_pos + vec3(0.0, 1.7, 0.0), e, app.dmask);
            let look = (app.death_pos + h * 6.0 + vec3(0.0, 1.0, 0.0) - e).normalize();
            (e, look)
        };
        app.last_cam = (eye, dir);
        let (eye, dir) = if app.cine.on { app.cine.view() } else { (eye, dir) };
        if app.cine.on {
            app.fov = app.cine.show_fov;
        }
        listener = eye;
        let proj = Mat4::perspective_rh_gl(app.fov.to_radians(), aspect, 0.08, 900.0);
        let cam_right = dir.cross(Vec3::Y).normalize_or_zero();
        // The view tips a little into a strafe and more into a slide.
        let tip = if playing && !app.cine.on { (app.strafe_vis * 0.012 + app.slide_vis * 0.045) * (1.0 - app.ads) } else { 0.0 };
        let up_dir = (Vec3::Y + cam_right * tip).normalize();
        let view = Mat4::look_to_rh(eye, dir, up_dir);
        let vp = proj * view;
        let sky_vp = proj * Mat4::look_to_rh(Vec3::ZERO, dir, up_dir);
        let cam_up = cam_right.cross(dir);
        app.audio.right = cam_right;
        // Is the player standing in sunlight? The weapon in hand dims in shade.
        let sun_dir = scene::sun_for(app.world.map).normalize();
        let lit = !gpu.shadows() || app.world.los(eye, eye + sun_dir * 160.0, app.dmask);
        app.view_lit += (if lit { 1.0 } else { 0.0 } - app.view_lit) * (6.0 * dt).min(1.0);

        // ---- build dynamic geometry ----
        app.fx.update(if frozen { 0.0 } else { dt }, &app.world);
        opaque.clear();
        alpha.clear();
        add.clear();
        vm_opaque.clear();
        vm_add.clear();
        let time = now as f32;
        // Map events (machinery, the volcano, wind, beacons) run on the
        // server's clock so all players see them together; `time` stays for
        // our own HUD and effects.
        let world_t = app.net.world_time(now);
        let world_prev = app.net.world_time(now - dt as f64);
        let blob = !gpu.shadows();
        // A quiet, positional ambient cue once per gust, behind combat audio.
        // This is cosmetic and never creates a network event or damage.
        if app.world.map == 2 && app.kc.is_none() && (world_t / 7.0).floor() != (world_prev / 7.0).floor() {
            let panel = vec3(12.0, app.world.height(0.0, 0.0) + 6.3, 6.25);
            if panel.distance(eye) < 42.0 {
                app.audio.play_at(Sfx::MetalRattle, panel, eye, 0.22, 12.0);
            }
        }
        // The monuments' creaks, groans and siren tests, on the same clock
        // as their motion. Cosmetic: no network event, no damage.
        if app.kc.is_none() {
            for (cue, at) in scene::monuments::cues(&app.world, cue_clock, world_t) {
                let (sfx, vol, reach) = match cue {
                    scene::monuments::Cue::Creak => (Sfx::Creak, 0.35, 45.0),
                    scene::monuments::Cue::Groan => (Sfx::Groan, 0.4, 90.0),
                    scene::monuments::Cue::Siren => (Sfx::Siren, 0.5, 160.0),
                };
                app.audio.play_at(sfx, at, eye, vol, reach);
            }
            // Spillway's sluice gates and the water they let out.
            for (cue, at, loud) in scene::spillway::cues(&app.world, cue_clock, world_t) {
                let (sfx, vol, reach) = match cue {
                    scene::spillway::Cue::Clank => (Sfx::Clank, 0.45, 60.0),
                    scene::spillway::Cue::Rush => (Sfx::Rush, 0.3 * loud, 55.0),
                };
                app.audio.play_at(sfx, at, eye, vol, reach);
            }
            // The volcano rumbling and blowing, heard (and felt) across the map.
            for (cue, at, loud) in scene::special_landmarks::cues(&app.world, cue_clock, world_t) {
                let (sfx, vol, reach) = match cue {
                    scene::special_landmarks::Cue::Rumble => (Sfx::Rumble, 0.6 * loud, 200.0),
                    scene::special_landmarks::Cue::Blast => (Sfx::Eruption, 1.0, 260.0),
                };
                app.audio.play_at(sfx, at, eye, vol, reach);
                if matches!(cue, scene::special_landmarks::Cue::Blast) {
                    let d = at.distance(eye) / 220.0;
                    app.shake = (app.shake + 0.45 / (1.0 + d * d)).min(1.0);
                }
            }
        }
        // Canopy: the gust through the crown, curtains rolling, and the
        // slings' motors.
        if app.world.map == ferrite_shared::map::canopy::MAP_ID && app.kc.is_none() {
            use ferrite_shared::events::{canopy_gust, GUST_WARN};
            use ferrite_shared::map::canopy;
            if cue_clock.is_finite() && world_t > cue_clock && world_t - cue_clock < 5.0 {
                let (a, b) = (canopy_gust(cue_clock).phase, canopy_gust(world_t).phase);
                if a < GUST_WARN && b >= GUST_WARN {
                    app.audio.play(Sfx::Gust, 0.7);
                }
            }
            if let Some(s) = app.net.latest() {
                let goal = s.shades.map(|v| if v > 127 { 1u8 } else { 0 });
                if let Some(was) = app.shade_heard {
                    for k in 0..2 {
                        if was[k] != goal[k] {
                            for (x, z0, z1) in canopy::shade_panels(k) {
                                let at = vec3(x, canopy::SHADE_TOP, (z0 + z1) * 0.5);
                                app.audio.play_at(Sfx::Furl, at, eye, 0.5, 45.0);
                            }
                        }
                    }
                }
                app.shade_heard = Some(goal);
            }
            let beat = |t: f32| (t / 0.5).floor();
            if beat(time) != beat(time - dt) {
                for v in app.views.iter() {
                    let moving = v.vel.0.abs() + v.vel.1.abs() > 1.5;
                    if v.flags2 & ferrite_shared::proto::flag2::RIDE != 0 && moving && v.pos.distance(eye) < 70.0 {
                        app.audio.play_at(Sfx::Sling, v.pos + Vec3::Y * 2.4, eye, 0.35, 50.0);
                    }
                }
                if app.alive && app.st.ride > 0 && app.st.ride_v.abs() > 1.5 {
                    app.audio.play(Sfx::Sling, 0.18 + 0.02 * app.st.ride_v.abs());
                }
            }
        }
        cue_clock = world_t;
        for v in app.views.iter().chain(&cast_views) {
            let far = v.pos.distance(eye) > scene::FINISH_DETAIL_RANGE;
            scene::soldier(&mut opaque, &mut alpha, &app.world, &scene::PlayerView { far, ..*v }, time, blob);
        }
        app.corpses.retain(|c| now - c.1 < 7.0);
        // In the killcam the clock is back before we fell: no body yet.
        let cnow = app.kc.as_ref().map_or(now, |k| k.death_now - (k.death_tick as f64 - k.rt) / 60.0);
        for (v, at) in &app.corpses {
            if *at > cnow {
                continue;
            }
            let age = (cnow - at) as f32;
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
        // The world as it stands - or, in the killcam, as it stood.
        let at_rt = replay.and_then(|(rt, _)| app.net.snaps.iter().rev().find(|s| s.tick as f64 <= rt)).or(app.net.latest());
        let visual_dmask = at_rt.map_or(app.dmask, |s| s.dmask);
        scene::crates(&mut opaque, &app.world, visual_dmask);
        let nades: Vec<Vec3> = at_rt.map_or(Vec::new(), |s| s.grenades.iter().map(|g| Vec3::from(g.pos)).collect());
        if trail {
            for p in &nades {
                app.fx.trail(*p);
            }
        }
        if let Some(s) = at_rt {
            // Only the objectives this mode uses stand on the field.
            let owners: Vec<(u8, f32, bool)> = s
                .points
                .iter()
                .enumerate()
                .map(|(k, p)| {
                    let used = match s.mode {
                        mode::CONQUEST => true,
                        mode::KOTH => k == 2,
                        _ => false,
                    };
                    (if used { p.owner } else { 255 }, p.progress as f32 / 100.0, p.n[0] > 0 && p.n[1] > 0)
                })
                .collect();
            scene::beacons(&mut opaque, &mut alpha, &mut add, &app.world, &owners, world_t, visual_dmask);
            scene::ambience(&mut opaque, &mut alpha, &mut add, &app.world, eye, cam_right, cam_up, world_t);
            scene::shades(&mut opaque, &app.world, world_t);
            if s.mode == mode::CTF {
                for (f, fl) in s.flags.iter().enumerate() {
                    let carried = fl.state == 1;
                    // On its carrier's back; not drawn across our own view.
                    let pos =
                        if !carried { Some(Vec3::from(fl.pos)) } else { app.views.iter().find(|v| v.id == fl.carrier).map(|v| v.pos) };
                    if let Some(pos) = pos {
                        scene::flag(&mut opaque, &mut add, pos, f as u8, carried, time);
                    }
                }
            }
            for p in &s.pickups {
                scene::pickup(&mut opaque, &mut add, Vec3::from(p.pos_f()), p.kind, time, p.id as f32 * 1.7);
            }
            for g in &s.grenades {
                let p = Vec3::from(g.pos);
                let m = Mat4::from_translation(p) * Mat4::from_rotation_y(time * 9.0) * Mat4::from_rotation_x(time * 7.0);
                opaque.obox(&m, Vec3::ZERO, Vec3::splat(0.09), gfx::rgb(0.14, 0.16, 0.12));
                let c = scene::team_col(g.team);
                let s = 0.2 + 0.1 * (time * 20.0).sin();
                add.quad(
                    p - cam_right * s - cam_up * s,
                    p + cam_right * s - cam_up * s,
                    p + cam_right * s + cam_up * s,
                    p - cam_right * s + cam_up * s,
                    [c[0], c[1], c[2], 0.8],
                );
            }
        }
        app.fx.build(&mut opaque, &mut alpha, &mut add, eye, cam_right, cam_up);
        // No weapon in hand with the HUD hidden or the camera off on its own.
        let show_vm = !app.cine.on && !app.cine.hide_hud;
        if playing && show_vm {
            scene::viewmodel(
                &mut vm_opaque,
                &mut vm_add,
                &scene::ViewModel {
                    sight,
                    kit: wd.arch,
                    wid: kit,
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
                    preview: None,
                    detail: true,
                    skin: app.wskin_of(app.my_id, app.st.slot == 1),
                    melee: args.vm_melee.unwrap_or(app.st.melee as f32 / sim::MELEE_TICKS as f32),
                },
            );
        } else if let (true, Some((wid, ksight, kads, kv)), Some((_, kid))) = (show_vm, kc_vm, replay) {
            // The killer's weapon, posed from what the snapshots say of them.
            let i = kid as usize % 64;
            let kwd = sim::weapon_def(wid);
            let speed = (app.rvel[i].0 * app.rvel[i].0 + app.rvel[i].1 * app.rvel[i].1).sqrt();
            scene::viewmodel(
                &mut vm_opaque,
                &mut vm_add,
                &scene::ViewModel {
                    sight: ksight,
                    kit: kwd.arch,
                    wid,
                    team: app.team(kid),
                    ads: kads,
                    recoil: app.rfire[i],
                    bob: app.walk[i] * 1.35,
                    bob_amt: (speed / sim::WALK).min(1.5),
                    reload: app.rreload_phase[i],
                    draw: if kv.flags2 & flag2::DRAW != 0 { 0.5 } else { 0.0 },
                    cycle: app.last_view[i].cycle,
                    sway: (0.0, 0.0),
                    flash: app.rfire[i],
                    sprint: ((speed - 6.4) / 1.6).clamp(0.0, 1.0) * (1.0 - app.rfire[i]),
                    land: 0.0,
                    strafe: (app.rvel[i].1 / sim::WALK).clamp(-1.0, 1.0),
                    slide: app.rslide[i],
                    air: app.rair[i],
                    mantle: 0.0,
                    time,
                    seq: (now * 20.0) as u32,
                    preview: None,
                    detail: true,
                    skin: app.wskin_of(kid, kv.flags & flag::SIDEARM != 0),
                    melee: if kv.flags2 & flag2::MELEE != 0 { 0.72 } else { 0.0 },
                },
            );
        }
        let scoped = (playing && sight >= scene::SCOPE4 && app.ads > 0.85) || kc_vm.is_some_and(|k| k.1 >= scene::SCOPE4 && k.2 > 0.85);
        let empty = gfx::Mesh::default();
        gpu.dof = {
            let (w, views, dmask) = (&app.world, &app.views, app.dmask);
            app.cine.dof(|| cine::focus_distance(w, &[views.as_slice(), &cast_views].concat(), eye, dir, dmask), dt)
        };
        let frame = gfx::Frame {
            vp,
            sky_vp,
            cam: eye,
            sun: scene::sun_for(app.world.map).normalize(),
            fog: scene::fog_for(app.world.map),
            tone: scene::tone_for(app.world.map),
            // Wind in the grass and trees: the same gusts for everyone.
            time: world_t,
            lights: scene::map_lights(&app.world, eye, app.fx.lights(eye)),
            opaque: &opaque,
            alpha: &alpha,
            add: &add,
            view_vp: Mat4::perspective_rh_gl(52f32.to_radians(), aspect, 0.01, 10.0),
            view_sun: view.transform_vector3(scene::sun_for(app.world.map).normalize()),
            view_lit: app.view_lit,
            view_opaque: if scoped { &empty } else { &vm_opaque },
            view_add: if scoped { &empty } else { &vm_add },
        };
        gpu.render(&frame);

        let frame_tris = gpu.tris + opaque.triangles() + alpha.triangles() + add.triangles();
        peak_tris = peak_tris.max(frame_tris);
        frames += 1;
        // Weapon models go on top of the loadout panel.
        let icons = if app.cine.hide_hud && !app.paused() { Vec::new() } else { hud::draw(&app, &vp, now, scoped, frame_tris) };
        if !icons.is_empty() {
            over_mesh.clear();
            let sh = mq::screen_height();
            for ic in &icons {
                let turn = if ic.turn { (now as f32 * 0.8).sin() * 0.6 } else { 0.3 };
                scene::gun_icon(&mut over_mesh, ic.wid, app.my_team, ic.skin, ic.cx, sh - ic.cy, ic.w, ic.h, turn);
            }
            gpu.overlay(&over_mesh, mq::screen_width(), sh);
        }

        if !grabbed {
            controller::draw_cursor();
        }

        let t = now - app.start;
        if let Some(path) = &args.shot {
            let kc_ok = !args.shot_killcam || app.kc.as_ref().is_some_and(|k| k.rt > k.from + 110.0);
            if !shot_done && t >= args.shot_at && kc_ok && (!args.shot_dead || (app.spec_killer && app.dead_for > 2.2)) {
                shot_done = true;
                if app.cine.on || args.shot_scale > 1 {
                    // Drawn again off screen: no HUD, any size.
                    let (w, h) = (mq::screen_width() as u32 * args.shot_scale, mq::screen_height() as u32 * args.shot_scale);
                    let bytes = gpu.capture(&frame, w, h);
                    mq::Image { bytes, width: w as u16, height: h as u16 }.export_png(path);
                } else {
                    mq::get_screen_data().export_png(path);
                }
            }
        }
        if want_shot {
            // What is on screen; with the HUD hidden (or bigger than the
            // screen), the world drawn again off screen at the chosen size.
            let path = cine::screenshot_path();
            let k = app.cine.scale;
            let (w, h) = (mq::screen_width() as u32, mq::screen_height() as u32);
            let (bytes, w, h) = if app.cine.hide_hud || k > 1 {
                (gpu.capture(&frame, w * k, h * k), w * k, h * k)
            } else {
                let img = mq::get_screen_data();
                (img.bytes, img.width as u32, img.height as u32)
            };
            let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
            cine::save_png(path, bytes, w, h);
            app.cine.notify(format!("SCREENSHOT SAVED  -  screenshots/{name}  ({w} x {h})"), now);
            app.audio.play(Sfx::Step, 0.5);
        }
        // After any picture is taken, so it is never in one.
        app.cine.draw_note(now);
        if let Some(why) = &app.net.rejected {
            ended = Some(format!("Disconnected: {why}"));
            break;
        }
        if app.net.snaps_recv > 0 && now - app.net.last_recv > 6.0 {
            ended = Some(format!("Lost connection to {addr}"));
            break;
        }
        if args.quit_at.is_some_and(|q| t >= q) {
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
    *volume = app.audio.volume;
    *loadout = [app.kit_choice, app.prims[0], app.prims[1], app.prims[2], app.side_choice, app.skin_choice];
    *wskins = app.wskins;
    *profile = app.profile.clone();
    if args.sight.is_none() {
        *sights = app.sights; // --sight forces one for this run only
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
    println!(
        "triangles: {} static world, {} peak per frame; {:.0} frames a second",
        gpu.tris,
        peak_tris,
        frames as f64 / (mq::get_time() - began).max(0.001)
    );
    if let Some(mut h) = server {
        h.stop();
    }
    gpu.destroy();
    mq::set_cursor_grab(false);
    mq::show_mouse(true);
    let _ = NEUTRAL;
    (true, ended)
}
