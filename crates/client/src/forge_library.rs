//! The map maker's library drawer: folders of models as a grid of cards,
//! dragged into the world or clicked to stamp again and again.
//!
//! The library itself (`map::library`) is a folder on disk plus the
//! built-in models; this is only its window. It looks for changes on disk
//! every couple of seconds, so a model saved by the MCP server, or a
//! folder made in the file browser, shows up on its own.

use crate::forge_thumbs;
use crate::ui;
use ferrite_shared::map::library::{self, Entry, Library};
use macroquad::prelude as mq;
use std::collections::HashMap;
use ui::color as th;

const PAD: f32 = 14.0;
const CARD_W: f32 = 124.0;
const CARD_H: f32 = 152.0;
const GAP: f32 = 10.0;
const THUMB: usize = 104;
/// Above the grid: the title and the folder path.
const HEAD: f32 = 76.0;
/// Below it: the hint line.
const FOOT: f32 = 34.0;
/// Pictures made a frame, so opening a big folder does not stall.
const THUMBS_A_FRAME: usize = 6;

#[derive(Clone, PartialEq, Debug)]
pub enum Card {
    Back,
    Folder(String),
    /// A model, by its key (which survives the library being reread).
    Model(u64),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Drag {
    Idle,
    /// Pressed on a card; a click unless the mouse moves.
    Press(Card, (f32, f32)),
    /// A model being carried to the world.
    Carry(u64),
}

#[derive(Clone, PartialEq, Debug)]
enum Hit {
    Card(Card),
    Tab,
    Hide,
    Reveal,
}

pub struct Shelf {
    pub lib: Library,
    sig: u64,
    checked: f64,
    pub open: bool,
    folder: String,
    /// Rows scrolled past.
    scroll: usize,
    thumbs: HashMap<u64, mq::Texture2D>,
    /// The hovered model, turning: its key, since when, and its picture.
    spin: Option<(u64, f64, mq::Texture2D)>,
    hover: Option<Card>,
    pub drag: Drag,
    /// A model clicked to stamp with each click on the world.
    pub armed: Option<u64>,
    /// Quarter turns for the next stamp.
    pub turn: u8,
    /// The window's size, as of the last frame.
    screen: (f32, f32),
}

struct Layout {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    cols: usize,
    rows: usize,
}

/// x, y, width, height.
pub(crate) type Rect = (f32, f32, f32, f32);

pub(crate) fn in_rect(mx: f32, my: f32, r: Rect) -> bool {
    mx >= r.0 && my >= r.1 && mx < r.0 + r.2 && my < r.1 + r.3
}

/// `s` cut to fit `w` pixels at `size`.
pub(crate) fn fit(s: &str, size: f32, w: f32) -> String {
    if ui::width(s, size) <= w {
        return s.to_string();
    }
    let mut t: String = s.to_string();
    while !t.is_empty() && ui::width(&format!("{t}..."), size) > w {
        t.pop();
    }
    format!("{}...", t.trim_end())
}

/// Show folder `root` in the system's file browser, making it if need be.
pub(crate) fn open_in_files(root: &std::path::Path) {
    let _ = std::fs::create_dir_all(root);
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(root).spawn();
}

impl Shelf {
    pub fn new() -> Shelf {
        let root = library::default_root();
        Shelf {
            sig: library::signature(&root),
            lib: Library::load(&root),
            checked: 0.0,
            open: true,
            folder: String::new(),
            scroll: 0,
            thumbs: HashMap::new(),
            spin: None,
            hover: None,
            drag: Drag::Idle,
            armed: None,
            turn: 0,
            screen: (1280.0, 720.0),
        }
    }

    /// Read the library again if it changed on disk.
    pub fn refresh(&mut self, now: f64) {
        if !self.open || now - self.checked < 2.0 {
            return;
        }
        self.checked = now;
        let sig = library::signature(&self.lib.root);
        if sig != self.sig {
            self.sig = sig;
            self.lib = Library::load(&self.lib.root);
            let keys: Vec<u64> = self.lib.entries.iter().map(|e| e.key).collect();
            self.thumbs.retain(|k, _| keys.contains(k));
            if self.armed.is_some_and(|k| !keys.contains(&k)) {
                self.armed = None;
            }
            if !self.folder.is_empty() && !self.lib.folders.contains(&self.folder) {
                self.folder.clear();
                self.scroll = 0;
            }
        }
    }

    /// Open the drawer at `folder`, if there is one by that name.
    pub fn show(&mut self, folder: &str) {
        let f = library::clean_folder(folder);
        if f.is_empty() || self.lib.folders.contains(&f) {
            self.folder = f;
            self.scroll = 0;
            self.open = true;
        }
    }

    pub fn entry(&self, key: u64) -> Option<&Entry> {
        self.lib.entries.iter().find(|e| e.key == key)
    }

    /// The model the next click or drop on the world puts down.
    pub fn stamping(&self) -> Option<u64> {
        match self.drag {
            Drag::Carry(k) => Some(k),
            _ => self.armed,
        }
    }

    /// Stop carrying or stamping. True if there was something to stop.
    pub fn cancel(&mut self) -> bool {
        let busy = self.stamping().is_some();
        self.drag = Drag::Idle;
        self.armed = None;
        busy
    }

    fn layout(&self) -> Layout {
        let (sw, sh) = self.screen;
        let cols = if sw < 1150.0 { 2 } else { 3 };
        let w = PAD * 2.0 + cols as f32 * CARD_W + (cols - 1) as f32 * GAP;
        let h = (sh - 64.0 - 32.0).max(HEAD + FOOT + CARD_H);
        let rows = (((h - HEAD - FOOT) + GAP) / (CARD_H + GAP)).floor().max(1.0) as usize;
        Layout { x: sw - w - 16.0, y: 16.0, w, h, cols, rows }
    }

    fn tab_rect(&self) -> Rect {
        (self.screen.0 - 16.0 - 156.0, 16.0, 156.0, 40.0)
    }

    fn buttons(&self, l: &Layout) -> [(Hit, Rect, &'static str); 2] {
        let y = l.y + 12.0;
        [
            (Hit::Reveal, (l.x + l.w - PAD - 160.0, y, 96.0, 28.0), "OPEN FOLDER"),
            (Hit::Hide, (l.x + l.w - PAD - 56.0, y, 56.0, 28.0), "HIDE"),
        ]
    }

    /// Whether the mouse is over the drawer (or its tab when closed).
    /// Where the open drawer's left edge is on screen.
    pub fn left(&self) -> Option<f32> {
        self.open.then(|| self.layout().x)
    }

    pub fn over(&self, mx: f32, my: f32) -> bool {
        if !self.open {
            return in_rect(mx, my, self.tab_rect());
        }
        let l = self.layout();
        in_rect(mx, my, (l.x, l.y, l.w, l.h))
    }

    fn cards(&self) -> Vec<Card> {
        let (subs, items) = self.lib.children(&self.folder);
        let mut out = Vec::new();
        if !self.folder.is_empty() {
            out.push(Card::Back);
        }
        out.extend(subs.into_iter().map(Card::Folder));
        out.extend(items.into_iter().map(|i| Card::Model(self.lib.entries[i].key)));
        out
    }

    fn card_rect(&self, l: &Layout, k: usize) -> Option<Rect> {
        let row = k / l.cols;
        if row < self.scroll || row >= self.scroll + l.rows {
            return None;
        }
        let (c, r) = ((k % l.cols) as f32, (row - self.scroll) as f32);
        Some((l.x + PAD + c * (CARD_W + GAP), l.y + HEAD + r * (CARD_H + GAP), CARD_W, CARD_H))
    }

    fn hit(&self, mx: f32, my: f32) -> Option<Hit> {
        if !self.open {
            return in_rect(mx, my, self.tab_rect()).then_some(Hit::Tab);
        }
        let l = self.layout();
        for (h, r, _) in self.buttons(&l) {
            if in_rect(mx, my, r) {
                return Some(h);
            }
        }
        self.cards()
            .into_iter()
            .enumerate()
            .find(|(k, _)| self.card_rect(&l, *k).is_some_and(|r| in_rect(mx, my, r)))
            .map(|(_, c)| Hit::Card(c))
    }

    /// Scroll if the mouse is over the drawer. True if it took the wheel.
    pub fn wheel(&mut self, mx: f32, my: f32, dy: f32) -> bool {
        if !self.open || !self.over(mx, my) {
            return false;
        }
        let l = self.layout();
        let rows = self.cards().len().div_ceil(l.cols);
        let most = rows.saturating_sub(l.rows);
        self.scroll = if dy > 0.0 { self.scroll.saturating_sub(1) } else { (self.scroll + 1).min(most) };
        true
    }

    fn reveal(&self) {
        open_in_files(&self.lib.root);
    }

    fn click(&mut self, c: Card) {
        match c {
            Card::Back => {
                self.folder = self.folder.rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_default();
                self.scroll = 0;
            }
            Card::Folder(f) => {
                self.folder = f;
                self.scroll = 0;
            }
            Card::Model(k) => self.armed = if self.armed == Some(k) { None } else { Some(k) },
        }
    }

    /// Presses, drags and releases. Returns a model dropped on the world.
    pub fn mouse(&mut self, (mx, my): (f32, f32)) -> Option<u64> {
        self.screen = (mq::screen_width(), mq::screen_height());
        let left = mq::MouseButton::Left;
        self.pointer(mx, my, mq::is_mouse_button_pressed(left), mq::is_mouse_button_down(left))
    }

    /// The pointer at `mx, my`, its left button just `pressed` and/or held
    /// `down`.
    fn pointer(&mut self, mx: f32, my: f32, pressed: bool, down: bool) -> Option<u64> {
        if pressed && self.over(mx, my) {
            match self.hit(mx, my) {
                Some(Hit::Card(c)) => self.drag = Drag::Press(c, (mx, my)),
                Some(Hit::Tab) => self.open = true,
                Some(Hit::Hide) => self.open = false,
                Some(Hit::Reveal) => self.reveal(),
                None => {}
            }
        }
        if let Drag::Press(Card::Model(k), (x0, y0)) = self.drag {
            if (mx - x0).hypot(my - y0) > 6.0 {
                self.drag = Drag::Carry(k);
            }
        }
        if !down {
            match std::mem::replace(&mut self.drag, Drag::Idle) {
                Drag::Press(c, _) => self.click(c),
                Drag::Carry(k) if !self.over(mx, my) => return Some(k),
                _ => {}
            }
        }
        None
    }

    /// Make the pictures the grid needs, and turn the hovered one.
    pub fn prepare(&mut self, now: f64, (mx, my): (f32, f32)) {
        self.screen = (mq::screen_width(), mq::screen_height());
        if !self.open {
            self.spin = None;
            return;
        }
        self.hover = match self.hit(mx, my) {
            Some(Hit::Card(c)) => Some(c),
            _ => None,
        };
        let l = self.layout();
        let mut made = 0;
        let mut want = Vec::new();
        for (k, c) in self.cards().into_iter().enumerate() {
            if self.card_rect(&l, k).is_none() {
                continue;
            }
            match c {
                Card::Model(key) => want.push(key),
                Card::Folder(f) => want.extend(self.preview(&f)),
                Card::Back => {}
            }
        }
        for key in want {
            if self.thumbs.contains_key(&key) || made >= THUMBS_A_FRAME {
                continue;
            }
            if let Some(e) = self.entry(key) {
                let bytes = forge_thumbs::render(&e.prefab, THUMB, 0.75);
                let tex = mq::Texture2D::from_rgba8(THUMB as u16, THUMB as u16, &bytes);
                tex.set_filter(mq::FilterMode::Linear);
                self.thumbs.insert(key, tex);
                made += 1;
            }
        }
        // The hovered model turns slowly, so its back can be seen.
        let hovered = match (&self.hover, &self.drag) {
            (Some(Card::Model(k)), Drag::Idle) => Some(*k),
            _ => None,
        };
        match hovered {
            Some(k) => {
                let since = match &self.spin {
                    Some((s, t, _)) if *s == k && !t.is_nan() => *t,
                    _ => now,
                };
                if let Some(e) = self.entry(k) {
                    let yaw = 0.75 + ((now - since) as f32 * 1.1);
                    let bytes = forge_thumbs::render(&e.prefab, THUMB, yaw);
                    let tex = match self.spin.take() {
                        Some((_, _, t)) => {
                            t.update_from_bytes(THUMB as u32, THUMB as u32, &bytes);
                            t
                        }
                        None => {
                            let t = mq::Texture2D::from_rgba8(THUMB as u16, THUMB as u16, &bytes);
                            t.set_filter(mq::FilterMode::Linear);
                            t
                        }
                    };
                    self.spin = Some((k, since, tex));
                }
            }
            None => {
                // Keep the texture for the next hover, but stop showing it.
                if let Some(sp) = self.spin.as_mut() {
                    sp.1 = f64::NAN;
                }
            }
        }
    }

    /// The first few models in a folder, shown on its card.
    fn preview(&self, folder: &str) -> Vec<u64> {
        self.lib.under(folder).take(4).map(|e| e.key).collect()
    }

    fn picture(&self, key: u64) -> Option<&mq::Texture2D> {
        match &self.spin {
            Some((k, since, t)) if *k == key && !since.is_nan() => Some(t),
            _ => self.thumbs.get(&key),
        }
    }

    pub fn draw(&self, (mx, my): (f32, f32)) {
        if !self.open {
            let r = self.tab_rect();
            let hot = in_rect(mx, my, r);
            ui::plate(r.0, r.1, r.2, r.3, th::GLASS_STRONG, if hot { th::COBALT } else { th::STROKE });
            ui::text("LIBRARY", r.0 + 14.0, r.1 + 26.0, 17.0, th::WHITE);
            ui::text("L", r.0 + r.2 - 24.0, r.1 + 26.0, 15.0, th::QUIET);
            self.draw_carry(mx, my);
            return;
        }
        let l = self.layout();
        ui::plate(l.x, l.y, l.w, l.h, th::GLASS_STRONG, th::STROKE);
        ui::text("LIBRARY", l.x + PAD, l.y + 34.0, 22.0, th::WHITE);
        for (_, r, label) in self.buttons(&l) {
            let hot = in_rect(mx, my, r);
            ui::plate(
                r.0,
                r.1,
                r.2,
                r.3,
                if hot { th::PANEL_RAISED } else { ui::fade(th::PANEL, 0.8) },
                if hot { th::COBALT } else { th::STROKE },
            );
            ui::text_c(label, r.0 + r.2 / 2.0, r.1 + 19.0, 13.0, if hot { th::WHITE } else { th::MUTED });
        }
        // Where we are.
        let path = if self.folder.is_empty() { "ALL".to_string() } else { format!("ALL / {}", self.folder.replace('/', " / ")) };
        ui::text(&fit(&path, 15.0, l.w - PAD * 2.0 - 90.0), l.x + PAD, l.y + 62.0, 15.0, th::MUTED);
        let n = self.lib.count(&self.folder);
        let count = format!("{n} MODEL{}", if n == 1 { "" } else { "S" });
        ui::text(&count, l.x + l.w - PAD - ui::width(&count, 13.0), l.y + 62.0, 13.0, th::QUIET);

        let cards = self.cards();
        for (k, c) in cards.iter().enumerate() {
            let Some(r) = self.card_rect(&l, k) else { continue };
            let hot = self.hover.as_ref() == Some(c);
            let armed = matches!(c, Card::Model(key) if self.armed == Some(*key));
            let fill = if hot { th::PANEL_RAISED } else { ui::fade(th::PANEL, 0.85) };
            let stroke = if armed {
                th::EMBER
            } else if hot {
                th::COBALT
            } else {
                ui::fade(th::STROKE, 0.6)
            };
            mq::draw_rectangle(r.0, r.1, r.2, r.3, fill);
            mq::draw_rectangle_lines(r.0, r.1, r.2, r.3, if armed { 3.0 } else { 2.0 }, stroke);
            let (cx, cy) = (r.0, r.1);
            match c {
                Card::Back => {
                    ui::arrow(cx + CARD_W / 2.0, cy + 57.0, 30.0, 3.5, ui::Point::Left, if hot { th::WHITE } else { th::MUTED });
                    ui::text_c("BACK", cx + CARD_W / 2.0, cy + 130.0, 14.0, th::WHITE);
                }
                Card::Folder(f) => {
                    // A folder, holding a look at what is inside.
                    let (fx, fy, fw, fh) = (cx + 10.0, cy + 18.0, CARD_W - 20.0, 92.0);
                    let edge = if hot { th::COBALT } else { th::COBALT_DIM };
                    mq::draw_rectangle(fx, fy - 9.0, 34.0, 10.0, ui::fade(edge, 0.9));
                    mq::draw_rectangle(fx, fy, fw, fh, ui::fade(th::COBALT_DIM, if hot { 0.55 } else { 0.35 }));
                    mq::draw_rectangle_lines(fx, fy, fw, fh, 2.0, edge);
                    let cell = (fw - 12.0) / 2.0;
                    for (i, key) in self.preview(f).into_iter().enumerate() {
                        if let Some(t) = self.thumbs.get(&key) {
                            let (px, py) = (fx + 4.0 + (i % 2) as f32 * (cell + 4.0), fy + 2.0 + (i / 2) as f32 * (cell - 2.0));
                            mq::draw_texture_ex(
                                t,
                                px,
                                py,
                                mq::WHITE,
                                mq::DrawTextureParams { dest_size: Some(mq::vec2(cell, cell)), ..Default::default() },
                            );
                        }
                    }
                    let name = f.rsplit('/').next().unwrap_or(f);
                    ui::text(&fit(name, 14.0, CARD_W - 16.0), cx + 8.0, cy + 128.0, 14.0, th::WHITE);
                    let n = self.lib.count(f);
                    ui::text(&format!("{n} MODEL{}", if n == 1 { "" } else { "S" }), cx + 8.0, cy + 145.0, 12.0, th::QUIET);
                }
                Card::Model(key) => {
                    let Some(e) = self.entry(*key) else { continue };
                    match self.picture(*key) {
                        Some(t) => mq::draw_texture_ex(
                            t,
                            cx + (CARD_W - THUMB as f32) / 2.0,
                            cy + 6.0,
                            mq::WHITE,
                            mq::DrawTextureParams { dest_size: Some(mq::vec2(THUMB as f32, THUMB as f32)), ..Default::default() },
                        ),
                        None => ui::text_c("...", cx + CARD_W / 2.0, cy + 60.0, 16.0, th::QUIET),
                    }
                    ui::text(&fit(&e.prefab.name, 14.0, CARD_W - 16.0), cx + 8.0, cy + 128.0, 14.0, th::WHITE);
                    let s = e.size();
                    let what = match (e.prefab.blocks.len(), e.prefab.trees.len()) {
                        (0, t) => format!("{t} TREES"),
                        (1, 0) => "1 BLOCK".into(),
                        (b, _) => format!("{b} BLOCKS"),
                    };
                    let meta = format!("{:.0}x{:.0} M  {what}", s[0].max(1.0), s[1].max(1.0));
                    ui::text(&fit(&meta, 12.0, CARD_W - 16.0), cx + 8.0, cy + 145.0, 12.0, th::QUIET);
                    if e.file.is_some() {
                        // Yours, not built in.
                        mq::draw_circle(cx + CARD_W - 10.0, cy + 10.0, 3.5, th::SUCCESS);
                    }
                }
            }
        }
        // A scroll bar when the folder does not fit.
        let rows = cards.len().div_ceil(l.cols);
        if rows > l.rows {
            let (top, span) = (l.y + HEAD, l.rows as f32 * (CARD_H + GAP) - GAP);
            let bar = span * l.rows as f32 / rows as f32;
            let at = top + (span - bar) * self.scroll as f32 / (rows - l.rows) as f32;
            mq::draw_rectangle(l.x + l.w - 6.0, at, 3.0, bar, ui::fade(th::LINE, 0.7));
        }
        let hint = match self.armed.and_then(|k| self.entry(k)) {
            Some(e) => (format!("STAMPING {} - CLICK THE WORLD  R TURN  ESC STOP", e.prefab.name), th::EMBER),
            None => ("DRAG INTO THE WORLD  -  CLICK TO STAMP  -  R TURNS".to_string(), th::QUIET),
        };
        ui::text(&fit(&hint.0, 13.0, l.w - PAD * 2.0), l.x + PAD, l.y + l.h - 14.0, 13.0, hint.1);
        self.draw_carry(mx, my);
    }

    /// What is being carried: its picture over the drawer, a label over
    /// the world (where the editor draws its ghost).
    fn draw_carry(&self, mx: f32, my: f32) {
        let Some(key) = self.stamping() else { return };
        let Some(e) = self.entry(key) else { return };
        if self.over(mx, my) {
            if let (Drag::Carry(_), Some(t)) = (&self.drag, self.thumbs.get(&key)) {
                let s = THUMB as f32 * 0.8;
                mq::draw_texture_ex(
                    t,
                    mx - s / 2.0,
                    my - s / 2.0,
                    ui::fade(mq::WHITE, 0.85),
                    mq::DrawTextureParams { dest_size: Some(mq::vec2(s, s)), ..Default::default() },
                );
            }
            return;
        }
        let label = format!("{}  {} DEG", e.prefab.name, self.turn as u32 * 90);
        let w = ui::width(&label, 14.0) + 20.0;
        ui::plate(mx + 18.0, my + 14.0, w, 26.0, th::GLASS_STRONG, th::EMBER_DIM);
        ui::text(&label, mx + 28.0, my + 32.0, 14.0, th::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centre(s: &Shelf, k: usize) -> (f32, f32) {
        let r = s.card_rect(&s.layout(), k).expect("card on screen");
        (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0)
    }

    fn click(s: &mut Shelf, (x, y): (f32, f32)) {
        s.pointer(x, y, true, true);
        s.pointer(x, y, false, false);
    }

    #[test]
    fn folders_open_models_drag_out_and_clicks_stamp() {
        let mut s = Shelf::new();
        s.screen = (1280.0, 720.0);
        assert_eq!(s.cards()[2], Card::Folder("STRUCTURES".into()));
        // A click opens a folder.
        let c = centre(&s, 2);
        click(&mut s, c);
        assert_eq!(s.folder, "STRUCTURES");
        let tower = s.lib.find("watchtower").unwrap().key;
        let k = s.cards().iter().position(|c| *c == Card::Model(tower)).unwrap();
        // Press on a model and move: it is carried; let go on the world
        // and it is placed there.
        let (x, y) = centre(&s, k);
        s.pointer(x, y, true, true);
        assert_eq!(s.stamping(), None, "a press alone carries nothing");
        s.pointer(x - 40.0, y, false, true);
        assert_eq!(s.drag, Drag::Carry(tower));
        s.pointer(500.0, 400.0, false, true);
        assert_eq!(s.stamping(), Some(tower));
        assert_eq!(s.pointer(500.0, 400.0, false, false), Some(tower));
        assert_eq!(s.drag, Drag::Idle);
        // Carried back onto the drawer, it is dropped nowhere.
        s.pointer(x, y, true, true);
        s.pointer(x - 30.0, y + 20.0, false, true);
        assert_eq!(s.pointer(x - 30.0, y + 20.0, false, false), None);
        // A click arms a model for stamping, a second disarms it.
        click(&mut s, (x, y));
        assert_eq!(s.armed, Some(tower));
        assert_eq!(s.stamping(), Some(tower));
        click(&mut s, (x, y));
        assert_eq!(s.armed, None);
        // Esc stops stamping; Back goes up a level.
        s.armed = Some(tower);
        assert!(s.cancel() && s.armed.is_none() && !s.cancel());
        let c = centre(&s, 0);
        click(&mut s, c);
        assert_eq!(s.folder, "");
        // Hidden, only its tab is in the way, and a click opens it.
        s.open = false;
        assert!(!s.over(900.0, 300.0));
        let t = s.tab_rect();
        click(&mut s, (t.0 + 5.0, t.1 + 5.0));
        assert!(s.open);
    }
}
