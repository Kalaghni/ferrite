//! Home screen: pick a saved server, type an address, or host a match.
//! Saved servers and the player name live in `ferrite-client.txt` next to
//! the executable.

use ferrite_shared::map::{map_by_name, MAPS};
use ferrite_shared::proto::{decode_info, ServerInfo, DEFAULT_PORT, DISCOVERY_GROUP, DISCOVERY_PORT, DISCOVERY_QUERY, PROTO};
use macroquad::prelude as mq;
use std::path::PathBuf;

/// Team preference: 0 EMBER, 1 COBALT, 2 auto-balance.
pub const TEAM_LABELS: [&str; 3] = ["EMBER", "COBALT", "AUTO"];

pub enum Choice {
    Host { port: u16, per_team: usize, map: u8, rotate: bool, lobby: u32 },
    Join(String),
    Quit,
}

const F_NAME: usize = 0;
const F_HOST: usize = 1;
const F_PORT: usize = 2;
const F_BOTS: usize = 3;
const F_HPORT: usize = 4;
const MAX_SAVED: usize = 6;

pub struct Menu {
    /// name, join host, join port, bots per team, hosting port
    f: [String; 5],
    servers: Vec<String>,
    focus: usize,
    pub team: u8,
    /// Mouse sensitivity, remembered between matches and launches.
    pub sens: f32,
    /// Map to host: index into the shared map table.
    pub map: u8,
    /// Play every map in turn, one per round.
    pub rotate: bool,
    /// Class, the primary chosen for each class, and the sidearm.
    pub loadout: [u8; 5],
    pub error: Option<String>,
    lan_ip: Option<String>,
    repeat_at: f64,
    /// Verification aid: save a picture of the home screen, then quit.
    pub shot: Option<String>,
    frames: u32,
    lan: Lan,
}

const BG: mq::Color = mq::Color::new(0.07, 0.065, 0.065, 1.0);
const PANEL: mq::Color = mq::Color::new(0.12, 0.115, 0.115, 1.0);
const LINE: mq::Color = mq::Color::new(0.3, 0.29, 0.28, 1.0);
const DIM: mq::Color = mq::Color::new(0.62, 0.6, 0.58, 1.0);
const EMBER: mq::Color = mq::Color::new(0.95, 0.45, 0.16, 1.0);
const COBALT: mq::Color = mq::Color::new(0.25, 0.55, 0.98, 1.0);

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

fn hit(x: f32, y: f32, w: f32, h: f32) -> bool {
    let (mx, my) = mq::mouse_position();
    mx >= x && mx < x + w && my >= y && my < y + h
}

fn button(x: f32, y: f32, w: f32, h: f32, label: &str, col: mq::Color, size: f32) -> bool {
    let over = hit(x, y, w, h);
    let fill = if over { col } else { mq::Color::new(col.r * 0.55, col.g * 0.55, col.b * 0.55, 1.0) };
    mq::draw_rectangle(x, y, w, h, fill);
    let m = mq::measure_text(label, None, size as u16, 1.0);
    mq::draw_text(label, x + (w - m.width) / 2.0, y + h / 2.0 + size * 0.3, size, mq::WHITE);
    over && mq::is_mouse_button_pressed(mq::MouseButton::Left)
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
            ],
            servers: Vec::new(),
            focus: F_HOST,
            team: 2,
            sens: 0.0022,
            map: 0,
            rotate: true,
            loadout: [0, 0, 1, 2, 4],
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
                    "team" => m.team = v.parse::<u8>().unwrap_or(2).min(2),
                    "sens" => m.sens = v.parse::<f32>().unwrap_or(0.0022).clamp(0.0004, 0.01),
                    "map" => m.map = map_by_name(v).unwrap_or(0),
                    "rotate" => m.rotate = v != "0",
                    "loadout" => {
                        for (k, n) in v.split(',').take(5).enumerate() {
                            m.loadout[k] = n.trim().parse().unwrap_or(m.loadout[k]);
                        }
                    }
                    _ => {}
                }
            }
        }
        m
    }

    fn save(&self) {
        let map = MAPS[self.map as usize % MAPS.len()].name.to_lowercase();
        let mut out = format!("name={}\nbots={}\nteam={}\nsens={}\nmap={}\nrotate={}\n", self.f[F_NAME], self.f[F_BOTS], self.team, self.sens, map, self.rotate as u8);
        let l = self.loadout;
        out += &format!("loadout={},{},{},{},{}\n", l[0], l[1], l[2], l[3], l[4]);
        for s in &self.servers {
            out += &format!("server={s}\n");
        }
        let _ = std::fs::write(cfg_path(), out);
    }

    /// Write settings changed during a match (sensitivity) back to disk.
    pub fn persist(&self) {
        self.save();
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
            map: self.map % MAPS.len() as u8,
            rotate: self.rotate,
            lobby: 30,
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

    fn field(&mut self, id: usize, x: f32, y: f32, w: f32, label: &str, hint: &str, now: f64) {
        mq::draw_text(label, x, y - 8.0, 16.0, DIM);
        let h = 38.0;
        let focused = self.focus == id;
        mq::draw_rectangle(x, y, w, h, mq::Color::new(0.05, 0.05, 0.05, 1.0));
        mq::draw_rectangle_lines(x, y, w, h, 2.0, if focused { EMBER } else { LINE });
        let v = &self.f[id];
        if v.is_empty() && !focused {
            mq::draw_text(hint, x + 10.0, y + 26.0, 22.0, mq::Color::new(0.4, 0.39, 0.38, 1.0));
        } else {
            mq::draw_text(v, x + 10.0, y + 26.0, 22.0, mq::WHITE);
        }
        if focused && (now * 2.0) as i64 % 2 == 0 {
            let m = mq::measure_text(v, None, 22, 1.0);
            mq::draw_rectangle(x + 12.0 + m.width, y + 9.0, 2.0, 20.0, EMBER);
        }
        if hit(x, y, w, h) && mq::is_mouse_button_pressed(mq::MouseButton::Left) {
            self.focus = id;
        }
    }

    fn typing(&mut self, now: f64) {
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
            let back = mq::is_key_down(mq::KeyCode::LeftShift) || mq::is_key_down(mq::KeyCode::RightShift);
            self.focus = (self.focus + if back { 4 } else { 1 }) % 5;
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

    pub async fn run(&mut self) -> Choice {
        mq::set_cursor_grab(false);
        mq::show_mouse(true);
        while mq::get_char_pressed().is_some() {}
        // Skip the frame that carried the click or key which brought us here.
        mq::next_frame().await;
        loop {
            let now = mq::get_time();
            let (sw, sh) = (mq::screen_width(), mq::screen_height());
            mq::clear_background(BG);
            self.typing(now);
            let mut out: Option<Choice> = None;

            let w = 760.0f32.min(sw - 40.0);
            let x = ((sw - w) / 2.0).floor();
            let mut y = (sh * 0.04).max(14.0).floor();

            // Title with the two team colours.
            mq::draw_rectangle(x, y, 6.0, 46.0, EMBER);
            mq::draw_rectangle(x + 10.0, y, 6.0, 46.0, COBALT);
            mq::draw_text("FERRITE", x + 30.0, y + 40.0, 58.0, mq::WHITE);
            let tag = "12v12 front-line conquest";
            let tm = mq::measure_text(tag, None, 18, 1.0);
            mq::draw_text(tag, x + w - tm.width, y + 36.0, 18.0, DIM);
            y += 80.0;

            self.field(F_NAME, x, y, 260.0, "CALLSIGN", "Player", now);
            // Team picker: which side to ask the server for.
            let tx = x + 290.0;
            mq::draw_text("TEAM", tx, y - 8.0, 16.0, DIM);
            let bw = ((w - 290.0 - 16.0) / 3.0).floor();
            for (k, id) in [2u8, 0, 1].into_iter().enumerate() {
                let bx = tx + k as f32 * (bw + 8.0);
                let c = [EMBER, COBALT, mq::Color::new(0.5, 0.49, 0.47, 1.0)][id as usize];
                let on = self.team == id;
                mq::draw_rectangle(bx, y, bw, 38.0, if on { c } else { mq::Color::new(c.r * 0.28, c.g * 0.28, c.b * 0.28, 1.0) });
                if hit(bx, y, bw, 38.0) || on {
                    mq::draw_rectangle_lines(bx, y, bw, 38.0, 2.0, mq::WHITE);
                }
                let label = TEAM_LABELS[id as usize];
                let m = mq::measure_text(label, None, 22, 1.0);
                mq::draw_text(label, bx + (bw - m.width) / 2.0, y + 26.0, 22.0, if on { mq::WHITE } else { DIM });
                if hit(bx, y, bw, 38.0) && mq::is_mouse_button_pressed(mq::MouseButton::Left) {
                    self.team = id;
                }
            }
            y += 62.0;

            if let Some(e) = &self.error {
                mq::draw_rectangle(x, y, w, 34.0, mq::Color::new(0.35, 0.1, 0.06, 1.0));
                mq::draw_text(e, x + 12.0, y + 23.0, 20.0, mq::Color::new(1.0, 0.8, 0.7, 1.0));
                y += 46.0;
            }

            // ---- join ----
            self.lan.poll(now);
            let lan_rows = self.lan.found.len().min(if sh < 800.0 { 2 } else { 4 });
            // With games found nearby, an empty saved list needs no placeholder.
            let saved_rows = if lan_rows > 0 { self.servers.len() } else { self.servers.len().max(1) };
            let join_h = 118.0 + 30.0 + 44.0 * (lan_rows + saved_rows) as f32 + 34.0;
            mq::draw_rectangle(x, y, w, join_h, PANEL);
            mq::draw_rectangle(x, y, 4.0, join_h, COBALT);
            mq::draw_text("JOIN A SERVER", x + 20.0, y + 30.0, 24.0, mq::WHITE);
            let fy = y + 62.0;
            let port_w = 110.0;
            let btn_w = 120.0;
            let host_w = w - 40.0 - port_w - btn_w - 24.0;
            self.field(F_HOST, x + 20.0, fy, host_w, "ADDRESS", "hostname or IP", now);
            self.field(F_PORT, x + 32.0 + host_w, fy, port_w, "PORT", &DEFAULT_PORT.to_string(), now);
            let can_join = self.join_addr().is_some();
            if button(x + w - 20.0 - btn_w, fy, btn_w, 38.0, "JOIN", if can_join { COBALT } else { LINE }, 22.0) && can_join {
                out = self.join_addr().map(Choice::Join);
            }
            let mut ry = fy + 56.0;
            // Games answering on the local network.
            let dots = ["", ".", "..", "..."][(now * 2.0) as usize % 4];
            let head = if lan_rows == 0 { format!("ON YOUR NETWORK  -  searching{dots}") } else { "ON YOUR NETWORK".to_string() };
            mq::draw_text(&head, x + 20.0, ry + 14.0, 16.0, DIM);
            ry += 24.0;
            for f in self.lan.found.iter().take(lan_rows) {
                let i = &f.info;
                let ok = i.proto == PROTO;
                let over = hit(x + 20.0, ry, w - 40.0, 38.0);
                mq::draw_rectangle(x + 20.0, ry, w - 40.0, 38.0, if over { mq::Color::new(0.14, 0.19, 0.25, 1.0) } else { mq::Color::new(0.08, 0.11, 0.15, 1.0) });
                let name: String = i.name.chars().filter(|c| !c.is_control()).take(22).collect();
                mq::draw_text(&name, x + 32.0, ry + 26.0, 22.0, mq::WHITE);
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
                mq::draw_text(&detail, x + 290.0, ry + 25.0, 17.0, DIM);
                if ok && (button(x + w - 20.0 - 100.0, ry + 4.0, 96.0, 30.0, "JOIN", COBALT, 20.0) || (over && mq::is_mouse_button_pressed(mq::MouseButton::Left))) {
                    out = Some(Choice::Join(f.addr.clone()));
                }
                ry += 44.0;
            }
            ry += 6.0;
            mq::draw_text("SAVED SERVERS", x + 20.0, ry + 14.0, 16.0, DIM);
            ry += 24.0;
            if self.servers.is_empty() && lan_rows == 0 {
                mq::draw_text("None yet - servers you join are remembered here.", x + 20.0, ry + 26.0, 20.0, DIM);
            }
            let mut remove = None;
            for (i, s) in self.servers.iter().enumerate() {
                let over = hit(x + 20.0, ry, w - 40.0, 38.0);
                mq::draw_rectangle(x + 20.0, ry, w - 40.0, 38.0, if over { mq::Color::new(0.17, 0.17, 0.18, 1.0) } else { mq::Color::new(0.09, 0.09, 0.09, 1.0) });
                mq::draw_text(s, x + 32.0, ry + 26.0, 22.0, mq::WHITE);
                if button(x + w - 20.0 - 44.0, ry + 4.0, 40.0, 30.0, "X", mq::Color::new(0.5, 0.2, 0.18, 1.0), 20.0) {
                    remove = Some(i);
                } else if button(x + w - 20.0 - 44.0 - 96.0, ry + 4.0, 90.0, 30.0, "JOIN", COBALT, 20.0) {
                    out = Some(Choice::Join(s.clone()));
                } else if over && mq::is_mouse_button_pressed(mq::MouseButton::Left) {
                    let (h, p) = split_addr(s);
                    self.f[F_HOST] = h;
                    self.f[F_PORT] = if p.is_empty() { DEFAULT_PORT.to_string() } else { p };
                    self.focus = F_HOST;
                }
                ry += 44.0;
            }
            if let Some(i) = remove {
                self.servers.remove(i);
                self.save();
            }
            y += join_h + 18.0;

            // ---- host ----
            let host_h = 204.0;
            mq::draw_rectangle(x, y, w, host_h, PANEL);
            mq::draw_rectangle(x, y, 4.0, host_h, EMBER);
            mq::draw_text("HOST A MATCH", x + 20.0, y + 30.0, 24.0, mq::WHITE);
            let fy = y + 62.0;
            self.field(F_BOTS, x + 20.0, fy, 150.0, "PLAYERS PER TEAM", "12", now);
            self.field(F_HPORT, x + 186.0, fy, port_w, "PORT", &DEFAULT_PORT.to_string(), now);
            // Map selector: click the right side for the next map, the left
            // side for the previous one.
            let (mx, mw) = (x + 186.0 + port_w + 16.0, (w - 20.0 - 200.0 - 16.0) - (186.0 + port_w + 16.0));
            let def = &MAPS[self.map as usize % MAPS.len()];
            mq::draw_text("MAP", mx, fy - 8.0, 16.0, DIM);
            let over = hit(mx, fy, mw, 38.0);
            mq::draw_rectangle(mx, fy, mw, 38.0, mq::Color::new(0.05, 0.05, 0.05, 1.0));
            mq::draw_rectangle_lines(mx, fy, mw, 38.0, 2.0, if over { mq::WHITE } else { LINE });
            let nm = mq::measure_text(def.name, None, 20, 1.0);
            mq::draw_text(def.name, mx + (mw - nm.width) / 2.0, fy + 25.0, 20.0, mq::WHITE);
            mq::draw_text("<", mx + 9.0, fy + 26.0, 22.0, if over { mq::WHITE } else { DIM });
            mq::draw_text(">", mx + mw - 20.0, fy + 26.0, 22.0, if over { mq::WHITE } else { DIM });
            if over && mq::is_mouse_button_pressed(mq::MouseButton::Left) {
                let n = MAPS.len() as u8;
                let back = mq::mouse_position().0 < mx + mw * 0.3;
                self.map = (self.map % n + if back { n - 1 } else { 1 }) % n;
            }
            if button(x + w - 20.0 - 200.0, fy, 200.0, 38.0, "HOST AND PLAY", EMBER, 22.0) {
                out = Some(self.host_choice());
            }
            let hp = self.f[F_HPORT].parse::<u16>().ok().filter(|p| *p != 0).unwrap_or(DEFAULT_PORT);
            let note = match &self.lan_ip {
                Some(ip) => format!("Empty slots are filled by bots. Others on your network can join at {ip}:{hp}"),
                None => "Empty slots are filled by bots.".to_string(),
            };
            mq::draw_text(&format!("{}: {}.", def.name, def.blurb), x + 20.0, fy + 64.0, 18.0, mq::Color::new(0.8, 0.78, 0.74, 1.0));
            mq::draw_text(&note, x + 20.0, fy + 88.0, 18.0, DIM);
            // Rotation: after each round the server moves to the next map.
            let rl = if self.rotate { "MAP ROTATION: ON" } else { "MAP ROTATION: OFF" };
            let rc = if self.rotate { mq::Color::new(0.25, 0.3, 0.2, 1.0) } else { mq::Color::new(0.16, 0.16, 0.16, 1.0) };
            if button(x + 20.0, fy + 100.0, 220.0, 28.0, rl, rc, 18.0) {
                self.rotate = !self.rotate;
            }
            let rn = if self.rotate { "Each round is played on the next map in the list." } else { "Every round is played on this map." };
            mq::draw_text(rn, x + 256.0, fy + 120.0, 18.0, DIM);
            y += host_h + 18.0;

            if button(x + w - 110.0, y, 110.0, 34.0, "QUIT", LINE, 20.0) || mq::is_key_pressed(mq::KeyCode::Escape) {
                out = Some(Choice::Quit);
            }
            mq::draw_text("TAB next field    ENTER join / host    CTRL+V paste", x, y + 23.0, 18.0, DIM);

            if out.is_none() && (mq::is_key_pressed(mq::KeyCode::Enter) || mq::is_key_pressed(mq::KeyCode::KpEnter)) {
                out = if self.focus >= F_BOTS { Some(self.host_choice()) } else { self.join_addr().map(Choice::Join) };
            }
            self.frames += 1;
            if self.frames == 20 {
                if let Some(p) = &self.shot {
                    mq::get_screen_data().export_png(p);
                    return Choice::Quit;
                }
            }
            if let Some(c) = out {
                self.error = None;
                self.save();
                return c;
            }
            mq::next_frame().await;
        }
    }
}
