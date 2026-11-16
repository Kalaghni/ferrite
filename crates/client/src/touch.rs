//! On-screen controls for touch screens: a thumb stick on the left that
//! appears where the thumb lands, drag-to-look on the right, and a cluster
//! of buttons. Every finger is tracked on its own, so moving, aiming and
//! firing can happen together.
//!
//! On a machine without a touch screen the left mouse button stands in for
//! one finger (`--touch`), which is enough to try the layout: hold FIRE and
//! drag to shoot and aim at once, as a thumb would.

use crate::bindings::Action;
use macroquad::prelude as mq;
use std::cell::RefCell;

/// The buttons, in the order of `LAYOUT`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Btn {
    Fire,
    Aim,
    Jump,
    Crouch,
    Reload,
    Swap,
    Grenade,
    Melee,
    Sight,
    Pause,
    Scores,
    Loadout,
}

#[cfg(test)]
const BTNS: [Btn; 12] = [
    Btn::Fire,
    Btn::Aim,
    Btn::Jump,
    Btn::Crouch,
    Btn::Reload,
    Btn::Swap,
    Btn::Grenade,
    Btn::Melee,
    Btn::Sight,
    Btn::Pause,
    Btn::Scores,
    Btn::Loadout,
];

impl Btn {
    fn bit(self) -> u32 {
        1 << self as u32
    }
    fn label(self) -> &'static str {
        match self {
            Btn::Fire => "FIRE",
            Btn::Aim => "AIM",
            Btn::Jump => "JUMP",
            Btn::Crouch => "CROUCH",
            Btn::Reload => "RELOAD",
            Btn::Swap => "SWAP",
            Btn::Grenade => "FRAG",
            Btn::Melee => "MELEE",
            Btn::Sight => "SIGHT",
            Btn::Pause => "II",
            Btn::Scores => "SCORES",
            Btn::Loadout => "KIT",
        }
    }
    /// Tapped on, tapped off, rather than held.
    fn toggles(self) -> bool {
        matches!(self, Btn::Aim | Btn::Crouch)
    }
}

/// Where each button sits: (button, centre x, centre y, radius), for a
/// screen `w` by `h`. Sizes follow the shorter side, so a phone held
/// sideways and a tablet both get thumb-sized targets.
pub fn layout(w: f32, h: f32) -> [(Btn, f32, f32, f32); 12] {
    let u = (h / 720.0).clamp(0.75, 1.6);
    let (r, b) = (w - 24.0 * u, h - 24.0 * u);
    [
        (Btn::Fire, r - 150.0 * u, b - 150.0 * u, 62.0 * u),
        (Btn::Aim, r - 290.0 * u, b - 96.0 * u, 44.0 * u),
        (Btn::Jump, r - 48.0 * u, b - 250.0 * u, 44.0 * u),
        (Btn::Crouch, r - 48.0 * u, b - 132.0 * u, 40.0 * u),
        (Btn::Reload, r - 262.0 * u, b - 220.0 * u, 38.0 * u),
        (Btn::Swap, r - 166.0 * u, b - 292.0 * u, 38.0 * u),
        (Btn::Grenade, r - 66.0 * u, b - 360.0 * u, 36.0 * u),
        (Btn::Melee, r - 372.0 * u, b - 190.0 * u, 36.0 * u),
        (Btn::Sight, r - 270.0 * u, b - 330.0 * u, 30.0 * u),
        // Top right, above the kill feed.
        (Btn::Pause, r - 30.0 * u, 36.0 * u, 26.0 * u),
        (Btn::Scores, r - 92.0 * u, 36.0 * u, 26.0 * u),
        (Btn::Loadout, r - 154.0 * u, 36.0 * u, 26.0 * u),
    ]
}

/// How far the stick's knob travels from where the thumb came down.
fn stick_reach(h: f32) -> f32 {
    78.0 * (h / 720.0).clamp(0.75, 1.6)
}

/// A finger (or the mouse standing in for one) and what it is doing.
#[derive(Clone, Copy, PartialEq)]
enum Role {
    Stick,
    Look,
    Button(Btn),
    /// It came down on a menu or before play began: leave it alone.
    Ignored,
}

#[derive(Clone, Copy)]
struct Finger {
    id: u64,
    role: Role,
    start: mq::Vec2,
    pos: mq::Vec2,
    seen: bool,
}

#[derive(Default)]
struct State {
    enabled: bool,
    /// A real touch has been seen: the mouse stops standing in for one.
    real: bool,
    fingers: Vec<Finger>,
    movement: (f32, f32),
    look: (f32, f32),
    held: u32,
    pressed: u32,
    toggled: u32,
    playing: bool,
}

thread_local! {
    static TOUCH: RefCell<State> = RefCell::new(State::default());
}

const MOUSE_ID: u64 = u64::MAX;

pub fn set_enabled(on: bool) {
    TOUCH.with(|t| t.borrow_mut().enabled = on);
}

pub fn enabled() -> bool {
    TOUCH.with(|t| t.borrow().enabled)
}

/// Read the fingers for this frame. `playing`: the player is in control
/// of a soldier, so touches drive it; otherwise they are left to the menus
/// (which see them as mouse clicks) and everything is released.
pub fn poll(playing: bool) {
    let (w, h) = (mq::screen_width(), mq::screen_height());
    let real: Vec<(u64, mq::Vec2, bool)> = mq::touches()
        .into_iter()
        .map(|t| (t.id, t.position, matches!(t.phase, mq::TouchPhase::Ended | mq::TouchPhase::Cancelled)))
        .collect();
    TOUCH.with(|slot| {
        let mut t = slot.borrow_mut();
        if !real.is_empty() {
            // A touch screen: turn the controls on by themselves.
            t.real = true;
            t.enabled = true;
        }
        t.look = (0.0, 0.0);
        t.pressed = 0;
        if !t.enabled {
            t.fingers.clear();
            t.held = 0;
            t.movement = (0.0, 0.0);
            return;
        }
        let mut now: Vec<(u64, mq::Vec2)> = real.iter().filter(|r| !r.2).map(|r| (r.0, r.1)).collect();
        if !t.real && mq::is_mouse_button_down(mq::MouseButton::Left) {
            let (x, y) = mq::mouse_position();
            now.push((MOUSE_ID, mq::vec2(x, y)));
        }
        for f in t.fingers.iter_mut() {
            f.seen = false;
        }
        let buttons = layout(w, h);
        let mut look = (0.0, 0.0);
        for (id, pos) in now {
            if let Some(k) = t.fingers.iter().position(|f| f.id == id) {
                let f = &mut t.fingers[k];
                // Looking: the free finger, or the one holding FIRE.
                if matches!(f.role, Role::Look | Role::Button(Btn::Fire)) {
                    look.0 += pos.x - f.pos.x;
                    look.1 += pos.y - f.pos.y;
                }
                f.pos = pos;
                f.seen = true;
                continue;
            }
            let on = buttons.iter().find(|b| (pos.x - b.1).powi(2) + (pos.y - b.2).powi(2) < (b.3 * 1.12).powi(2)).map(|b| b.0);
            let role = match on {
                _ if !playing => Role::Ignored,
                Some(b) => Role::Button(b),
                None if pos.x < w * 0.42 && !t.fingers.iter().any(|f| f.role == Role::Stick) => Role::Stick,
                None => Role::Look,
            };
            if let Role::Button(b) = role {
                t.pressed |= b.bit();
                if b.toggles() {
                    t.toggled ^= b.bit();
                }
            }
            t.fingers.push(Finger { id, role, start: pos, pos, seen: true });
        }
        t.fingers.retain(|f| f.seen);
        if !playing {
            // Whatever was held is let go; the next touch starts afresh.
            for f in t.fingers.iter_mut() {
                f.role = Role::Ignored;
            }
            t.toggled = 0;
        }
        t.playing = playing;
        t.look = look;
        t.held = t.fingers.iter().fold(0, |m, f| if let Role::Button(b) = f.role { m | b.bit() } else { m });
        let reach = stick_reach(h);
        t.movement = t.fingers.iter().find(|f| f.role == Role::Stick).map_or((0.0, 0.0), |f| {
            let d = (f.pos - f.start) / reach;
            let len = d.length();
            // A small dead centre, then all the way out at the rim.
            if len < 0.12 {
                (0.0, 0.0)
            } else {
                let k = ((len - 0.12) / 0.88).min(1.0) / len;
                (d.x * k, -d.y * k)
            }
        });
    });
}

/// (right, forward) from the stick, each -1..1.
pub fn movement() -> (f32, f32) {
    TOUCH.with(|t| t.borrow().movement)
}

/// Pixels the looking finger moved this frame: (right, down).
pub fn look_delta() -> (f32, f32) {
    TOUCH.with(|t| t.borrow().look)
}

fn button_for(action: Action) -> Option<Btn> {
    Some(match action {
        Action::Fire => Btn::Fire,
        Action::Aim => Btn::Aim,
        Action::Jump => Btn::Jump,
        Action::Crouch => Btn::Crouch,
        Action::Reload => Btn::Reload,
        Action::SwapWeapon => Btn::Swap,
        Action::Grenade => Btn::Grenade,
        Action::Melee => Btn::Melee,
        Action::CycleSight => Btn::Sight,
        Action::Scoreboard => Btn::Scores,
        Action::Loadout => Btn::Loadout,
        _ => return None,
    })
}

pub fn down(action: Action) -> bool {
    TOUCH.with(|t| {
        let t = t.borrow();
        if !t.enabled {
            return false;
        }
        // The stick pushed right out is a sprint.
        if action == Action::Sprint {
            let (x, y) = t.movement;
            return y > 0.6 && x * x + y * y > 0.92;
        }
        button_for(action).is_some_and(|b| if b.toggles() { t.toggled & b.bit() != 0 } else { t.held & b.bit() != 0 })
    })
}

pub fn pressed(action: Action) -> bool {
    TOUCH.with(|t| {
        let t = t.borrow();
        t.enabled && button_for(action).is_some_and(|b| t.pressed & b.bit() != 0)
    })
}

pub fn pause_pressed() -> bool {
    TOUCH.with(|t| {
        let t = t.borrow();
        t.enabled && t.pressed & Btn::Pause.bit() != 0
    })
}

/// Draw the controls. Call last, over the HUD, while the player is in
/// control of a soldier.
pub fn draw() {
    let (w, h) = (mq::screen_width(), mq::screen_height());
    TOUCH.with(|slot| {
        let t = slot.borrow();
        if !t.enabled || !t.playing {
            return;
        }
        let ink = |a: f32| mq::Color::new(0.965, 0.953, 0.98, a);
        let dark = |a: f32| mq::Color::new(0.047, 0.063, 0.125, a);
        // The stick: where the thumb is, or a hint of where it could go.
        let reach = stick_reach(h);
        match t.fingers.iter().find(|f| f.role == Role::Stick) {
            Some(f) => {
                let d = f.pos - f.start;
                let knob = f.start + if d.length() > reach { d / d.length() * reach } else { d };
                mq::draw_circle(f.start.x, f.start.y, reach, dark(0.35));
                mq::draw_circle_lines(f.start.x, f.start.y, reach, 3.0, ink(0.55));
                mq::draw_circle(knob.x, knob.y, reach * 0.42, ink(0.55));
            }
            None => {
                let (cx, cy) = (reach + 70.0, h - reach - 86.0);
                mq::draw_circle(cx, cy, reach, dark(0.2));
                mq::draw_circle_lines(cx, cy, reach, 2.0, ink(0.28));
                mq::draw_circle(cx, cy, reach * 0.42, ink(0.16));
                crate::ui::text_c("MOVE", cx, cy + 6.0, 16.0, ink(0.5));
            }
        }
        for (b, x, y, r) in layout(w, h) {
            let on = if b.toggles() { t.toggled & b.bit() != 0 } else { t.held & b.bit() != 0 };
            let fill = match b {
                Btn::Fire => mq::Color::new(1.0, 0.459, 0.298, if on { 0.85 } else { 0.42 }),
                _ if on => mq::Color::new(0.412, 0.678, 1.0, 0.7),
                _ => dark(0.42),
            };
            mq::draw_circle(x, y, r, fill);
            mq::draw_circle_lines(x, y, r, 2.0, ink(if on { 0.95 } else { 0.5 }));
            let size = (r * 0.44).clamp(12.0, 26.0);
            crate::ui::text_c(b.label(), x, y + size * 0.3, size, ink(if on { 1.0 } else { 0.8 }));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_fit_the_screen_and_do_not_overlap() {
        // A phone held sideways, the default window, a tablet, a desktop.
        for (w, h) in [(844.0, 390.0), (1280.0, 720.0), (1024.0, 768.0), (1920.0, 1080.0), (2560.0, 1440.0)] {
            let l = layout(w, h);
            for (k, a) in l.iter().enumerate() {
                assert!(a.1 - a.3 >= 0.0 && a.1 + a.3 <= w && a.2 - a.3 >= 0.0 && a.2 + a.3 <= h, "{:?} is off a {w}x{h} screen", a.0);
                assert!(a.3 >= 19.0, "{:?} is too small to hit on {w}x{h}", a.0);
                for b in &l[k + 1..] {
                    let d = ((a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt();
                    assert!(d >= a.3 + b.3, "{:?} overlaps {:?} on {w}x{h}", a.0, b.0);
                }
                // The left of the screen belongs to the stick.
                if !matches!(a.0, Btn::Pause | Btn::Scores | Btn::Loadout) {
                    assert!(a.1 - a.3 > w * 0.42, "{:?} sits in the stick's half on {w}x{h}", a.0);
                }
            }
        }
        assert_eq!(BTNS.len(), layout(1280.0, 720.0).len());
    }

    #[test]
    fn every_button_has_a_distinct_bit_and_the_fight_actions_are_covered() {
        let mut seen = 0u32;
        for b in BTNS {
            assert_eq!(seen & b.bit(), 0);
            seen |= b.bit();
        }
        for a in
            [Action::Fire, Action::Aim, Action::Jump, Action::Crouch, Action::Reload, Action::SwapWeapon, Action::Grenade, Action::Melee]
        {
            assert!(button_for(a).is_some(), "no touch button for {a:?}");
        }
        assert!(button_for(Action::Forward).is_none());
    }
}
