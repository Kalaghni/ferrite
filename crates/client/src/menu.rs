//! Home screen: pick a saved server, type an address, or host a match.
//! Saved servers and the player name live in `ferrite-client.txt` next to
//! the executable.

use ferrite_shared::proto::DEFAULT_PORT;
use macroquad::prelude as mq;
use std::path::PathBuf;

pub enum Choice {
    Host { port: u16, per_team: usize },
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
    pub error: Option<String>,
    lan_ip: Option<String>,
    repeat_at: f64,
    /// Verification aid: save a picture of the home screen, then quit.
    pub shot: Option<String>,
    frames: u32,
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
            error: None,
            lan_ip: lan_ip(),
            repeat_at: 0.0,
            shot: None,
            frames: 0,
        };
        if let Ok(text) = std::fs::read_to_string(cfg_path()) {
            for line in text.lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                let v = v.trim();
                match k.trim() {
                    "name" if !v.is_empty() => m.f[F_NAME] = v.chars().take(16).collect(),
                    "server" if !v.is_empty() && m.servers.len() < MAX_SAVED => m.servers.push(v.to_string()),
                    "bots" if v.parse::<usize>().is_ok() => m.f[F_BOTS] = v.to_string(),
                    _ => {}
                }
            }
        }
        m
    }

    fn save(&self) {
        let mut out = format!("name={}\nbots={}\n", self.f[F_NAME], self.f[F_BOTS]);
        for s in &self.servers {
            out += &format!("server={s}\n");
        }
        let _ = std::fs::write(cfg_path(), out);
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
            let mut y = (sh * 0.07).max(20.0).floor();

            // Title with the two team colours.
            mq::draw_rectangle(x, y, 6.0, 46.0, EMBER);
            mq::draw_rectangle(x + 10.0, y, 6.0, 46.0, COBALT);
            mq::draw_text("FERRITE", x + 30.0, y + 40.0, 58.0, mq::WHITE);
            let tag = "12v12 front-line conquest";
            let tm = mq::measure_text(tag, None, 18, 1.0);
            mq::draw_text(tag, x + w - tm.width, y + 36.0, 18.0, DIM);
            y += 92.0;

            self.field(F_NAME, x, y, 260.0, "CALLSIGN", "Player", now);
            y += 62.0;

            if let Some(e) = &self.error {
                mq::draw_rectangle(x, y, w, 34.0, mq::Color::new(0.35, 0.1, 0.06, 1.0));
                mq::draw_text(e, x + 12.0, y + 23.0, 20.0, mq::Color::new(1.0, 0.8, 0.7, 1.0));
                y += 46.0;
            }

            // ---- join ----
            let join_h = 118.0 + 44.0 * self.servers.len().max(1) as f32 + 34.0;
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
            mq::draw_text("SAVED SERVERS", x + 20.0, ry + 14.0, 16.0, DIM);
            ry += 24.0;
            if self.servers.is_empty() {
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
            let host_h = 150.0;
            mq::draw_rectangle(x, y, w, host_h, PANEL);
            mq::draw_rectangle(x, y, 4.0, host_h, EMBER);
            mq::draw_text("HOST A MATCH", x + 20.0, y + 30.0, 24.0, mq::WHITE);
            let fy = y + 62.0;
            self.field(F_BOTS, x + 20.0, fy, 150.0, "PLAYERS PER TEAM", "12", now);
            self.field(F_HPORT, x + 186.0, fy, port_w, "PORT", &DEFAULT_PORT.to_string(), now);
            if button(x + w - 20.0 - 200.0, fy, 200.0, 38.0, "HOST AND PLAY", EMBER, 22.0) {
                out = Some(self.host_choice());
            }
            let hp = self.f[F_HPORT].parse::<u16>().ok().filter(|p| *p != 0).unwrap_or(DEFAULT_PORT);
            let note = match &self.lan_ip {
                Some(ip) => format!("Empty slots are filled by bots. Others on your network can join at {ip}:{hp}"),
                None => "Empty slots are filled by bots.".to_string(),
            };
            mq::draw_text(&note, x + 20.0, fy + 66.0, 18.0, DIM);
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
