#[derive(Debug, Copy, Clone, PartialEq, Hash, Eq)]
#[repr(u8)]
pub enum MouseButton {
    Left = 0,
    Middle = 1,
    Right = 2,
    /// First side button (Mouse 4 / Back).
    Extra1 = 3,
    /// Second side button (Mouse 5 / Forward).
    Extra2 = 4,
    Unknown = 255,
}

// FERRITE: keep platform translation testable on every desktop target.
impl MouseButton {
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn from_windows_xbutton(button: u16) -> Self {
        match button {
            1 => Self::Extra1,
            2 => Self::Extra2,
            _ => Self::Unknown,
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn from_x11(button: i32) -> Self {
        match button {
            1 => Self::Left,
            2 => Self::Middle,
            3 => Self::Right,
            8 => Self::Extra1,
            9 => Self::Extra2,
            // 4..7 are scroll directions, not side buttons.
            _ => Self::Unknown,
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn from_evdev(button: u32) -> Self {
        match button {
            272 => Self::Left,
            273 => Self::Right,
            274 => Self::Middle,
            275 | 278 => Self::Extra1, // BTN_SIDE / BTN_BACK
            276 | 277 => Self::Extra2, // BTN_EXTRA / BTN_FORWARD
            _ => Self::Unknown,
        }
    }

    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn from_cocoa_other(button: isize) -> Self {
        match button {
            2 => Self::Middle,
            3 => Self::Extra1,
            4 => Self::Extra2,
            _ => Self::Unknown,
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn from_web_bridge(button: i32) -> Self {
        // gl.js already swaps DOM middle/right into the legacy SAPP order.
        match button {
            0 => Self::Left,
            1 => Self::Right,
            2 => Self::Middle,
            3 => Self::Extra1,
            4 => Self::Extra2,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod mouse_button_tests {
    use super::MouseButton::*;

    #[test]
    fn original_discriminants_remain_stable_and_side_buttons_are_distinct() {
        assert_eq!([Left as u8, Middle as u8, Right as u8, Extra1 as u8, Extra2 as u8, Unknown as u8], [0, 1, 2, 3, 4, 255]);
    }

    #[test]
    fn windows_xbuttons_translate_without_confusing_key_state_flags() {
        assert_eq!(super::MouseButton::from_windows_xbutton(1), Extra1);
        assert_eq!(super::MouseButton::from_windows_xbutton(2), Extra2);
        for unknown in [0, 3, 0x20, 0x40, u16::MAX] {
            assert_eq!(super::MouseButton::from_windows_xbutton(unknown), Unknown);
        }
    }

    #[test]
    fn x11_scroll_buttons_remain_scroll_and_side_buttons_are_forwarded() {
        let expected = [(1, Left), (2, Middle), (3, Right), (8, Extra1), (9, Extra2)];
        for (native, button) in expected { assert_eq!(super::MouseButton::from_x11(native), button); }
        for native in [0, 4, 5, 6, 7, 10, -1] { assert_eq!(super::MouseButton::from_x11(native), Unknown); }
    }

    #[test]
    fn wayland_evdev_side_and_navigation_aliases_are_consistent() {
        for (native, button) in [(272, Left), (273, Right), (274, Middle), (275, Extra1), (276, Extra2), (277, Extra2), (278, Extra1)] {
            assert_eq!(super::MouseButton::from_evdev(native), button);
        }
        assert_eq!(super::MouseButton::from_evdev(279), Unknown);
    }

    #[test]
    fn cocoa_other_buttons_do_not_all_alias_middle() {
        assert_eq!(super::MouseButton::from_cocoa_other(2), Middle);
        assert_eq!(super::MouseButton::from_cocoa_other(3), Extra1);
        assert_eq!(super::MouseButton::from_cocoa_other(4), Extra2);
        for native in [-1, 0, 1, 5] { assert_eq!(super::MouseButton::from_cocoa_other(native), Unknown); }
    }

    #[test]
    fn web_bridge_preserves_existing_buttons_and_passes_side_buttons() {
        for (native, button) in [(0, Left), (1, Right), (2, Middle), (3, Extra1), (4, Extra2)] {
            assert_eq!(super::MouseButton::from_web_bridge(native), button);
        }
        assert_eq!(super::MouseButton::from_web_bridge(5), Unknown);
    }
}
#[derive(Debug, Copy, Clone)]
pub struct Touch {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash, Eq)]
#[repr(u16)]
/// These keycode values are based off of X11's `keysymdef.h`.
/// Missing keycodes from that list are given the prefix 0x01.
pub enum KeyCode {
    Space = 0x0020,
    Apostrophe = 0x0027,
    Comma = 0x002c,
    Minus = 0x002d,
    Period = 0x002e,
    Slash = 0x002f,
    Key0 = 0x0030,
    Key1 = 0x0031,
    Key2 = 0x0032,
    Key3 = 0x0033,
    Key4 = 0x0034,
    Key5 = 0x0035,
    Key6 = 0x0036,
    Key7 = 0x0037,
    Key8 = 0x0038,
    Key9 = 0x0039,
    Semicolon = 0x003b,
    Equal = 0x003d,
    A = 0x0041,
    B = 0x0042,
    C = 0x0043,
    D = 0x0044,
    E = 0x0045,
    F = 0x0046,
    G = 0x0047,
    H = 0x0048,
    I = 0x0049,
    J = 0x004a,
    K = 0x004b,
    L = 0x004c,
    M = 0x004d,
    N = 0x004e,
    O = 0x004f,
    P = 0x0050,
    Q = 0x0051,
    R = 0x0052,
    S = 0x0053,
    T = 0x0054,
    U = 0x0055,
    V = 0x0056,
    W = 0x0057,
    X = 0x0058,
    Y = 0x0059,
    Z = 0x005a,
    LeftBracket = 0x005b,
    Backslash = 0x005c,
    RightBracket = 0x005d,
    GraveAccent = 0x0060,
    World1 = 0x0100,
    World2 = 0x0101,
    Escape = 0xff1b,
    Enter = 0xff0d,
    Tab = 0xff09,
    Backspace = 0xff08,
    Insert = 0xff63,
    Delete = 0xffff,
    Right = 0xff53,
    Left = 0xff51,
    Down = 0xff54,
    Up = 0xff52,
    PageUp = 0xff55,
    PageDown = 0xff56,
    Home = 0xff50,
    End = 0xff57,
    CapsLock = 0xffe5,
    ScrollLock = 0xff14,
    NumLock = 0xff7f,
    PrintScreen = 0xfd1d,
    Pause = 0xff13,
    F1 = 0xffbe,
    F2 = 0xffbf,
    F3 = 0xffc0,
    F4 = 0xffc1,
    F5 = 0xffc2,
    F6 = 0xffc3,
    F7 = 0xffc4,
    F8 = 0xffc5,
    F9 = 0xffc6,
    F10 = 0xffc7,
    F11 = 0xffc8,
    F12 = 0xffc9,
    F13 = 0xffca,
    F14 = 0xffcb,
    F15 = 0xffcc,
    F16 = 0xffcd,
    F17 = 0xffce,
    F18 = 0xffcf,
    F19 = 0xffd0,
    F20 = 0xffd1,
    F21 = 0xffd2,
    F22 = 0xffd3,
    F23 = 0xffd4,
    F24 = 0xffd5,
    F25 = 0xffd6,
    Kp0 = 0xffb0,
    Kp1 = 0xffb1,
    Kp2 = 0xffb2,
    Kp3 = 0xffb3,
    Kp4 = 0xffb4,
    Kp5 = 0xffb5,
    Kp6 = 0xffb6,
    Kp7 = 0xffb7,
    Kp8 = 0xffb8,
    Kp9 = 0xffb9,
    KpDecimal = 0xffae,
    KpDivide = 0xffaf,
    KpMultiply = 0xffaa,
    KpSubtract = 0xffad,
    KpAdd = 0xffab,
    KpEnter = 0xff8d,
    KpEqual = 0xffbd,
    LeftShift = 0xffe1,
    LeftControl = 0xffe3,
    LeftAlt = 0xffe9,
    LeftSuper = 0xffeb,
    RightShift = 0xffe2,
    RightControl = 0xffe4,
    RightAlt = 0xffea,
    RightSuper = 0xffec,
    Menu = 0xff67,
    // Android back button
    Back = 0xff04,
    Unknown = 0x01ff,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct KeyMods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// A trait defining event callbacks.
pub trait EventHandler {
    /// On most platforms update() and draw() are called each frame, sequentially,
    /// draw right after update.
    /// But on Android (and maybe some other platforms in the future) update might
    /// be called without draw.
    /// When the app is in background, Android destroys the rendering surface,
    /// while app is still alive and can do some usefull calculations.
    /// Note that in this case drawing from update may lead to crashes.
    fn update(&mut self);
    fn draw(&mut self);
    fn resize_event(&mut self, _width: f32, _height: f32) {}
    fn mouse_motion_event(&mut self, _x: f32, _y: f32) {}
    fn mouse_wheel_event(&mut self, _x: f32, _y: f32) {}
    fn mouse_button_down_event(&mut self, _button: MouseButton, _x: f32, _y: f32) {}
    fn mouse_button_up_event(&mut self, _button: MouseButton, _x: f32, _y: f32) {}
    fn mouse_leave_event(&mut self) {}
    fn mouse_enter_event(&mut self, _button: MouseButton ,_x: f32, _y: f32 ) {}

    fn char_event(&mut self, _character: char, _keymods: KeyMods, _repeat: bool) {}

    fn key_down_event(&mut self, _keycode: KeyCode, _keymods: KeyMods, _repeat: bool) {}

    /// Note: you are not always guaranteed to receive a key_up event. For example on
    /// Wayland when leaving the focused window, the key_up event will not be caught.
    fn key_up_event(&mut self, _keycode: KeyCode, _keymods: KeyMods) {}

    /// Default implementation emulates mouse clicks
    fn touch_event(&mut self, phase: TouchPhase, _id: u64, x: f32, y: f32) {
        if phase == TouchPhase::Started {
            self.mouse_button_down_event(MouseButton::Left, x, y);
        }

        if phase == TouchPhase::Ended {
            self.mouse_button_up_event(MouseButton::Left, x, y);
        }

        if phase == TouchPhase::Moved {
            self.mouse_motion_event(x, y);
        }
    }

    /// Represents raw hardware mouse motion event
    /// Note that these events are delivered regardless of input focus and not in pixels, but in
    /// hardware units instead. And those units may be different from pixels depending on the target platform
    fn raw_mouse_motion(&mut self, _dx: f32, _dy: f32) {}

    /// Window has been minimized
    /// Right now is only implemented on Android, Windows, OSX, X11 and wasm,
    /// On Andoid window_minimized_event is called on a Pause ndk callback
    /// On X11, OSX, Windows and wasm it will be called on focus change events.
    fn window_minimized_event(&mut self) {}

    /// Window has been restored
    /// Right now is only implemented on Android, X11 and wasm,
    /// On Andoid window_restored_event is called on a Resume ndk callback
    /// On X11 and wasm it will be called on focus change events.
    fn window_restored_event(&mut self) {}

    /// This event is sent when the userclicks the window's close button
    /// or application code calls the ctx.request_quit() function. The event
    /// handler callback code can handle this event by calling
    /// ctx.cancel_quit() to cancel the quit.
    /// If the event is ignored, the application will quit as usual.
    /// On Andoid quit_requested_event is called on a Destroy ndk callback
    fn quit_requested_event(&mut self) {}

    /// A file has been dropped over the application.
    /// Applications can request the number of dropped files with
    /// `ctx.dropped_file_count()`, path of an individual file with
    /// `ctx.dropped_file_path()`, and for wasm targets the file bytes
    /// can be requested with `ctx.dropped_file_bytes()`.
    fn files_dropped_event(&mut self) {}
}
