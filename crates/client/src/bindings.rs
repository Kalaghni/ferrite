//! Configurable keyboard and mouse inputs, independent of chat/UI focus.
//! The caller gates gameplay while typing; text editing and Escape stay fixed.
use macroquad::prelude as mq;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Action {
    Forward,
    Backward,
    StrafeLeft,
    StrafeRight,
    Jump,
    Sprint,
    Crouch,
    Fire,
    Aim,
    Reload,
    Grenade,
    SwapWeapon,
    Melee,
    CycleSight,
    Loadout,
    ChangeTeam,
    Scoreboard,
    ChatAll,
    ChatTeam,
    ClassVanguard,
    ClassBreacher,
    ClassMarksman,
    NextPrimary,
    NextSidearm,
    NextLook,
    PrimaryFinish,
    SidearmFinish,
    ToggleStats,
    SensitivityDown,
    SensitivityUp,
    Cinematic,
    HideHud,
    Screenshot,
}

impl Action {
    pub const ALL: [Self; 33] = [
        Self::Forward,
        Self::Backward,
        Self::StrafeLeft,
        Self::StrafeRight,
        Self::Jump,
        Self::Sprint,
        Self::Crouch,
        Self::Fire,
        Self::Aim,
        Self::Reload,
        Self::Grenade,
        Self::SwapWeapon,
        Self::Melee,
        Self::CycleSight,
        Self::Loadout,
        Self::ChangeTeam,
        Self::Scoreboard,
        Self::ChatAll,
        Self::ChatTeam,
        Self::ClassVanguard,
        Self::ClassBreacher,
        Self::ClassMarksman,
        Self::NextPrimary,
        Self::NextSidearm,
        Self::NextLook,
        Self::PrimaryFinish,
        Self::SidearmFinish,
        Self::ToggleStats,
        Self::SensitivityDown,
        Self::SensitivityUp,
        Self::Cinematic,
        Self::HideHud,
        Self::Screenshot,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Forward => "move_forward",
            Self::Backward => "move_backward",
            Self::StrafeLeft => "strafe_left",
            Self::StrafeRight => "strafe_right",
            Self::Jump => "jump",
            Self::Sprint => "sprint",
            Self::Crouch => "crouch",
            Self::Fire => "fire",
            Self::Aim => "aim",
            Self::Reload => "reload",
            Self::Grenade => "grenade",
            Self::SwapWeapon => "swap_weapon",
            Self::Melee => "melee",
            Self::CycleSight => "cycle_sight",
            Self::Loadout => "loadout",
            Self::ChangeTeam => "change_team",
            Self::Scoreboard => "scoreboard",
            Self::ChatAll => "chat_all",
            Self::ChatTeam => "chat_team",
            Self::ClassVanguard => "class_vanguard",
            Self::ClassBreacher => "class_breacher",
            Self::ClassMarksman => "class_marksman",
            Self::NextPrimary => "next_primary",
            Self::NextSidearm => "next_sidearm",
            Self::NextLook => "next_look",
            Self::PrimaryFinish => "primary_finish",
            Self::SidearmFinish => "sidearm_finish",
            Self::ToggleStats => "toggle_stats",
            Self::SensitivityDown => "sensitivity_down",
            Self::SensitivityUp => "sensitivity_up",
            Self::Cinematic => "cinematic_camera",
            Self::HideHud => "hide_hud",
            Self::Screenshot => "screenshot",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "MOVE FORWARD",
            Self::Backward => "MOVE BACKWARD",
            Self::StrafeLeft => "STRAFE LEFT",
            Self::StrafeRight => "STRAFE RIGHT",
            Self::Jump => "JUMP / CLIMB",
            Self::Sprint => "SPRINT",
            Self::Crouch => "CROUCH / SLIDE",
            Self::Fire => "FIRE",
            Self::Aim => "AIM",
            Self::Reload => "RELOAD",
            Self::Grenade => "GRENADE",
            Self::SwapWeapon => "SWAP WEAPON",
            Self::Melee => "MELEE",
            Self::CycleSight => "CYCLE SIGHT",
            Self::Loadout => "LOADOUT",
            Self::ChangeTeam => "CHANGE TEAM",
            Self::Scoreboard => "SCOREBOARD",
            Self::ChatAll => "CHAT TO ALL",
            Self::ChatTeam => "TEAM CHAT",
            Self::ClassVanguard => "VANGUARD",
            Self::ClassBreacher => "BREACHER",
            Self::ClassMarksman => "MARKSMAN",
            Self::NextPrimary => "NEXT PRIMARY",
            Self::NextSidearm => "NEXT SIDEARM",
            Self::NextLook => "NEXT SOLDIER LOOK",
            Self::PrimaryFinish => "PRIMARY FINISH",
            Self::SidearmFinish => "SIDEARM FINISH",
            Self::ToggleStats => "FPS / PING",
            Self::SensitivityDown => "SENSITIVITY DOWN",
            Self::SensitivityUp => "SENSITIVITY UP",
            Self::Cinematic => "CINEMATIC CAMERA",
            Self::HideHud => "HIDE HUD",
            Self::Screenshot => "SCREENSHOT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Key(mq::KeyCode),
    Mouse(mq::MouseButton),
    Wheel,
}

// Stable textual names instead of platform-dependent key discriminants.
const KEYS: &[mq::KeyCode] = &[
    mq::KeyCode::Space,
    mq::KeyCode::Apostrophe,
    mq::KeyCode::Comma,
    mq::KeyCode::Minus,
    mq::KeyCode::Period,
    mq::KeyCode::Slash,
    mq::KeyCode::Key0,
    mq::KeyCode::Key1,
    mq::KeyCode::Key2,
    mq::KeyCode::Key3,
    mq::KeyCode::Key4,
    mq::KeyCode::Key5,
    mq::KeyCode::Key6,
    mq::KeyCode::Key7,
    mq::KeyCode::Key8,
    mq::KeyCode::Key9,
    mq::KeyCode::Semicolon,
    mq::KeyCode::Equal,
    mq::KeyCode::A,
    mq::KeyCode::B,
    mq::KeyCode::C,
    mq::KeyCode::D,
    mq::KeyCode::E,
    mq::KeyCode::F,
    mq::KeyCode::G,
    mq::KeyCode::H,
    mq::KeyCode::I,
    mq::KeyCode::J,
    mq::KeyCode::K,
    mq::KeyCode::L,
    mq::KeyCode::M,
    mq::KeyCode::N,
    mq::KeyCode::O,
    mq::KeyCode::P,
    mq::KeyCode::Q,
    mq::KeyCode::R,
    mq::KeyCode::S,
    mq::KeyCode::T,
    mq::KeyCode::U,
    mq::KeyCode::V,
    mq::KeyCode::W,
    mq::KeyCode::X,
    mq::KeyCode::Y,
    mq::KeyCode::Z,
    mq::KeyCode::LeftBracket,
    mq::KeyCode::Backslash,
    mq::KeyCode::RightBracket,
    mq::KeyCode::GraveAccent,
    mq::KeyCode::World1,
    mq::KeyCode::World2,
    mq::KeyCode::Enter,
    mq::KeyCode::Tab,
    mq::KeyCode::Insert,
    mq::KeyCode::Right,
    mq::KeyCode::Left,
    mq::KeyCode::Down,
    mq::KeyCode::Up,
    mq::KeyCode::PageUp,
    mq::KeyCode::PageDown,
    mq::KeyCode::Home,
    mq::KeyCode::End,
    mq::KeyCode::CapsLock,
    mq::KeyCode::ScrollLock,
    mq::KeyCode::NumLock,
    mq::KeyCode::PrintScreen,
    mq::KeyCode::Pause,
    mq::KeyCode::F1,
    mq::KeyCode::F2,
    mq::KeyCode::F3,
    mq::KeyCode::F4,
    mq::KeyCode::F5,
    mq::KeyCode::F6,
    mq::KeyCode::F7,
    mq::KeyCode::F8,
    mq::KeyCode::F9,
    mq::KeyCode::F10,
    mq::KeyCode::F11,
    mq::KeyCode::F12,
    mq::KeyCode::F13,
    mq::KeyCode::F14,
    mq::KeyCode::F15,
    mq::KeyCode::F16,
    mq::KeyCode::F17,
    mq::KeyCode::F18,
    mq::KeyCode::F19,
    mq::KeyCode::F20,
    mq::KeyCode::F21,
    mq::KeyCode::F22,
    mq::KeyCode::F23,
    mq::KeyCode::F24,
    mq::KeyCode::F25,
    mq::KeyCode::Kp0,
    mq::KeyCode::Kp1,
    mq::KeyCode::Kp2,
    mq::KeyCode::Kp3,
    mq::KeyCode::Kp4,
    mq::KeyCode::Kp5,
    mq::KeyCode::Kp6,
    mq::KeyCode::Kp7,
    mq::KeyCode::Kp8,
    mq::KeyCode::Kp9,
    mq::KeyCode::KpDecimal,
    mq::KeyCode::KpDivide,
    mq::KeyCode::KpMultiply,
    mq::KeyCode::KpSubtract,
    mq::KeyCode::KpAdd,
    mq::KeyCode::KpEnter,
    mq::KeyCode::KpEqual,
    mq::KeyCode::LeftShift,
    mq::KeyCode::LeftControl,
    mq::KeyCode::LeftAlt,
    mq::KeyCode::RightShift,
    mq::KeyCode::RightControl,
    mq::KeyCode::RightAlt,
    mq::KeyCode::Menu,
    mq::KeyCode::Back,
];

impl Input {
    pub fn id(self) -> String {
        match self {
            Self::Key(k) => format!("key:{k:?}"),
            Self::Mouse(mq::MouseButton::Left) => "mouse:left".into(),
            Self::Mouse(mq::MouseButton::Right) => "mouse:right".into(),
            Self::Mouse(mq::MouseButton::Middle) => "mouse:middle".into(),
            Self::Mouse(mq::MouseButton::Extra1) => "mouse:4".into(),
            Self::Mouse(mq::MouseButton::Extra2) => "mouse:5".into(),
            Self::Mouse(_) => "none".into(),
            Self::Wheel => "wheel".into(),
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "mouse:left" => Some(Self::Mouse(mq::MouseButton::Left)),
            "mouse:right" => Some(Self::Mouse(mq::MouseButton::Right)),
            "mouse:middle" => Some(Self::Mouse(mq::MouseButton::Middle)),
            "mouse:4" => Some(Self::Mouse(mq::MouseButton::Extra1)),
            "mouse:5" => Some(Self::Mouse(mq::MouseButton::Extra2)),
            "wheel" => Some(Self::Wheel),
            _ => {
                let name = s.strip_prefix("key:")?;
                KEYS.iter().copied().find(|key| format!("{key:?}").eq_ignore_ascii_case(name)).map(Self::Key)
            }
        }
    }
    pub fn label(self) -> String {
        match self {
            Self::Mouse(mq::MouseButton::Left) => "MOUSE 1".into(),
            Self::Mouse(mq::MouseButton::Right) => "MOUSE 2".into(),
            Self::Mouse(mq::MouseButton::Middle) => "MOUSE 3".into(),
            Self::Mouse(mq::MouseButton::Extra1) => "MOUSE 4".into(),
            Self::Mouse(mq::MouseButton::Extra2) => "MOUSE 5".into(),
            Self::Mouse(_) => "UNBOUND".into(),
            Self::Wheel => "WHEEL".into(),
            Self::Key(k) => match k {
                mq::KeyCode::LeftControl => "LEFT CTRL".into(),
                mq::KeyCode::RightControl => "RIGHT CTRL".into(),
                mq::KeyCode::LeftShift => "LEFT SHIFT".into(),
                mq::KeyCode::RightShift => "RIGHT SHIFT".into(),
                mq::KeyCode::LeftAlt => "LEFT ALT".into(),
                mq::KeyCode::RightAlt => "RIGHT ALT".into(),
                mq::KeyCode::Minus => "-".into(),
                mq::KeyCode::Equal => "=".into(),
                _ => {
                    let name = format!("{k:?}");
                    name.strip_prefix("Key").unwrap_or(&name).to_uppercase()
                }
            },
        }
    }
    pub fn down(self) -> bool {
        match self {
            Self::Key(k) => mq::is_key_down(k),
            Self::Mouse(b) => mq::is_mouse_button_down(b),
            Self::Wheel => mq::mouse_wheel().1 != 0.0,
        }
    }
    pub fn pressed(self) -> bool {
        match self {
            Self::Key(k) => mq::is_key_pressed(k),
            Self::Mouse(b) => mq::is_mouse_button_pressed(b),
            Self::Wheel => mq::mouse_wheel().1 != 0.0,
        }
    }
    pub fn capture() -> Option<Self> {
        // Stable priority makes simultaneous modifier/key presses predictable.
        let mut keys: Vec<_> = mq::get_keys_pressed().into_iter().filter(|k| KEYS.contains(k)).collect();
        keys.sort_by_key(|k| *k as u16);
        if let Some(key) = keys.first() {
            return Some(Self::Key(*key));
        }
        for button in
            [mq::MouseButton::Left, mq::MouseButton::Right, mq::MouseButton::Middle, mq::MouseButton::Extra1, mq::MouseButton::Extra2]
        {
            if mq::is_mouse_button_pressed(button) {
                return Some(Self::Mouse(button));
            }
        }
        (mq::mouse_wheel().1 != 0.0).then_some(Self::Wheel)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bindings {
    slots: [Vec<Input>; Action::ALL.len()],
}

impl Default for Bindings {
    fn default() -> Self {
        use mq::{KeyCode as K, MouseButton as M};
        use Input::{Key, Mouse, Wheel};
        Self {
            slots: [
                vec![Key(K::W), Key(K::Up)],
                vec![Key(K::S), Key(K::Down)],
                vec![Key(K::A), Key(K::Left)],
                vec![Key(K::D), Key(K::Right)],
                vec![Key(K::Space)],
                vec![Key(K::LeftShift)],
                vec![Key(K::LeftControl), Key(K::C)],
                vec![Mouse(M::Left)],
                vec![Mouse(M::Right)],
                vec![Key(K::R)],
                vec![Key(K::G)],
                vec![Key(K::Q), Key(K::F), Wheel],
                vec![Key(K::V), Mouse(M::Middle)],
                vec![Key(K::T)],
                vec![Key(K::L)],
                vec![Key(K::M)],
                vec![Key(K::Tab)],
                vec![Key(K::Enter), Key(K::KpEnter), Key(K::Y)],
                vec![Key(K::U)],
                vec![Key(K::Key1)],
                vec![Key(K::Key2)],
                vec![Key(K::Key3)],
                vec![Key(K::Key4)],
                vec![Key(K::Key5)],
                vec![Key(K::Key6)],
                vec![Key(K::Key7)],
                vec![Key(K::Key8)],
                vec![Key(K::F3)],
                vec![Key(K::Minus)],
                vec![Key(K::Equal)],
                vec![Key(K::P)],
                vec![Key(K::F1)],
                vec![Key(K::F12)],
            ],
        }
    }
}

impl Bindings {
    fn accepts(action: Action, input: Input) -> bool {
        input != Input::Wheel
            || !matches!(
                action,
                Action::Forward
                    | Action::Backward
                    | Action::StrafeLeft
                    | Action::StrafeRight
                    | Action::Sprint
                    | Action::Crouch
                    | Action::Aim
                    | Action::Scoreboard
            )
    }
    pub fn down(&self, action: Action) -> bool {
        self.slots[action as usize].iter().any(|i| i.down())
    }
    /// As `down` and `pressed`, keys only: with touch controls on, a
    /// finger on the glass is not the mouse button an action is bound to.
    pub fn down_keys(&self, action: Action) -> bool {
        self.slots[action as usize].iter().any(|i| matches!(i, Input::Key(_)) && i.down())
    }
    pub fn pressed_keys(&self, action: Action) -> bool {
        self.slots[action as usize].iter().any(|i| matches!(i, Input::Key(_)) && i.pressed())
    }
    pub fn pressed(&self, action: Action) -> bool {
        self.slots[action as usize].iter().any(|i| i.pressed())
    }
    pub fn inputs(&self, action: Action) -> &[Input] {
        &self.slots[action as usize]
    }
    pub fn display(&self, action: Action) -> String {
        let inputs = self.inputs(action);
        if inputs.is_empty() {
            "UNBOUND".into()
        } else {
            inputs.iter().map(|i| i.label()).collect::<Vec<_>>().join(" / ")
        }
    }
    /// Reject conflicts without mutating either action.
    fn validate(&self, action: Action, input: Input) -> Result<(), String> {
        if Input::parse(&input.id()) != Some(input) {
            return Err("That key is reserved for navigation.".into());
        }
        if !Self::accepts(action, input) {
            return Err("This action needs a held key or mouse button; the wheel is a press input.".into());
        }
        if let Some(other) = Action::ALL.into_iter().find(|&other| other != action && self.inputs(other).contains(&input)) {
            return Err(format!("{} is assigned to {}. Clear that row first.", input.label(), other.label()));
        }
        Ok(())
    }
    pub fn replace(&mut self, action: Action, input: Input) -> Result<(), String> {
        self.validate(action, input)?;
        self.slots[action as usize] = vec![input];
        Ok(())
    }
    /// Add an alternative input, keeping the action's current assignments.
    pub fn add(&mut self, action: Action, input: Input) -> Result<(), String> {
        self.validate(action, input)?;
        let slots = &mut self.slots[action as usize];
        if !slots.contains(&input) {
            if slots.len() >= 8 {
                return Err("An action can have up to 8 bindings. Replace or clear this row first.".into());
            }
            slots.push(input);
        }
        Ok(())
    }
    pub fn clear(&mut self, action: Action) {
        self.slots[action as usize].clear();
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn write_config(&self) -> String {
        Action::ALL
            .into_iter()
            .map(|action| {
                let inputs = self.inputs(action);
                let value = if inputs.is_empty() { "none".into() } else { inputs.iter().map(|i| i.id()).collect::<Vec<_>>().join(",") };
                format!("bind_{}={}\n", action.id(), value)
            })
            .collect()
    }
    /// Read overrides without applying duplicate checks until all rows have
    /// loaded, so a valid swap survives serialization regardless of row order.
    pub fn read_entry(&mut self, key: &str, value: &str) -> bool {
        let Some(id) = key.strip_prefix("bind_") else {
            return false;
        };
        let Some(action) = Action::ALL.into_iter().find(|a| a.id() == id) else {
            return false;
        };
        if value == "none" {
            self.clear(action);
            return true;
        }
        let parsed: Option<Vec<_>> = value.split(',').take(8).map(|s| Input::parse(s.trim())).collect();
        if let Some(mut inputs) = parsed {
            inputs.retain(|input| Self::accepts(action, *input));
            if !inputs.is_empty() {
                self.slots[action as usize] = inputs;
            }
        }
        true
    }
    /// Hand-edited duplicates use the first action in the list. Unknown keys
    /// leave a row's defaults intact instead of disabling an essential action.
    pub fn normalize(&mut self) {
        let mut used = Vec::new();
        for (index, slots) in self.slots.iter_mut().enumerate() {
            slots.retain(|input| {
                if !Self::accepts(Action::ALL[index], *input) || used.contains(input) {
                    false
                } else {
                    used.push(*input);
                    true
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_remaps_preserve_swapped_keys_unbound_rows_and_mouse_inputs() {
        let mut bindings = Bindings::default();
        bindings.clear(Action::Backward);
        bindings.replace(Action::Forward, Input::Key(mq::KeyCode::S)).unwrap();
        bindings.replace(Action::Backward, Input::Key(mq::KeyCode::W)).unwrap();
        bindings.clear(Action::Grenade);
        bindings.clear(Action::Melee);
        bindings.replace(Action::Grenade, Input::Mouse(mq::MouseButton::Middle)).unwrap();
        let mut loaded = Bindings::default();
        for line in bindings.write_config().lines() {
            let (key, value) = line.split_once('=').unwrap();
            loaded.read_entry(key, value);
        }
        loaded.normalize();
        assert_eq!(loaded, bindings);
    }
    #[test]
    fn melee_keeps_keyboard_and_mouse_five_across_restart() {
        let mut bindings = Bindings::default();
        let keyboard = Input::Key(mq::KeyCode::V);
        let mouse_five = Input::Mouse(mq::MouseButton::Extra2);
        bindings.replace(Action::Melee, keyboard).unwrap();
        bindings.add(Action::Melee, mouse_five).unwrap();
        bindings.add(Action::Melee, keyboard).unwrap(); // same-action duplicates are idempotent
        assert_eq!(bindings.inputs(Action::Melee), &[keyboard, mouse_five]);
        assert_eq!(bindings.display(Action::Melee), "V / MOUSE 5");
        let config = bindings.write_config();
        assert!(config.lines().any(|line| line == "bind_melee=key:V,mouse:5"));
        let mut loaded = Bindings::default();
        for line in config.lines() {
            let (key, value) = line.split_once('=').unwrap();
            loaded.read_entry(key, value);
        }
        loaded.normalize();
        assert_eq!(loaded, bindings);
        let before = loaded.clone();
        assert!(loaded.add(Action::Jump, mouse_five).unwrap_err().contains("MELEE"));
        assert!(loaded.add(Action::Melee, Input::Key(mq::KeyCode::Escape)).is_err());
        assert_eq!(loaded, before);
    }

    #[test]
    fn side_mouse_buttons_have_distinct_persistent_ids() {
        for (button, id, label) in [(mq::MouseButton::Extra1, "mouse:4", "MOUSE 4"), (mq::MouseButton::Extra2, "mouse:5", "MOUSE 5")] {
            let input = Input::Mouse(button);
            assert_eq!(input.id(), id);
            assert_eq!(input.label(), label);
            assert_eq!(Input::parse(id), Some(input));
        }
        let mut bindings = Bindings::default();
        bindings.add(Action::Melee, Input::Mouse(mq::MouseButton::Extra1)).unwrap();
        bindings.add(Action::Grenade, Input::Mouse(mq::MouseButton::Extra2)).unwrap();
        assert_ne!(bindings.inputs(Action::Melee).last(), bindings.inputs(Action::Grenade).last());
    }

    #[test]
    fn wheel_bindings_are_reserved_for_press_actions() {
        let mut bindings = Bindings::default();
        bindings.clear(Action::SwapWeapon);
        let before = bindings.clone();
        assert!(bindings.replace(Action::Forward, Input::Wheel).unwrap_err().contains("held"));
        assert_eq!(bindings, before);
        bindings.read_entry("bind_aim", "wheel");
        assert_eq!(bindings.inputs(Action::Aim), before.inputs(Action::Aim));
        bindings.replace(Action::Jump, Input::Wheel).unwrap();
        bindings.clear(Action::Jump);
        bindings.replace(Action::Fire, Input::Wheel).unwrap();
        assert_eq!(bindings.inputs(Action::Fire), &[Input::Wheel]);
    }

    #[test]
    fn conflicting_or_reserved_keys_do_not_silently_remove_another_action() {
        let mut bindings = Bindings::default();
        let before = bindings.clone();
        assert!(bindings.replace(Action::Jump, Input::Key(mq::KeyCode::R)).unwrap_err().contains("RELOAD"));
        assert!(bindings.replace(Action::Jump, Input::Key(mq::KeyCode::Escape)).is_err());
        bindings.read_entry("bind_jump", "key:DoesNotExist");
        assert_eq!(before, bindings);
    }
}
