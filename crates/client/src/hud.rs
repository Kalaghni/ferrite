//! 2D overlay, drawn with macroquad on top of the 3D frame.

use crate::scene::{team_col, NEUTRAL_COL};
use crate::App;
use ferrite_shared::glam::{Mat4, Vec3, Vec4};
use ferrite_shared::map::{BlockKind, HALF_X, HALF_Z};
use ferrite_shared::mode;
use ferrite_shared::proto::{flag, pickup, SPECTATOR_ID};
use ferrite_shared::sim;
use ferrite_shared::{eligible, NEUTRAL, POINT_NAMES, POINT_RADIUS, TEAM_NAMES};
use macroquad::prelude::{
    draw_circle, draw_circle_lines, draw_line, draw_poly, draw_rectangle, draw_rectangle_lines, draw_triangle, screen_height, screen_width,
    Color, Vec2,
};

/// Text goes through the UI kit's typeface.
struct Dim {
    width: f32,
}
fn measure_text(s: &str, _font: Option<()>, size: u16, _scale: f32) -> Dim {
    Dim { width: crate::ui::width(s, size as f32) }
}
fn draw_text(s: &str, x: f32, y: f32, size: f32, c: Color) {
    crate::ui::text(s, x, y, size, c);
}

fn col(c: [f32; 3], a: f32) -> Color {
    Color::new(c[0], c[1], c[2], a)
}

const WHITE: [f32; 3] = [0.965, 0.953, 0.98];
const DARK: [f32; 3] = [0.047, 0.063, 0.125];
const SUCCESS: [f32; 3] = [0.506, 0.878, 0.729];
const WARNING: [f32; 3] = [1.0, 0.824, 0.541];

fn text(s: &str, x: f32, y: f32, size: f32, c: Color) {
    draw_text(s, x + 1.5, y + 1.5, size, Color::new(0.0, 0.0, 0.0, 0.6 * c.a));
    draw_text(s, x, y, size, c);
}

fn text_c(s: &str, cx: f32, y: f32, size: f32, c: Color) {
    let w = measure_text(s, None, size as u16, 1.0).width;
    text(s, cx - w * 0.5, y, size, c);
}

fn text_r(s: &str, rx: f32, y: f32, size: f32, c: Color) {
    let w = measure_text(s, None, size as u16, 1.0).width;
    text(s, rx - w, y, size, c);
}

/// A player icon, `s` pixels square.
fn pic(t: &macroquad::prelude::Texture2D, x: f32, y: f32, s: f32, a: f32) {
    use macroquad::prelude::{draw_texture_ex, vec2, DrawTextureParams};
    draw_texture_ex(
        t,
        x.round(),
        y.round(),
        Color::new(1.0, 1.0, 1.0, a),
        DrawTextureParams { dest_size: Some(vec2(s, s)), ..Default::default() },
    );
}

/// Break `s` into lines: the first at most `first` pixels wide, the rest
/// `rest`. Words too long for a line are cut.
fn wrap(s: &str, size: f32, first: f32, rest: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let room = |n: usize| if n == 0 { first } else { rest };
    for word in s.split(' ') {
        let mut word = word.to_string();
        loop {
            let trial = if cur.is_empty() { word.clone() } else { format!("{cur} {word}") };
            if crate::ui::width(&trial, size) <= room(lines.len()) {
                cur = trial;
                break;
            }
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                continue;
            }
            // One word wider than the line: put down what fits.
            let mut cut = word.chars().count().max(1);
            while cut > 1 && crate::ui::width(&word.chars().take(cut).collect::<String>(), size) > room(lines.len()) {
                cut -= 1;
            }
            lines.push(word.chars().take(cut).collect());
            word = word.chars().skip(cut).collect();
            if word.is_empty() {
                break;
            }
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// The chat log and, while typing, the line being written. It sits bottom
/// left in a match, above the panel when one is up, beside the menu in the
/// pause overlay, and in the roster panel of the lobby.
fn draw_chat(app: &App, w: f32, h: f32, now: f64) {
    use crate::ui::color as th;
    let k = pause_scale(w, h);
    let pw = (w - 40.0).min(1080.0);
    let x0 = w / 2.0 - pw / 2.0;
    let dead = !app.alive && !app.spectator;
    let (x, bottom, cw, max_rows) = if app.lobby {
        (x0 + pw * 0.64, loadout_top(h) - 18.0, pw * 0.36 - 6.0, ((loadout_top(h) - 18.0 - 246.0) / 18.0).floor().max(2.0) as usize)
    } else if app.team_pick {
        return;
    } else if app.paused() {
        (500.0 * k, h - 34.0, (w - 500.0 * k - 480.0 * k).max(280.0), 10)
    } else if app.loadout_shown() || (dead && app.kc.is_none()) {
        (24.0, loadout_top(h) - 14.0, 460.0, 8)
    } else if app.kc.is_some() {
        (24.0, h - 130.0, 460.0, 6)
    } else {
        (24.0, h - 104.0, 460.0, 8)
    };
    let size = 16.0;
    let lh = 19.0;
    let mut y = bottom;
    if app.chat_open {
        let edge = if app.chat_team { team_col(app.my_team) } else { [0.412, 0.678, 1.0] };
        crate::ui::plate(x - 6.0, y - 19.0, cw + 12.0, 27.0, th::GLASS_STRONG, col(edge, 1.0));
        let tag = if app.chat_team { "TEAM" } else { "ALL" };
        draw_text(tag, x, y, size, col(edge, 1.0));
        let tx = x + crate::ui::width("TEAM", size) + 10.0;
        if app.chat_buf.is_empty() {
            draw_text("ENTER send    TAB all / team    ESC cancel", tx, y, 14.0, th::QUIET);
        }
        // A long line scrolls: the end of it stays in view.
        let mut shown = app.chat_buf.clone();
        while crate::ui::width(&shown, size) > x + cw - tx - 8.0 && !shown.is_empty() {
            shown.remove(0);
        }
        draw_text(&shown, tx, y, size, th::WHITE);
        if (now * 2.0) as i64 % 2 == 0 {
            draw_rectangle(tx + crate::ui::width(&shown, size) + 2.0, y - 14.0, 2.0, 17.0, col(edge, 1.0));
        }
        y -= 32.0;
    }
    // Newest at the bottom; a message fades after a while unless the box
    // is open or we are standing in the lobby.
    let mut rows = 0;
    for m in app.chat.iter().rev() {
        let age = (now - m.at) as f32;
        let a = if app.chat_open || app.lobby { 1.0 } else { (12.0 - age).clamp(0.0, 1.0) };
        if a <= 0.0 {
            break;
        }
        let tag = if m.all || m.system { "" } else { "(TEAM) " };
        let head = if m.system { String::new() } else { format!("{tag}{}: ", m.name) };
        let iw = if m.icon.is_some() { 18.0 } else { 0.0 };
        let hw = crate::ui::width(&head, size);
        let lines = wrap(&m.text, size, (cw - hw - iw).max(60.0), cw - 14.0);
        if rows + lines.len() > max_rows {
            break;
        }
        rows += lines.len();
        y -= lh * (lines.len() as f32 - 1.0);
        for (n, line) in lines.iter().enumerate() {
            let ly = y + n as f32 * lh;
            let (lx, lw) = if n == 0 {
                (x + iw + hw, iw + hw + crate::ui::width(line, size))
            } else {
                (x + 14.0, 14.0 + crate::ui::width(line, size))
            };
            draw_rectangle(x - 6.0, ly - 15.0, lw + 12.0, lh, col(DARK, 0.5 * a));
            if n == 0 && !m.system {
                if let Some(t) = &m.icon {
                    pic(t, x, ly - 13.0, 15.0, a);
                }
                text(&head, x + iw, ly, size, col(team_col(m.team), a));
            }
            let c = if m.system { th::MUTED } else { th::WHITE };
            text(line, lx, ly, size, Color::new(c.r, c.g, c.b, a * if m.system { 0.85 } else { 1.0 }));
        }
        y -= lh;
    }
}

fn project(vp: &Mat4, p: Vec3, w: f32, h: f32) -> Option<(f32, f32, f32)> {
    let c = *vp * Vec4::new(p.x, p.y, p.z, 1.0);
    if c.w < 0.2 {
        return None;
    }
    Some(((c.x / c.w * 0.5 + 0.5) * w, (1.0 - (c.y / c.w * 0.5 + 0.5)) * h, c.w))
}

/// Screen rectangles (x, y, w, h) of the two panels in the team picker.
pub fn team_pick_rects(w: f32, h: f32) -> [(f32, f32, f32, f32); 2] {
    let pw = (w * 0.3).clamp(220.0, 380.0);
    let ph = 220.0;
    let gap = 28.0;
    let y = h / 2.0 - ph / 2.0;
    [(w / 2.0 - gap / 2.0 - pw, y, pw, ph), (w / 2.0 + gap / 2.0, y, pw, ph)]
}

pub const LOADOUT_H: f32 = 302.0;

pub fn loadout_top(h: f32) -> f32 {
    (h * 0.56).min(h - LOADOUT_H - 8.0)
}

/// Clickable rows of the loadout panel: (column, row, x, y, w, h).
/// Columns are class, primary (for class `kit`) and sidearm; shared with
/// the click handler.
pub fn loadout_rects(w: f32, h: f32, kit: u8, prim: u8, side: u8) -> Vec<(u8, u8, f32, f32, f32, f32)> {
    let y0 = loadout_top(h) + 106.0;
    let rows = [3, sim::kit(kit).prims.len(), sim::SIDEARMS.len()];
    let mut v = Vec::new();
    for c in 0..3u8 {
        let (x, cw) = loadout_col(w, c);
        for r in 0..rows[c as usize] as u8 {
            v.push((c, r, x + 4.0, y0 + r as f32 * 38.0, cw - 8.0, 35.0));
        }
    }
    // Under the classes: the look worn. One row; a click moves to the next.
    let (x, cw) = loadout_col(w, 0);
    v.push((3, 0, x + 4.0, y0 + 4.0 * 38.0, cw - 8.0, 35.0));
    // The big models at the top: where a weapon has more than one finish,
    // a click moves to the next (columns 4 and 5).
    for (k, wid) in [prim, side].into_iter().enumerate() {
        if sim::weapon_skins_for(wid).len() > 1 {
            let (bx, bw) = big_gun_box(w, k);
            v.push((4 + k as u8, 0, bx, loadout_top(h) + 4.0, bw, 84.0));
        }
    }
    v
}

/// Left edge and width of the big model of the primary (0) or sidearm (1).
fn big_gun_box(w: f32, k: usize) -> (f32, f32) {
    let pw = (w - 40.0).min(1080.0);
    let bw = (pw * 0.27).min(290.0);
    (if k == 0 { w / 2.0 - pw / 2.0 + 8.0 } else { w / 2.0 + pw / 2.0 - 8.0 - bw }, bw)
}

/// Left edge and width of a loadout column. The weapon columns are wider:
/// each row carries a model of the gun.
fn loadout_col(w: f32, c: u8) -> (f32, f32) {
    let pw = (w - 40.0).min(1080.0);
    let x0 = w / 2.0 - pw / 2.0;
    match c {
        0 => (x0, pw * 0.26),
        1 => (x0 + pw * 0.26, pw * 0.37),
        _ => (x0 + pw * 0.63, pw * 0.37),
    }
}

/// A weapon model to draw over the HUD: fitted into a `w` x `h` box
/// centred on (`cx`, `cy`), in screen pixels.
pub struct GunIcon {
    pub wid: u8,
    /// Finish: index into `sim::WEAPON_SKINS`.
    pub skin: u8,
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
    /// The big ones turn slowly; the row icons stay put.
    pub turn: bool,
}

/// Width of the model at the left of a weapon row.
const ICON_W: f32 = 92.0;

fn key_hint(app: &App, action: crate::bindings::Action) -> String {
    app.bindings.inputs(action).first().map(|input| input.label()).unwrap_or_else(|| "UNBOUND".into())
}

/// A piece of a hint line: words, a key drawn as a keycap, or a wider gap
/// between one hint and the next.
enum Hint<'a> {
    Words(&'a str),
    Key(&'a str),
    Gap,
}

/// A line of hints on baseline `y` from `x`, keys as keycaps so they stand
/// out from busy art; returns its width. Nothing is drawn unless `draw`.
fn hints(parts: &[Hint], x: f32, y: f32, size: f32, c: Color, draw: bool) -> f32 {
    let mut cx = x;
    for (i, part) in parts.iter().enumerate() {
        if i > 0 && !matches!(part, Hint::Gap) {
            cx += size * 0.4;
        }
        match part {
            Hint::Words(t) => {
                if draw {
                    text(t, cx, y, size, c);
                }
                cx += crate::ui::width(t, size);
            }
            Hint::Key(k) => {
                let ks = if k.chars().count() > 3 { size * 0.72 } else { size * 0.9 };
                let kw = (crate::ui::width(k, ks) + size * 0.7).max(size * 1.45);
                if draw {
                    crate::ui::plate(cx, y - size * 1.02, kw, size * 1.38, col(DARK, 0.85), Color::new(0.706, 0.761, 0.882, 0.6));
                    crate::ui::text_c(k, cx + kw / 2.0, y - (size - ks) * 0.35, ks, col(WHITE, 1.0));
                }
                cx += kw;
            }
            Hint::Gap => cx += size * 1.2,
        }
    }
    cx - x
}

/// `hints`, ending at `right`.
fn hints_r(parts: &[Hint], right: f32, y: f32, size: f32, c: Color) {
    let w = hints(parts, 0.0, y, size, c, false);
    hints(parts, right - w, y, size, c, true);
}

fn draw_loadout(app: &App, w: f32, h: f32, icons: &mut Vec<GunIcon>) {
    let (mx, my) = crate::controller::mouse_position();
    let top = loadout_top(h);
    let kd = sim::kit(app.kit_choice);
    let tc = team_col(app.my_team);
    use crate::bindings::Action;
    let classes = [Action::ClassVanguard, Action::ClassBreacher, Action::ClassMarksman].map(|a| key_hint(app, a));
    let (np, ns) = (key_hint(app, Action::NextPrimary), key_hint(app, Action::NextSidearm));
    let titles = [
        vec![Hint::Words("CLASS"), Hint::Key(&classes[0]), Hint::Key(&classes[1]), Hint::Key(&classes[2])],
        vec![Hint::Words("PRIMARY"), Hint::Key(&np), Hint::Words("next")],
        vec![Hint::Words("SIDEARM"), Hint::Key(&ns), Hint::Words("next")],
    ];
    for (c, title) in titles.iter().enumerate() {
        hints(title, loadout_col(w, c as u8).0 + 14.0, top + 98.0, 16.0, col(WHITE, 0.88), true);
    }
    // The chosen primary and sidearm, large, either side of the heading.
    // Pointing at a row shows that weapon instead.
    let (_, prim, side) = app.loadout();
    let mut big = [prim, side];
    for (c, r, x, y, rw, rh) in loadout_rects(w, h, app.kit_choice, prim, side) {
        if c >= 4 {
            continue; // the big models, drawn below
        }
        let (name, blurb, sel, stats) = match c {
            0 => {
                let k = sim::kit(r);
                (k.name, k.blurb, app.kit_choice == r, String::new())
            }
            3 => {
                let n = sim::SKINS.len();
                let s = sim::SKINS[app.skin_choice as usize % n];
                let nl = key_hint(app, Action::NextLook);
                hints(&[Hint::Words("LOOK"), Hint::Key(&nl), Hint::Words("next")], x + 10.0, y - 8.0, 16.0, col(WHITE, 0.88), true);
                (s.0, s.1, true, format!("{} / {}", app.skin_choice as usize % n + 1, n))
            }
            _ => {
                let id = if c == 1 { kd.prims[r as usize] } else { sim::SIDEARMS[r as usize] };
                let wd = sim::weapon_def(id);
                let sel = if c == 1 { app.prims[app.kit_choice as usize % 3] == id } else { app.side_choice == id };
                let dmg = if wd.pellets > 1 { format!("{}x{:.0}", wd.pellets, wd.damage) } else { format!("{:.0}", wd.damage) };
                let rate = if wd.burst > 0 { format!("{}-burst", wd.burst) } else { format!("{} rpm", 3600 / wd.interval.max(1) as u32) };
                (wd.name, wd.blurb, sel, format!("{} dmg  {}  {} rds", dmg, rate, wd.mag))
            }
        };
        let over = mx >= x && mx < x + rw && my >= y && my < y + rh;
        draw_rectangle(x, y, rw, rh, if sel { col(tc, 0.32) } else { col(WHITE, if over { 0.14 } else { 0.05 }) });
        if sel {
            draw_rectangle(x, y, 3.0, rh, col(tc, 1.0));
        }
        let mut tx = x + 12.0;
        if c == 1 || c == 2 {
            let id = if c == 1 { kd.prims[r as usize] } else { sim::SIDEARMS[r as usize] };
            draw_rectangle(x + 5.0, y + 2.0, ICON_W, rh - 4.0, col(DARK, 0.45));
            icons.push(GunIcon {
                wid: id,
                skin: app.wskins[id as usize % 20],
                cx: x + 5.0 + ICON_W / 2.0,
                cy: y + rh / 2.0,
                w: ICON_W - 10.0,
                h: rh - 10.0,
                turn: false,
            });
            tx = x + ICON_W + 12.0;
            if over {
                big[c as usize - 1] = id;
            }
        }
        text(&name.to_uppercase(), tx, y + 16.0, 18.0, col(WHITE, if sel { 1.0 } else { 0.8 }));
        text_r(&stats, x + rw - 8.0, y + 15.0, 14.0, col(WHITE, if sel { 0.9 } else { 0.55 }));
        text(blurb, tx, y + 30.0, 14.0, col(WHITE, if sel { 0.85 } else { 0.55 }));
    }
    for (k, id) in big.into_iter().enumerate() {
        let (bx, bw) = big_gun_box(w, k);
        text(&sim::weapon_def(id).name.to_uppercase(), bx + 4.0, top + 20.0, 16.0, col(tc, 0.95));
        // Its finish, where it has a choice of them.
        let skin = sim::fix_weapon_skin(id, app.wskins[id as usize % 20]);
        if sim::weapon_skins_for(id).len() > 1 {
            let over = mx >= bx && mx < bx + bw && my >= top + 4.0 && my < top + 88.0;
            let finish = format!("{} FINISH", sim::weapon_skin(skin).name);
            let key = key_hint(app, if k == 0 { Action::PrimaryFinish } else { Action::SidearmFinish });
            hints_r(
                &[Hint::Words(&finish), Hint::Key(&key), Hint::Words("or click")],
                bx + bw - 4.0,
                top + 20.0,
                14.0,
                col(WHITE, if over { 1.0 } else { 0.85 }),
            );
        }
        icons.push(GunIcon { wid: id, skin, cx: bx + bw / 2.0, cy: top + 54.0, w: bw - 30.0, h: 64.0, turn: true });
    }
}

/// Lobby click targets: one card per map, the READY button, then one
/// chip per game mode.
#[allow(clippy::type_complexity)]
pub fn lobby_rects(w: f32, h: f32) -> (Vec<(f32, f32, f32, f32)>, (f32, f32, f32, f32), Vec<(f32, f32, f32, f32)>) {
    let head = LobbyHead::new(w, h);
    let (x0, pw) = (head.x0, head.pw);
    let n = ferrite_shared::map::MAPS.len();
    let gap = 12.0;
    let cw = (pw - gap * (n as f32 - 1.0)) / n as f32;
    let cards = (0..n).map(|k| (x0 + k as f32 * (cw + gap), head.cards, cw, 62.0)).collect();
    let m = ferrite_shared::MODES.len();
    let mw = (pw - gap * (m as f32 - 1.0)) / m as f32;
    let modes = (0..m).map(|k| (x0 + k as f32 * (mw + gap), head.cards + 72.0, mw, 26.0)).collect();
    (cards, (x0 + pw - 240.0, head.top + 2.0, 240.0, 48.0), modes)
}

/// The lobby's header. The wordmark grows with the screen (it is placed in
/// key-art pixels), so the briefing beside it and everything below are
/// laid out from where it actually ends, not at fixed heights.
struct LobbyHead {
    /// The panel's left edge and width.
    x0: f32,
    pw: f32,
    /// Left edge of the briefing beside the wordmark, and its top.
    hx: f32,
    top: f32,
    /// Bottom of the header band, and the top of the map cards.
    bottom: f32,
    cards: f32,
}

impl LobbyHead {
    /// From the top of the briefing to the baseline of its last line.
    const TEXT_H: f32 = 98.0;

    fn new(w: f32, h: f32) -> Self {
        let pw = (w - 40.0).min(1080.0);
        let x0 = w / 2.0 - pw / 2.0;
        let k = crate::ui::art_scale(w, h);
        let (lx, ly, lw) = crate::ui::LOBBY_LOGO;
        let logo_right = (lx + lw) * k;
        let logo_bottom = (ly + lw * crate::ui::WORDMARK_ASPECT) * k;
        let top = ly * k;
        let bottom = logo_bottom.max(top + Self::TEXT_H) + 14.0;
        LobbyHead { x0, pw, hx: (logo_right + 30.0).max(x0), top, bottom, cards: bottom + 16.0 }
    }

    /// Top of the roster panel, under the mode chips.
    fn roster(&self) -> f32 {
        self.cards + 72.0 + 26.0 + 16.0
    }
}

/// "4:05" from a number of seconds.
fn clock(secs: u16) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Whether objective `k` is in play in game mode `m`.
fn point_used(m: u8, k: usize) -> bool {
    match m {
        mode::CONQUEST => true,
        mode::KOTH => k == 2,
        _ => false,
    }
}

/// What the two scores are called in game mode `m`.
fn score_word(m: u8) -> &'static str {
    match m {
        mode::CONQUEST => "tickets",
        mode::TDM => "kills",
        mode::CTF => "captures",
        _ => "points",
    }
}

fn draw_lobby(app: &App, snap: &ferrite_shared::proto::Snapshot, w: f32, h: f32) {
    // Staging-area key art under a scrim, the wordmark top left.
    draw_rectangle(0.0, 0.0, w, h, col(DARK, 1.0));
    crate::ui::backdrop(&app.lobby_bg, w, h);
    draw_rectangle(0.0, 0.0, w, h, col(DARK, 0.5));
    let head = LobbyHead::new(w, h);
    // A band under the header so its small print reads over any art.
    draw_rectangle(0.0, 0.0, w, head.bottom, col(DARK, 0.55));
    let (lx, ly, lw) = crate::ui::LOBBY_LOGO;
    crate::ui::logo(&app.art, w, h, lx, ly, lw);
    let pw = (w - 40.0).min(1080.0);
    let x0 = w / 2.0 - pw / 2.0;
    let tc = team_col(app.my_team);
    let (mx, my) = crate::controller::mouse_position();
    let (hx, ty) = (head.hx, head.top);
    let soon = snap.phase_secs > 0.05 && snap.phase_secs <= 3.0;
    let status = if snap.phase_secs > 0.05 {
        format!("MATCH STARTS IN 00:{:02.0}", snap.phase_secs.ceil())
    } else {
        "WAITING FOR PLAYERS".to_string()
    };
    text("PRE-MATCH BRIEFING", hx, ty + 12.0, 15.0, Color::new(0.718, 0.863, 1.0, 1.0));
    text(&status, hx, ty + 43.0, 30.0, col(if soon { WARNING } else { WHITE }, 1.0));
    text(
        "Vote for the map and the game mode, and choose a loadout. It starts early once everyone is ready.",
        hx,
        ty + 68.0,
        17.0,
        col(WHITE, 0.92),
    );
    let (team, chat) = (key_hint(app, crate::bindings::Action::ChangeTeam), key_hint(app, crate::bindings::Action::ChatAll));
    hints(
        &[
            Hint::Key(&team),
            Hint::Words("team"),
            Hint::Gap,
            Hint::Key("R"),
            Hint::Words("ready"),
            Hint::Gap,
            Hint::Key(&chat),
            Hint::Words("chat"),
            Hint::Gap,
            Hint::Key("Q"),
            Hint::Words("leave"),
        ],
        hx,
        ty + LobbyHead::TEXT_H,
        15.0,
        col(WHITE, 0.92),
        true,
    );
    if let Some(t) = &app.now_playing {
        text_r(&format!("now playing: {t}"), x0 + pw, ty + LobbyHead::TEXT_H, 15.0, col(WHITE, 0.7));
    }
    let (cards, rb, modes) = lobby_rects(w, h);
    if !app.spectator {
        text_r(&format!("{} CREDITS", app.profile.credits), rb.0 - 18.0, rb.1 + 31.0, 20.0, col(WARNING, 0.95));
    }
    for (k, &(x, y, cw, ch)) in modes.iter().enumerate() {
        let def = &ferrite_shared::MODES[k];
        let mine = app.mode_vote == k as u8;
        let next = snap.next_mode == k as u8;
        let over = mx >= x && mx < x + cw && my >= y && my < y + ch;
        draw_rectangle(x, y, cw, ch, col(DARK, 0.82));
        if mine {
            draw_rectangle(x, y, cw, ch, col(tc, 0.3));
        }
        draw_rectangle_lines(
            x,
            y,
            cw,
            ch,
            if next { 3.0 } else { 2.0 },
            if next {
                col(WHITE, 0.95)
            } else if over {
                Color::new(0.412, 0.678, 1.0, 1.0)
            } else {
                Color::new(0.706, 0.761, 0.882, 0.38)
            },
        );
        let v = snap.mode_votes[k % 4];
        let label = if cw < 200.0 { def.name.to_string() } else { format!("{}  -  {}", def.name, def.pace) };
        let size = (15.0 * (cw - 70.0) / crate::ui::width(&label, 15.0).max(1.0)).min(15.0);
        text(&label, x + 10.0, y + 17.0, size, col(WHITE, if next { 1.0 } else { 0.8 }));
        let tag = match (next, v) {
            (true, 0) => "NEXT".to_string(),
            (true, v) => format!("NEXT  {v}"),
            (false, 0) => String::new(),
            (false, v) => v.to_string(),
        };
        text_r(&tag, x + cw - 8.0, y + 17.0, 13.0, col(WHITE, 0.9));
    }
    for (k, &(x, y, cw, ch)) in cards.iter().enumerate() {
        let def = &ferrite_shared::map::MAPS[k];
        let mine = app.vote == k as u8;
        let next = snap.next_map == k as u8;
        let over = mx >= x && mx < x + cw && my >= y && my < y + ch;
        draw_rectangle(x, y, cw, ch, col(DARK, 0.82));
        if mine {
            draw_rectangle(x, y, cw, ch, col(tc, 0.3));
        }
        draw_rectangle_lines(
            x,
            y,
            cw,
            ch,
            if next { 3.0 } else { 2.0 },
            if next {
                col(WHITE, 0.95)
            } else if over {
                Color::new(0.412, 0.678, 1.0, 1.0)
            } else {
                Color::new(0.706, 0.761, 0.882, 0.38)
            },
        );
        draw_rectangle(x + 12.0, y + ch - 6.0, cw - 24.0, 2.0, col(if mine { tc } else { [0.412, 0.678, 1.0] }, 0.8));
        // Keep all map names inside their cards as the catalogue grows.
        let name_size = (20.0 * (cw - 24.0) / crate::ui::width(def.name, 20.0).max(1.0)).min(20.0);
        text(def.name, x + 12.0, y + 22.0, name_size, col(WHITE, 1.0));
        let v = snap.votes[k % 8];
        text(&format!("{} vote{}", v, if v == 1 { "" } else { "s" }), x + 12.0, y + 38.0, 14.0, col(WHITE, 0.75));
        let tag = match (next, mine) {
            (true, true) if cw < 230.0 => "VOTED / NEXT",
            (true, false) if cw < 230.0 => "NEXT MAP",
            (true, true) => "YOUR VOTE - WILL BE PLAYED",
            (true, false) => "WILL BE PLAYED",
            (false, true) => "YOUR VOTE",
            _ => "",
        };
        let tag_size = (14.0 * (cw - 24.0) / crate::ui::width(tag, 14.0).max(1.0)).min(14.0);
        text(tag, x + 12.0, y + 52.0, tag_size.min(13.0), col(WHITE, 0.9));
    }
    if !app.spectator {
        let over = mx >= rb.0 && mx < rb.0 + rb.2 && my >= rb.1 && my < rb.1 + rb.3;
        // Ember until you are ready, then green: the kit's call to action.
        let c = if app.ready { col(SUCCESS, 1.0) } else { Color::new(1.0, 0.459, 0.298, if over { 1.0 } else { 0.88 }) };
        draw_rectangle(rb.0, rb.1, rb.2, rb.3, c);
        crate::ui::text_c(if app.ready { "READY" } else { "READY UP" }, rb.0 + rb.2 / 2.0, rb.1 + 32.0, 24.0, col(DARK, 1.0));
    }
    // Who is here.
    let y0 = head.roster() + 24.0;
    draw_rectangle(x0 - 12.0, y0 - 24.0, pw + 24.0, loadout_top(h) - (y0 - 24.0) - 8.0, col(DARK, 0.72));
    draw_rectangle(x0 - 12.0, loadout_top(h), pw + 24.0, LOADOUT_H, col(DARK, 0.82));
    // The teams take the left of the panel, the chat the right. People
    // come first, bright, with their icons; bots are dim; ready is green.
    for t in 0..2u8 {
        let tx = x0 + t as f32 * pw * 0.31;
        text(TEAM_NAMES[t as usize], tx, y0, 22.0, col(team_col(t), 1.0));
        let mut team: Vec<_> = app.net.roster.iter().filter(|r| r.team == t).collect();
        team.sort_by_key(|r| r.bot);
        for (k, r) in team.iter().enumerate() {
            let (mut nx, ny) = (tx + (k % 2) as f32 * (pw * 0.155), y0 + 24.0 + (k / 2) as f32 * 18.0);
            if ny > loadout_top(h) - 14.0 {
                break;
            }
            let a = if r.id == app.my_id {
                1.0
            } else if r.bot {
                0.4
            } else {
                0.9
            };
            if let Some(tex) = app.icon(r.id) {
                pic(tex, nx, ny - 13.0, 15.0, 1.0);
                nx += 19.0;
            }
            let ready = r.ready && !r.bot;
            text(&r.name, nx, ny, 16.0, col(if ready { SUCCESS } else { WHITE }, a));
            if ready {
                text("READY", nx + crate::ui::width(&r.name, 16.0) + 8.0, ny, 12.0, col(SUCCESS, 0.8));
            }
        }
    }
    let chat_x = x0 + pw * 0.64;
    draw_line(chat_x - 14.0, y0 - 14.0, chat_x - 14.0, loadout_top(h) - 18.0, 1.0, Color::new(0.706, 0.761, 0.882, 0.38));
    text("CHAT", chat_x, y0 - 4.0, 15.0, col(WHITE, 0.75));
    if !app.chat_open && !app.spectator {
        let key = key_hint(app, crate::bindings::Action::ChatAll);
        hints_r(&[Hint::Key(&key), Hint::Words("to type")], x0 + pw, y0 - 4.0, 14.0, col(WHITE, 0.8));
    }
}

fn draw_team_pick(app: &App, w: f32, h: f32) {
    draw_rectangle(0.0, 0.0, w, h, col(DARK, 0.8));
    let rects = team_pick_rects(w, h);
    text_c("CHOOSE YOUR TEAM", w / 2.0, rects[0].1 - 44.0, 44.0, col(WHITE, 1.0));
    let (mx, my) = crate::controller::mouse_position();
    for (t, &(x, y, pw, ph)) in rects.iter().enumerate() {
        let c = team_col(t as u8);
        let over = mx >= x && mx < x + pw && my >= y && my < y + ph;
        let mine = app.my_team as usize == t;
        draw_rectangle(x, y, pw, ph, col([c[0] * 0.3, c[1] * 0.3, c[2] * 0.3], 0.95));
        draw_rectangle(x, y, pw, 8.0, col(c, 1.0));
        if over && !mine {
            draw_rectangle_lines(x, y, pw, ph, 4.0, col(WHITE, 1.0));
        }
        let total = app.net.roster.iter().filter(|r| r.team as usize == t).count();
        let humans = app.net.roster.iter().filter(|r| r.team as usize == t && !r.bot).count();
        let cx = x + pw / 2.0;
        text_c(TEAM_NAMES[t], cx, y + 70.0, 52.0, col(c, 1.0));
        text_c(&format!("{} fighters  -  {} human", total, humans), cx, y + 108.0, 20.0, col(WHITE, 0.85));
        if let Some(s) = app.net.latest() {
            let held = s.points.iter().filter(|p| p.owner as usize == t).count();
            let line = if s.mode == mode::CONQUEST {
                format!("{} tickets  -  holds {} of 5", s.tickets[t], held)
            } else {
                format!("{} {}", s.tickets[t], score_word(s.mode))
            };
            text_c(&line, cx, y + 136.0, 20.0, col(WHITE, 0.85));
        }
        let (label, a) = if mine {
            ("YOUR TEAM", 0.7)
        } else if total > humans {
            ("CLICK TO JOIN", 1.0)
        } else {
            ("FULL", 0.5)
        };
        text_c(label, cx, y + ph - 28.0, 24.0, col(WHITE, a));
    }
    text_c(
        &format!("Switching sides redeploys you.     {} or ESC to cancel", key_hint(app, crate::bindings::Action::ChangeTeam)),
        w / 2.0,
        rects[0].1 + rects[0].3 + 40.0,
        20.0,
        col(WHITE, 0.75),
    );
}

/// Size of the pause overlay relative to its 1672 x 941 design.
fn pause_scale(w: f32, h: f32) -> f32 {
    (w / 1672.0).min(h / 941.0).max(0.62)
}

/// Click targets of the pause overlay: (id, x, y, w, h). The menu is
/// 0 resume, 1 loadout, 2 settings, 3 change team, 4 leave. Its settings
/// page is four rows with a minus (10, 12, 14, 16) and a plus (11, 13, 15,
/// 17) each, then 18 back. Shared with the click handler.
pub fn pause_rects(w: f32, h: f32, settings: bool) -> Vec<(u8, f32, f32, f32, f32)> {
    let k = pause_scale(w, h);
    let (x, bw, bh, step) = (62.0 * k, 400.0 * k, 74.0 * k, 88.0 * k);
    let y0 = 290.0 * k;
    let mut v = Vec::new();
    if settings {
        for r in 0..4u8 {
            let y = y0 + r as f32 * step + 12.0 * k;
            v.push((10 + r * 2, x + bw - 124.0 * k, y, 50.0 * k, 50.0 * k));
            v.push((11 + r * 2, x + bw - 62.0 * k, y, 50.0 * k, 50.0 * k));
        }
        v.push((18, x, y0 + 4.0 * step, bw, bh));
    } else {
        for b in 0..5u8 {
            v.push((b, x, y0 + b as f32 * step, bw, bh));
        }
    }
    v
}

/// The pause overlay: match control on the left, the state of the match
/// and the controls on the right, over the live game.
fn draw_pause(app: &App, snap: &ferrite_shared::proto::Snapshot, w: f32, h: f32) {
    use crate::ui::color as th;
    use macroquad::prelude::{draw_texture_ex, vec2, DrawTextureParams};
    let k = pause_scale(w, h);
    let sz = |n: f32| (n * k).max(13.0);
    let (mx, my) = crate::controller::mouse_position();
    let over = |x: f32, y: f32, rw: f32, rh: f32| mx >= x && mx < x + rw && my >= y && my < y + rh;
    draw_rectangle(0.0, 0.0, w, h, col(DARK, 0.74));
    let pic = |t: &macroquad::prelude::Texture2D, x: f32, y: f32, pw: f32, ph: f32| {
        draw_texture_ex(t, x, y, macroquad::prelude::WHITE, DrawTextureParams { dest_size: Some(vec2(pw, ph)), ..Default::default() });
    };

    // ---- left: match control ----
    let x = 62.0 * k;
    pic(&app.art.wordmark, 52.0 * k, 30.0 * k, 330.0 * k, 330.0 * k * crate::ui::WORDMARK_ASPECT);
    draw_text("MATCH CONTROL", x + 2.0, 150.0 * k, sz(20.0), th::EMBER_BRIGHT);
    let title = if app.pause_settings {
        "SETTINGS"
    } else if app.deployed_once {
        "PAUSED"
    } else {
        "STAND BY"
    };
    draw_text(title, x, 234.0 * k, 96.0 * k, th::WHITE);
    draw_line(x, 256.0 * k, x + 420.0 * k, 256.0 * k, 1.0, th::STROKE);
    let rects = pause_rects(w, h, app.pause_settings);
    if app.pause_settings {
        let rows = [
            ("MOUSE SENSITIVITY", format!("{:.2}", app.sens * 1000.0)),
            ("EFFECTS VOLUME", format!("{:.0}%", app.audio.volume * 100.0)),
            ("MUSIC VOLUME", format!("{:.0}%", app.music_vol * 100.0)),
            ("MUSIC IN A MATCH", if app.music_in_match { "ON".to_string() } else { "OFF".to_string() }),
        ];
        for (r, (label, value)) in rows.iter().enumerate() {
            let y = (290.0 + r as f32 * 88.0) * k;
            crate::ui::plate(x, y, 400.0 * k, 74.0 * k, th::GLASS_STRONG, th::STROKE);
            draw_text(label, x + 20.0 * k, y + 28.0 * k, sz(18.0), th::MUTED);
            draw_text(value, x + 20.0 * k, y + 60.0 * k, sz(30.0), th::WHITE);
        }
    }
    for &(id, bx, by, bw, bh) in &rects {
        let hot = over(bx, by, bw, bh);
        if (10..18).contains(&id) {
            crate::ui::plate(bx, by, bw, bh, th::GLASS_STRONG, if hot { th::COBALT } else { th::STROKE });
            crate::ui::text_c(if id % 2 == 0 { "-" } else { "+" }, bx + bw / 2.0, by + bh * 0.68, sz(30.0), th::WHITE);
            continue;
        }
        let label = match id {
            0 if app.deployed_once => "RESUME",
            0 => "DEPLOY",
            1 => "LOADOUT",
            2 => "SETTINGS",
            3 => "CHANGE TEAM",
            4 => "LEAVE MATCH",
            _ => "BACK",
        };
        let ink = if id == 0 {
            let c = if hot { th::EMBER_BRIGHT } else { th::EMBER };
            draw_rectangle(bx, by, bw, bh, c);
            th::INK
        } else {
            crate::ui::plate(bx, by, bw, bh, th::GLASS_STRONG, if hot { th::COBALT } else { th::STROKE });
            th::WHITE
        };
        draw_text(label, bx + 28.0 * k, by + bh * 0.5 + 10.0 * k, sz(32.0), ink);
        draw_text(">", bx + bw - 40.0 * k, by + bh * 0.5 + 10.0 * k, sz(32.0), ink);
    }
    if let Some(hs) = &app.hosting {
        draw_text(&format!("{hs}  -  others on your network can join from JOIN SERVER"), x, h - 30.0 * k, sz(17.0), th::MUTED);
    }
    if let Some(t) = &app.now_playing {
        let line = format!("NOW PLAYING   {t}");
        let lw = crate::ui::width(&line, sz(17.0));
        crate::ui::plate(x, h - 100.0 * k, lw + 36.0 * k, 40.0 * k, th::GLASS_STRONG, th::STROKE);
        draw_text(&line, x + 18.0 * k, h - 73.0 * k, sz(17.0), th::WHITE);
    }

    // ---- right: match status ----
    let pw = 408.0 * k;
    let px = w - 46.0 * k - pw;
    let py = 62.0 * k;
    if px < x + 430.0 * k {
        return; // a very narrow window: the menu alone
    }
    crate::ui::plate(px, py, pw, 566.0 * k, th::GLASS_STRONG, th::STROKE);
    let ix = px + 22.0 * k;
    let iw = pw - 44.0 * k;
    draw_text("MATCH STATUS", ix, py + 32.0 * k, sz(17.0), th::MUTED);
    draw_text(&ferrite_shared::map::map_def(snap.map).name.to_uppercase(), ix, py + 70.0 * k, sz(36.0), th::WHITE);
    let mdef = ferrite_shared::mode_def(snap.mode);
    let mline =
        if snap.mode == mode::CONQUEST { "FRONT-LINE CONQUEST".to_string() } else { format!("{}  -  FIRST TO {}", mdef.name, snap.goal) };
    draw_text(&mline, ix, py + 96.0 * k, sz(17.0), th::MUTED);
    let cw = (iw - 12.0 * k) / 2.0;
    for t in 0..2usize {
        let cx = ix + t as f32 * (cw + 12.0 * k);
        let cy = py + 122.0 * k;
        let tc = team_col(t as u8);
        let mine = app.my_team as usize == t;
        crate::ui::plate(
            cx,
            cy,
            cw,
            72.0 * k,
            col([tc[0] * 0.22, tc[1] * 0.22, tc[2] * 0.22], 0.95),
            if mine { col(tc, 1.0) } else { col(tc, 0.4) },
        );
        pic(&app.art.emblems[t], cx + 10.0 * k, cy + 12.0 * k, 48.0 * k, 48.0 * k);
        draw_text(TEAM_NAMES[t], cx + 70.0 * k, cy + 28.0 * k, sz(17.0), col(tc, 1.0));
        draw_text(&snap.tickets[t].to_string(), cx + 70.0 * k, cy + 60.0 * k, sz(36.0), th::WHITE);
    }
    let top_t = snap.tickets[0].max(snap.tickets[1]).max(snap.goal.max(1)) as f32;
    for t in 0..2usize {
        let bx = ix + t as f32 * (cw + 12.0 * k);
        let by = py + 208.0 * k;
        draw_rectangle(bx, by, cw, 10.0 * k, col(WHITE, 0.12));
        draw_rectangle(bx, by, cw * (snap.tickets[t] as f32 / top_t).min(1.0), 10.0 * k, col(team_col(t as u8), 1.0));
    }
    draw_line(ix, py + 236.0 * k, ix + iw, py + 236.0 * k, 1.0, th::STROKE);
    draw_text(if snap.mode == mode::CONQUEST { "THE FRONT LINE" } else { "THE OBJECTIVE" }, ix, py + 264.0 * k, sz(17.0), th::MUTED);
    if snap.mode != mode::CONQUEST {
        for (n, line) in wrap(mdef.blurb, sz(16.0), iw, iw).iter().take(3).enumerate() {
            draw_text(line, ix, py + (292.0 + n as f32 * 20.0) * k, sz(16.0), th::WHITE);
        }
    }
    for (p, pt) in snap.points.iter().enumerate().take(if snap.mode == mode::CONQUEST { 5 } else { 0 }) {
        let dx = ix + iw * (p as f32 + 0.5) / 5.0;
        let dy = py + 304.0 * k;
        if p < 4 {
            draw_line(dx + 24.0 * k, dy, dx + iw / 5.0 - 24.0 * k, dy, 2.0, th::STROKE);
        }
        let c = team_col(pt.owner);
        draw_poly(dx, dy, 4, 24.0 * k, 0.0, col(c, 1.0));
        draw_poly(dx, dy, 4, 20.0 * k, 0.0, col(DARK, 0.9));
        crate::ui::text_c(POINT_NAMES[p], dx, dy + 8.0 * k, sz(24.0), col(c, 1.0));
    }
    draw_line(ix, py + 342.0 * k, ix + iw, py + 342.0 * k, 1.0, th::STROKE);
    let mut squad: Vec<_> = app.net.roster.iter().filter(|r| r.team == app.my_team).collect();
    squad.sort_by_key(|r| (r.id != app.my_id, r.bot, std::cmp::Reverse(r.score)));
    let humans = squad.iter().filter(|r| !r.bot).count();
    draw_text(&format!("YOUR SQUAD  ({} / {} human)", humans, squad.len()), ix, py + 370.0 * k, sz(17.0), th::MUTED);
    for (i, r) in squad.iter().take(6).enumerate() {
        let ry = py + (384.0 + i as f32 * 28.0) * k;
        let me = r.id == app.my_id;
        if me {
            draw_rectangle(ix - 6.0 * k, ry, iw + 12.0 * k, 26.0 * k, col(WHITE, 0.08));
        }
        match app.icon(r.id) {
            Some(t) => pic(t, ix, ry + 4.0 * k, 18.0 * k, 1.0),
            None => draw_circle(ix + 8.0 * k, ry + 13.0 * k, 5.0 * k, if r.bot { th::QUIET } else { th::SUCCESS }),
        }
        draw_text(&r.name, ix + 30.0 * k, ry + 20.0 * k, sz(19.0), if r.bot { th::MUTED } else { th::WHITE });
        let tag = if me {
            "YOU"
        } else if r.bot {
            "BOT"
        } else {
            ""
        };
        crate::ui::text_r(tag, ix + iw, ry + 20.0 * k, sz(15.0), if me { th::EMBER_BRIGHT } else { th::QUIET });
        crate::ui::text_r(&format!("{} / {}", r.kills, r.deaths), ix + iw - 60.0 * k, ry + 20.0 * k, sz(15.0), th::MUTED);
    }

    // ---- right: controls ----
    let qy = py + 592.0 * k;
    let qh = (h - qy - 40.0 * k).min(250.0 * k);
    if qh < 150.0 * k {
        return;
    }
    crate::ui::plate(px, qy, pw, qh, th::GLASS_STRONG, th::STROKE);
    draw_text("CONTROLS", ix, qy + 32.0 * k, sz(17.0), th::MUTED);
    use crate::bindings::Action;
    let keys = [
        [
            ("ESC".into(), "resume"),
            (key_hint(app, Action::Loadout), "loadout"),
            (key_hint(app, Action::ChatAll), "chat"),
            (key_hint(app, Action::Scoreboard), "scoreboard"),
            ("Q".into(), "leave (here)"),
        ],
        [
            (key_hint(app, Action::CycleSight), "change sight"),
            (key_hint(app, Action::Grenade), "frag grenade"),
            (key_hint(app, Action::SwapWeapon), "sidearm"),
            (key_hint(app, Action::Crouch), "crouch / slide"),
            (key_hint(app, Action::Melee), "melee"),
        ],
    ];
    for (c, column) in keys.iter().enumerate() {
        for (r, (key, what)) in column.iter().enumerate() {
            let kx = ix + c as f32 * iw * 0.5;
            let ky = qy + (50.0 + r as f32 * 36.0) * k;
            if ky + 30.0 * k > qy + qh {
                break;
            }
            crate::ui::plate(kx, ky, 52.0 * k, 28.0 * k, col(DARK, 0.6), th::STROKE);
            crate::ui::text_c(key, kx + 26.0 * k, ky + 21.0 * k, if key.len() > 3 { sz(12.0) } else { sz(16.0) }, th::WHITE);
            draw_text(what, kx + 62.0 * k, ky + 21.0 * k, sz(16.0), th::MUTED);
        }
    }
}

/// Scoreboard: the series so far (who won each round, on which map, and
/// its top scorer), then both teams with this round's figures and each
/// player's record on this server.
fn draw_scoreboard(app: &App, snap: &ferrite_shared::proto::Snapshot, w: f32, h: f32, top: f32) {
    use crate::ui::color as th;
    let bw = (w - 32.0).min(1120.0);
    let x0 = w / 2.0 - bw / 2.0;
    let bh = (h - top - 12.0).min(620.0);
    crate::ui::plate(x0, top, bw, bh, col(DARK, 0.9), th::STROKE);

    // ---- the series ----
    let (lx, rx) = (x0 + 18.0, x0 + bw - 18.0);
    text("ROUND BY ROUND", lx, top + 24.0, 15.0, th::MUTED);
    let wins = app.net.wins;
    let mid = x0 + 300.0;
    crate::ui::text_r(&format!("{}  {}", TEAM_NAMES[0], wins[0]), mid - 14.0, top + 26.0, 22.0, col(team_col(0), 1.0));
    text_c("-", mid, top + 26.0, 22.0, th::MUTED);
    text(&format!("{}  {}", wins[1], TEAM_NAMES[1]), mid + 14.0, top + 26.0, 22.0, col(team_col(1), 1.0));
    let live = match snap.phase {
        0 => format!(
            "round {} in progress: {} on {}",
            app.net.rounds.len().max((wins[0] + wins[1]) as usize) + 1,
            ferrite_shared::mode_def(snap.mode).name.to_lowercase(),
            ferrite_shared::map::map_def(snap.map).name
        ),
        1 => "round over".to_string(),
        _ => "in the lobby".to_string(),
    };
    crate::ui::text_r(&live, rx, top + 24.0, 15.0, th::MUTED);
    let cards_y = top + 38.0;
    let ch = 62.0;
    let shown = app.net.rounds.len().min(if bw < 900.0 { 4 } else { 6 });
    let first = app.net.rounds.len() - shown;
    let number0 = (wins[0] + wins[1]) as usize - app.net.rounds.len().min((wins[0] + wins[1]) as usize);
    if shown == 0 {
        text("No round has finished yet. Each one is listed here as it ends.", lx, cards_y + 36.0, 16.0, th::QUIET);
    }
    let gap = 8.0;
    let cw = ((bw - 36.0 - gap * 5.0) / 6.0).min(190.0);
    for (k, r) in app.net.rounds[first..].iter().enumerate() {
        let x = lx + k as f32 * (cw + gap);
        let tc = team_col(r.winner);
        crate::ui::plate(x, cards_y, cw, ch, col([tc[0] * 0.2, tc[1] * 0.2, tc[2] * 0.2], 0.95), col(tc, 0.7));
        draw_rectangle(x, cards_y, 4.0, ch, col(tc, 1.0));
        let name = ferrite_shared::map::map_def(r.map).name.to_uppercase();
        text(&format!("{}  {}", number0 + first + k + 1, name), x + 12.0, cards_y + 18.0, 15.0, th::WHITE);
        text(TEAM_NAMES[r.winner as usize % 2], x + 12.0, cards_y + 38.0, 20.0, col(tc, 1.0));
        crate::ui::text_r(&format!("{} - {}", r.tickets[0], r.tickets[1]), x + cw - 8.0, cards_y + 37.0, 15.0, th::MUTED);
        let mut mvp = format!("{}  {}", r.mvp, r.mvp_score);
        while crate::ui::width(&mvp, 14.0) > cw - 62.0 && mvp.len() > 4 {
            mvp.pop();
        }
        text(&mvp, x + 12.0, cards_y + 55.0, 14.0, col(team_col(r.mvp_team), 0.9));
        crate::ui::text_r(&format!("{}:{:02}", r.secs / 60, r.secs % 60), x + cw - 8.0, cards_y + 55.0, 14.0, th::QUIET);
    }

    // ---- the teams ----
    let ty = cards_y + ch + 14.0;
    draw_line(lx, ty - 6.0, rx, ty - 6.0, 1.0, th::STROKE);
    let half = (bw - 36.0) / 2.0;
    // Columns, as offsets from the right edge of a team's half: this
    // round's score, headshots, deaths and kills, then the server record.
    let wide = half > 500.0;
    let (c_wins, c_kd, c_score, c_hs, c_d, c_k) = (10.0, 78.0, 176.0, 240.0, 278.0, 316.0);
    for t in 0..2u8 {
        let x = lx + t as f32 * (half + 8.0) + 4.0;
        let right = x + half - 12.0;
        let tc = team_col(t);
        text(TEAM_NAMES[t as usize], x, ty + 22.0, 24.0, col(tc, 1.0));
        crate::ui::text_r(
            &format!("{} {}", snap.tickets[t as usize], score_word(snap.mode)),
            x + 250.0f32.min(half * 0.5),
            ty + 20.0,
            15.0,
            th::MUTED,
        );
        let head = |s: &str, off: f32| crate::ui::text_r(s, right - off, ty + 42.0, 13.0, th::QUIET);
        head("K", c_k);
        head("D", c_d);
        head("HS", c_hs);
        head("SCORE", c_score);
        head("K / D", c_kd);
        head("WON", c_wins);
        crate::ui::text_r("THIS ROUND", right - c_score, ty + 20.0, 13.0, th::MUTED);
        crate::ui::text_r("ON THIS SERVER", right - c_wins, ty + 20.0, 13.0, th::MUTED);
        draw_line(right - c_score + 14.0, ty + 10.0, right - c_score + 14.0, top + bh - 10.0, 1.0, th::STROKE);
        let mut rows: Vec<_> = app.net.roster.iter().filter(|r| r.team == t).collect();
        rows.sort_by(|a, b| b.score.cmp(&a.score).then(b.total.score.cmp(&a.total.score)));
        for (i, r) in rows.iter().enumerate() {
            let y = ty + 64.0 + i as f32 * 21.0;
            if y > top + bh - 8.0 {
                break;
            }
            let me = r.id == app.my_id && !app.spectator;
            if me {
                draw_rectangle(x - 6.0, y - 16.0, half - 2.0, 21.0, col(WHITE, 0.14));
            }
            let c = if r.bot { th::MUTED } else { th::WHITE };
            let mut name = r.name.clone();
            if !wide {
                name.truncate(9);
            }
            // A column for icons, once anyone here has one.
            let ic = if app.icons.is_empty() { 0.0 } else { 20.0 };
            if let Some(t) = app.icon(r.id) {
                pic(t, x, y - 14.0, 16.0, 1.0);
            }
            text(&name, x + ic, y, 17.0, c);
            if wide {
                text(&sim::kit(r.kit).name[..3], x + ic + 150.0, y, 13.0, th::QUIET);
            }
            let cell = |s: String, off: f32, c: Color| crate::ui::text_r(&s, right - off, y, 16.0, c);
            cell(r.kills.to_string(), c_k, c);
            cell(r.deaths.to_string(), c_d, c);
            cell(r.heads.to_string(), c_hs, c);
            cell(r.score.to_string(), c_score, c);
            let dim = if r.bot { th::QUIET } else { th::MUTED };
            cell(format!("{} / {}", r.total.kills, r.total.deaths), c_kd, dim);
            cell(format!("{} / {}", r.total.wins, r.total.rounds), c_wins, dim);
        }
    }
}

/// Draws the HUD and returns the weapon models that belong on top of it.
pub fn draw(app: &App, vp: &Mat4, now: f64, scoped: bool, tris: usize) -> Vec<GunIcon> {
    let mut icons = Vec::new();
    let (w, h) = (screen_width(), screen_height());
    let (cx, cy) = (w * 0.5, h * 0.5);
    let Some(snap) = app.net.latest() else {
        text_c("waiting for server...", cx, cy, 30.0, col(WHITE, 1.0));
        return icons;
    };
    let playing = app.alive && !app.spectator;
    let my_pos = if app.spectator { app.views.iter().find(|v| v.id == app.spec_target).map_or(Vec3::ZERO, |v| v.pos) } else { app.st.pos };
    let owners = [snap.points[0].owner, snap.points[1].owner, snap.points[2].owner, snap.points[3].owner, snap.points[4].owner];

    // ---- scope ----
    if scoped {
        let r = h * 0.46;
        let dark = col(DARK, 1.0);
        draw_rectangle(0.0, 0.0, cx - r, h, dark);
        draw_rectangle(cx + r, 0.0, w - cx - r, h, dark);
        draw_rectangle(cx - r, 0.0, 2.0 * r, cy - r, dark);
        draw_rectangle(cx - r, cy + r, 2.0 * r, cy - r, dark);
        for k in 0..128 {
            let a0 = k as f32 / 128.0 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / 128.0 * std::f32::consts::TAU;
            let p = |a: f32, rr: f32| Vec2::new(cx + a.cos() * rr, cy + a.sin() * rr);
            draw_triangle(p(a0, r), p(a1, r), p(a0, r * 1.5), dark);
            draw_triangle(p(a1, r), p(a1, r * 1.5), p(a0, r * 1.5), dark);
        }
        draw_circle_lines(cx, cy, r, 3.0, col(DARK, 1.0));
        draw_line(cx - r, cy, cx - 14.0, cy, 1.5, col(DARK, 0.9));
        draw_line(cx + 14.0, cy, cx + r, cy, 1.5, col(DARK, 0.9));
        draw_line(cx, cy + 14.0, cx, cy + r, 1.5, col(DARK, 0.9));
        draw_circle(cx, cy, 1.5, Color::new(1.0, 0.2, 0.15, 0.9));
    }

    // ---- seedlines: how to take one, and how to ride ----
    if playing && !app.world.lines.is_empty() {
        use crate::bindings::Action;
        let use_key = key_hint(app, Action::Use);
        let y = h * 0.66;
        if app.st.ride > 0 {
            let (fw, bk, cr, jp) = (
                key_hint(app, Action::Forward),
                key_hint(app, Action::Backward),
                key_hint(app, Action::Crouch),
                key_hint(app, Action::Jump),
            );
            let parts = [
                Hint::Key(&fw),
                Hint::Words("reel"),
                Hint::Gap,
                Hint::Key(&bk),
                Hint::Words("back"),
                Hint::Gap,
                Hint::Key(&cr),
                Hint::Words("brake"),
                Hint::Gap,
                Hint::Key(&jp),
                Hint::Words("let go"),
            ];
            let wd = hints(&parts, 0.0, y, 16.0, col(WHITE, 0.9), false);
            hints(&parts, cx - wd / 2.0, y, 16.0, col(WHITE, 0.9), true);
        } else if app.st.on_ground && sim::dock_at(&app.world, app.st.pos, sim::view_dir(app.yaw, 0.0)).is_some() {
            let parts = [Hint::Key(&use_key), Hint::Words("RIDE THE SEEDLINE")];
            let wd = hints(&parts, 0.0, y, 18.0, col(WHITE, 0.95), false);
            hints(&parts, cx - wd / 2.0, y, 18.0, col(WHITE, 0.95), true);
        }
    }

    // ---- world-space markers ----
    for (k, p) in app.world.points.iter().enumerate() {
        if !point_used(snap.mode, k) {
            continue;
        }
        if let Some((sx, sy, d)) = project(vp, *p + Vec3::new(0.0, 9.3, 0.0), w, h) {
            let pt = &snap.points[k];
            let c = team_col(pt.owner);
            let contested = pt.n[0] > 0 && pt.n[1] > 0;
            let pulse = if contested { 0.75 + 0.25 * (now as f32 * 8.0).sin() } else { 1.0 };
            draw_poly(sx, sy, 4, 15.0, 45.0, col(DARK, 0.75));
            draw_poly(sx, sy, 4, 12.0, 45.0, col(c, 0.92 * pulse));
            text_c(POINT_NAMES[k], sx, sy + 6.0, 21.0, col(DARK, 1.0));
            text_c(&format!("{:.0}m", d), sx, sy + 30.0, 16.0, col(WHITE, 0.85));
        }
    }
    // Flags: where each one is, and what to do about it.
    let have_flag = snap.mode == mode::CTF && playing && snap.flags.iter().any(|f| f.state == 1 && f.carrier == app.my_id);
    if snap.mode == mode::CTF && app.kc.is_none() {
        for (f, fl) in snap.flags.iter().enumerate() {
            let ours = f as u8 == app.my_team && !app.spectator;
            let mine = fl.state == 1 && fl.carrier == app.my_id && !app.spectator;
            // Our flag at home stands where BRING IT HERE points; while we
            // carry theirs that one marker says all there is to say.
            if have_flag && ours && fl.state == 0 {
                continue;
            }
            // Carrying theirs: the marker that matters is our own stand.
            let (at, label) = if mine {
                (app.world.points[ferrite_shared::FLAG_POINT[app.my_team as usize % 2]], "BRING IT HERE")
            } else {
                let pos = if fl.state == 1 {
                    app.views.iter().find(|v| v.id == fl.carrier).map_or(Vec3::from(fl.pos), |v| v.pos)
                } else {
                    Vec3::from(fl.pos)
                };
                let label = match (fl.state, ours) {
                    (0, true) => "DEFEND",
                    (0, false) => "TAKE",
                    (1, true) => "STOP THEM",
                    (1, false) => "ESCORT",
                    (_, true) => "RETURN",
                    _ => "PICK UP",
                };
                (pos, label)
            };
            if let Some((sx, sy, d)) = project(vp, at + Vec3::new(0.0, 4.4, 0.0), w, h) {
                let c = team_col(f as u8);
                let pulse = if fl.state != 0 { 0.7 + 0.3 * (now as f32 * 8.0).sin() } else { 1.0 };
                draw_poly(sx, sy, 3, 15.0, 90.0, col(DARK, 0.8));
                draw_poly(sx, sy, 3, 11.0, 90.0, col(c, pulse));
                text_c(label, sx, sy - 18.0, 15.0, col(WHITE, 0.95));
                text_c(&format!("{:.0}m", d), sx, sy + 28.0, 15.0, col(WHITE, 0.85));
            }
        }
    }
    // Things to pick up, once close enough to go for.
    if app.kc.is_none() {
        for p in &snap.pickups {
            let pos = Vec3::from(p.pos_f());
            let d = (pos - my_pos).length();
            if d > 45.0 {
                continue;
            }
            if let Some((sx, sy, _)) = project(vp, pos + Vec3::new(0.0, 1.5, 0.0), w, h) {
                let k = p.kind as usize % 5;
                let c = crate::scene::PICKUP_COL[k];
                draw_poly(sx, sy, 4, 7.0, 0.0, col(DARK, 0.75));
                draw_poly(sx, sy, 4, 5.0, 0.0, col(c, 0.95));
                if d < 16.0 {
                    text_c(pickup::NAMES[k], sx, sy - 11.0, 14.0, col(c, 0.95));
                }
            }
        }
    }
    let view_dir = sim::view_dir(app.yaw, app.pitch);
    let eye = my_pos + Vec3::new(0.0, sim::eye_h(app.st.crouch), 0.0);
    for v in &app.views {
        let friendly = v.team == app.my_team && !app.spectator;
        let head = v.pos + Vec3::new(0.0, 2.05, 0.0);
        let Some((sx, sy, d)) = project(vp, head, w, h) else { continue };
        let c = team_col(v.team);
        if app.kc.is_some() {
            // The killcam shows what the killer saw; only we are pointed out.
            if v.id == app.my_id {
                draw_triangle(Vec2::new(sx - 7.0, sy - 10.0), Vec2::new(sx + 7.0, sy - 10.0), Vec2::new(sx, sy), col(WHITE, 0.95));
                text_c("YOU", sx, sy - 15.0, 16.0, col(WHITE, 0.95));
            }
        } else if friendly || app.spectator {
            if d < 70.0 {
                let s = (7.0 - d * 0.04).max(3.5);
                draw_triangle(Vec2::new(sx - s, sy - s * 1.4), Vec2::new(sx + s, sy - s * 1.4), Vec2::new(sx, sy), col(c, 0.9));
                if d < 28.0 {
                    let name = app.name(v.id);
                    text_c(&name, sx, sy - s * 1.4 - 5.0, 15.0, col(WHITE, 0.85));
                    if let Some(t) = app.icon(v.id) {
                        pic(t, sx - crate::ui::width(&name, 15.0) / 2.0 - 18.0, sy - s * 1.4 - 17.0, 14.0, 0.9);
                    }
                }
            }
        } else if v.flags & flag::MARKED != 0 {
            // Spotted by a Marksman: visible to the whole team.
            draw_poly(sx, sy - 8.0, 4, 8.0, 0.0, Color::new(1.0, 0.2, 0.15, 0.95));
            text_c(&format!("{:.0}m", d), sx, sy - 20.0, 14.0, Color::new(1.0, 0.5, 0.4, 0.9));
        } else if playing {
            let to = (v.pos + Vec3::new(0.0, 1.1, 0.0) - eye).normalize_or_zero();
            if to.dot(view_dir) > 0.9992 && d < 90.0 && app.world.los(eye, v.pos + Vec3::new(0.0, 1.3, 0.0), app.dmask) {
                text_c(&app.name(v.id), sx, sy - 6.0, 15.0, Color::new(1.0, 0.45, 0.35, 0.95));
            }
        }
    }

    // ---- grenade warning: a marker toward any live frag close enough to hurt ----
    if playing {
        for g in &snap.grenades {
            let gp = Vec3::from(g.pos);
            let d = gp - my_pos;
            let dist = d.length();
            if dist > 11.0 || (g.team == app.my_team && dist > 3.0) {
                continue;
            }
            let rel = ferrite_shared::wrap_angle(app.yaw - d.x.atan2(d.z));
            let a = rel - std::f32::consts::FRAC_PI_2;
            let r = 96.0;
            let (px, py) = (cx + a.cos() * r, cy + a.sin() * r);
            let pulse = 0.65 + 0.35 * (now as f32 * 18.0).sin();
            let c = Color::new(1.0, 0.75, 0.15, pulse * (1.0 - dist / 13.0).clamp(0.3, 1.0));
            draw_poly(px, py, 6, 13.0, 0.0, col(DARK, 0.7));
            draw_poly(px, py, 6, 10.0, 0.0, c);
            text_c("!", px, py + 6.0, 20.0, col(DARK, 1.0));
            text_c(&format!("{:.0}m", dist), px, py + 26.0, 14.0, c);
        }
    }

    // ---- suppression: the edges close in when rounds pass close ----
    if app.suppress > 0.0 {
        let a = app.suppress * 0.3;
        let (bw, bh) = (w * 0.16, h * 0.16);
        for k in 0..6 {
            let f = k as f32 / 6.0;
            let c = col(DARK, a * (1.0 - f) / 3.0);
            draw_rectangle(0.0, 0.0, w, bh * (1.0 - f), c);
            draw_rectangle(0.0, h - bh * (1.0 - f), w, bh * (1.0 - f), c);
            draw_rectangle(0.0, 0.0, bw * (1.0 - f), h, c);
            draw_rectangle(w - bw * (1.0 - f), 0.0, bw * (1.0 - f), h, c);
        }
    }

    // ---- damage feedback ----
    if app.dmg_flash > 0.0 {
        let a = app.dmg_flash * 0.35;
        let red = Color::new(0.8, 0.05, 0.02, a);
        let t = 70.0;
        draw_rectangle(0.0, 0.0, w, t, red);
        draw_rectangle(0.0, h - t, w, t, red);
        draw_rectangle(0.0, t, t, h - 2.0 * t, red);
        draw_rectangle(w - t, t, t, h - 2.0 * t, red);
    }
    if playing {
        for (from, at) in &app.dmg_dirs {
            let d = *from - my_pos;
            let world_a = d.x.atan2(d.z);
            let rel = ferrite_shared::wrap_angle(app.yaw - world_a); // + = to the right
            let a = rel - std::f32::consts::FRAC_PI_2;
            let fade = (1.0 - (now - at) as f32 / 1.6).clamp(0.0, 1.0);
            let r = 130.0;
            let (px, py) = (cx + a.cos() * r, cy + a.sin() * r);
            let (tx, ty) = (-a.sin(), a.cos());
            draw_triangle(
                Vec2::new(px + tx * 34.0, py + ty * 34.0),
                Vec2::new(px - tx * 34.0, py - ty * 34.0),
                Vec2::new(px + a.cos() * 26.0, py + a.sin() * 26.0),
                Color::new(1.0, 0.15, 0.1, 0.8 * fade),
            );
        }
    }

    // ---- crosshair & hit marker ----
    if playing {
        let kit = sim::wid(&app.st);
        let sight = app.sight();
        let optic = (crate::scene::RED_DOT..=crate::scene::OPTIC).contains(&sight);
        if optic && app.ads > 0.8 && app.st.reload == 0 && app.st.swap == 0 {
            // Illuminated reticle, drawn where the optic's window sits.
            let a = ((app.ads - 0.8) / 0.15).clamp(0.0, 1.0);
            let red = Color::new(1.0, 0.16, 0.1, a);
            let glow = Color::new(1.0, 0.2, 0.1, 0.22 * a);
            match sight {
                crate::scene::RED_DOT => {
                    draw_circle(cx, cy, 7.0, glow);
                    draw_circle(cx, cy, 3.0, red);
                }
                crate::scene::HOLO => {
                    draw_circle_lines(cx, cy, 22.0, 2.0, red);
                    draw_circle(cx, cy, 2.2, red);
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        draw_line(cx + dx * 22.0, cy + dy * 22.0, cx + dx * 29.0, cy + dy * 29.0, 2.0, red);
                    }
                }
                _ => {
                    draw_line(cx, cy, cx - 9.0, cy + 13.0, 2.5, red);
                    draw_line(cx, cy, cx + 9.0, cy + 13.0, 2.5, red);
                    for (i, wd) in [6.0f32, 4.5, 3.0].iter().enumerate() {
                        let y = cy + 19.0 + i as f32 * 5.0;
                        draw_line(cx - wd, y, cx + wd, y, 1.5, col(DARK, 0.85 * a));
                    }
                    draw_line(cx, cy + 16.0, cx, cy + 30.0, 1.5, col(DARK, 0.85 * a));
                }
            }
        }
        if !scoped && !(optic && app.ads > 0.5) {
            let ads = app.ads > 0.5;
            let cone = sim::spread(kit, ads, app.hspeed(), !app.st.on_ground, app.st.crouch, app.st.heat);
            let px_per_rad = h / app.fov.to_radians();
            let gap = (cone * px_per_rad + 3.0).min(140.0);
            let c = col(WHITE, 0.9);
            let len = 9.0;
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                draw_line(
                    cx + dx * gap + 1.0,
                    cy + dy * gap + 1.0,
                    cx + dx * (gap + len) + 1.0,
                    cy + dy * (gap + len) + 1.0,
                    2.0,
                    col(DARK, 0.5),
                );
                draw_line(cx + dx * gap, cy + dy * gap, cx + dx * (gap + len), cy + dy * (gap + len), 2.0, c);
            }
            draw_circle(cx, cy, 1.5, c);
        }
        if app.hitmarker > 0.0 {
            let a = (app.hitmarker / 0.22).min(1.0);
            let c = if app.hit_kill > 0.0 {
                Color::new(1.0, 0.25, 0.15, a)
            } else if app.hit_head {
                Color::new(1.0, 0.85, 0.2, a)
            } else {
                col(WHITE, a)
            };
            let (r0, r1) = (7.0 + (1.0 - a) * 5.0, 17.0 + (1.0 - a) * 5.0);
            for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                draw_line(cx + dx * r0, cy + dy * r0, cx + dx * r1, cy + dy * r1, 2.5, c);
            }
        }
    }

    // ---- top bar: tickets and the front line ----
    let bar_w = 190.0;
    let slot = 46.0;
    let line_w = slot * 5.0;
    let top = 14.0;
    draw_rectangle(cx - line_w * 0.5 - bar_w - 24.0, top - 6.0, line_w + 2.0 * bar_w + 48.0, 50.0, col(DARK, 0.55));
    for t in 0..2usize {
        let c = team_col(t as u8);
        let frac = (snap.tickets[t] as f32 / snap.goal.max(1) as f32).clamp(0.0, 1.0);
        let x = if t == 0 { cx - line_w * 0.5 - 12.0 - bar_w } else { cx + line_w * 0.5 + 12.0 };
        draw_rectangle(x, top + 4.0, bar_w, 12.0, col(DARK, 0.8));
        if t == 0 {
            draw_rectangle(x + bar_w * (1.0 - frac), top + 4.0, bar_w * frac, 12.0, col(c, 0.95));
            text(&format!("{}", snap.tickets[t]), x, top + 38.0, 24.0, col(c, 1.0));
            text_r(TEAM_NAMES[t], x + bar_w, top + 36.0, 18.0, col(WHITE, if app.my_team == 0 { 1.0 } else { 0.6 }));
        } else {
            draw_rectangle(x, top + 4.0, bar_w * frac, 12.0, col(c, 0.95));
            text_r(&format!("{}", snap.tickets[t]), x + bar_w, top + 38.0, 24.0, col(c, 1.0));
            text(TEAM_NAMES[t], x, top + 36.0, 18.0, col(WHITE, if app.my_team == 1 { 1.0 } else { 0.6 }));
        }
    }
    // Outside conquest the middle of the bar is the target and the clock.
    if snap.mode != mode::CONQUEST {
        let left = if snap.phase == 0 && snap.secs_left > 0 { clock(snap.secs_left) } else { "--:--".to_string() };
        let low = snap.phase == 0 && snap.secs_left > 0 && snap.secs_left <= 30;
        let tx = if snap.mode == mode::KOTH { cx + 34.0 } else { cx };
        text_c(&format!("FIRST TO {}", snap.goal), tx, top + 13.0, 15.0, col(WHITE, 0.75));
        text_c(&left, tx, top + 38.0, 26.0, col(if low { WARNING } else { WHITE }, 1.0));
    }
    if snap.mode == mode::CTF {
        // Each side's flag: at home, taken, or lying where it fell.
        for (f, fl) in snap.flags.iter().enumerate() {
            let x = cx + if f == 0 { -86.0 } else { 86.0 };
            let c = team_col(f as u8);
            let pulse = if fl.state != 0 { 0.6 + 0.4 * (now as f32 * 8.0).sin() } else { 1.0 };
            draw_poly(x, top + 14.0, 3, 13.0, 90.0, col(DARK, 0.9));
            draw_poly(x, top + 14.0, 3, 10.0, 90.0, col(c, pulse));
            text_c(
                ["HOME", "TAKEN", "DOWN"][fl.state as usize % 3],
                x,
                top + 40.0,
                13.0,
                col(if fl.state == 0 { WHITE } else { WARNING }, 0.9),
            );
        }
    }
    for k in 0..5 {
        if snap.mode != mode::CONQUEST && !(snap.mode == mode::KOTH && k == 2) {
            continue;
        }
        let pt = &snap.points[k];
        let x = if snap.mode == mode::KOTH { cx - 62.0 } else { cx - line_w * 0.5 + slot * (k as f32 + 0.5) };
        let y = top + 19.0;
        let c = team_col(pt.owner);
        if k < 4 && snap.mode == mode::CONQUEST {
            draw_line(x + 15.0, y, x + slot - 15.0, y, 3.0, col(WHITE, 0.25));
        }
        let hot = snap.mode != mode::CONQUEST || eligible(&owners, 0, k) && pt.owner != 0 || eligible(&owners, 1, k) && pt.owner != 1;
        let contested = pt.n[0] > 0 && pt.n[1] > 0;
        let pulse = if contested { 0.6 + 0.4 * (now as f32 * 8.0).sin() } else { 1.0 };
        draw_poly(x, y, 4, 17.0, 45.0, col(if hot { WHITE } else { DARK }, 0.9));
        draw_poly(x, y, 4, 14.0, 45.0, col(DARK, 1.0));
        draw_poly(x, y, 4, 13.0, 45.0, col(c, 0.35));
        // progress fill, growing from the leading team's colour
        let prog = pt.progress as f32 / 100.0;
        if prog.abs() > 0.02 {
            let lead = team_col(if prog > 0.0 { 1 } else { 0 });
            draw_poly(x, y, 4, 13.0 * prog.abs().sqrt(), 45.0, col(lead, pulse));
        }
        text_c(POINT_NAMES[k], x, y + 6.0, 20.0, col(WHITE, 1.0));
    }

    // ---- banners ----
    if let Some((msg, c, at)) = &app.banner {
        let age = (now - at) as f32;
        // The round-over banner takes this spot when a round ends.
        if age < 3.5 && snap.phase != 1 {
            let a = (1.0 - (age - 2.6).max(0.0) / 0.9).clamp(0.0, 1.0) * (age * 5.0).clamp(0.0, 1.0);
            let tw = measure_text(msg, None, 34, 1.0).width;
            draw_rectangle(cx - tw * 0.5 - 22.0, 92.0, tw + 44.0, 46.0, col(DARK, 0.6 * a));
            draw_rectangle(cx - tw * 0.5 - 22.0, 92.0, 6.0, 46.0, col(*c, a));
            text_c(msg, cx, 125.0, 34.0, col(WHITE, a));
        }
    }
    if let Some((msg, at)) = &app.sub_banner {
        let age = (now - at) as f32;
        if age < 2.5 {
            let a = (1.0 - (age - 1.8).max(0.0) / 0.7).clamp(0.0, 1.0);
            text_c(msg, cx, cy + 96.0, 24.0, col([1.0, 0.86, 0.45], a));
        }
    }

    // ---- kill feed ----
    // A kill by or of a person stands out and stays; bots shooting bots
    // are small print that soon goes.
    let mut fy = 92.0;
    for k in app.feed.iter().rev() {
        let age = (now - k.at) as f32;
        let life = if k.mine {
            9.0
        } else if k.human {
            8.0
        } else {
            4.5
        };
        if age > life {
            continue;
        }
        let a = (life - age).clamp(0.0, 1.0);
        let (size, row, dim) = if k.human { (19.0, 28.0, 1.0) } else { (15.0, 19.0, 0.55) };
        let isz = size - 3.0;
        let mid = if k.head { format!("  [{} HS]  ", k.weapon) } else { format!("  [{}]  ", k.weapon) };
        let iw = |i: &Option<macroquad::prelude::Texture2D>| if i.is_some() { isz + 5.0 } else { 0.0 };
        let (wk, wm, wv) = (crate::ui::width(&k.killer, size), crate::ui::width(&mid, size), crate::ui::width(&k.victim, size));
        let total = iw(&k.kicon) + wk + wm + iw(&k.vicon) + wv;
        let x = w - 18.0 - total;
        let y = fy + size - 4.0;
        if k.human {
            let back = if k.mine { [0.34, 0.27, 0.08] } else { DARK };
            draw_rectangle(x - 12.0, fy - 3.0, total + 22.0, row - 3.0, col(back, 0.78 * a));
            draw_rectangle(x - 12.0, fy - 3.0, 4.0, row - 3.0, col(team_col(k.kteam), a));
            if k.mine {
                draw_rectangle_lines(x - 12.0, fy - 3.0, total + 22.0, row - 3.0, 2.0, col(WARNING, 0.9 * a));
            }
        } else {
            draw_rectangle(x - 8.0, fy - 2.0, total + 16.0, row - 2.0, col(DARK, 0.3 * a));
        }
        let mut tx = x;
        if let Some(t) = &k.kicon {
            pic(t, tx, y - isz + 2.0, isz, a);
            tx += isz + 5.0;
        }
        text(&k.killer, tx, y, size, col(team_col(k.kteam), a * dim));
        tx += wk;
        text(&mid, tx, y, size, col(WHITE, 0.8 * a * dim));
        tx += wm;
        if let Some(t) = &k.vicon {
            pic(t, tx, y - isz + 2.0, isz, a);
            tx += isz + 5.0;
        }
        text(&k.victim, tx, y, size, col(team_col(k.vteam), a * dim));
        fy += row;
    }

    // ---- minimap ----
    {
        // The same size on screen as before the maps were spread out.
        let scale = 0.92 / ferrite_shared::map::SPREAD;
        let (mw, mh) = (2.0 * HALF_X * scale, 2.0 * HALF_Z * scale);
        let (mx, my) = (16.0, 16.0);
        // Team 1 sees the map rotated so "forward" is always up-right consistent.
        let flip = if app.my_team == 1 && !app.spectator { -1.0 } else { 1.0 };
        let to = |p: Vec3| (mx + (p.x * flip + HALF_X) * scale, my + (p.z * flip + HALF_Z) * scale);
        draw_rectangle(mx, my, mw, mh, col([0.13, 0.14, 0.11], 0.72));
        for b in &app.world.blocks {
            if matches!(b.kind, BlockKind::Trunk | BlockKind::Crate) {
                continue;
            }
            let (x0, y0) = to(if flip > 0.0 { b.min } else { b.max });
            draw_rectangle(x0, y0, (b.max.x - b.min.x) * scale, (b.max.z - b.min.z) * scale, col(WHITE, 0.22));
        }
        draw_rectangle_lines(mx, my, mw, mh, 2.0, col(WHITE, 0.35));
        for p in &snap.pickups {
            let (x, y) = to(Vec3::from(p.pos_f()));
            draw_rectangle(x - 2.0, y - 2.0, 4.0, 4.0, col(crate::scene::PICKUP_COL[p.kind as usize % 5], 0.9));
        }
        if snap.mode == mode::CTF {
            for (f, fl) in snap.flags.iter().enumerate() {
                let (x, y) = to(Vec3::from(fl.pos));
                let pulse = if fl.state != 0 { 0.6 + 0.4 * (now as f32 * 8.0).sin() } else { 1.0 };
                draw_poly(x, y, 3, 8.0, 90.0, col(DARK, 0.9));
                draw_poly(x, y, 3, 6.0, 90.0, col(team_col(f as u8), pulse));
            }
        }
        for (k, p) in app.world.points.iter().enumerate() {
            if !point_used(snap.mode, k) {
                continue;
            }
            let (x, y) = to(*p);
            let pt = &snap.points[k];
            draw_circle(x, y, POINT_RADIUS * scale, col(team_col(pt.owner), 0.3));
            draw_circle_lines(x, y, POINT_RADIUS * scale, 1.5, col(team_col(pt.owner), 0.9));
            text_c(POINT_NAMES[k], x, y + 5.0, 16.0, col(WHITE, 0.95));
        }
        for v in &app.views {
            let friendly = v.team == app.my_team || app.spectator;
            if friendly || v.flags & (flag::MARKED | flag::FIRING) != 0 {
                let (x, y) = to(v.pos);
                draw_circle(x, y, 2.6, col(team_col(v.team), if friendly { 0.95 } else { 0.8 }));
            }
        }
        if playing {
            let (x, y) = to(my_pos);
            let f = sim::view_dir(app.yaw, 0.0) * flip;
            let (fx, fy) = (f.x, f.z);
            draw_triangle(
                Vec2::new(x + fx * 7.0, y + fy * 7.0),
                Vec2::new(x - fx * 4.0 - fy * 4.0, y - fy * 4.0 + fx * 4.0),
                Vec2::new(x - fx * 4.0 + fy * 4.0, y - fy * 4.0 - fx * 4.0),
                col(WHITE, 1.0),
            );
        }
    }

    // ---- capture panel ----
    if playing || app.spectator {
        for k in 0..app.world.points.len() {
            // The server's rule: on Spillway's gate deck, not under it.
            if !app.world.on_point(k, my_pos) || !point_used(snap.mode, k) {
                continue;
            }
            let pt = &snap.points[k];
            let prog = pt.progress as f32 / 100.0;
            let (bw, bh) = (300.0, 12.0);
            let y = 158.0;
            let me = app.my_team;
            let status = if pt.n[0] > 0 && pt.n[1] > 0 {
                "CONTESTED"
            } else if pt.owner == me && prog.abs() > 0.98 {
                "HOLDING"
            } else if snap.mode == mode::CONQUEST && pt.owner != me && !eligible(&owners, me, k) {
                "LOCKED - take the point before it first"
            } else if pt.n[me as usize] > 0 && pt.owner != me {
                "CAPTURING"
            } else if pt.n[me as usize] > 0 {
                "SECURING"
            } else {
                "LOSING"
            };
            let sector = if app.world.map == ferrite_shared::map::slag_cathedral::MAP_ID {
                format!("{} / {}", POINT_NAMES[k], ferrite_shared::map::slag_cathedral::SECTOR_NAMES[k])
            } else {
                format!("POINT {}", POINT_NAMES[k])
            };
            text_c(&format!("{}  -  {}", sector, status), cx, y - 8.0, 20.0, col(WHITE, 0.95));
            draw_rectangle(cx - bw * 0.5 - 2.0, y - 2.0, bw + 4.0, bh + 4.0, col(DARK, 0.8));
            draw_rectangle(cx - bw * 0.5, y, bw, bh, col(NEUTRAL_COL, 0.25));
            if prog < 0.0 {
                draw_rectangle(cx + prog * bw * 0.5, y, -prog * bw * 0.5, bh, col(team_col(0), 0.95));
            } else {
                draw_rectangle(cx, y, prog * bw * 0.5, bh, col(team_col(1), 0.95));
            }
            draw_line(cx, y - 3.0, cx, y + bh + 3.0, 2.0, col(WHITE, 0.9));
            text_r(&format!("{}", pt.n[0]), cx - bw * 0.5 - 10.0, y + 12.0, 20.0, col(team_col(0), 1.0));
            text(&format!("{}", pt.n[1]), cx + bw * 0.5 + 10.0, y + 12.0, 20.0, col(team_col(1), 1.0));
        }
    }

    // ---- vitals & weapon ----
    if playing {
        let kd = sim::kit(app.st.kit);
        let max = kd.hp;
        let frac = (app.hp as f32 / max).clamp(0.0, 1.0);
        let (x, y) = (24.0, h - 44.0);
        draw_rectangle(x - 4.0, y - 4.0, 248.0, 28.0, col(DARK, 0.6));
        let hc = if frac > 0.5 {
            WHITE
        } else if frac > 0.25 {
            [1.0, 0.75, 0.2]
        } else {
            [1.0, 0.25, 0.15]
        };
        draw_rectangle(x, y, 240.0 * frac, 20.0, col(hc, 0.92));
        text(&format!("{}", app.hp), x + 6.0, y + 16.0, 20.0, col(DARK, 1.0));
        // Armour sits over the health it protects.
        let mut ly = y - 12.0;
        if app.armor > 0 {
            let ac = crate::scene::PICKUP_COL[pickup::ARMOR as usize];
            draw_rectangle(x - 4.0, y - 18.0, 248.0, 14.0, col(DARK, 0.6));
            draw_rectangle(x, y - 15.0, 240.0 * (app.armor as f32 / 100.0).min(1.0), 8.0, col(ac, 0.95));
            text(&format!("ARMOUR {}", app.armor), x + 250.0, y - 6.0, 15.0, col(ac, 1.0));
            ly -= 16.0;
        }
        text(kd.name, x, ly, 20.0, col(team_col(app.my_team), 1.0));
        let mut sx = x + 110.0;
        let blink = 0.7 + 0.3 * (now as f32 * 10.0).sin();
        if app.st.surge {
            text("SURGE", sx, ly, 20.0, col([1.0, 0.86, 0.45], blink));
            sx += 74.0;
        }
        if app.power > 0 {
            text(&format!("OVERCHARGE {}s", app.power), sx, ly, 20.0, col(crate::scene::PICKUP_COL[pickup::OVERCHARGE as usize], blink));
        }
        if have_flag {
            text_c("YOU HAVE THEIR FLAG", cx, h - 150.0, 24.0, col(WARNING, blink));
        }
        // The streak, and the kill its next reward comes at.
        if app.streak >= 2 {
            let next = [3u8, 5, 7, 10].into_iter().find(|n| *n > app.streak).unwrap_or((app.streak / 5 + 1) * 5);
            text(&format!("STREAK {}", app.streak), x, ly - 24.0, 18.0, col(WARNING, 1.0));
            text(&format!("reward at {next}"), x + 96.0, ly - 24.0, 15.0, col(WHITE, 0.6));
        }
        // With touch controls the fire cluster has the bottom right.
        let w = if app.touch { w - 470.0 * (h / 720.0).clamp(0.75, 1.6) } else { w };
        let wd = sim::weapon_def(sim::wid(&app.st));
        let ammo = format!("{}", app.st.ammo);
        let low = app.st.ammo as f32 <= wd.mag as f32 * 0.25;
        text_r(&ammo, w - 96.0, h - 26.0, 52.0, if low { Color::new(1.0, 0.35, 0.2, 1.0) } else { col(WHITE, 1.0) });
        text(&format!("/ {}", wd.mag), w - 90.0, h - 28.0, 24.0, col(WHITE, 0.6));
        text_r(
            &format!("{}  -  {}", wd.name, crate::scene::SIGHT_NAMES[app.sight() as usize % 6]),
            w - 24.0,
            h - 74.0,
            20.0,
            col(WHITE, 0.75),
        );
        text_r(
            &format!("[{}] change sight", app.bindings.display(crate::bindings::Action::CycleSight)),
            w - 24.0,
            h - 150.0,
            18.0,
            col(WHITE, 0.55),
        );
        let other = sim::weapon_def(if app.st.slot == 1 { app.st.prim } else { app.st.side }).name;
        text_r(
            &format!("[{}] {}  {}", key_hint(app, crate::bindings::Action::SwapWeapon), other, app.st.ammo_alt),
            w - 24.0,
            h - 128.0,
            18.0,
            col(WHITE, 0.6),
        );
        for g in 0..app.st.grenades {
            draw_poly(w - 34.0 - g as f32 * 22.0, h - 104.0, 6, 8.0, 0.0, col([0.75, 0.8, 0.5], 0.95));
        }
        if app.st.reload > 0 {
            let f = 1.0 - app.st.reload as f32 / wd.reload as f32;
            draw_rectangle(cx - 60.0, cy + 46.0, 120.0, 6.0, col(DARK, 0.7));
            draw_rectangle(cx - 60.0, cy + 46.0, 120.0 * f, 6.0, col(WHITE, 0.9));
            text_c("RELOADING", cx, cy + 72.0, 18.0, col(WHITE, 0.8));
        } else if app.st.ammo == 0 {
            text_c("EMPTY", cx, cy + 72.0, 18.0, Color::new(1.0, 0.35, 0.2, 1.0));
        }
    }

    // ---- lobby, death screen and loadout ----
    let dead = !app.alive && !app.spectator;
    let panel = app.loadout_shown();
    if app.lobby {
        draw_lobby(app, snap, w, h);
    }
    if let Some(k) = app.kc.as_ref().filter(|_| !app.lobby) {
        // Killcam: say whose eyes these are, and how long is left to wait.
        let title = format!("KILLCAM   {}  -  {}", app.name(k.killer), sim::weapon_def(k.weapon).name);
        let tw = crate::ui::width(&title, 26.0);
        draw_rectangle(cx - tw / 2.0 - 24.0, 146.0, tw + 48.0, 40.0, col(DARK, 0.72));
        draw_rectangle(cx - tw / 2.0 - 24.0, 146.0, 6.0, 40.0, Color::new(1.0, 0.25, 0.15, 1.0));
        text_c(&title, cx, 175.0, 26.0, col(team_col(app.team(k.killer)), 1.0));
        if let Some(t) = app.icon(k.killer) {
            pic(t, cx + tw / 2.0 + 30.0, 154.0, 24.0, 1.0);
        }
        let rs = if app.respawn_in > 0.05 { format!("Reinforcing in {:.1}s", app.respawn_in) } else { "Deploying...".to_string() };
        let line = format!("{rs}      SPACE or click to skip");
        let lw = crate::ui::width(&line, 20.0);
        draw_rectangle(cx - lw / 2.0 - 16.0, h - 92.0, lw + 32.0, 32.0, col(DARK, 0.6));
        text_c(&line, cx, h - 70.0, 20.0, col(WHITE, 0.9));
    }
    if (dead && app.kc.is_none()) || panel {
        let y0 = loadout_top(h);
        if app.lobby {
            if panel {
                text_c("YOUR LOADOUT", cx, y0 + 64.0, 26.0, col(WHITE, 0.9));
            }
        } else {
            draw_rectangle(0.0, y0, w, if panel { LOADOUT_H } else { 86.0 }, col(DARK, if panel { 0.86 } else { 0.68 }));
            if dead {
                let who = if app.killer == SPECTATOR_ID || app.killer == app.my_id {
                    "DOWN".to_string()
                } else {
                    format!("ELIMINATED BY {}", app.name(app.killer))
                };
                text_c(&who, cx, y0 + 38.0, 34.0, col(WHITE, 1.0));
                let rs = if app.respawn_in > 0.05 { format!("Reinforcing in {:.1}s", app.respawn_in) } else { "Deploying...".to_string() };
                text_c(&rs, cx, y0 + 66.0, 22.0, col(WHITE, 0.8));
                if app.spec_killer {
                    let hp = app.views.iter().find(|v| v.id == app.killer).map_or(0, |v| v.hp);
                    let gun = sim::weapon_def(app.wid_of(app.killer, false)).name;
                    let line = format!("SPECTATING  {}  -  {} HP  -  {}", app.name(app.killer), hp, gun);
                    let tw = measure_text(&line, None, 22, 1.0).width;
                    draw_rectangle(cx - tw / 2.0 - 14.0, y0 - 36.0, tw + 28.0, 30.0, col(DARK, 0.6));
                    text_c(&line, cx, y0 - 14.0, 22.0, col(team_col(app.team(app.killer)), 1.0));
                }
            } else {
                text_c("LOADOUT", cx, y0 + 38.0, 34.0, col(WHITE, 1.0));
                text_c("Used the next time you deploy.   L or ESC to close", cx, y0 + 66.0, 20.0, col(WHITE, 0.8));
            }
        }
        if panel {
            draw_loadout(app, w, h, &mut icons);
        }
    }
    if app.spectator {
        text_c(&format!("SPECTATING  {}", app.name(app.spec_target)), cx, h - 30.0, 24.0, col(team_col(app.team(app.spec_target)), 1.0));
    }

    // ---- round over ----
    // The banner sits high so the scoreboard has the rest of the screen.
    let over = snap.phase == 1 && snap.winner != NEUTRAL;
    if over {
        let by = (h * 0.1).max(66.0);
        draw_rectangle(0.0, by, w, 112.0, col(DARK, 0.78));
        let c = team_col(snap.winner);
        text_c(&format!("{} HOLDS THE FIELD", TEAM_NAMES[snap.winner as usize % 2]), cx, by + 52.0, 52.0, col(c, 1.0));
        let res = if app.spectator {
            ""
        } else if snap.winner == app.my_team {
            "VICTORY"
        } else {
            "DEFEAT"
        };
        let mut line = format!("{}   next round in {:.0}s", res, snap.phase_secs);
        if let Some((credits, _, _)) = app.award.filter(|_| !app.spectator) {
            line = format!("{res}   +{credits} CREDITS   next round in {:.0}s", snap.phase_secs);
        }
        if snap.next_map != snap.map {
            line += &format!("   -   next map: {}", ferrite_shared::map::map_def(snap.next_map).name);
        }
        text_c(&line, cx, by + 88.0, 22.0, col(WHITE, 0.9));
    }

    // ---- scoreboard ----
    if (app.action_down(crate::bindings::Action::Scoreboard) && !app.chat_open) || snap.phase == 1 || app.demo_scores {
        icons.clear(); // the scoreboard covers the panel
        let top = if over { (h * 0.1).max(66.0) + 124.0 } else { (h * 0.1).max(66.0) };
        draw_scoreboard(app, snap, w, h, top);
    }

    // ---- team picker / pause / help ----
    if app.team_pick {
        icons.clear();
        draw_team_pick(app, w, h);
    } else if app.paused() {
        icons.clear();
        draw_pause(app, snap, w, h);
    }

    if app.show_net {
        let s = &app.stats;
        let l = format!(
            "{:.0} fps | ping {:.0} ms | corrections {} (max {:.2} m) | {} tris | {} players drawn",
            s.frames as f64 / s.frame_sum.max(0.001),
            app.net.ping_ms,
            s.corrections,
            s.max_err,
            tris,
            app.views.len()
        );
        text(&l, 16.0, h - 78.0, 16.0, col(WHITE, 0.8));
    }
    if now - app.net.last_recv > 2.0 {
        text_c("CONNECTION LOST - waiting for server", cx, cy - 60.0, 30.0, Color::new(1.0, 0.4, 0.2, 1.0));
    }
    draw_chat(app, w, h, now);
    crate::touch::draw();
    icons
}
