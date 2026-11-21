//! FORGE's start screen: every map in the maps folder as a card with its
//! plan, when it was last changed and what is in it, and a card to start a
//! new one - blank, on a built-in map's ground, or as a copy to remix.
//!
//! The folder (`map::maps`) is read again whenever it changes on disk, on a
//! worker thread that also draws each map's plan, so a map made by the MCP
//! server or copied in by hand shows up on its own. Opening a map whose
//! autosave is newer than the file asks whether to bring that work back.

use crate::audio::{Music, Scene};
use crate::forge_library::{fit, in_rect, open_in_files, Rect};
use crate::plan::{self, Detail, Marks, Raster};
use crate::ui;
use ferrite_shared::map::maps::{self, MapFile, Start, Summary};
use ferrite_shared::map::{library, HALF_X, HALF_Z, MAPS};
use ferrite_shared::POINT_NAMES;
use macroquad::prelude as mq;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::SystemTime;
use ui::color as th;

/// Cards are at least this wide; as many fit across as can.
const MIN_CARD: f32 = 270.0;
const GAP: f32 = 18.0;
/// Where the grid starts, below the title.
const TOP: f32 = 146.0;
/// A card's name and details, below its plan.
const INFO: f32 = 62.0;
/// How often the folder is looked at for changes.
const CHECK: f64 = 1.0;

/// What the start screen was left for.
pub enum Pick {
    /// Edit this map; `restore` brings back its newer autosave.
    Open { path: PathBuf, restore: bool },
    /// Back to where FORGE was opened from.
    Leave,
    /// The window is closing.
    Quit,
}

enum Msg {
    /// The folder's maps, for listing `gen`.
    List(u64, Vec<MapFile>),
    /// A map's plan, as of its file's time; `None` if it cannot be drawn.
    Plan(PathBuf, SystemTime, Option<Raster>),
}

/// A line of typing: a map's name.
pub struct Field {
    pub text: String,
    repeat_at: f64,
}

impl Field {
    pub fn new(s: &str) -> Field {
        // What was typed before the field opened is not for it.
        while mq::get_char_pressed().is_some() {}
        Field { text: s.to_string(), repeat_at: 0.0 }
    }

    fn put(&mut self, c: char) {
        if (c.is_ascii_alphanumeric() || c == ' ') && self.text.chars().count() < 32 {
            self.text.push(c.to_ascii_uppercase());
        }
    }

    /// Take this frame's typing.
    pub fn typing(&mut self, now: f64) {
        while let Some(c) = mq::get_char_pressed() {
            self.put(c);
        }
        let ctrl = mq::is_key_down(mq::KeyCode::LeftControl) || mq::is_key_down(mq::KeyCode::RightControl);
        if ctrl && mq::is_key_pressed(mq::KeyCode::V) {
            if let Some(text) = macroquad::miniquad::window::clipboard_get() {
                for c in text.trim().chars() {
                    self.put(c);
                }
            }
        }
        if mq::is_key_pressed(mq::KeyCode::Backspace) {
            self.text.pop();
            self.repeat_at = now + 0.4;
        } else if mq::is_key_down(mq::KeyCode::Backspace) && now > self.repeat_at {
            self.text.pop();
            self.repeat_at = now + 0.04;
        }
    }

    pub fn draw(&self, x: f32, y: f32, w: f32, placeholder: &str, now: f64) {
        ui::plate(x, y, w, 42.0, th::INK, th::COBALT);
        let shown = if self.text.is_empty() { placeholder } else { &self.text };
        ui::text(&fit(shown, 21.0, w - 30.0), x + 12.0, y + 28.0, 21.0, if self.text.is_empty() { th::QUIET } else { th::WHITE });
        if (now * 2.0) as i64 % 2 == 0 {
            let cx = x + 13.0 + if self.text.is_empty() { 0.0 } else { ui::width(&self.text, 21.0) };
            mq::draw_rectangle(cx, y + 11.0, 2.0, 21.0, th::COBALT);
        }
    }
}

/// The "new map" panel.
struct NewMap {
    name: Field,
    /// The built-in map to start from; `None` is flat ground.
    from: Option<u8>,
    /// Copy everything on it, rather than only its ground.
    copy: bool,
    error: Option<String>,
}

impl NewMap {
    fn start(&self) -> Start {
        match self.from {
            None => Start::Blank,
            Some(id) if self.copy && maps::copyable(id) => Start::Copy(id),
            Some(id) => Start::Ground(id),
        }
    }

    /// The name used when none is typed.
    fn default_name(&self) -> String {
        match self.start() {
            Start::Blank => "UNTITLED".into(),
            Start::Ground(id) => format!("{} GROUND", MAPS[id as usize].name),
            Start::Copy(id) => format!("{} REMIX", MAPS[id as usize].name),
        }
    }

    fn name(&self) -> String {
        let n = maps::clean_name(&self.name.text);
        if n.is_empty() {
            maps::clean_name(&self.default_name())
        } else {
            n
        }
    }
}

enum Modal {
    Off,
    New(NewMap),
    /// The map at `path` has unsaved work from `at`.
    Restore {
        path: PathBuf,
        name: String,
        at: SystemTime,
    },
}

pub struct Home {
    root: PathBuf,
    sig: Option<u64>,
    checked: f64,
    maps: Vec<MapFile>,
    /// Whether the folder has been read yet.
    listed: bool,
    /// Each map's plan, as of its file's time (`None`: it cannot be drawn).
    plans: HashMap<PathBuf, (SystemTime, Option<mq::Texture2D>)>,
    rx: Option<mpsc::Receiver<Msg>>,
    /// Which listing is wanted; older ones are dropped.
    gen: u64,
    scroll: f32,
    modal: Modal,
    note: Option<(String, f64)>,
}

/// Where the cards go this frame.
struct Grid {
    x: f32,
    cols: usize,
    cw: f32,
    /// The plan's height on a card, and the card's.
    ph: f32,
    ch: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Primary,
    Plain,
    Off,
}

/// A button; whether it was clicked.
fn button(r: Rect, label: &str, style: Style, m: (f32, f32), click: bool) -> bool {
    let hot = style != Style::Off && in_rect(m.0, m.1, r);
    let (fill, stroke, ink) = match style {
        Style::Primary => (if hot { th::EMBER_BRIGHT } else { th::EMBER }, th::EMBER_BRIGHT, th::INK),
        Style::Plain => {
            (if hot { th::PANEL_RAISED } else { ui::fade(th::PANEL, 0.9) }, if hot { th::COBALT } else { th::STROKE }, th::WHITE)
        }
        Style::Off => (ui::fade(th::PANEL, 0.5), ui::fade(th::STROKE, 0.4), th::QUIET),
    };
    ui::plate(r.0, r.1, r.2, r.3, fill, stroke);
    ui::text_c(label, r.0 + r.2 / 2.0, r.1 + r.3 / 2.0 + 6.0, 16.0, ink);
    hot && click
}

/// Objectives and bases over a plan drawn at `x, y, w, h`.
fn marks(s: &Summary, x: f32, y: f32, w: f32, h: f32) {
    let to = |[px, pz]: [f32; 2]| (x + (px + HALF_X) / (2.0 * HALF_X) * w, y + (pz + HALF_Z) / (2.0 * HALF_Z) * h);
    let r = (w * 0.021).clamp(2.0, 8.0);
    let dark = mq::Color::new(0.047, 0.063, 0.125, 0.92);
    for (t, b) in s.bases.iter().enumerate() {
        let (bx, by) = to(*b);
        let [cr, cg, cb] = crate::scene::team_col(t as u8);
        mq::draw_rectangle(bx - r - 1.5, by - r - 1.5, 2.0 * r + 3.0, 2.0 * r + 3.0, dark);
        mq::draw_rectangle(bx - r, by - r, 2.0 * r, 2.0 * r, mq::Color::new(cr, cg, cb, 1.0));
    }
    let [nr, ng, nb] = crate::scene::NEUTRAL_COL;
    for (k, p) in s.points.iter().enumerate() {
        let (px, py) = to(*p);
        mq::draw_circle(px, py, r + 1.5, dark);
        mq::draw_circle(px, py, r, mq::Color::new(nr, ng, nb, 0.9));
        if w >= 200.0 {
            ui::text_c(POINT_NAMES[k], px, py + 4.5, 12.0, th::INK);
        }
    }
}

impl Home {
    pub fn new(root: PathBuf) -> Home {
        Home {
            root,
            sig: None,
            checked: 0.0,
            maps: Vec::new(),
            listed: false,
            plans: HashMap::new(),
            rx: None,
            gen: 0,
            scroll: 0.0,
            modal: Modal::Off,
            note: None,
        }
    }

    /// Open the "new map" panel.
    pub fn show_new(&mut self) {
        self.modal = Modal::New(NewMap { name: Field::new(""), from: None, copy: false, error: None });
    }

    /// Show `s` for a few seconds.
    pub fn say(&mut self, s: &str) {
        self.note = Some((s.to_string(), mq::get_time()));
    }

    /// Read the folder again if it has changed.
    fn refresh(&mut self, now: f64) {
        if self.sig.is_some() && now - self.checked < CHECK {
            return;
        }
        self.checked = now;
        let sig = library::signature(&self.root);
        if self.sig == Some(sig) {
            return;
        }
        self.sig = Some(sig);
        self.gen += 1;
        let (tx, rx) = mpsc::channel();
        let (root, gen) = (self.root.clone(), self.gen);
        let known: HashMap<PathBuf, SystemTime> = self.plans.iter().map(|(p, (t, _))| (p.clone(), *t)).collect();
        let spawned = std::thread::Builder::new().name("forge-maps".into()).spawn(move || {
            let list = maps::scan(&root);
            let todo: Vec<(PathBuf, SystemTime)> = list
                .iter()
                .filter(|m| m.doc.is_ok() && known.get(&m.path) != Some(&m.modified))
                .map(|m| (m.path.clone(), m.modified))
                .collect();
            if tx.send(Msg::List(gen, list)).is_err() {
                return;
            }
            for (path, at) in todo {
                let plan = maps::read(&path).ok().and_then(|d| plan::raster_doc(&d, Detail::Full).ok());
                if tx.send(Msg::Plan(path, at, plan)).is_err() {
                    return;
                }
            }
        });
        // A newer listing replaces the old one; its worker stops at its
        // next send.
        if spawned.is_ok() {
            self.rx = Some(rx);
        }
    }

    /// Take what the worker has finished.
    fn pump(&mut self) {
        let Some(rx) = &self.rx else { return };
        let msgs: Vec<Msg> = rx.try_iter().collect();
        for m in msgs {
            match m {
                Msg::List(gen, list) if gen == self.gen => {
                    self.plans.retain(|p, _| list.iter().any(|m| &m.path == p));
                    self.maps = list;
                    self.listed = true;
                }
                Msg::List(..) => {}
                Msg::Plan(path, at, plan) => {
                    let tex = plan.map(|r| {
                        let t = mq::Texture2D::from_rgba8(r.w, r.h, &r.rgba);
                        t.set_filter(mq::FilterMode::Linear);
                        t
                    });
                    self.plans.insert(path, (at, tex));
                }
            }
        }
    }

    fn grid(&self, sw: f32) -> Grid {
        let x = if sw < 900.0 { 20.0 } else { 44.0 };
        let avail = sw - 2.0 * x;
        let cols = (((avail + GAP) / (MIN_CARD + GAP)).floor() as usize).clamp(1, 6);
        let cw = (avail - GAP * (cols - 1) as f32) / cols as f32;
        let ph = (cw - 16.0) / plan::ASPECT;
        Grid { x, cols, cw, ph, ch: ph + 16.0 + INFO }
    }

    fn card_rect(&self, g: &Grid, k: usize) -> Rect {
        let (c, r) = (k % g.cols, k / g.cols);
        (g.x + c as f32 * (g.cw + GAP), TOP + r as f32 * (g.ch + GAP) - self.scroll, g.cw, g.ch)
    }

    fn most_scroll(&self, g: &Grid, sh: f32) -> f32 {
        let rows = (self.maps.len() + 1).div_ceil(g.cols);
        (rows as f32 * (g.ch + GAP) - GAP - (sh - TOP - 24.0)).max(0.0)
    }

    /// Open the map at `path`, or ask first if it has newer unsaved work.
    fn choose(&mut self, path: PathBuf) -> Option<Pick> {
        match maps::newer_autosave(&path) {
            Some(at) => {
                let name = self
                    .maps
                    .iter()
                    .find(|m| m.path == path)
                    .map(|m| m.title())
                    .or_else(|| maps::read(&path).ok().map(|d| d.meta.name))
                    .unwrap_or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default());
                self.modal = Modal::Restore { path, name, at };
                None
            }
            None => Some(Pick::Open { path, restore: false }),
        }
    }

    /// The start screen until a map is picked or it is left. `want` opens
    /// that map straight away (asking first if it has unsaved work);
    /// `shot` saves a picture of it at a time and then quits.
    pub async fn run(&mut self, art: &ui::Art, music: &mut Music, want: Option<PathBuf>, shot: Option<(&str, f64)>) -> Pick {
        if let Some(p) = want {
            if let Some(pick) = self.choose(p) {
                return pick;
            }
        }
        mq::set_cursor_grab(false);
        mq::show_mouse(true);
        let start = mq::get_time();
        let mut shot_done = false;
        loop {
            let now = mq::get_time();
            music.update(Scene::Forge, mq::get_frame_time().min(0.1)).await;
            if mq::is_quit_requested() {
                return Pick::Quit;
            }
            self.refresh(now);
            self.pump();
            if let Some(pick) = self.frame(art, now) {
                return pick;
            }
            if let Some((path, at)) = shot {
                if !shot_done && now - start >= at {
                    shot_done = true;
                    mq::get_screen_data().export_png(path);
                }
                if now - start >= at + 0.5 {
                    return Pick::Quit;
                }
            }
            mq::next_frame().await;
        }
    }

    /// Draw the screen and act on this frame's input.
    fn frame(&mut self, art: &ui::Art, now: f64) -> Option<Pick> {
        let (sw, sh) = (mq::screen_width(), mq::screen_height());
        let m = mq::mouse_position();
        let click = mq::is_mouse_button_pressed(mq::MouseButton::Left);
        let ctrl = mq::is_key_down(mq::KeyCode::LeftControl) || mq::is_key_down(mq::KeyCode::RightControl);
        let esc = mq::is_key_pressed(mq::KeyCode::Escape);
        let free = matches!(self.modal, Modal::Off);
        // The grid only answers when nothing is over it.
        let (gm, gclick) = if free { (m, click) } else { ((-1.0, -1.0), false) };
        let mut out = None;

        mq::clear_background(th::INK);
        ui::backdrop(&art.lobby, sw, sh);
        mq::draw_rectangle(0.0, 0.0, sw, sh, ui::fade(th::INK, 0.86));

        // The cards.
        let g = self.grid(sw);
        let most = self.most_scroll(&g, sh);
        let wheel = mq::mouse_wheel().1;
        if free && wheel != 0.0 {
            self.scroll -= wheel.signum() * (g.ch + GAP) * 0.5;
        }
        self.scroll = self.scroll.clamp(0.0, most);
        let mut picked = None;
        for k in 0..=self.maps.len() {
            let r = self.card_rect(&g, k);
            if r.1 > sh || r.1 + r.3 < TOP - 20.0 {
                continue;
            }
            let hot = in_rect(gm.0, gm.1, r) && gm.1 > TOP - 12.0;
            if hot && gclick {
                picked = Some(k);
            }
            if k == 0 {
                self.new_card(&g, r, hot);
            } else {
                self.map_card(&g, k - 1, r, hot);
            }
        }
        if self.listed && self.maps.is_empty() {
            let (x, y) = if g.cols > 1 {
                let r = self.card_rect(&g, 1);
                (r.0 + 8.0, r.1 + 40.0)
            } else {
                (g.x, TOP + g.ch + 40.0)
            };
            ui::text("No maps here yet.", x, y, 24.0, th::WHITE);
            ui::text("Start one, or put .fmap files in the folder.", x, y + 30.0, 17.0, th::MUTED);
        }

        // The title over the top, which cards scroll under.
        mq::draw_rectangle(0.0, 0.0, sw, TOP - 12.0, mq::Color::new(0.047, 0.063, 0.125, 0.97));
        let x = g.x;
        ui::text("FORGE  /  MAP MAKER", x, 46.0, 17.0, th::EMBER_BRIGHT);
        ui::text("Your maps", x, 86.0, 36.0, th::WHITE);
        mq::draw_line(x, 102.0, sw - x, 102.0, 1.0, th::STROKE);
        let count = if self.listed {
            let n = self.maps.len();
            format!("{n} MAP{}  -  {}", if n == 1 { "" } else { "S" }, self.root.display())
        } else {
            format!("READING  -  {}", self.root.display())
        };
        ui::text(&fit(&count, 14.0, sw - 2.0 * x), x, 124.0, 14.0, th::QUIET);
        let (bw, bh, by) = (150.0, 38.0, 52.0);
        let back = (sw - x - 110.0, by, 110.0, bh);
        let folder = (back.0 - 12.0 - bw, by, bw, bh);
        let new = (folder.0 - 12.0 - bw, by, bw, bh);
        if button(back, "BACK  ESC", Style::Plain, gm, gclick) || (free && esc) {
            out = Some(Pick::Leave);
        }
        if button(folder, "OPEN FOLDER", Style::Plain, gm, gclick) {
            open_in_files(&self.root);
        }
        if button(new, "NEW MAP", Style::Primary, gm, gclick) || (free && ctrl && mq::is_key_pressed(mq::KeyCode::N)) {
            self.show_new();
        }
        match picked {
            Some(0) => self.show_new(),
            Some(k) => match self.maps.get(k - 1) {
                Some(f) if f.doc.is_err() && maps::newer_autosave(&f.path).is_none() => {
                    let e = f.doc.clone().err().unwrap_or_default();
                    self.say(&format!("CAN'T OPEN {}: {}", f.title(), e.to_uppercase()));
                }
                Some(f) => out = out.or(self.choose(f.path.clone())),
                None => {}
            },
            None => {}
        }

        // Whatever is open over the grid.
        if !matches!(self.modal, Modal::Off) {
            mq::draw_rectangle(0.0, 0.0, sw, sh, ui::fade(th::INK, 0.7));
        }
        let modal = std::mem::replace(&mut self.modal, Modal::Off);
        self.modal = match modal {
            Modal::Off => Modal::Off,
            Modal::New(mut n) => match self.new_panel(&mut n, now, m, click, esc) {
                Some(Ok(pick)) => {
                    out = Some(pick);
                    Modal::Off
                }
                Some(Err(())) => Modal::Off,
                None => Modal::New(n),
            },
            Modal::Restore { path, name, at } => {
                let pw = (sw - 40.0).min(600.0);
                let (px, py, ph) = ((sw - pw) / 2.0, (sh - 220.0) / 2.0, 220.0);
                ui::plate(px, py, pw, ph, th::PANEL, th::WARNING);
                ui::text(&fit(&format!("UNSAVED WORK ON {name}"), 22.0, pw - 48.0), px + 24.0, py + 42.0, 22.0, th::WARNING);
                ui::text(
                    &format!("FORGE kept changes from {} that were never saved.", maps::ago(at)),
                    px + 24.0,
                    py + 78.0,
                    17.0,
                    th::WHITE,
                );
                ui::text("Bring them back to keep working on them, or open the last save.", px + 24.0, py + 102.0, 17.0, th::MUTED);
                let bw = (pw - 48.0 - 24.0) / 3.0;
                let row = py + ph - 64.0;
                let restore =
                    button((px + 24.0, row, bw, 42.0), "RESTORE THEM", Style::Primary, m, click) || mq::is_key_pressed(mq::KeyCode::Enter);
                let saved = button((px + 36.0 + bw, row, bw, 42.0), "OPEN LAST SAVE", Style::Plain, m, click);
                let cancel = button((px + 48.0 + 2.0 * bw, row, bw, 42.0), "CANCEL", Style::Plain, m, click) || esc;
                if restore {
                    out = Some(Pick::Open { path, restore: true });
                    Modal::Off
                } else if saved {
                    // The last save it is: the autosave would only ask again.
                    let _ = std::fs::remove_file(maps::autosave_of(&path));
                    out = Some(Pick::Open { path, restore: false });
                    Modal::Off
                } else if cancel {
                    Modal::Off
                } else {
                    Modal::Restore { path, name, at }
                }
            }
        };

        if let Some((msg, at)) = &self.note {
            if now - at < 5.0 {
                let w = (ui::width(msg, 18.0) + 40.0).min(sw - 40.0);
                ui::plate((sw - w) / 2.0, sh - 70.0, w, 40.0, th::GLASS_STRONG, th::DANGER);
                ui::text_c(&fit(msg, 18.0, w - 30.0), sw / 2.0, sh - 44.0, 18.0, th::WHITE);
            }
        }
        out
    }

    fn new_card(&self, g: &Grid, r: Rect, hot: bool) {
        let (x, y, w, h) = r;
        mq::draw_rectangle(x, y, w, h, if hot { th::PANEL_RAISED } else { ui::fade(th::PANEL, 0.9) });
        mq::draw_rectangle_lines(x, y, w, h, 2.0, if hot { th::EMBER } else { th::EMBER_DIM });
        let (ix, iy, iw, ih) = (x + 8.0, y + 8.0, w - 16.0, g.ph);
        mq::draw_rectangle(ix, iy, iw, ih, th::NIGHT);
        mq::draw_rectangle_lines(ix, iy, iw, ih, 1.5, ui::fade(th::EMBER_DIM, if hot { 0.9 } else { 0.5 }));
        let (cx, cy) = (ix + iw / 2.0, iy + ih / 2.0);
        let c = if hot { th::EMBER_BRIGHT } else { th::EMBER };
        mq::draw_rectangle(cx - 26.0, cy - 3.0, 52.0, 6.0, c);
        mq::draw_rectangle(cx - 3.0, cy - 26.0, 6.0, 52.0, c);
        ui::text("NEW MAP", x + 12.0, y + g.ph + 42.0, 20.0, th::WHITE);
        ui::text(&fit("BLANK, ON A BUILT-IN GROUND, OR A REMIX", 13.0, w - 24.0), x + 12.0, y + g.ph + 64.0, 13.0, th::QUIET);
    }

    fn map_card(&self, g: &Grid, k: usize, r: Rect, hot: bool) {
        let Some(f) = self.maps.get(k) else { return };
        let (x, y, w, h) = r;
        mq::draw_rectangle(x, y, w, h, if hot { th::PANEL_RAISED } else { ui::fade(th::PANEL, 0.9) });
        mq::draw_rectangle_lines(x, y, w, h, 2.0, if hot { th::COBALT } else { ui::fade(th::STROKE, 0.6) });
        let (ix, iy, iw, ih) = (x + 8.0, y + 8.0, w - 16.0, g.ph);
        match self.plans.get(&f.path) {
            Some((_, Some(t))) => {
                mq::draw_texture_ex(t, ix, iy, mq::WHITE, mq::DrawTextureParams { dest_size: Some(mq::vec2(iw, ih)), ..Default::default() })
            }
            other => {
                mq::draw_rectangle(ix, iy, iw, ih, th::NIGHT);
                let what = match (&f.doc, other) {
                    (Err(_), _) => "CAN'T READ THIS MAP",
                    (_, Some(_)) => "NO PLAN",
                    _ => "DRAWING PLAN...",
                };
                ui::text_c(what, ix + iw / 2.0, iy + ih / 2.0 + 5.0, 14.0, if f.doc.is_err() { th::DANGER } else { th::QUIET });
            }
        }
        if let Ok(s) = &f.doc {
            marks(s, ix, iy, iw, ih);
        }
        if f.autosave.is_some() {
            let label = "UNSAVED WORK";
            let bw = ui::width(label, 12.0) + 18.0;
            ui::plate(ix + 8.0, iy + 8.0, bw, 24.0, ui::fade(th::INK, 0.88), th::WARNING);
            ui::text(label, ix + 17.0, iy + 25.0, 12.0, th::WARNING);
        }
        ui::text(&fit(&f.title(), 20.0, w - 24.0), x + 12.0, y + g.ph + 42.0, 20.0, th::WHITE);
        // When and how big; under the mouse, which file it is.
        let file = f.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (info, ink) = match &f.doc {
            _ if hot => (format!("OPEN  {file}"), th::COBALT_BRIGHT),
            Ok(s) => (
                format!("EDITED {}  /  {} BLOCK{}", maps::ago(f.modified).to_uppercase(), s.blocks, if s.blocks == 1 { "" } else { "S" }),
                th::QUIET,
            ),
            Err(e) => (e.to_uppercase(), th::DANGER),
        };
        ui::text(&fit(&info, 13.0, w - 24.0), x + 12.0, y + g.ph + 64.0, 13.0, ink);
    }

    /// The "new map" panel: `Some(Ok)` makes and opens the map, `Some(Err)`
    /// closes the panel.
    fn new_panel(&mut self, n: &mut NewMap, now: f64, m: (f32, f32), click: bool, esc: bool) -> Option<Result<Pick, ()>> {
        let (sw, sh) = (mq::screen_width(), mq::screen_height());
        let pw = (sw - 32.0).min(860.0);
        let pad = 24.0;
        let cols = 5;
        let tw = (pw - 2.0 * pad - 12.0 * (cols - 1) as f32) / cols as f32;
        let tih = tw / plan::ASPECT;
        let th_ = tih + 30.0;
        let ph = 410.0 + 2.0 * th_;
        let (px, py) = ((sw - pw) / 2.0, ((sh - ph) / 2.0).max(12.0));
        ui::plate(px, py, pw, ph, th::PANEL, th::STROKE);
        ui::text("NEW MAP", px + pad, py + 42.0, 26.0, th::WHITE);
        let mut y = py + 72.0;

        ui::text("NAME", px + pad, y + 14.0, 14.0, th::QUIET);
        y += 22.0;
        n.name.typing(now);
        n.name.draw(px + pad, y, pw - 2.0 * pad, &n.default_name(), now);
        y += 62.0;

        ui::text("START FROM", px + pad, y + 14.0, 14.0, th::QUIET);
        y += 24.0;
        for k in 0..=MAPS.len() {
            let (c, r) = (k % cols, k / cols);
            let rect = (px + pad + c as f32 * (tw + 12.0), y + r as f32 * (th_ + 12.0), tw, th_);
            let hot = in_rect(m.0, m.1, rect);
            let id = k.checked_sub(1).map(|i| i as u8);
            let on = n.from == id;
            mq::draw_rectangle(rect.0, rect.1, rect.2, rect.3, if hot { th::PANEL_RAISED } else { th::NIGHT });
            let (ix, iy, iw) = (rect.0 + 4.0, rect.1 + 4.0, tw - 8.0);
            let ih = iw / plan::ASPECT;
            match id {
                None => {
                    let [r_, g_, b_] = MAPS[0].ground[0];
                    mq::draw_rectangle(ix, iy, iw, ih, mq::Color::new(r_ * 0.7, g_ * 0.7, b_ * 0.7, 1.0));
                    ui::text_c("FLAT", ix + iw / 2.0, iy + ih / 2.0 + 6.0, 16.0, th::WHITE);
                }
                Some(id) => plan::draw(id, ix, iy, iw, ih, Marks { flip: false, mode: None, alpha: 1.0 }),
            }
            let label = match id {
                None => "BLANK".to_string(),
                Some(id) => MAPS[id as usize].name.to_string(),
            };
            ui::text_c(&fit(&label, 12.0, tw - 8.0), rect.0 + tw / 2.0, rect.1 + th_ - 9.0, 12.0, if on { th::WHITE } else { th::MUTED });
            mq::draw_rectangle_lines(
                rect.0,
                rect.1,
                rect.2,
                rect.3,
                if on { 3.0 } else { 1.5 },
                if on {
                    th::EMBER
                } else if hot {
                    th::COBALT
                } else {
                    ui::fade(th::STROKE, 0.5)
                },
            );
            if hot && click {
                n.from = id;
                n.error = None;
            }
        }
        y += 2.0 * (th_ + 12.0) + 8.0;

        // Ground only, or everything on it.
        let half = (pw - 2.0 * pad - 12.0) / 2.0;
        let can_copy = n.from.is_some_and(maps::copyable);
        let seg = |x: f32, label: &str, on: bool, live: bool| -> bool {
            let r = (x, y, half, 38.0);
            let hot = live && in_rect(m.0, m.1, r);
            let fill = if on {
                ui::fade(th::COBALT_DIM, 0.8)
            } else if hot {
                th::PANEL_RAISED
            } else {
                th::NIGHT
            };
            ui::plate(r.0, r.1, r.2, r.3, fill, if on { th::COBALT } else { ui::fade(th::STROKE, 0.5) });
            ui::text_c(
                label,
                x + half / 2.0,
                y + 25.0,
                15.0,
                if !live {
                    th::QUIET
                } else if on {
                    th::WHITE
                } else {
                    th::MUTED
                },
            );
            hot && click
        };
        let live = n.from.is_some();
        if seg(px + pad, "GROUND ONLY", live && !(n.copy && can_copy), live) {
            n.copy = false;
        }
        if seg(px + pad + half + 12.0, "EVERYTHING ON IT, TO REMIX", live && n.copy && can_copy, live && can_copy) {
            n.copy = true;
        }
        y += 60.0;
        let about = match n.start() {
            Start::Blank => "Flat ground and the standard objective layout: build everything yourself.".to_string(),
            Start::Ground(id) if n.copy => {
                format!("{} has tunnels, which maps cannot hold yet: you get its ground, sky and light.", MAPS[id as usize].name)
            }
            Start::Ground(id) => format!("{}'s ground, sky and light, with nothing built on it.", MAPS[id as usize].name),
            Start::Copy(id) => format!("Everything on {}, as a map of your own to change.", MAPS[id as usize].name),
        };
        ui::text(&fit(&about, 17.0, pw - 2.0 * pad), px + pad, y, 17.0, th::WHITE);
        y += 26.0;
        let name = n.name();
        let path = maps::fresh_path(&self.root, &name);
        let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let saves = format!("SAVES AS {file} IN THE MAPS FOLDER");
        ui::text(&fit(&saves, 14.0, pw - 2.0 * pad), px + pad, y, 14.0, th::QUIET);
        if let Some(e) = &n.error {
            ui::text(&fit(&e.to_uppercase(), 15.0, pw - 2.0 * pad), px + pad, y + 24.0, 15.0, th::DANGER);
        }

        let row = py + ph - 66.0;
        let cancel = button((px + pw - pad - 300.0 - 12.0, row, 150.0, 44.0), "CANCEL  ESC", Style::Plain, m, click) || esc;
        let create =
            button((px + pw - pad - 150.0, row, 150.0, 44.0), "CREATE", Style::Primary, m, click) || mq::is_key_pressed(mq::KeyCode::Enter);
        if cancel {
            return Some(Err(()));
        }
        if create {
            let made = maps::starter(&name, n.start()).and_then(|d| {
                let p = maps::fresh_path(&self.root, &d.meta.name);
                maps::write(&p, &d).map(|_| p)
            });
            match made {
                Ok(path) => return Some(Ok(Pick::Open { path, restore: false })),
                Err(e) => n.error = Some(e),
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(text: &str, from: Option<u8>, copy: bool) -> NewMap {
        NewMap { name: Field { text: text.into(), repeat_at: 0.0 }, from, copy, error: None }
    }

    #[test]
    fn new_maps_start_from_what_was_picked_and_are_named_for_it() {
        assert_eq!(panel("", None, true).start(), Start::Blank);
        assert_eq!(panel("", None, false).name(), "UNTITLED");
        assert_eq!(panel("", Some(3), false).start(), Start::Ground(3));
        assert_eq!(panel("", Some(3), false).name(), "DEPOT GROUND");
        assert_eq!(panel("", Some(3), true).start(), Start::Copy(3));
        assert_eq!(panel("", Some(3), true).name(), "DEPOT REMIX");
        // A map with tunnels can only lend its ground.
        let tunnels = (0..MAPS.len() as u8).find(|&id| !maps::copyable(id)).expect("a map with tunnels");
        assert_eq!(panel("", Some(tunnels), true).start(), Start::Ground(tunnels));
        // What is typed wins, cleaned up.
        assert_eq!(panel("  dock   fight ", Some(3), true).name(), "DOCK FIGHT");
    }
}
