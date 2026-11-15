//! Launcher: a home page over the key art, with pages to host a match,
//! join one (found on the network, saved, or typed in) and change
//! settings. Saved servers and settings live in `ferrite-client.txt` next
//! to the executable.

use ferrite_shared::map::{map_by_name, MAPS};
use ferrite_shared::proto::{decode_info, ServerInfo, DEFAULT_PORT, DISCOVERY_GROUP, DISCOVERY_PORT, DISCOVERY_QUERY, PROTO};
use crate::audio::{Music, Scene};
use crate::update::{self, State as Up};
use crate::{controller, gfx, ident, scene};
use crate::bindings::{Action, Bindings, Input};
use std::cell::Cell;
use ferrite_shared::proto::{icon_hash, uuid_parse, uuid_text};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use crate::ui::{self, color as th};
use macroquad::prelude as mq;
use std::path::PathBuf;


/// Video and control settings a match needs.
#[derive(Clone)]
pub struct Prefs {
    /// Vertical field of view, degrees.
    pub fov: f32,
    pub fullscreen: bool,
    /// Side of the sun shadow map in texels; 0 = off.
    pub shadows: u32,
    pub show_fps: bool,
    pub invert_y: bool,
    /// Aim with a click instead of holding the button.
    pub toggle_aim: bool,
    pub bindings: Bindings,
    pub controller: controller::Settings,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs { fov: 66.0, fullscreen: false, shadows: 4096, show_fps: false, invert_y: false, toggle_aim: false, bindings: Bindings::default(), controller: controller::Settings::default() }
    }
}

pub enum Choice {
    Host { port: u16, per_team: usize, max_bots: usize, map: u8, rotate: bool, lobby: u32, skill: f32 },
    Join(String),
    Quit,
}

const F_NAME: usize = 0;
const F_HOST: usize = 1;
const F_PORT: usize = 2;
const F_BOTS: usize = 3;
const F_HPORT: usize = 4;
const F_MAXBOTS: usize = 5;
const MAX_SAVED: usize = 6;

pub struct Menu {
    /// name, join host, join port, bots per team, hosting port
    f: [String; 6],
    servers: Vec<String>,
    focus: usize,
    /// Mouse sensitivity, remembered between matches and launches.
    pub sens: f32,
    /// Map to host: index into the shared map table.
    pub map: u8,
    /// Play every map in turn, one per round.
    pub rotate: bool,
    /// Class, the primary chosen for each class, and the sidearm.
    pub loadout: [u8; 6],
    /// Look for a newer release when the launcher opens, and what came of it.
    pub updates: bool,
    pub updater: update::Updater,
    /// Finish chosen for each weapon id.
    pub wskins: [u8; 20],
    /// Sight chosen for each weapon id.
    pub sights: [u8; 20],
    /// Who we are on every server, whatever the callsign says.
    pub uuid: [u8; 16],
    /// Icon: 0 none, 1.. a built-in emblem (in `icon_colour`), or
    /// `ident::CUSTOM` for the picture in `custom`.
    pub icon_kind: u8,
    pub icon_colour: u8,
    custom: Option<Vec<u8>>,
    /// The last files dropped on the window (path, size, hash).
    last_drop: Vec<(Option<std::path::PathBuf>, usize, u32)>,
    /// The icon as drawn on the home page, and what it was made from.
    icon_tex: Option<((u8, u8, u32), Option<mq::Texture2D>)>,
    /// Bot difficulty when hosting: index into `ferrite_server::DIFFICULTIES`.
    pub difficulty: u8,
    /// Sound effects and music volume, 0..1, and the music switch.
    pub volume: f32,
    pub music_vol: f32,
    pub music_on: bool,
    /// Keep the song rotation playing during a match.
    pub music_in_match: bool,
    /// Chosen sound output device (its id); `None` follows the system.
    pub audio_device: Option<String>,
    /// World seed and shadow map size, for the map shown behind HOST & PLAY.
    pub seed: u64,
    pub prefs: Prefs,
    /// Settings tab: 0 audio, 1 video, 2 controls.
    pub tab: u8,
    controls_page: u8,
    binding_capture: Option<Action>,
    binding_add: bool,
    binding_note: Option<String>,
    binding_cancel_rect: Option<(f32, f32, f32, f32)>,
    /// That map, built for drawing: (map index, world, renderer).
    preview: Option<(u8, World, gfx::Gpu)>,
    /// Output devices found when the settings page was opened: (id, name).
    devices: Vec<(String, String)>,
    /// Page on screen: 0 home, 1 join, 2 host, 3 settings.
    pub page: u8,
    last: f64,
    pub error: Option<String>,
    lan_ip: Option<String>,
    repeat_at: f64,
    /// Verification aid: save a picture of the home screen, then quit.
    pub shot: Option<String>,
    frames: u32,
    lan: Lan,
}

fn cfg_path() -> PathBuf {
    let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default();
    dir.join("ferrite-client.txt")
}

/// The address other machines on the network would use to reach a match
/// hosted here. Connecting a UDP socket only picks a route; nothing is sent.
/// A game found on the local network.
struct Found {
    addr: String,
    info: ServerInfo,
    seen: f64,
}

/// Looks for servers on the local network: a query goes to the discovery
/// multicast group and to the broadcast address every second or so, and
/// whoever answers is listed until it stops answering.
struct Lan {
    socks: Vec<std::net::UdpSocket>,
    found: Vec<Found>,
    next_query: f64,
}

impl Lan {
    fn new() -> Lan {
        use std::net::{Ipv4Addr, UdpSocket};
        let mut socks = Vec::new();
        // One socket on the default route, one pinned to the LAN address so
        // the multicast query leaves by the right interface on machines
        // that also have a VPN or virtual adapters.
        let mut binds = vec![Ipv4Addr::UNSPECIFIED];
        binds.extend(ferrite_server::net::lan_ip());
        for ip in binds {
            if let Ok(s) = UdpSocket::bind((ip, 0)) {
                let _ = s.set_nonblocking(true);
                let _ = s.set_broadcast(true);
                let _ = s.set_multicast_ttl_v4(2);
                socks.push(s);
            }
        }
        Lan { socks, found: Vec::new(), next_query: 0.0 }
    }

    fn poll(&mut self, now: f64) {
        use std::net::Ipv4Addr;
        if now >= self.next_query {
            self.next_query = now + 1.2;
            for s in &self.socks {
                let _ = s.send_to(DISCOVERY_QUERY, (Ipv4Addr::from(DISCOVERY_GROUP), DISCOVERY_PORT));
                let _ = s.send_to(DISCOVERY_QUERY, (Ipv4Addr::BROADCAST, DISCOVERY_PORT));
            }
        }
        let mut buf = [0u8; 512];
        for s in &self.socks {
            while let Ok((n, from)) = s.recv_from(&mut buf) {
                let Some(info) = decode_info(&buf[..n]) else { continue };
                let addr = format!("{}:{}", from.ip(), info.port);
                if let Some(f) = self.found.iter_mut().find(|f| f.addr == addr || (f.info.id == info.id && f.info.port == info.port)) {
                    f.info = info;
                    f.seen = now;
                } else if self.found.len() < 6 {
                    self.found.push(Found { addr, info, seen: now });
                }
            }
        }
        self.found.retain(|f| now - f.seen < 4.5);
    }
}

fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("192.0.2.1", 9)).ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then(|| ip.to_string())
}

/// "host" or "host:port" -> (host, port text). An empty port means default.
fn split_addr(a: &str) -> (String, String) {
    match a.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => (h.to_string(), p.to_string()),
        _ => (a.to_string(), String::new()),
    }
}

thread_local! { static CAPTURING_INPUT: Cell<bool> = const { Cell::new(false) }; }

fn hit(x: f32, y: f32, w: f32, h: f32) -> bool {
    let (mx, my) = controller::mouse_position();
    mx >= x && mx < x + w && my >= y && my < y + h
}

fn clicked() -> bool {
    !CAPTURING_INPUT.with(Cell::get) && controller::left_pressed()
}

/// Plain button. `solid` is the fill for a call to action; without one it
/// is dark glass that picks up a cobalt edge under the pointer.
fn button(x: f32, y: f32, w: f32, h: f32, label: &str, solid: Option<mq::Color>, size: f32) -> bool {
    let over = hit(x, y, w, h);
    match solid {
        Some(c) => {
            let k = if over { 1.0 } else { 0.86 };
            mq::draw_rectangle(x, y, w, h, mq::Color::new(c.r * k, c.g * k, c.b * k, 1.0));
            ui::text_c(label, x + w / 2.0, y + h / 2.0 + size * 0.28, size, th::INK);
        }
        None => {
            ui::plate(x, y, w, h, th::GLASS_STRONG, if over { th::COBALT } else { th::STROKE });
            ui::text_c(label, x + w / 2.0, y + h / 2.0 + size * 0.28, size, if over { th::WHITE } else { th::MUTED });
        }
    }
    over && clicked()
}

/// One of the big home page buttons: a title, a line under it, a chevron.
fn big_button(x: f32, y: f32, w: f32, h: f32, title: &str, sub: &str, primary: bool) -> bool {
    let over = hit(x, y, w, h);
    let lift = if over { 1.0 } else { 0.0 };
    let (ink, quiet) = if primary {
        let k = if over { 1.07 } else { 1.0 };
        mq::draw_rectangle(x, y - lift, w, h, mq::Color::new((th::EMBER.r * k).min(1.0), (th::EMBER.g * k).min(1.0), (th::EMBER.b * k).min(1.0), 1.0));
        (th::INK, ui::fade(th::INK, 0.75))
    } else {
        ui::plate(x, y - lift, w, h, th::GLASS_STRONG, if over { th::COBALT } else { th::STROKE });
        (th::WHITE, th::MUTED)
    };
    ui::text(title, x + 22.0, y - lift + h * 0.5 + 2.0, 30.0, ink);
    ui::text(sub, x + 23.0, y - lift + h * 0.5 + 21.0, 15.0, quiet);
    ui::text(">", x + w - 36.0, y - lift + h * 0.5 + 10.0, 30.0, ink);
    over && clicked()
}

impl Menu {
    pub fn load(default_name: &str, host_port: u16, per_team: usize) -> Menu {
        let mut m = Menu {
            f: [
                default_name.chars().take(16).collect(),
                String::new(),
                DEFAULT_PORT.to_string(),
                per_team.to_string(),
                host_port.to_string(),
                String::new(),
            ],
            servers: Vec::new(),
            focus: F_NAME,
            sens: 0.0022,
            map: 0,
            rotate: true,
            loadout: [0, 0, 1, 2, 4, 0],
            updates: true,
            updater: update::Updater::new(false, None),
            wskins: [0; 20],
            sights: std::array::from_fn(|w| scene::sights_for(w as u8)[0]),
            uuid: [0; 16],
            icon_kind: 0,
            icon_colour: 0,
            custom: None,
            last_drop: Vec::new(),
            icon_tex: None,
            difficulty: 1,
            volume: 0.7,
            music_vol: 0.5,
            music_on: true,
            music_in_match: false,
            audio_device: None,
            seed: 0xFE221,
            prefs: Prefs::default(),
            tab: 0,
            controls_page: 0,
            binding_capture: None,
            binding_add: false,
            binding_note: None,
            binding_cancel_rect: None,
            preview: None,
            devices: Vec::new(),
            page: 0,
            last: 0.0,
            error: None,
            lan_ip: lan_ip(),
            repeat_at: 0.0,
            shot: None,
            frames: 0,
            lan: Lan::new(),
        };
        if let Ok(text) = std::fs::read_to_string(cfg_path()) {
            for line in text.lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                let v = v.trim();
                match k.trim() {
                    "name" if !v.is_empty() => m.f[F_NAME] = v.chars().take(16).collect(),
                    "server" if !v.is_empty() && m.servers.len() < MAX_SAVED => m.servers.push(v.to_string()),
                    "bots" if v.parse::<usize>().is_ok() => m.f[F_BOTS] = v.to_string(),
                    "max_bots" if v.parse::<usize>().is_ok() => m.f[F_MAXBOTS] = v.chars().take(2).collect(),
                    "sens" => m.sens = v.parse::<f32>().unwrap_or(0.0022).clamp(0.0004, 0.01),
                    "map" => m.map = map_by_name(v).unwrap_or(0),
                    "rotate" => m.rotate = v != "0",
                    "difficulty" => m.difficulty = v.parse::<u8>().unwrap_or(1).min(3),
                    "volume" => m.volume = v.parse::<f32>().unwrap_or(0.7).clamp(0.0, 1.0),
                    "music_volume" => m.music_vol = v.parse::<f32>().unwrap_or(0.5).clamp(0.0, 1.0),
                    "music" => m.music_on = v != "0",
                    "music_in_match" => m.music_in_match = v == "1",
                    "fov" => m.prefs.fov = v.parse::<f32>().unwrap_or(66.0).clamp(56.0, 90.0),
                    "fullscreen" => m.prefs.fullscreen = v == "1",
                    "shadows" => m.prefs.shadows = match v { "0" => 0, "2048" => 2048, _ => 4096 },
                    "show_fps" => m.prefs.show_fps = v == "1",
                    "invert_y" => m.prefs.invert_y = v == "1",
                    "toggle_aim" => m.prefs.toggle_aim = v == "1",
                    "controller_enabled" => m.prefs.controller.enabled = v != "0",
                    "controller_sensitivity" => m.prefs.controller.sensitivity = v.parse::<f32>().ok().filter(|n| n.is_finite()).unwrap_or(2.8).clamp(0.4, 7.0),
                    "controller_deadzone" => m.prefs.controller.deadzone = v.parse::<f32>().ok().filter(|n| n.is_finite()).unwrap_or(0.18).clamp(0.05, 0.4),
                    "controller_invert_y" => m.prefs.controller.invert_y = v == "1",
                    "audio_device" if !v.is_empty() => m.audio_device = Some(v.to_string()),
                    "loadout" => {
                        for (k, n) in v.split(',').take(6).enumerate() {
                            m.loadout[k] = n.trim().parse().unwrap_or(m.loadout[k]);
                        }
                    }
                    "sights" => {
                        // Keep only sights that still fit the weapon.
                        for (k, n) in v.split(',').take(20).enumerate() {
                            if let Ok(n) = n.trim().parse::<u8>() {
                                if scene::sights_for(k as u8).contains(&n) {
                                    m.sights[k] = n;
                                }
                            }
                        }
                    }
                    "weapon_skins" => {
                        for (k, n) in v.split(',').take(20).enumerate() {
                            m.wskins[k] = n.trim().parse().unwrap_or(0);
                        }
                    }
                    "updates" => m.updates = v != "0",
                    "id" => m.uuid = uuid_parse(v).unwrap_or([0; 16]),
                    "icon" => m.icon_kind = v.parse::<u8>().unwrap_or(0),
                    "icon_colour" => m.icon_colour = v.parse::<u8>().unwrap_or(0) % ident::COLOURS.len() as u8,
                    "icon_data" => m.custom = ident::from_hex(v),
                    _ => { m.prefs.bindings.read_entry(k.trim(), v); }
                }
            }
        }
        m.prefs.bindings.normalize();
        // First launch: this is who we are from now on. (It is written to
        // the settings file the next time they are saved.)
        if m.uuid == [0; 16] {
            m.uuid = ident::new_uuid();
        }
        // A picture left beside the game as icon.png becomes the icon.
        if m.custom.is_none() {
            if let Some(px) = std::fs::read(cfg_path().with_file_name("icon.png")).ok().and_then(|b| ident::from_image_bytes(&b)) {
                m.custom = Some(px);
                m.icon_kind = ident::CUSTOM;
            }
        }
        if !m.icon_kinds().contains(&m.icon_kind) {
            m.icon_kind = 0;
        }
        m
    }

    /// Icon kinds on offer: none, the emblems, and our own picture if
    /// there is one.
    fn icon_kinds(&self) -> Vec<u8> {
        let mut v: Vec<u8> = (0..=ident::SHAPES.len() as u8).collect();
        if self.custom.is_some() {
            v.push(ident::CUSTOM);
        }
        v
    }

    /// The icon to wear: `ICON_BYTES` of RGBA, or nothing.
    pub fn icon_rgba(&self) -> Vec<u8> {
        match self.icon_kind {
            0 => Vec::new(),
            ident::CUSTOM => self.custom.clone().unwrap_or_default(),
            k => ident::preset(k - 1, self.icon_colour),
        }
    }

    fn icon_texture(&mut self) -> Option<mq::Texture2D> {
        let px = self.icon_rgba();
        let key = (self.icon_kind, self.icon_colour, if px.is_empty() { 0 } else { icon_hash(&px) });
        if self.icon_tex.as_ref().map(|t| t.0) != Some(key) {
            self.icon_tex = Some((key, ident::texture(&px)));
        }
        self.icon_tex.as_ref().and_then(|t| t.1.clone())
    }

    /// A file was dropped on the window: if it is a picture, wear it.
    /// The window system is asked directly: on Windows the drop is
    /// recorded but never announced, so macroquad's own list stays empty.
    /// What it holds stays there, so a drop is "new" when it differs from
    /// the last one seen.
    fn take_dropped(&mut self) {
        use macroquad::miniquad::window as win;
        let n = win::dropped_file_count();
        let drop: Vec<(Option<std::path::PathBuf>, Option<Vec<u8>>)> = (0..n).map(|i| (win::dropped_file_path(i), win::dropped_file_bytes(i))).collect();
        let key: Vec<(Option<std::path::PathBuf>, usize, u32)> =
            drop.iter().map(|(p, b)| (p.clone(), b.as_ref().map_or(0, |b| b.len()), b.as_ref().map_or(0, |b| icon_hash(b)))).collect();
        if key == self.last_drop {
            return;
        }
        self.last_drop = key;
        for (path, bytes) in drop {
            let bytes = bytes.or_else(|| path.as_ref().and_then(|p| std::fs::read(p).ok())).unwrap_or_default();
            match ident::from_image_bytes(&bytes) {
                Some(px) => {
                    self.custom = Some(px);
                    self.icon_kind = ident::CUSTOM;
                    self.error = None;
                    self.save();
                }
                None => self.error = Some("That file is not a picture the game can read. Use a PNG.".to_string()),
            }
        }
    }

    fn save(&self) {
        let map = MAPS[self.map as usize % MAPS.len()].name.to_lowercase();
        let mut out = format!("name={}\nbots={}\nsens={}\nmap={}\nrotate={}\n", self.f[F_NAME], self.f[F_BOTS], self.sens, map, self.rotate as u8);
        out += &format!("difficulty={}\nvolume={:.2}\nmusic_volume={:.2}\nmusic={}\nmusic_in_match={}\n", self.difficulty, self.volume, self.music_vol, self.music_on as u8, self.music_in_match as u8);
        let p = &self.prefs;
        out += &format!("fov={}\nfullscreen={}\nshadows={}\nshow_fps={}\ninvert_y={}\ntoggle_aim={}\n", p.fov, p.fullscreen as u8, p.shadows, p.show_fps as u8, p.invert_y as u8, p.toggle_aim as u8);
        out += &p.bindings.write_config();
        out += &format!("controller_enabled={}\ncontroller_sensitivity={}\ncontroller_deadzone={}\ncontroller_invert_y={}\n", p.controller.enabled as u8, p.controller.sensitivity, p.controller.deadzone, p.controller.invert_y as u8);
        if let Some(d) = &self.audio_device {
            out += &format!("audio_device={d}
");
        }
        let l = self.loadout;
        out += &format!("loadout={},{},{},{},{},{}\n", l[0], l[1], l[2], l[3], l[4], l[5]);
        out += &format!("weapon_skins={}\n", self.wskins.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(","));
        out += &format!("sights={}\n", self.sights.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(","));
        out += &format!("id={}\nicon={}\nicon_colour={}\nupdates={}\n", uuid_text(&self.uuid), self.icon_kind, self.icon_colour, self.updates as u8);
        if let Some(px) = &self.custom {
            out += &format!("icon_data={}\n", ident::to_hex(px));
        }
        if !self.f[F_MAXBOTS].is_empty() {
            out += &format!("max_bots={}\n", self.f[F_MAXBOTS]);
        }
        for s in &self.servers {
            out += &format!("server={s}\n");
        }
        let _ = std::fs::write(cfg_path(), out);
    }

    /// Write settings changed during a match (sensitivity) back to disk.
    pub fn persist(&self) {
        self.save();
    }

    /// Send sound to the device saved in the settings file, if any. Used
    /// at startup, whichever way the game was launched.
    pub fn apply_saved_audio_device() {
        let Ok(text) = std::fs::read_to_string(cfg_path()) else { return };
        if let Some(id) = text.lines().find_map(|l| l.strip_prefix("audio_device=")) {
            if !id.trim().is_empty() {
                quad_snd::set_output_device(Some(id.trim().to_string()));
            }
        }
    }

    pub fn name(&self) -> String {
        let n = self.f[F_NAME].trim();
        if n.is_empty() { "Player".to_string() } else { n.to_string() }
    }

    /// Put a server we actually joined at the top of the saved list.
    pub fn remember(&mut self, addr: &str) {
        self.servers.retain(|s| s != addr);
        self.servers.insert(0, addr.to_string());
        self.servers.truncate(MAX_SAVED);
        self.save();
    }

    fn join_addr(&self) -> Option<String> {
        let (host, port_in_host) = split_addr(self.f[F_HOST].trim());
        if host.is_empty() {
            return None;
        }
        let port = if !port_in_host.is_empty() { port_in_host } else { self.f[F_PORT].clone() };
        let port: u16 = if port.is_empty() { DEFAULT_PORT } else { port.parse().ok().filter(|p| *p != 0)? };
        Some(format!("{host}:{port}"))
    }

    fn host_choice(&self) -> Choice {
        Choice::Host {
            port: self.f[F_HPORT].parse().ok().filter(|p| *p != 0).unwrap_or(DEFAULT_PORT),
            per_team: self.f[F_BOTS].parse::<usize>().unwrap_or(12).clamp(1, 16),
            // Left empty: bots fill every slot.
            max_bots: self.f[F_MAXBOTS].parse::<usize>().unwrap_or(64),
            map: self.map % MAPS.len() as u8,
            rotate: self.rotate,
            lobby: 30,
            skill: ferrite_server::DIFFICULTIES[self.difficulty as usize % 4].1,
        }
    }

    fn accepts(field: usize, c: char, len: usize) -> bool {
        match field {
            F_NAME => !c.is_control() && len < 16 && c != '=',
            F_HOST => (c.is_ascii_alphanumeric() || ".-:_".contains(c)) && len < 64,
            F_PORT | F_HPORT => c.is_ascii_digit() && len < 5,
            _ => c.is_ascii_digit() && len < 2,
        }
    }

    /// Text fields on each page, in TAB order.
    fn fields(page: u8) -> &'static [usize] {
        match page {
            0 => &[F_NAME],
            1 => &[F_HOST, F_PORT],
            2 => &[F_BOTS, F_MAXBOTS, F_HPORT],
            _ => &[],
        }
    }

    pub fn go_to(&mut self, page: u8) {
        self.go(page);
    }

    fn go(&mut self, page: u8) {
        self.page = page;
        self.binding_capture = None;
        self.binding_note = None;
        if page == 3 {
            self.devices = quad_snd::output_devices();
        }
        if let Some(f) = Self::fields(page).first() {
            self.focus = *f;
        }
    }

    fn field(&mut self, id: usize, x: f32, y: f32, w: f32, label: &str, hint: &str, now: f64) {
        ui::text(label, x, y - 8.0, 18.0, th::MUTED);
        let h = 38.0;
        let focused = self.focus == id;
        ui::plate(x, y, w, h, th::GLASS_STRONG, if focused { th::COBALT } else { th::STROKE });
        let v = &self.f[id];
        if v.is_empty() && !focused {
            ui::text(hint, x + 10.0, y + 26.0, 22.0, th::QUIET);
        } else {
            ui::text(v, x + 10.0, y + 26.0, 22.0, th::WHITE);
        }
        if focused && (now * 2.0) as i64 % 2 == 0 {
            mq::draw_rectangle(x + 12.0 + ui::width(v, 22.0), y + 9.0, 2.0, 20.0, th::COBALT);
        }
        if hit(x, y, w, h) && clicked() {
            self.focus = id;
        }
    }

    fn typing(&mut self, now: f64) {
        let fields = Self::fields(self.page);
        if !fields.contains(&self.focus) {
            while mq::get_char_pressed().is_some() {}
            return;
        }
        while let Some(c) = mq::get_char_pressed() {
            if Self::accepts(self.focus, c, self.f[self.focus].chars().count()) {
                self.f[self.focus].push(c);
            }
        }
        let ctrl = mq::is_key_down(mq::KeyCode::LeftControl) || mq::is_key_down(mq::KeyCode::RightControl);
        if ctrl && mq::is_key_pressed(mq::KeyCode::V) {
            if let Some(text) = macroquad::miniquad::window::clipboard_get() {
                for c in text.trim().chars() {
                    if Self::accepts(self.focus, c, self.f[self.focus].chars().count()) {
                        self.f[self.focus].push(c);
                    }
                }
            }
        }
        // Backspace with key repeat.
        if mq::is_key_pressed(mq::KeyCode::Backspace) {
            self.f[self.focus].pop();
            self.repeat_at = now + 0.4;
        } else if mq::is_key_down(mq::KeyCode::Backspace) && now > self.repeat_at {
            self.f[self.focus].pop();
            self.repeat_at = now + 0.04;
        }
        if mq::is_key_pressed(mq::KeyCode::Tab) {
            let at = fields.iter().position(|f| *f == self.focus).unwrap_or(0);
            self.focus = fields[(at + 1) % fields.len()];
        }
        // Typing "host:port" into the address box moves the port across.
        if self.focus != F_HOST {
            let (h, p) = split_addr(&self.f[F_HOST]);
            if !p.is_empty() {
                self.f[F_HOST] = h;
                self.f[F_PORT] = p;
            }
        }
    }

    /// The selected map, seen from a camera circling it. Built when the
    /// selection changes; drawn in place of the key art.
    fn draw_map(&mut self, now: f64, sw: f32, sh: f32) {
        use ferrite_shared::map::{HALF_X, HALF_Z};
        let map = self.map % MAPS.len() as u8;
        if self.preview.as_ref().map(|p| p.0) != Some(map) {
            self.drop_map();
            let world = World::with_map(self.seed, map);
            let bounds = scene::shadow_bounds(world.map);
            let gpu = gfx::Gpu::new(scene::build_world(&world), scene::build_sky_for(world.map), self.prefs.shadows.min(2048), scene::sun_for(world.map).normalize(), bounds);
            self.preview = Some((map, world, gpu));
        }
        let Some((_, world, gpu)) = &mut self.preview else { return };
        let (mut opaque, mut alpha, mut add) = (gfx::Mesh::default(), gfx::Mesh::default(), gfx::Mesh::default());
        let time = now as f32;
        scene::crates(&mut opaque, world, 0);
        let owners = [(ferrite_shared::NEUTRAL, 0.0, false); 5];
        scene::beacons(&mut opaque, &mut alpha, &mut add, world, &owners, time);
        // Circle the middle of the map. The page covers the left of the
        // screen, so look a little to the left of centre: that puts the map
        // on the right.
        let a = time * 0.11;
        let r = HALF_X.max(HALF_Z) * 1.05;
        let eye = vec3(a.cos() * r, 62.0, a.sin() * r);
        let to = (vec3(0.0, 4.0, 0.0) - eye).normalize();
        let right = to.cross(Vec3::Y).normalize();
        let dir = (vec3(0.0, 4.0, 0.0) - right * r * 0.3 - eye).normalize();
        let proj = Mat4::perspective_rh_gl(52f32.to_radians(), sw / sh.max(1.0), 0.5, 900.0);
        let view = Mat4::look_to_rh(eye, dir, Vec3::Y);
        let empty = gfx::Mesh::default();
        gpu.render(&gfx::Frame {
            vp: proj * view,
            sky_vp: proj * Mat4::look_to_rh(Vec3::ZERO, dir, Vec3::Y),
            cam: eye,
            sun: scene::sun_for(world.map).normalize(),
            fog: scene::fog_for(world.map),
            lights: scene::map_lights(world, eye, [gfx::Light::default(); 4]),
            opaque: &opaque,
            alpha: &alpha,
            add: &add,
            view_vp: Mat4::IDENTITY,
            view_sun: Vec3::Y,
            view_lit: 1.0,
            view_opaque: &empty,
            view_add: &empty,
        });
    }

    fn drop_map(&mut self) {
        if let Some((_, _, gpu)) = self.preview.take() {
            gpu.destroy();
        }
    }

    /// Heading shared by the inner pages; returns true when BACK is clicked.
    fn page_head(x: f32, y: f32, w: f32, label: &str, title: &str, accent: mq::Color) -> bool {
        ui::text(label, x, y, 15.0, accent);
        ui::text(title, x, y + 34.0, 34.0, th::WHITE);
        mq::draw_line(x, y + 48.0, x + w, y + 48.0, 1.0, th::STROKE);
        button(x + w - 110.0, y - 2.0, 110.0, 34.0, "< BACK", None, 18.0)
    }

    fn capture_binding(&mut self) {
        let Some(action) = self.binding_capture else { return };
        let cancel_clicked = self.binding_cancel_rect.is_some_and(|(x, y, w, h)| hit(x, y, w, h) && controller::left_pressed());
        if cancel_clicked || mq::is_key_pressed(mq::KeyCode::Escape) || controller::menu_back() {
            self.binding_capture = None;
            self.binding_note = Some("Assignment cancelled.".into());
        } else if mq::is_key_pressed(mq::KeyCode::Backspace) || mq::is_key_pressed(mq::KeyCode::Delete) {
            self.prefs.bindings.clear(action);
            self.binding_capture = None;
            self.binding_note = Some(format!("{} is now unbound.", action.label()));
            self.save();
        } else if let Some(input) = Input::capture() {
            let result = if self.binding_add {
                self.prefs.bindings.add(action, input)
            } else {
                self.prefs.bindings.replace(action, input)
            };
            match result {
                Ok(()) => {
                    self.binding_capture = None;
                    self.binding_note = Some(format!("{}: {}", action.label(), self.prefs.bindings.display(action)));
                    self.save();
                }
                Err(message) => self.binding_note = Some(message),
            }
        }
    }

    fn controls(&mut self, x: f32, mut y: f32, w: f32, sh: f32) {
        self.binding_cancel_rect = None;
        let tabs = ["MOUSE", "PAD", "MOVE", "FIGHT", "UI", "KIT"];
        let gap = 6.0;
        let width = (w - gap * 5.0) / 6.0;
        for (index, name) in tabs.into_iter().enumerate() {
            let bx = x + index as f32 * (width + gap);
            let selected = self.controls_page as usize == index;
            if button(bx, y, width, 30.0, name, selected.then_some(th::COBALT), 16.0) {
                self.controls_page = index as u8;
                self.binding_capture = None;
                self.binding_note = None;
            }
        }
        y += 44.0;
        let setting_row = |label: &str, value: String, y: f32| -> i32 {
            ui::plate(x, y, w, 42.0, th::GLASS_STRONG, th::STROKE);
            ui::text(label, x + 12.0, y + 28.0, 18.0, th::WHITE);
            ui::text_c(&value, x + w - 108.0, y + 28.0, 18.0, th::COBALT_BRIGHT);
            button(x + w - 43.0, y + 5.0, 36.0, 32.0, "+", None, 20.0) as i32
                - button(x + w - 198.0, y + 5.0, 36.0, 32.0, "-", None, 20.0) as i32
        };
        if self.controls_page == 0 {
            let d = setting_row("MOUSE SENSITIVITY", format!("{:.2}", self.sens * 1000.0), y);
            if d != 0 { self.sens = (self.sens * if d > 0 { 1.1 } else { 1.0 / 1.1 }).clamp(0.0004, 0.01); }
            y += 50.0;
            if setting_row("INVERT MOUSE Y", if self.prefs.invert_y { "ON" } else { "OFF" }.into(), y) != 0 { self.prefs.invert_y = !self.prefs.invert_y; }
            y += 50.0;
            if setting_row("AIM", if self.prefs.toggle_aim { "TOGGLE" } else { "HOLD" }.into(), y) != 0 { self.prefs.toggle_aim = !self.prefs.toggle_aim; }
            ui::text("MOVE, FIGHT, UI and KIT contain keyboard and mouse bindings.", x, y + 76.0, 17.0, th::MUTED);
            ui::text("Escape and chat editing keep their familiar navigation keys.", x, y + 100.0, 17.0, th::QUIET);
            return;
        }
        if self.controls_page == 1 {
            let mut status = controller::status();
            while ui::width(&status, 17.0) > w && status.len() > 4 { status.pop(); }
            ui::text(&status, x, y + 16.0, 17.0, th::COBALT_BRIGHT);
            y += 28.0;
            if setting_row("CONTROLLER", if self.prefs.controller.enabled { "ON" } else { "OFF" }.into(), y) != 0 { self.prefs.controller.enabled = !self.prefs.controller.enabled; }
            y += 50.0;
            let d = setting_row("LOOK SENSITIVITY", format!("{:.1}", self.prefs.controller.sensitivity), y);
            self.prefs.controller.sensitivity = ((self.prefs.controller.sensitivity * 10.0).round() + d as f32 * 2.0).clamp(4.0, 70.0) / 10.0;
            y += 50.0;
            let d = setting_row("STICK DEADZONE", format!("{:.0}%", self.prefs.controller.deadzone * 100.0), y);
            self.prefs.controller.deadzone = ((self.prefs.controller.deadzone * 100.0).round() + d as f32).clamp(5.0, 40.0) / 100.0;
            y += 50.0;
            if setting_row("INVERT STICK Y", if self.prefs.controller.invert_y { "ON" } else { "OFF" }.into(), y) != 0 { self.prefs.controller.invert_y = !self.prefs.controller.invert_y; }
            ui::text("LS move / press sprint. RS look. LT aim. RT fire. START pause.", x, y + 70.0, 15.0, th::MUTED);
            ui::text("A jump. B crouch. X reload. Y swap. LB grenade. RB / RS melee.", x, y + 92.0, 15.0, th::MUTED);
            ui::text("D-pad: sight / loadout / team / stats. BACK shows the scoreboard.", x, y + 114.0, 15.0, th::QUIET);
            ui::text("Menus: stick / D-pad move the cursor. A select. B back. X clear.", x, y + 136.0, 15.0, th::QUIET);
            return;
        }
        use Action::*;
        let actions: &[Action] = match self.controls_page {
            2 => &[Forward, Backward, StrafeLeft, StrafeRight, Jump, Sprint, Crouch],
            3 => &[Fire, Aim, Reload, Grenade, SwapWeapon, Melee, CycleSight],
            4 => &[Loadout, ChangeTeam, Scoreboard, ChatAll, ChatTeam, ToggleStats, SensitivityDown, SensitivityUp],
            _ => &[ClassVanguard, ClassBreacher, ClassMarksman, NextPrimary, NextSidearm, NextLook, PrimaryFinish, SidearmFinish],
        };
        let row_h = ((sh - y - 126.0) / 8.0).clamp(25.0, 36.0).floor();
        for (row, &action) in actions.iter().enumerate() {
            let by = y + row as f32 * row_h;
            let active = self.binding_capture == Some(action);
            ui::plate(x, by, w, row_h - 3.0, th::GLASS_STRONG, if active { th::EMBER } else { th::STROKE });
            ui::text(action.label(), x + 10.0, by + row_h * 0.67, 17.0, th::WHITE);
            let bx = x + w * 0.43;
            let bw = w * 0.57 - 94.0;
            let value = if active { if self.binding_add { "ADD KEY / MOUSE..." } else { "PRESS KEY / MOUSE..." }.to_string() } else { self.prefs.bindings.display(action) };
            let mut size = 17.0;
            while size > 12.0 && ui::width(&value, size) > bw - 12.0 { size -= 1.0; }
            if button(bx, by + 2.0, bw, row_h - 7.0, &value, active.then_some(th::EMBER), size) {
                self.binding_capture = Some(action);
                self.binding_add = false;
                self.binding_note = None;
            }
            if button(x + w - 87.0, by + 2.0, 36.0, row_h - 7.0, "+", (active && self.binding_add).then_some(th::EMBER), 18.0) {
                self.binding_capture = Some(action);
                self.binding_add = true;
                self.binding_note = None;
            }
            let clear = button(x + w - 43.0, by + 2.0, 36.0, row_h - 7.0, "X", None, 16.0)
                || !CAPTURING_INPUT.with(Cell::get) && hit(bx, by, bw, row_h) && controller::right_pressed();
            if clear {
                self.prefs.bindings.clear(action);
                self.binding_capture = None;
                self.binding_note = Some(format!("{} is now unbound.", action.label()));
                self.save();
            }
        }
        let note_y = y + actions.len() as f32 * row_h + 18.0;
        let note = self.binding_note.as_deref().unwrap_or("Click to replace; + adds another input. X / right-click clears. Escape cancels.");
        let mut note_size = 16.0;
        while note_size > 12.0 && ui::width(note, note_size) > w { note_size -= 1.0; }
        ui::text(note, x, note_y, note_size, if self.binding_capture.is_some() { th::WARNING } else { th::MUTED });
        ui::text("Duplicates are rejected. Delete / Backspace clears during assignment.", x, note_y + 21.0, 15.0, th::QUIET);
        let cancel = (x + w - 132.0, sh - 72.0, 132.0, 32.0);
        self.binding_cancel_rect = Some(cancel);
        if self.binding_capture.is_some() {
            ui::plate(cancel.0, cancel.1, cancel.2, cancel.3, th::GLASS_STRONG, th::EMBER);
            ui::text_c("CANCEL", cancel.0 + cancel.2 * 0.5, cancel.1 + 22.0, 17.0, th::EMBER_BRIGHT);
        }
        if button(x, sh - 72.0, 220.0, 32.0, "RESET KEYBINDS", None, 17.0) {
            self.prefs.bindings.reset();
            self.binding_capture = None;
            self.binding_note = Some("Default keyboard and mouse bindings restored.".into());
            self.save();
        }
    }

    pub async fn run(&mut self, music: &mut Music, art: &ui::Art) -> Choice {
        mq::set_cursor_grab(false);
        mq::show_mouse(true);
        while mq::get_char_pressed().is_some() {}
        // Skip the frame that carried the click or key which brought us here.
        mq::next_frame().await;
        self.last = mq::get_time();
        loop {
            if mq::is_quit_requested() {
                self.drop_map();
                self.save();
                return Choice::Quit;
            }
            let now = mq::get_time();
            let dt = (now - self.last).clamp(0.0, 0.1) as f32;
            self.last = now;
            controller::poll(&self.prefs.controller, dt, true);
            let capturing = self.binding_capture.is_some();
            CAPTURING_INPUT.with(|active| active.set(capturing));
            self.capture_binding();
            let (sw, sh) = (mq::screen_width(), mq::screen_height());
            music.on = self.music_on;
            music.vol = self.music_vol;
            music.in_match = self.music_in_match;
            music.update(Scene::Launcher, dt).await;
            mq::clear_background(th::INK);
            if self.page == 2 {
                self.draw_map(now, sw, sh);
            } else {
                self.drop_map();
                ui::backdrop(&art.launcher, sw, sh);
            }
            self.typing(now);
            self.take_dropped();
            self.lan.poll(now);
            self.updater.poll();
            if matches!(self.updater.state, Up::Installed(_)) {
                // The new version is in place: start it.
                self.drop_map();
                self.save();
                if self.updater.restart() {
                    return Choice::Quit;
                }
            }
            let mut out: Option<Choice> = None;

            // The art carries the logo at the top left; the page hangs under it.
            let k = ui::art_scale(sw, sh);
            let x = (78.0 * k).max(24.0).floor();
            let mut y = (216.0 * k).max(130.0).floor();
            let wide: f32 = if self.page == 0 { 500.0 } else { 660.0 };
            let w = wide.min(sw - x - 24.0);
            if self.page != 0 {
                // Inner pages are denser: put them on glass.
                mq::draw_rectangle(0.0, 0.0, (x + w + 40.0).min(sw), sh, ui::fade(th::INK, 0.62));
            }
            // F11 flips fullscreen, unless it is being bound to an action.
            if !capturing && crate::f11_fullscreen() {
                self.prefs.fullscreen = crate::is_fullscreen();
                self.save();
            }
            let esc = !capturing && (mq::is_key_pressed(mq::KeyCode::Escape) || controller::menu_back());
            let enter = mq::is_key_pressed(mq::KeyCode::Enter) || mq::is_key_pressed(mq::KeyCode::KpEnter);

            match self.page {
                0 => {
                    ui::text("FIELD OPERATIONS", x, y, 18.0, th::EMBER_BRIGHT);
                    ui::text("Choose your deployment", x, y + 34.0, 34.0, th::WHITE);
                    mq::draw_line(x, y + 48.0, x + w, y + 48.0, 1.0, th::STROKE);
                    y += 84.0;
                    self.field(F_NAME, x, y, 190.0, "CALLSIGN", "Player", now);
                    // Player icon, to the right of the callsign it is shown beside in a match.
                    {
                        let ix = x + 206.0;
                        ui::text("PLAYER ICON", ix, y - 8.0, 18.0, th::MUTED);
                        ui::plate(ix, y, 38.0, 38.0, th::GLASS_STRONG, th::STROKE);
                        match self.icon_texture() {
                            Some(t) => mq::draw_texture_ex(&t, ix + 3.0, y + 3.0, mq::WHITE, mq::DrawTextureParams { dest_size: Some(mq::vec2(32.0, 32.0)), ..Default::default() }),
                            None => ui::text_c("-", ix + 19.0, y + 26.0, 20.0, th::QUIET),
                        }
                        let kinds = self.icon_kinds();
                        let at = kinds.iter().position(|k| *k == self.icon_kind).unwrap_or(0) as i32;
                        // [icon] [<] [label] [>] [colour]
                        let (lx, lw) = (ix + 78.0, 128.0);
                        let step = button(lx + lw + 4.0, y, 30.0, 38.0, ">", None, 20.0) as i32 - button(ix + 44.0, y, 30.0, 38.0, "<", None, 20.0) as i32;
                        if step != 0 {
                            self.icon_kind = kinds[(at + step).rem_euclid(kinds.len() as i32) as usize];
                        }
                        let label = match self.icon_kind {
                            0 => "NONE",
                            ident::CUSTOM => "YOUR PICTURE",
                            k => ident::SHAPES[(k as usize - 1) % ident::SHAPES.len()],
                        };
                        ui::plate(lx, y, lw, 38.0, th::GLASS_STRONG, th::STROKE);
                        let size = (16.0 * (lw - 12.0) / ui::width(label, 16.0).max(1.0)).min(16.0);
                        ui::text_c(label, lx + lw / 2.0, y + 25.0, size, th::WHITE);
                        if self.icon_kind != 0 && self.icon_kind != ident::CUSTOM {
                            // Colour of the emblem: click for the next one.
                            let hx = lx + lw + 40.0;
                            let c = ident::COLOURS[self.icon_colour as usize % ident::COLOURS.len()].1;
                            let over = hit(hx, y, 38.0, 38.0);
                            mq::draw_rectangle(hx + 5.0, y + 5.0, 28.0, 28.0, mq::Color::from_rgba(c[0], c[1], c[2], 255));
                            mq::draw_rectangle_lines(hx, y, 38.0, 38.0, 2.0, if over { th::WHITE } else { th::STROKE });
                            if over && clicked() {
                                self.icon_colour = (self.icon_colour + 1) % ident::COLOURS.len() as u8;
                            }
                        }
                        ui::text("or drop a PNG on this window", ix, y + 56.0, 15.0, th::QUIET);
                    }
                    y += 80.0;
                    if let Some(e) = &self.error {
                        ui::plate(x, y, w, 34.0, th::GLASS_STRONG, th::DANGER);
                        ui::text(e, x + 12.0, y + 23.0, 18.0, th::DANGER);
                        y += 44.0;
                    }
                    let found = self.lan.found.len();
                    let join_sub = match found {
                        0 => "BROWSE OR CONNECT".to_string(),
                        1 => "1 GAME ON YOUR NETWORK".to_string(),
                        n => format!("{n} GAMES ON YOUR NETWORK"),
                    };
                    let bh = if sh < 800.0 { 58.0 } else { 70.0 };
                    if big_button(x, y, w, bh, "HOST & PLAY", "CREATE A MATCH  -  BOTS FILL THE EMPTY SLOTS", true) {
                        self.go(2);
                    }
                    y += bh + 12.0;
                    if big_button(x, y, w, bh, "JOIN SERVER", &join_sub, false) {
                        self.go(1);
                    }
                    y += bh + 12.0;
                    if big_button(x, y, w, bh, "SETTINGS", "AUDIO  -  VIDEO  -  CONTROLS", false) {
                        self.go(3);
                    }
                    y += bh + 12.0;
                    ui::plate(x, y, w, 44.0, th::GLASS_STRONG, if hit(x, y, w, 44.0) { th::COBALT } else { th::STROKE });
                    ui::text("QUIT TO DESKTOP", x + 22.0, y + 29.0, 20.0, th::MUTED);
                    if (hit(x, y, w, 44.0) && clicked()) || esc {
                        out = Some(Choice::Quit);
                    }
                    if enter && out.is_none() {
                        self.go(2);
                    }
                    let fy = (y + 80.0).max(sh - 46.0).min(sh - 30.0);
                    ui::text(&format!("FERRITE / {}", env!("CARGO_PKG_VERSION")), x, fy, 16.0, th::WHITE);
                    ui::text("ORIGINAL MUSIC BY breadKernel", x, fy + 20.0, 17.0, th::MUTED);
                    // Status plate, bottom right.
                    let status = match &self.lan_ip {
                        Some(ip) => format!("SYSTEMS ONLINE / {ip} / 60 TICK"),
                        None => "SYSTEMS ONLINE / 60 TICK".to_string(),
                    };
                    let pw = ui::width(&status, 18.0) + 52.0;
                    if sw - pw - 40.0 > x + w + 20.0 {
                        ui::plate(sw - pw - 40.0, sh - 62.0, pw, 34.0, th::GLASS, th::STROKE);
                        mq::draw_circle(sw - pw - 22.0, sh - 45.0, 4.0, th::SUCCESS);
                        ui::text(&status, sw - pw - 10.0, sh - 40.0, 18.0, th::SUCCESS);
                    }
                }
                1 => {
                    if Self::page_head(x, y, w, "FIELD OPERATIONS", "JOIN SERVER", th::COBALT_BRIGHT) || esc {
                        self.go(0);
                    }
                    y += 90.0;
                    let port_w = 100.0;
                    let btn_w = 110.0;
                    let host_w = w - port_w - btn_w - 24.0;
                    self.field(F_HOST, x, y, host_w, "ADDRESS", "hostname or IP", now);
                    self.field(F_PORT, x + host_w + 12.0, y, port_w, "PORT", &DEFAULT_PORT.to_string(), now);
                    let can_join = self.join_addr().is_some();
                    if (button(x + w - btn_w, y, btn_w, 38.0, "JOIN", Some(if can_join { th::COBALT } else { th::QUIET }), 22.0) || enter) && can_join {
                        out = self.join_addr().map(Choice::Join);
                    }
                    let mut ry = y + 58.0;
                    // Games answering on the local network.
                    let lan_rows = self.lan.found.len().min(if sh < 800.0 { 3 } else { 4 });
                    let dots = ["", ".", "..", "..."][(now * 2.0) as usize % 4];
                    let head = if lan_rows == 0 { format!("ON YOUR NETWORK  -  searching{dots}") } else { "ON YOUR NETWORK".to_string() };
                    ui::text(&head, x, ry + 14.0, 18.0, th::MUTED);
                    ry += 24.0;
                    for f in self.lan.found.iter().take(lan_rows) {
                        let i = &f.info;
                        let ok = i.proto == PROTO;
                        let over = hit(x, ry, w, 40.0);
                        ui::plate(x, ry, w, 40.0, if over { th::PANEL_RAISED } else { th::GLASS_STRONG }, if over { th::COBALT } else { th::STROKE });
                        let name: String = i.name.chars().filter(|c| !c.is_control()).take(22).collect();
                        ui::text(&name, x + 12.0, ry + 27.0, 20.0, th::WHITE);
                        let state = match i.phase {
                            2 => "in lobby",
                            1 => "round over",
                            _ => "in game",
                        };
                        let detail = if ok {
                            format!("{}   {}/{} players   {}", MAPS[i.map as usize % MAPS.len()].name, i.humans, i.slots, state)
                        } else {
                            "different game version".to_string()
                        };
                        ui::text(&detail, x + 250.0, ry + 26.0, 15.0, if ok { th::MUTED } else { th::DANGER });
                        if ok && (button(x + w - 86.0, ry + 5.0, 80.0, 30.0, "JOIN", Some(th::COBALT), 18.0) || (over && clicked())) {
                            out = Some(Choice::Join(f.addr.clone()));
                        }
                        ry += 46.0;
                    }
                    ry += 8.0;
                    ui::text("SAVED SERVERS", x, ry + 14.0, 18.0, th::MUTED);
                    ry += 24.0;
                    if self.servers.is_empty() {
                        ui::text("None yet - servers you join are remembered here.", x, ry + 22.0, 17.0, th::QUIET);
                    }
                    let mut remove = None;
                    for (i, s) in self.servers.iter().enumerate() {
                        if ry + 46.0 > sh - 40.0 {
                            break;
                        }
                        let over = hit(x, ry, w, 40.0);
                        ui::plate(x, ry, w, 40.0, if over { th::PANEL_RAISED } else { th::GLASS_STRONG }, if over { th::COBALT } else { th::STROKE });
                        ui::text(s, x + 12.0, ry + 27.0, 20.0, th::WHITE);
                        if button(x + w - 42.0, ry + 5.0, 36.0, 30.0, "X", None, 18.0) {
                            remove = Some(i);
                        } else if button(x + w - 42.0 - 86.0, ry + 5.0, 80.0, 30.0, "JOIN", Some(th::COBALT), 18.0) {
                            out = Some(Choice::Join(s.clone()));
                        } else if over && clicked() {
                            let (h, p) = split_addr(s);
                            self.f[F_HOST] = h;
                            self.f[F_PORT] = if p.is_empty() { DEFAULT_PORT.to_string() } else { p };
                            self.focus = F_HOST;
                        }
                        ry += 46.0;
                    }
                    if let Some(i) = remove {
                        self.servers.remove(i);
                        self.save();
                    }
                    ui::text("TAB next field    ENTER join    CTRL+V paste    ESC back", x, sh - 20.0, 18.0, th::QUIET);
                }
                2 => {
                    if Self::page_head(x, y, w, "FIELD OPERATIONS", "HOST & PLAY", th::EMBER_BRIGHT) || esc {
                        self.go(0);
                    }
                    y += 90.0;
                    self.field(F_BOTS, x, y, 170.0, "PLAYERS PER TEAM", "12", now);
                    self.field(F_MAXBOTS, x + 182.0, y, 110.0, "MAX BOTS", "all", now);
                    self.field(F_HPORT, x + 304.0, y, 90.0, "PORT", &DEFAULT_PORT.to_string(), now);
                    // Map: click the right side for the next one, the left
                    // side for the one before.
                    let (mx, mw) = (x + 406.0, w - 406.0);
                    let def = &MAPS[self.map as usize % MAPS.len()];
                    ui::text("MAP", mx, y - 8.0, 18.0, th::MUTED);
                    let over = hit(mx, y, mw, 38.0);
                    ui::plate(mx, y, mw, 38.0, th::GLASS_STRONG, if over { th::COBALT } else { th::STROKE });
                    ui::text_c(def.name, mx + mw / 2.0, y + 26.0, 20.0, th::WHITE);
                    ui::text("<", mx + 10.0, y + 26.0, 20.0, if over { th::WHITE } else { th::QUIET });
                    ui::text(">", mx + mw - 20.0, y + 26.0, 20.0, if over { th::WHITE } else { th::QUIET });
                    if over && clicked() {
                        let n = MAPS.len() as u8;
                        let back = controller::mouse_position().0 < mx + mw * 0.3;
                        self.map = (self.map % n + if back { n - 1 } else { 1 }) % n;
                    }
                    y += 58.0;
                    ui::text(&format!("{}: {}.", def.name, def.blurb), x, y, 16.0, th::WHITE);
                    y += 38.0;

                    // Rotation: after each round the server moves to the next map.
                    ui::text("MAP ROTATION", x, y - 8.0, 18.0, th::MUTED);
                    for (i, label) in ["ON", "OFF"].into_iter().enumerate() {
                        let on = self.rotate == (i == 0);
                        let bx = x + i as f32 * 78.0;
                        let over = hit(bx, y, 70.0, 34.0);
                        ui::plate(bx, y, 70.0, 34.0, if on { th::COBALT } else { th::GLASS_STRONG }, if on || over { th::WHITE } else { th::STROKE });
                        ui::text_c(label, bx + 35.0, y + 23.0, 18.0, if on { th::INK } else { th::MUTED });
                        if over && clicked() {
                            self.rotate = i == 0;
                        }
                    }
                    let rn = if self.rotate { "Rounds move to the next map, unless the lobby votes." } else { "Rounds stay on this map, unless the lobby votes." };
                    ui::text(rn, x + 170.0, y + 22.0, 18.0, th::MUTED);
                    y += 66.0;

                    // Bot difficulty.
                    ui::text("BOT DIFFICULTY", x, y - 8.0, 18.0, th::MUTED);
                    let dw = ((w - 24.0) / 4.0).floor();
                    for (i, (label, _)) in ferrite_server::DIFFICULTIES.iter().enumerate() {
                        let on = self.difficulty as usize == i;
                        let bx = x + i as f32 * (dw + 8.0);
                        let over = hit(bx, y, dw, 40.0);
                        let c = [th::SUCCESS, th::COBALT, th::WARNING, th::DANGER][i];
                        ui::plate(bx, y, dw, 40.0, if on { c } else { th::GLASS_STRONG }, if on || over { th::WHITE } else { th::STROKE });
                        ui::text_c(label, bx + dw / 2.0, y + 27.0, 20.0, if on { th::INK } else { c });
                        if over && clicked() {
                            self.difficulty = i as u8;
                        }
                    }
                    let dn = [
                        "Recruit: bots are slow to react and spray wide. A gentle start.",
                        "Regular: the standard fight.",
                        "Veteran: bots react quickly and hold their aim.",
                        "Elite: fast, accurate and unforgiving.",
                    ][self.difficulty as usize % 4];
                    ui::text(dn, x, y + 62.0, 18.0, th::MUTED);
                    y += 96.0;

                    let hp = self.f[F_HPORT].parse::<u16>().ok().filter(|p| *p != 0).unwrap_or(DEFAULT_PORT);
                    let note = match &self.lan_ip {
                        Some(ip) => format!("Others on your network can join at {ip}:{hp}."),
                        None => "Empty slots are filled by bots.".to_string(),
                    };
                    ui::text(&note, x, y, 18.0, th::MUTED);
                    y += 22.0;
                    if button(x, y, w, 62.0, "START MATCH", Some(th::EMBER), 30.0) || enter {
                        out = Some(self.host_choice());
                    }
                    ui::text("TAB next field    ENTER start    ESC back", x, sh - 20.0, 18.0, th::QUIET);
                }
                _ => {
                    if Self::page_head(x, y, w, "FIELD OPERATIONS", "SETTINGS", th::COBALT_BRIGHT) || esc {
                        self.go(0);
                        self.save();
                    }
                    y += 84.0;
                    // Tabs.
                    let tw = ((w - 16.0) / 3.0).floor();
                    for (i, label) in ["AUDIO", "VIDEO", "CONTROLS"].into_iter().enumerate() {
                        let bx = x + i as f32 * (tw + 8.0);
                        let on = self.tab as usize == i;
                        let over = hit(bx, y, tw, 36.0);
                        ui::plate(bx, y, tw, 36.0, if on { th::COBALT } else { th::GLASS_STRONG }, if on || over { th::WHITE } else { th::STROKE });
                        ui::text_c(label, bx + tw / 2.0, y + 25.0, 20.0, if on { th::INK } else { th::MUTED });
                        if over && clicked() {
                            self.tab = i as u8;
                            self.binding_capture = None;
                            self.binding_note = None;
                        }
                    }
                    y += 50.0;
                    // Each row: a label, the value, and - / + to change it.
                    let row = |label: &str, value: String, y: f32| -> i32 {
                        ui::plate(x, y, w, 46.0, th::GLASS_STRONG, th::STROKE);
                        ui::text(label, x + 14.0, y + 30.0, 20.0, th::WHITE);
                        ui::text_c(&value, x + w - 112.0, y + 30.0, 20.0, th::COBALT_BRIGHT);
                        let down = button(x + w - 210.0, y + 6.0, 40.0, 34.0, "-", None, 22.0);
                        let up = button(x + w - 48.0, y + 6.0, 40.0, 34.0, "+", None, 22.0);
                        up as i32 - down as i32
                    };
                    let pct = |v: f32| format!("{:.0}%", v * 100.0);
                    let on_off = |b: bool| if b { "ON".to_string() } else { "OFF".to_string() };
                    match self.tab {
                        0 => {
                            // Output device: step through what is plugged in. The
                            // sound moves across at once, so you can hear the choice.
                            {
                                ui::plate(x, y, w, 46.0, th::GLASS_STRONG, th::STROKE);
                                ui::text("OUTPUT", x + 14.0, y + 30.0, 20.0, th::WHITE);
                                let at = self.audio_device.as_ref().and_then(|id| self.devices.iter().position(|d| &d.0 == id));
                                let mut name = match (at, &self.audio_device) {
                                    (Some(i), _) => self.devices[i].1.clone(),
                                    (None, Some(_)) => "System default (saved device not connected)".to_string(),
                                    (None, None) => "System default".to_string(),
                                };
                                let room = w - 130.0 - 60.0 - 56.0;
                                while ui::width(&name, 16.0) > room && name.chars().count() > 4 {
                                    name = name.chars().take(name.chars().count() - 4).collect::<String>() + "...";
                                }
                                ui::text_c(&name, x + 130.0 + 48.0 + room / 2.0, y + 29.0, 16.0, th::COBALT_BRIGHT);
                                let n = self.devices.len() as i32 + 1;
                                let step = button(x + w - 48.0, y + 6.0, 40.0, 34.0, ">", None, 22.0) as i32 - button(x + 130.0, y + 6.0, 40.0, 34.0, "<", None, 22.0) as i32;
                                if step != 0 && (n > 1 || self.audio_device.is_some()) {
                                    // Slot 0 is the system default, then the devices.
                                    let cur = at.map_or(0, |i| i as i32 + 1);
                                    let next = (cur + step).rem_euclid(n);
                                    self.audio_device = if next == 0 { None } else { Some(self.devices[next as usize - 1].0.clone()) };
                                    quad_snd::set_output_device(self.audio_device.clone());
                                }
                                y += 54.0;
                            }
                            if row("MUSIC", on_off(self.music_on), y) != 0 {
                                self.music_on = !self.music_on; // either button flips it
                            }
                            y += 54.0;
                            if row("MUSIC IN A MATCH", on_off(self.music_in_match), y) != 0 {
                                self.music_in_match = !self.music_in_match;
                            }
                            y += 54.0;
                            let d = row("MUSIC VOLUME", pct(self.music_vol), y);
                            self.music_vol = ((self.music_vol * 10.0).round() + d as f32).clamp(0.0, 10.0) / 10.0;
                            y += 54.0;
                            let d = row("EFFECTS VOLUME", pct(self.volume), y);
                            self.volume = ((self.volume * 10.0).round() + d as f32).clamp(0.0, 10.0) / 10.0;
                            y += 72.0;
                            ui::text("Music by breadKernel.", x, y, 18.0, th::MUTED);
                            ui::text("Your own .ogg or .wav files in a 'music' folder beside the game", x, y + 24.0, 18.0, th::QUIET);
                            ui::text("join the rotation.", x, y + 46.0, 18.0, th::QUIET);
                        }
                        1 => {
                            if row("FULLSCREEN", on_off(self.prefs.fullscreen), y) != 0 {
                                self.prefs.fullscreen = !self.prefs.fullscreen;
                                crate::set_fullscreen(self.prefs.fullscreen);
                            }
                            y += 54.0;
                            let d = row("FIELD OF VIEW", format!("{:.0}", self.prefs.fov), y);
                            self.prefs.fov = (self.prefs.fov + 2.0 * d as f32).clamp(56.0, 90.0);
                            y += 54.0;
                            let levels = [0u32, 2048, 4096];
                            let at = levels.iter().position(|l| *l == self.prefs.shadows).unwrap_or(2) as i32;
                            let d = row("SHADOWS", ["OFF", "LOW", "HIGH"][at as usize].to_string(), y);
                            self.prefs.shadows = levels[(at + d).clamp(0, 2) as usize];
                            y += 54.0;
                            if row("SHOW FPS AND PING", on_off(self.prefs.show_fps), y) != 0 {
                                self.prefs.show_fps = !self.prefs.show_fps;
                            }
                            y += 54.0;
                            if row("CHECK FOR UPDATES", on_off(self.updates), y) != 0 {
                                self.updates = !self.updates;
                            }
                            y += 72.0;
                            ui::text("Field of view is vertical, in degrees. Aiming down a sight narrows it.", x, y, 18.0, th::QUIET);
                            ui::text("Lower the shadows on an older graphics card. F3 shows FPS in a match.", x, y + 24.0, 18.0, th::QUIET);
                        }
                        _ => self.controls(x, y, w, sh),
                    }
                    ui::text("ESC back", x, sh - 20.0, 18.0, th::QUIET);
                }
            }

            // A newer release, top right: one click downloads it, puts
            // it in place and starts it.
            {
                let (line, c, act) = match &self.updater.state {
                    Up::Available(tag) => (format!("UPDATE AVAILABLE   {tag}"), th::WARNING, true),
                    Up::Installing(tag) => (format!("DOWNLOADING {tag}{}", [".", "..", "..."][(now * 2.0) as usize % 3]), th::COBALT_BRIGHT, false),
                    Up::Failed(e) => (format!("UPDATE FAILED: {e}"), th::DANGER, false),
                    _ => (String::new(), th::WHITE, false),
                };
                if !line.is_empty() {
                    let bw = if act { 190.0 } else { 0.0 };
                    let pw = (ui::width(&line, 18.0) + 36.0 + bw).min(sw - 48.0);
                    let (px, py) = (sw - pw - 40.0, 30.0);
                    ui::plate(px, py, pw, 44.0, th::GLASS_STRONG, c);
                    ui::text(&line, px + 16.0, py + 29.0, 18.0, c);
                    if act && button(px + pw - bw - 4.0, py + 5.0, bw, 34.0, "UPDATE & RESTART", Some(th::EMBER), 18.0) {
                        self.updater.install();
                    }
                }
            }

            // Now playing, bottom right: moving bars, the title, and a
            // button for the next song when there is more than one.
            if let Some(title) = music.now_playing().map(|t| t.to_string()) {
                let skip = music.can_skip();
                let tw = ui::width(&title, 16.0);
                let pw = tw + 150.0 + if skip { 46.0 } else { 0.0 };
                let (px, py) = (sw - pw - 40.0, sh - 104.0);
                if px > x + w + 20.0 {
                    ui::plate(px, py, pw, 34.0, th::GLASS, th::STROKE);
                    for b in 0..4 {
                        let bh = 5.0 + 11.0 * (0.5 + 0.5 * ((now as f32) * (5.0 + b as f32 * 1.7) + b as f32 * 2.1).sin());
                        mq::draw_rectangle(px + 14.0 + b as f32 * 6.0, py + 26.0 - bh, 4.0, bh, th::COBALT);
                    }
                    ui::text("NOW PLAYING", px + 46.0, py + 23.0, 15.0, th::QUIET);
                    ui::text(&title, px + 136.0, py + 23.0, 16.0, th::WHITE);
                    if skip && button(px + pw - 44.0, py + 3.0, 40.0, 28.0, ">>", None, 16.0) {
                        music.skip();
                    }
                }
            }

            controller::draw_cursor();
            self.frames += 1;
            // (a picture waits for the music, so it shows what is playing)
            if self.frames >= 20 && self.shot.is_some() && (!music.available() || !self.music_on || music.now_playing().is_some() || self.frames > 900) {
                if let Some(p) = &self.shot {
                    mq::get_screen_data().export_png(p);
                    self.drop_map();
                    return Choice::Quit;
                }
            }
            if let Some(c) = out {
                if !matches!(c, Choice::Quit) {
                    self.error = None;
                }
                self.drop_map();
                self.save();
                return c;
            }
            mq::next_frame().await;
        }
    }
}
