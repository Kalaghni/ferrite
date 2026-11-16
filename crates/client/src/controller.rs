//! Controller input and a shared pointer for the launcher and in-game menus.
//! Gilrs supplies mapped inputs; one radial deadzone is applied here.
//! All public queries read the frame captured by poll, without consuming edges.

use crate::bindings::Action;
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs, GilrsBuilder};
use macroquad::prelude as mq;
use std::cell::RefCell;

#[derive(Clone)]
pub struct Settings {
    pub enabled: bool,
    /// Right-stick speed in radians per second at full deflection.
    pub sensitivity: f32,
    pub deadzone: f32,
    pub invert_y: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { enabled: true, sensitivity: 2.8, deadzone: 0.18, invert_y: false }
    }
}

const SOUTH: u32 = 1 << 0;
const EAST: u32 = 1 << 1;
const WEST: u32 = 1 << 2;
const NORTH: u32 = 1 << 3;
const LB: u32 = 1 << 4;
const RB: u32 = 1 << 5;
const LT: u32 = 1 << 6;
const RT: u32 = 1 << 7;
const L3: u32 = 1 << 8;
const R3: u32 = 1 << 9;
const SELECT: u32 = 1 << 10;
const START: u32 = 1 << 11;
const UP: u32 = 1 << 12;
const DOWN: u32 = 1 << 13;
const LEFT: u32 = 1 << 14;
const RIGHT: u32 = 1 << 15;

const BUTTONS: [(Button, u32); 16] = [
    (Button::South, SOUTH),
    (Button::East, EAST),
    (Button::West, WEST),
    (Button::North, NORTH),
    (Button::LeftTrigger, LB),
    (Button::RightTrigger, RB),
    (Button::LeftTrigger2, LT),
    (Button::RightTrigger2, RT),
    (Button::LeftThumb, L3),
    (Button::RightThumb, R3),
    (Button::Select, SELECT),
    (Button::Start, START),
    (Button::DPadUp, UP),
    (Button::DPadDown, DOWN),
    (Button::DPadLeft, LEFT),
    (Button::DPadRight, RIGHT),
];

fn button_bit(button: Button) -> u32 {
    BUTTONS.iter().find(|(b, _)| *b == button).map_or(0, |(_, bit)| *bit)
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// Circular deadzone with continuous, full-range output and unit-length diagonals.
fn radial(x: f32, y: f32, deadzone: f32) -> (f32, f32) {
    let x = finite(x, 0.0).clamp(-1.0, 1.0);
    let y = finite(y, 0.0).clamp(-1.0, 1.0);
    let deadzone = finite(deadzone, 0.18).clamp(0.0, 0.8);
    let magnitude = x.hypot(y);
    if magnitude <= deadzone || magnitude < f32::EPSILON {
        (0.0, 0.0)
    } else {
        let scale = (magnitude.min(1.0) - deadzone) / ((1.0 - deadzone) * magnitude);
        (x * scale, y * scale)
    }
}

#[derive(Clone, Copy, Default)]
struct RawPad {
    buttons: u32,
    /// Presses received during this frame, including taps released before polling ended.
    edges: u32,
    left: (f32, f32),
    right: (f32, f32),
    aim: f32,
    fire: f32,
}

#[derive(Default)]
struct Frame {
    held: u32,
    edges: u32,
    movement: (f32, f32),
    look: (f32, f32),
    sprint: bool,
    ui: bool,
}

impl Frame {
    fn update(&mut self, raw: Option<RawPad>, settings: &Settings, dt: f32, ui: bool) {
        let Some(raw) = raw.filter(|_| settings.enabled) else {
            *self = Self::default();
            return;
        };
        let previous = self.held;
        let mut held = raw.buttons & !(LT | RT);
        // Hysteresis prevents analogue trigger noise from generating extra semi-auto shots.
        for (bit, value) in [(LT, raw.aim), (RT, raw.fire)] {
            let value = if raw.buttons & bit != 0 { 1.0 } else { finite(value, 0.0) };
            let threshold = if previous & bit != 0 { 0.22 } else { 0.30 };
            if value > threshold {
                held |= bit;
            }
        }
        self.held = held;
        self.edges = (held & !previous) | (raw.edges & !(LT | RT));
        self.ui = ui;
        let movement = radial(raw.left.0, raw.left.1, settings.deadzone);
        if self.edges & L3 != 0 {
            self.sprint = !self.sprint;
        }
        if ui || movement.0.hypot(movement.1) < 0.15 || held & (EAST | LT) != 0 {
            self.sprint = false;
        }
        self.movement = if ui { (0.0, 0.0) } else { movement };
        let look = radial(raw.right.0, raw.right.1, settings.deadzone);
        let speed = finite(settings.sensitivity, 2.8).clamp(0.1, 8.0) * finite(dt, 0.0).clamp(0.0, 0.1);
        self.look = if ui { (0.0, 0.0) } else { (look.0 * speed, look.1 * speed * if settings.invert_y { -1.0 } else { 1.0 }) };
    }

    fn action(&self, action: Action, edge: bool) -> bool {
        if self.ui && action != Action::Scoreboard {
            return false;
        }
        if matches!(action, Action::Sprint) {
            return if edge { self.edges & L3 != 0 } else { self.sprint };
        }
        // Movement is available as analogue values; directional queries also work.
        if !edge {
            match action {
                Action::Forward => return self.movement.1 > 0.15,
                Action::Backward => return self.movement.1 < -0.15,
                Action::StrafeLeft => return self.movement.0 < -0.15,
                Action::StrafeRight => return self.movement.0 > 0.15,
                _ => {}
            }
        }
        let bit = match action {
            Action::Jump => SOUTH,
            Action::Crouch => EAST,
            Action::Reload => WEST,
            Action::SwapWeapon => NORTH,
            Action::Grenade => LB,
            Action::Melee => RB | R3,
            Action::Aim => LT,
            Action::Fire => RT,
            Action::CycleSight => UP,
            Action::Loadout => DOWN,
            Action::ChangeTeam => LEFT,
            Action::ToggleStats => RIGHT,
            Action::Scoreboard => SELECT,
            _ => 0,
        };
        (if edge { self.edges } else { self.held }) & bit != 0
    }
}

#[derive(Default)]
struct Pointer {
    position: (f32, f32),
    last_mouse: Option<(f32, f32)>,
    active: bool,
    ui: bool,
    /// A moving stick must return to rest after the physical mouse takes over.
    mouse_override: bool,
}

impl Pointer {
    fn update(&mut self, raw: Option<RawPad>, frame: &Frame, settings: &Settings, dt: f32, ui: bool, mouse: (f32, f32), size: (f32, f32)) {
        let moved = self.last_mouse.is_some_and(|last| (last.0 - mouse.0).hypot(last.1 - mouse.1) > 0.25);
        self.last_mouse = Some(mouse);
        if !ui || !settings.enabled || raw.is_none() {
            self.active = false;
            self.ui = ui;
            self.mouse_override = false;
            return;
        }
        let width = size.0.max(1.0);
        let height = size.1.max(1.0);
        let clamp = |p: (f32, f32)| (finite(p.0, width * 0.5).clamp(0.0, width - 1.0), finite(p.1, height * 0.5).clamp(0.0, height - 1.0));
        if !self.ui {
            self.position = clamp(mouse);
        }
        self.ui = true;
        let raw = raw.unwrap_or_default();
        let stick = radial(raw.left.0, raw.left.1, settings.deadzone);
        let fine = radial(raw.right.0, raw.right.1, settings.deadzone);
        let digital = (
            (frame.held & RIGHT != 0) as i32 - (frame.held & LEFT != 0) as i32,
            (frame.held & UP != 0) as i32 - (frame.held & DOWN != 0) as i32,
        );
        let velocity = (stick.0 + fine.0 * 0.35 + digital.0 as f32, stick.1 + fine.1 * 0.35 + digital.1 as f32);
        let magnitude = velocity.0.hypot(velocity.1);
        let click = frame.edges & (SOUTH | WEST) != 0;
        if moved {
            self.position = clamp(mouse);
            self.active = false;
            self.mouse_override = true;
        } else {
            if magnitude < 0.01 {
                self.mouse_override = false;
            }
            if click || magnitude > 0.01 && !self.mouse_override {
                if !self.active {
                    self.position = clamp(mouse);
                }
                self.active = true;
            }
            if self.active {
                let speed = (height * 0.9).clamp(350.0, 1250.0) * finite(dt, 0.0).clamp(0.0, 0.1);
                let scale = speed / magnitude.max(1.0);
                self.position = clamp((self.position.0 + velocity.0 * scale, self.position.1 - velocity.1 * scale));
            }
        }
    }
}

struct Controller {
    backend: Option<Gilrs>,
    error: Option<String>,
    active: Option<GamepadId>,
    name: Option<String>,
    frame: Frame,
    pointer: Pointer,
    enabled: bool,
}

impl Controller {
    fn new() -> Self {
        // Disable the backend's default deadzone to avoid applying two curves.
        let result = GilrsBuilder::new().with_default_filters(false).with_force_feedback(false).build();
        let (backend, error) = match result {
            Ok(backend) => (Some(backend), None),
            Err(error) => (None, Some(error.to_string())),
        };
        Self { backend, error, active: None, name: None, frame: Frame::default(), pointer: Pointer::default(), enabled: true }
    }

    fn sample(&mut self, settings: &Settings) -> Option<RawPad> {
        let backend = self.backend.as_mut()?;
        let mut edges = 0;
        let mut disconnected = false;
        while let Some(event) = backend.next_event() {
            match event.event {
                EventType::Disconnected if self.active == Some(event.id) => {
                    self.active = None;
                    self.name = None;
                    self.frame = Frame::default();
                    edges = 0;
                    disconnected = true;
                }
                EventType::Connected if self.active.is_none() => self.active = Some(event.id),
                EventType::ButtonPressed(button, _) => {
                    if self.active != Some(event.id) {
                        self.active = Some(event.id);
                        self.frame = Frame::default();
                        edges = 0;
                    }
                    edges |= button_bit(button);
                }
                EventType::AxisChanged(axis, value, _) if axis.is_stick() && value.abs() > settings.deadzone.max(0.3) => {
                    if self.active != Some(event.id) {
                        self.active = Some(event.id);
                        self.frame = Frame::default();
                        edges = 0;
                    }
                }
                _ => {}
            }
        }
        backend.inc();
        if disconnected {
            return None;
        }
        if self.active.is_none() {
            self.active = backend.gamepads().next().map(|(id, _)| id);
        }
        let gamepad = backend.gamepad(self.active?);
        if !gamepad.is_connected() {
            self.active = None;
            self.name = None;
            return None;
        }
        self.name = Some(gamepad.name().to_string());
        let mut buttons = 0;
        for (button, bit) in BUTTONS {
            if gamepad.is_pressed(button) {
                buttons |= bit;
            }
        }
        // With default filters disabled, hats can be exposed as axes or buttons.
        let dpad = (gamepad.value(Axis::DPadX), gamepad.value(Axis::DPadY));
        if dpad.0 < -0.5 {
            buttons |= LEFT;
        }
        if dpad.0 > 0.5 {
            buttons |= RIGHT;
        }
        if dpad.1 < -0.5 {
            buttons |= DOWN;
        }
        if dpad.1 > 0.5 {
            buttons |= UP;
        }
        let trigger = |button, axis| gamepad.button_data(button).map_or_else(|| gamepad.value(axis).max(0.0), |data| data.value());
        Some(RawPad {
            buttons,
            edges,
            left: (gamepad.value(Axis::LeftStickX), gamepad.value(Axis::LeftStickY)),
            right: (gamepad.value(Axis::RightStickX), gamepad.value(Axis::RightStickY)),
            aim: trigger(Button::LeftTrigger2, Axis::LeftZ),
            fire: trigger(Button::RightTrigger2, Axis::RightZ),
        })
    }

    fn status(&self) -> String {
        if !self.enabled {
            return "CONTROLLER DISABLED".to_string();
        }
        if let Some(error) = &self.error {
            return format!("CONTROLLER UNAVAILABLE: {error}");
        }
        self.name.as_ref().map_or_else(|| "NO CONTROLLER CONNECTED".to_string(), |name| format!("CONNECTED: {name}"))
    }
}

thread_local! {
    static CONTROLLER: RefCell<Controller> = RefCell::new(Controller::new());
}

/// Stop backend worker threads while the application is still running. On Windows,
/// joining WGI's worker from a TLS destructor during ExitProcess is too late.
pub struct ShutdownGuard;

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        let backend = CONTROLLER.try_with(|slot| slot.borrow_mut().backend.take()).ok().flatten();
        drop(backend);
    }
}

/// Capture one input frame. Call once per rendered frame, in either menu or match.
pub fn poll(settings: &Settings, dt: f32, ui: bool) {
    CONTROLLER.with(|slot| {
        let mut controller = slot.borrow_mut();
        // A disable/enable cycle retries a backend that was unavailable.
        if settings.enabled && !controller.enabled && controller.backend.is_none() {
            *controller = Controller::new();
        }
        controller.enabled = settings.enabled;
        let raw = controller.sample(settings).filter(|_| settings.enabled);
        controller.frame.update(raw, settings, dt, ui);
        let Controller { frame, pointer, .. } = &mut *controller;
        pointer.update(raw, frame, settings, dt, ui, mq::mouse_position(), (mq::screen_width(), mq::screen_height()));
        if ui {
            mq::show_mouse(!pointer.active);
        }
    });
}

/// (right, forward), with a maximum magnitude of one.
pub fn movement() -> (f32, f32) {
    CONTROLLER.with(|slot| slot.borrow().frame.movement)
}

/// (yaw toward the right, pitch upward), in radians for this frame.
pub fn look_delta() -> (f32, f32) {
    CONTROLLER.with(|slot| slot.borrow().frame.look)
}

pub fn down(action: Action) -> bool {
    CONTROLLER.with(|slot| slot.borrow().frame.action(action, false))
}

pub fn pressed(action: Action) -> bool {
    CONTROLLER.with(|slot| slot.borrow().frame.action(action, true))
}

pub fn pause_pressed() -> bool {
    CONTROLLER.with(|slot| slot.borrow().frame.edges & START != 0)
}

pub fn menu_back() -> bool {
    CONTROLLER.with(|slot| {
        let c = slot.borrow();
        c.frame.ui && c.frame.edges & EAST != 0
    })
}

pub fn mouse_position() -> (f32, f32) {
    CONTROLLER.with(|slot| {
        let c = slot.borrow();
        if c.pointer.active {
            c.pointer.position
        } else {
            mq::mouse_position()
        }
    })
}

pub fn left_pressed() -> bool {
    mq::is_mouse_button_pressed(mq::MouseButton::Left)
        || CONTROLLER.with(|slot| {
            let c = slot.borrow();
            c.pointer.active && c.frame.ui && c.frame.edges & SOUTH != 0
        })
}

pub fn right_pressed() -> bool {
    mq::is_mouse_button_pressed(mq::MouseButton::Right)
        || CONTROLLER.with(|slot| {
            let c = slot.borrow();
            c.pointer.active && c.frame.ui && c.frame.edges & WEST != 0
        })
}

pub fn status() -> String {
    CONTROLLER.with(|slot| slot.borrow().status())
}

/// Draw last, after all menu/HUD layers. Native mouse motion restores its own cursor.
pub fn draw_cursor() {
    let position = CONTROLLER.with(|slot| {
        let c = slot.borrow();
        (c.pointer.active && c.pointer.ui).then_some(c.pointer.position)
    });
    if let Some((x, y)) = position {
        mq::show_mouse(false);
        let s = (mq::screen_height() / 900.0).clamp(0.8, 1.6);
        let point = |dx, dy| mq::vec2(x + dx * s, y + dy * s);
        mq::draw_triangle(point(-2.0, -2.0), point(-2.0, 23.0), point(18.0, 14.0), mq::Color::new(0.04, 0.06, 0.12, 0.95));
        mq::draw_triangle(point(0.0, 0.0), point(0.0, 19.0), point(14.0, 13.0), mq::Color::new(0.92, 0.96, 1.0, 1.0));
        mq::draw_line(x + 5.0 * s, y + 13.0 * s, x + 10.0 * s, y + 23.0 * s, 3.0 * s, mq::Color::new(0.41, 0.68, 1.0, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_backend_and_no_device_keep_input_neutral() {
        let mut controller = Controller {
            backend: None,
            error: Some("input service unavailable".to_string()),
            active: None,
            name: None,
            frame: Frame::default(),
            pointer: Pointer::default(),
            enabled: true,
        };
        assert!(controller.sample(&Settings::default()).is_none());
        assert!(controller.status().contains("CONTROLLER UNAVAILABLE"));
        controller.error = None;
        assert_eq!(controller.status(), "NO CONTROLLER CONNECTED");
        controller.enabled = false;
        assert_eq!(controller.status(), "CONTROLLER DISABLED");
        assert!(!controller.frame.action(Action::Fire, false));
        assert_eq!(controller.frame.look, (0.0, 0.0));
    }

    #[test]
    fn radial_deadzone_is_continuous_circular_and_clamped() {
        assert_eq!(radial(0.1, 0.1, 0.18), (0.0, 0.0));
        assert_eq!(radial(0.18, 0.0, 0.18), (0.0, 0.0));
        assert!(radial(0.181, 0.0, 0.18).0 < 0.002);
        assert!((radial(0.59, 0.0, 0.18).0 - 0.5).abs() < 0.0001);
        assert_eq!(radial(1.0, 0.0, 0.18), (1.0, 0.0));
        let diagonal = radial(1.0, 1.0, 0.18);
        assert!((diagonal.0.hypot(diagonal.1) - 1.0).abs() < 0.0001);
        assert_eq!(radial(f32::NAN, f32::INFINITY, f32::NAN), (0.0, 0.0));
    }

    #[test]
    fn buttons_have_single_edges_and_disconnect_releases_everything() {
        let settings = Settings::default();
        let mut frame = Frame::default();
        let raw = RawPad { buttons: SOUTH | SELECT, fire: 0.8, left: (0.4, 0.8), ..RawPad::default() };
        frame.update(Some(raw), &settings, 0.016, false);
        assert!(frame.action(Action::Jump, true) && frame.action(Action::Fire, true));
        frame.update(Some(raw), &settings, 0.016, false);
        assert!(frame.action(Action::Jump, false) && frame.action(Action::Scoreboard, false));
        assert!(!frame.action(Action::Jump, true) && !frame.action(Action::Fire, true));
        frame.update(None, &settings, 0.016, false);
        assert_eq!(frame.held | frame.edges, 0);
        assert_eq!(frame.movement, (0.0, 0.0));
        frame.update(Some(raw), &settings, 0.016, false);
        assert!(frame.action(Action::Jump, true), "a reconnected device starts fresh");
        frame.update(Some(RawPad { edges: WEST, ..RawPad::default() }), &settings, 0.016, false);
        assert!(frame.action(Action::Reload, true), "a short tap must survive the polling frame");
        assert!(!frame.action(Action::Reload, false));
    }

    #[test]
    fn menus_allow_scoreboard_but_block_gameplay_actions() {
        let mut frame = Frame::default();
        frame.update(Some(RawPad { buttons: SELECT | SOUTH | RT, ..RawPad::default() }), &Settings::default(), 0.016, true);
        assert!(frame.action(Action::Scoreboard, false));
        assert!(!frame.action(Action::Jump, false));
        assert!(!frame.action(Action::Fire, false));
    }

    #[test]
    fn trigger_hysteresis_prevents_repeated_fire_edges() {
        let mut frame = Frame::default();
        for (value, held, edge) in
            [(0.29, false, false), (0.31, true, true), (0.27, true, false), (0.23, true, false), (0.21, false, false), (0.31, true, true)]
        {
            frame.update(Some(RawPad { fire: value, ..RawPad::default() }), &Settings::default(), 0.016, false);
            assert_eq!(frame.action(Action::Fire, false), held);
            assert_eq!(frame.action(Action::Fire, true), edge);
        }
    }

    #[test]
    fn look_uses_elapsed_time_and_inversion_and_disabled_input_is_neutral() {
        let mut frame = Frame::default();
        let raw = RawPad { buttons: SOUTH, right: (0.0, 1.0), left: (1.0, 0.0), ..RawPad::default() };
        let mut settings = Settings::default();
        frame.update(Some(raw), &settings, 0.02, false);
        assert!((frame.look.1 - 0.056).abs() < 0.0001);
        settings.invert_y = true;
        frame.update(Some(raw), &settings, 0.02, false);
        assert!((frame.look.1 + 0.056).abs() < 0.0001);
        settings.enabled = false;
        frame.update(Some(raw), &settings, 0.02, false);
        assert_eq!(frame.held | frame.edges, 0);
        assert_eq!(frame.look, (0.0, 0.0));
        assert_eq!(frame.movement, (0.0, 0.0));
        frame.update(None, &Settings::default(), 0.02, false);
        assert!(!frame.action(Action::Jump, false), "no device leaves keyboard/mouse control alone");
    }

    #[test]
    fn sprint_toggles_and_clears_on_idle_aim_or_crouch() {
        let settings = Settings::default();
        let mut frame = Frame::default();
        let moving = RawPad { left: (0.0, 1.0), ..RawPad::default() };
        frame.update(Some(RawPad { buttons: L3, ..moving }), &settings, 0.016, false);
        assert!(frame.action(Action::Sprint, false));
        frame.update(Some(moving), &settings, 0.016, false);
        assert!(frame.action(Action::Sprint, false));
        frame.update(Some(RawPad { aim: 1.0, ..moving }), &settings, 0.016, false);
        assert!(!frame.sprint);
        frame.update(Some(RawPad { buttons: L3, ..moving }), &settings, 0.016, false);
        assert!(frame.sprint);
        frame.update(Some(RawPad { buttons: EAST, ..moving }), &settings, 0.016, false);
        assert!(!frame.sprint);
        frame.update(Some(RawPad { buttons: L3, ..moving }), &settings, 0.016, false);
        frame.update(Some(RawPad::default()), &settings, 0.016, false);
        assert!(!frame.sprint);
    }

    #[test]
    fn ui_pointer_is_bounded_and_mouse_takes_over_until_stick_returns_to_idle() {
        let settings = Settings::default();
        let raw = RawPad { left: (1.0, 1.0), ..RawPad::default() };
        let mut frame = Frame::default();
        let mut pointer = Pointer::default();
        frame.update(Some(raw), &settings, 0.1, true);
        pointer.update(Some(raw), &frame, &settings, 0.1, true, (790.0, 10.0), (800.0, 600.0));
        assert!(pointer.active);
        assert!(pointer.position.0 <= 799.0 && pointer.position.1 >= 0.0);
        assert_eq!(frame.movement, (0.0, 0.0));
        assert!(!frame.action(Action::Jump, false));
        pointer.update(Some(raw), &frame, &settings, 0.1, true, (400.0, 300.0), (800.0, 600.0));
        assert!(!pointer.active);
        pointer.update(Some(raw), &frame, &settings, 0.1, true, (400.0, 300.0), (800.0, 600.0));
        assert!(!pointer.active, "held stick cannot steal the physical mouse");
        pointer.update(Some(RawPad::default()), &frame, &settings, 0.1, true, (400.0, 300.0), (800.0, 600.0));
        pointer.update(Some(raw), &frame, &settings, 0.1, true, (400.0, 300.0), (800.0, 600.0));
        assert!(pointer.active);
        pointer.update(None, &Frame::default(), &settings, 0.1, true, (400.0, 300.0), (800.0, 600.0));
        assert!(!pointer.active);
    }
}
