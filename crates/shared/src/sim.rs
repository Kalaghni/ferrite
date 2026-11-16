//! Deterministic per-command player simulation: movement, collision and
//! weapon timing. `step` is the single source of truth - the server runs it
//! on arrival of each command, the client runs it for prediction and again
//! when replaying unacknowledged commands after a snapshot.

use crate::map::{Block, Ladder, World, HALF_X, HALF_Z};
use crate::proto::{btn, btn2, Cmd};
use crate::rng::Rng;
use crate::DT;
use glam::{vec3, Vec3};
use serde::{Deserialize, Serialize};

pub const EYE: f32 = 1.62;
pub const P_R: f32 = 0.35;
pub const P_H: f32 = 1.8;
pub const EYE_CROUCH: f32 = 1.02;
pub const P_H_CROUCH: f32 = 1.2;

pub fn eye_h(crouch: bool) -> f32 {
    if crouch {
        EYE_CROUCH
    } else {
        EYE
    }
}

pub fn height_of(crouch: bool) -> f32 {
    if crouch {
        P_H_CROUCH
    } else {
        P_H
    }
}
pub const WALK: f32 = 5.6;
pub const SPRINT_MULT: f32 = 1.5;
pub const JUMP_V: f32 = 7.6;
pub const GRAVITY: f32 = 24.0;
pub const STEP_H: f32 = 0.5;
/// How far above your feet a ledge can be and still be climbed.
pub const MANTLE_REACH: f32 = 1.05;
pub const MANTLE_V: f32 = 4.6;
/// Ladders: climbing and descending speed, and the speed of a slide down
/// with crouch held.
pub const CLIMB_SPEED: f32 = 3.0;
pub const CLIMB_SLIDE: f32 = 8.0;
/// A climber's centre may be this far either side of a ladder's middle,
/// and this far out from its rungs, and still take hold.
pub const LADDER_GRIP: f32 = 0.45;
pub const LADDER_REACH: f32 = 0.9;
/// To take hold you must be looking within about 70 degrees of the rungs.
pub const LADDER_LOOK: f32 = 0.35;
/// A climber is drawn onto the ladder's centre line, just off the rungs,
/// at up to this speed.
const CLIMB_PULL: f32 = 3.0;
const CLIMB_HOLD: f32 = P_R + 0.02;
/// Over the top, a climber steps this far past the rungs onto the deck.
const CLIMB_STEP: f32 = 0.1;
/// Letting go (jump) pushes you gently back off the rungs.
const LET_GO: f32 = 1.2;
pub const SLIDE_TICKS: u8 = 42;
pub const HEAT_MAX: u8 = 100;
pub const GRENADE_WEAPON: u8 = 3;
/// Weapon id a melee kill is reported under.
pub const MELEE_WEAPON: u8 = 17;
/// Kill-feed id for burning to death in lava. Not an entry in `WEAPONS`.
pub const LAVA_WEAPON: u8 = 254;
/// Kill-feed id for being swept away by flood water.
pub const WATER_WEAPON: u8 = 253;

/// What the kill feed calls a weapon id, including environmental deaths.
pub fn weapon_name(wid: u8) -> &'static str {
    if wid == LAVA_WEAPON {
        "Lava"
    } else if wid == WATER_WEAPON {
        "Flood"
    } else {
        weapon_def(wid).name
    }
}
/// A swing: how long before another (or a shot), how far it reaches from
/// the eye, and what it does from the front. From behind it kills.
pub const MELEE_TICKS: u8 = 34;
pub const MELEE_REACH: f32 = 2.4;
pub const MELEE_DAMAGE: f32 = 75.0;
pub const MELEE_BACKSTAB: f32 = 250.0;

pub struct WeaponDef {
    pub name: &'static str,
    pub blurb: &'static str,
    /// Family the weapon handles and looks like: 0 rifle, 1 shotgun,
    /// 2 long rifle, 4 handgun.
    pub arch: u8,
    pub damage: f32,
    pub pellets: u32,
    pub interval: u8,
    pub mag: u8,
    pub reload: u8,
    pub spread_hip: f32,
    pub spread_ads: f32,
    pub head_mult: f32,
    pub falloff: (f32, f32, f32),
    pub recoil: f32,
    pub ads_fov: f32,
    /// Semi-automatic: one shot per trigger pull.
    pub semi: bool,
    /// Heat added per shot; sustained fire opens the cone.
    pub heat: u8,
    /// Extra cone half-angle at full heat.
    pub bloom: f32,
    /// Movement speed multiplier while it is in hand.
    pub mobility: f32,
    /// Rounds fired per trigger pull (0 = not a burst weapon), and the
    /// pause after a burst.
    pub burst: u8,
    pub burst_gap: u8,
}

pub struct KitDef {
    pub name: &'static str,
    pub blurb: &'static str,
    /// Primaries this class may carry; the first is the default.
    pub prims: &'static [u8],
    pub hp: f32,
    pub grenades: u8,
    pub speed: f32,
}

/// Looks a soldier can wear: name and a line about it. Purely cosmetic;
/// the team colour always stays on the torso.
pub static SKINS: [(&str, &str); 12] = [
    ("REGULAR", "Standard issue helmet and plate"),
    ("SCOUT", "Soft cap, goggles and a light rig"),
    ("HEAVY", "Full-face helmet, shoulder plates"),
    ("GHOST", "Hood and mask, dark fatigues"),
    ("VETERAN", "Beret, rolled sleeves, bandolier"),
    ("HAZARD", "Hard hat and striped work gear"),
    // Armory looks: the six shapes above in rarer colours.
    ("BLACKOUT", "Regular plate in matt black with white marks"),
    ("PATHFINDER", "Scout rig in desert tan and rust"),
    ("JUGGERNAUT", "Heavy plate in polished gunmetal and gold"),
    ("WRAITH", "Ghost hood in bone white and ash"),
    ("WARLORD", "Veteran's beret, crimson cloth, brass plate"),
    ("MELTDOWN", "Hazard gear in reactor green and black"),
];

/// What each look costs in the armory. 0 = standard issue, owned by all.
pub static SKIN_PRICES: [u32; 12] = [0, 0, 0, 0, 0, 0, 700, 700, 1100, 1100, 1500, 1500];

/// What each weapon finish costs in the armory, by finish id.
pub static WEAPON_SKIN_PRICES: [u32; 14] = [0, 0, 0, 0, 0, 0, 0, 350, 350, 550, 550, 850, 850, 1400];

/// Credits a new player starts with.
pub const WELCOME_CREDITS: u32 = 400;

pub fn skin_price(id: u8) -> u32 {
    SKIN_PRICES[id as usize % SKIN_PRICES.len()]
}

pub fn weapon_skin_price(id: u8) -> u32 {
    WEAPON_SKIN_PRICES[id as usize % WEAPON_SKIN_PRICES.len()]
}

pub static KITS: [KitDef; 3] = [
    KitDef { name: "VANGUARD", blurb: "100 HP - 2 frags - line fighter", prims: &[0, 12, 8, 6], hp: 100.0, grenades: 2, speed: 1.0 },
    KitDef {
        name: "BREACHER",
        blurb: "130 HP - 1 frag - faster - clears points",
        prims: &[1, 7, 13, 14, 8],
        hp: 130.0,
        grenades: 1,
        speed: 1.05,
    },
    KitDef {
        name: "MARKSMAN",
        blurb: "90 HP - 1 frag - its rifles mark targets",
        prims: &[2, 9, 15, 5],
        hp: 90.0,
        grenades: 1,
        speed: 1.0,
    },
];

/// Procedural paint treatment on a weapon's receiver and furniture.
/// Geometry follows the weapon in first person, world and loadout views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponPattern {
    Plain,
    /// Square pixels in two inks.
    Digital,
    /// Green streaks running down from the top edge, crust along the bottom.
    Patina,
    /// Rounded leaf blots in three colours.
    Blotch,
    /// Sharp angular shards with short rain marks.
    Splinter,
    /// Warning chevrons pointing at the muzzle.
    Chevron,
    /// Tapered stripes reaching in from the top and bottom edges.
    Tiger,
    /// Dark plates with glowing cracks between them.
    Magma,
    /// Wavy light and dark layers of forged steel.
    Damascus,
    /// Two-by-two twill of carbon fibre.
    Weave,
    /// Honeycomb plates over a glowing backing.
    Hex,
    /// Engraved border lines and pearl panels.
    Inlay,
    /// Faceted ice in several tints.
    Shards,
    /// Engraved scrolling vine inside a border line.
    Scroll,
}

/// A finish for a weapon: colours that replace the gun's own. `None`
/// leaves that part as the weapon comes. Purely cosmetic.
pub struct WeaponSkinDef {
    pub name: &'static str,
    pub blurb: &'static str,
    /// Weapon ids it fits; empty = every weapon.
    pub weapons: &'static [u8],
    /// Receiver and barrel.
    pub body: Option<[f32; 3]>,
    /// Stock, grip and handguard (wood or polymer).
    pub furniture: Option<[f32; 3]>,
    /// The strip that normally carries the team colour.
    pub accent: Option<[f32; 3]>,
    /// Small dark parts: sights, trigger, magazine.
    pub detail: Option<[f32; 3]>,
    /// Paint pattern on solid panels, clear of the bore and sight picture.
    pub pattern: WeaponPattern,
    /// Main pattern colour; `None` leaves the finish unpatterned.
    pub pattern_color: Option<[f32; 3]>,
    /// Second pattern colour, for patterns that use two.
    pub pattern_alt: Option<[f32; 3]>,
    /// Chamfered edges of the receiver: polished or worn-through metal.
    pub edge: Option<[f32; 3]>,
    /// Small metal hardware: charging handle, bolt, muzzle, trigger guard,
    /// magazine base. `None` = the body colour.
    pub hardware: Option<[f32; 3]>,
    /// 0 = flat paint or polymer, 1 = polished metal (lighter towards the
    /// top of each part, darker underneath).
    pub sheen: f32,
    /// How strongly the pattern colours and the accent glow, 0..1.
    pub glow: f32,
}

/// Every finish. Index 0 is the gun as it comes and fits everything; add
/// more below it and they appear in the loadout panel for the weapons
/// they list. Ids travel over the wire, so only ever append.
pub static WEAPON_SKINS: [WeaponSkinDef; 14] = [
    WeaponSkinDef {
        name: "FACTORY",
        blurb: "As issued",
        weapons: &[],
        body: None,
        furniture: None,
        accent: None,
        detail: None,
        pattern: WeaponPattern::Plain,
        pattern_color: None,
        pattern_alt: None,
        edge: None,
        hardware: None,
        sheen: 0.0,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "OXIDE",
        blurb: "Weathered copper streaked with green verdigris",
        weapons: &[],
        body: Some([0.50, 0.24, 0.12]),
        furniture: Some([0.27, 0.12, 0.07]),
        accent: Some([0.90, 0.55, 0.30]),
        detail: Some([0.09, 0.08, 0.075]),
        pattern: WeaponPattern::Patina,
        pattern_color: Some([0.27, 0.56, 0.47]),
        pattern_alt: Some([0.17, 0.08, 0.05]),
        edge: Some([0.95, 0.62, 0.40]),
        hardware: None,
        sheen: 0.6,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "ARCTIC",
        blurb: "Snow, ice grey and slate digital pixels",
        weapons: &[],
        body: Some([0.80, 0.83, 0.82]),
        furniture: Some([0.30, 0.34, 0.37]),
        accent: Some([0.43, 0.61, 0.66]),
        detail: Some([0.10, 0.13, 0.15]),
        pattern: WeaponPattern::Digital,
        pattern_color: Some([0.33, 0.39, 0.42]),
        pattern_alt: Some([0.58, 0.64, 0.66]),
        edge: None,
        hardware: None,
        sheen: 0.0,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "WOODLAND",
        blurb: "Olive, brown and black leaf blots",
        weapons: &[],
        body: Some([0.29, 0.35, 0.20]),
        furniture: Some([0.22, 0.20, 0.15]),
        accent: Some([0.61, 0.55, 0.31]),
        detail: Some([0.10, 0.12, 0.09]),
        pattern: WeaponPattern::Blotch,
        pattern_color: Some([0.36, 0.25, 0.15]),
        pattern_alt: Some([0.07, 0.075, 0.06]),
        edge: None,
        hardware: None,
        sheen: 0.0,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "MIDNIGHT",
        blurb: "Navy and black splinter camouflage",
        weapons: &[],
        body: Some([0.13, 0.19, 0.30]),
        furniture: Some([0.09, 0.10, 0.13]),
        accent: Some([0.30, 0.50, 0.64]),
        detail: Some([0.055, 0.065, 0.085]),
        pattern: WeaponPattern::Splinter,
        pattern_color: Some([0.04, 0.05, 0.08]),
        pattern_alt: Some([0.29, 0.38, 0.50]),
        edge: None,
        hardware: None,
        sheen: 0.0,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "HAZARD",
        blurb: "Warning chevrons, worn to bare steel at the edges",
        weapons: &[],
        body: Some([0.74, 0.52, 0.08]),
        furniture: Some([0.13, 0.14, 0.13]),
        accent: Some([0.94, 0.72, 0.19]),
        detail: Some([0.065, 0.07, 0.07]),
        pattern: WeaponPattern::Chevron,
        pattern_color: Some([0.09, 0.095, 0.09]),
        pattern_alt: Some([0.80, 0.58, 0.10]),
        edge: Some([0.56, 0.57, 0.56]),
        hardware: Some([0.13, 0.14, 0.13]),
        sheen: 0.0,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "SANDSTORM",
        blurb: "Desert tiger stripes in sand and umber",
        weapons: &[],
        body: Some([0.70, 0.60, 0.43]),
        furniture: Some([0.40, 0.33, 0.24]),
        accent: Some([0.86, 0.78, 0.58]),
        detail: Some([0.12, 0.11, 0.095]),
        pattern: WeaponPattern::Tiger,
        pattern_color: Some([0.28, 0.19, 0.12]),
        pattern_alt: Some([0.50, 0.38, 0.25]),
        edge: None,
        hardware: None,
        sheen: 0.0,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "EMBERFALL",
        blurb: "Cooling basalt split by glowing magma",
        weapons: &[],
        body: Some([0.11, 0.085, 0.075]),
        furniture: Some([0.085, 0.07, 0.065]),
        accent: Some([1.0, 0.46, 0.12]),
        detail: Some([0.07, 0.06, 0.06]),
        pattern: WeaponPattern::Magma,
        pattern_color: Some([1.0, 0.40, 0.07]),
        pattern_alt: Some([0.62, 0.12, 0.03]),
        edge: None,
        hardware: Some([0.55, 0.15, 0.06]),
        sheen: 0.0,
        glow: 0.8,
    },
    WeaponSkinDef {
        name: "COBALT STEEL",
        blurb: "Blued Damascus steel with cobalt hardware",
        weapons: &[],
        body: Some([0.17, 0.23, 0.35]),
        furniture: Some([0.10, 0.17, 0.36]),
        accent: Some([0.2, 0.5, 1.0]),
        detail: Some([0.05, 0.06, 0.09]),
        pattern: WeaponPattern::Damascus,
        pattern_color: Some([0.42, 0.52, 0.66]),
        pattern_alt: Some([0.07, 0.10, 0.18]),
        edge: Some([0.66, 0.74, 0.86]),
        hardware: Some([0.14, 0.40, 0.95]),
        sheen: 0.7,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "CARBON",
        blurb: "Carbon fibre twill with red anodised parts",
        weapons: &[],
        body: Some([0.09, 0.095, 0.10]),
        furniture: Some([0.06, 0.06, 0.065]),
        accent: Some([0.82, 0.14, 0.12]),
        detail: Some([0.04, 0.04, 0.05]),
        pattern: WeaponPattern::Weave,
        pattern_color: Some([0.31, 0.32, 0.34]),
        pattern_alt: Some([0.19, 0.195, 0.21]),
        edge: None,
        hardware: Some([0.78, 0.12, 0.10]),
        sheen: 0.5,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "TOXIC",
        blurb: "Black honeycomb over glowing acid",
        weapons: &[],
        body: Some([0.07, 0.085, 0.07]),
        furniture: Some([0.06, 0.065, 0.06]),
        accent: Some([0.62, 0.95, 0.16]),
        detail: Some([0.05, 0.06, 0.05]),
        pattern: WeaponPattern::Hex,
        pattern_color: Some([0.55, 1.0, 0.12]),
        pattern_alt: Some([0.18, 0.30, 0.08]),
        edge: None,
        hardware: None,
        sheen: 0.0,
        glow: 0.8,
    },
    WeaponSkinDef {
        name: "ROSE GOLD",
        blurb: "Polished rose gold, engraved, with pearl inlays",
        weapons: &[],
        body: Some([0.80, 0.50, 0.42]),
        furniture: Some([0.86, 0.83, 0.78]),
        accent: Some([0.97, 0.8, 0.72]),
        detail: Some([0.14, 0.1, 0.1]),
        pattern: WeaponPattern::Inlay,
        pattern_color: Some([0.36, 0.19, 0.16]),
        pattern_alt: Some([0.93, 0.91, 0.87]),
        edge: Some([1.0, 0.82, 0.72]),
        hardware: Some([0.92, 0.66, 0.56]),
        sheen: 1.0,
        glow: 0.0,
    },
    WeaponSkinDef {
        name: "GLACIER",
        blurb: "Faceted glacial ice with frosted edges",
        weapons: &[],
        body: Some([0.52, 0.76, 0.88]),
        furniture: Some([0.18, 0.27, 0.34]),
        accent: Some([0.85, 0.96, 1.0]),
        detail: Some([0.08, 0.12, 0.16]),
        pattern: WeaponPattern::Shards,
        pattern_color: Some([0.90, 0.97, 1.0]),
        pattern_alt: Some([0.24, 0.48, 0.68]),
        edge: Some([0.94, 0.98, 1.0]),
        hardware: Some([0.20, 0.42, 0.62]),
        sheen: 0.6,
        glow: 0.12,
    },
    WeaponSkinDef {
        name: "GILDED",
        blurb: "Black lacquer with gold scrollwork and gold-plated parts",
        weapons: &[],
        body: Some([0.05, 0.05, 0.06]),
        furniture: Some([0.09, 0.06, 0.045]),
        accent: Some([1.0, 0.8, 0.25]),
        detail: Some([0.05, 0.05, 0.05]),
        pattern: WeaponPattern::Scroll,
        pattern_color: Some([1.0, 0.76, 0.24]),
        pattern_alt: Some([0.72, 0.50, 0.14]),
        edge: Some([0.95, 0.74, 0.26]),
        hardware: Some([1.0, 0.78, 0.28]),
        sheen: 0.8,
        glow: 0.1,
    },
];

pub fn weapon_skin(id: u8) -> &'static WeaponSkinDef {
    &WEAPON_SKINS[id as usize % WEAPON_SKINS.len()]
}

/// Finishes weapon `wid` can wear, the factory one first.
pub fn weapon_skins_for(wid: u8) -> Vec<u8> {
    (0..WEAPON_SKINS.len() as u8)
        .filter(|&s| s == 0 || WEAPON_SKINS[s as usize].weapons.is_empty() || WEAPON_SKINS[s as usize].weapons.contains(&wid))
        .collect()
}

/// Clamp a requested finish to one the weapon can wear.
pub fn fix_weapon_skin(wid: u8, skin: u8) -> u8 {
    if weapon_skins_for(wid).contains(&skin) {
        skin
    } else {
        0
    }
}

/// Default sidearm. Weapon ids index `WEAPONS`; 3 is the frag grenade and
/// 17 the melee strike, neither of which is carried as a gun.
pub const SIDEARM_ID: u8 = 4;
/// Sidearms any class may carry.
pub const SIDEARMS: [u8; 4] = [4, 10, 11, 16];

pub static WEAPONS: [WeaponDef; 18] = [
    WeaponDef {
        name: "Carbine",
        blurb: "Automatic rifle - steady all-rounder",
        arch: 0,
        damage: 20.0,
        pellets: 1,
        interval: 6,
        mag: 30,
        reload: 120,
        spread_hip: 0.02,
        spread_ads: 0.005,
        head_mult: 1.7,
        falloff: (35.0, 90.0, 0.55),
        recoil: 0.011,
        ads_fov: 48.0,
        semi: false,
        heat: 18,
        bloom: 0.024,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Scattergun",
        blurb: "Pump shotgun - one shot kills up close",
        arch: 1,
        damage: 12.0,
        pellets: 9,
        interval: 46,
        mag: 6,
        reload: 150,
        spread_hip: 0.065,
        spread_ads: 0.05,
        head_mult: 1.3,
        falloff: (9.0, 30.0, 0.2),
        recoil: 0.05,
        ads_fov: 58.0,
        semi: false,
        heat: 0,
        bloom: 0.0,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Longrifle",
        blurb: "Bolt-action - 4x scope, marks targets",
        arch: 2,
        damage: 75.0,
        pellets: 1,
        interval: 62,
        mag: 5,
        reload: 160,
        spread_hip: 0.05,
        spread_ads: 0.0006,
        head_mult: 2.0,
        falloff: (220.0, 300.0, 0.8),
        recoil: 0.045,
        ads_fov: 22.0,
        semi: false,
        heat: 0,
        bloom: 0.0,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Frag",
        blurb: "Fragmentation grenade",
        arch: 0,
        damage: 0.0,
        pellets: 1,
        interval: 1,
        mag: 1,
        reload: 1,
        spread_hip: 0.0,
        spread_ads: 0.0,
        head_mult: 1.0,
        falloff: (1.0, 2.0, 1.0),
        recoil: 0.0,
        ads_fov: 60.0,
        semi: false,
        heat: 0,
        bloom: 0.0,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Sidearm",
        blurb: "Pistol - 12 rounds, quick and accurate",
        arch: 4,
        damage: 27.0,
        pellets: 1,
        interval: 8,
        mag: 12,
        reload: 84,
        spread_hip: 0.013,
        spread_ads: 0.005,
        head_mult: 1.8,
        falloff: (18.0, 50.0, 0.5),
        recoil: 0.02,
        ads_fov: 56.0,
        semi: true,
        heat: 34,
        bloom: 0.03,
        mobility: 1.06,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Warden",
        blurb: "Battle rifle - hits hard at range, kicks",
        arch: 0,
        damage: 31.0,
        pellets: 1,
        interval: 10,
        mag: 20,
        reload: 135,
        spread_hip: 0.026,
        spread_ads: 0.0035,
        head_mult: 1.8,
        falloff: (55.0, 120.0, 0.65),
        recoil: 0.019,
        ads_fov: 44.0,
        semi: false,
        heat: 30,
        bloom: 0.03,
        mobility: 0.98,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Bulwark",
        blurb: "Machine gun - 60 rounds, heavy and slow",
        arch: 0,
        damage: 21.0,
        pellets: 1,
        interval: 6,
        mag: 60,
        reload: 230,
        spread_hip: 0.034,
        spread_ads: 0.008,
        head_mult: 1.5,
        falloff: (40.0, 100.0, 0.6),
        recoil: 0.012,
        ads_fov: 48.0,
        semi: false,
        heat: 12,
        bloom: 0.03,
        mobility: 0.9,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Rattler",
        blurb: "Self-loading shotgun - fast, weaker shells",
        arch: 1,
        damage: 10.0,
        pellets: 6,
        interval: 22,
        mag: 8,
        reload: 170,
        spread_hip: 0.075,
        spread_ads: 0.06,
        head_mult: 1.3,
        falloff: (8.0, 26.0, 0.2),
        recoil: 0.035,
        ads_fov: 58.0,
        semi: true,
        heat: 0,
        bloom: 0.0,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Wasp",
        blurb: "Submachine gun - very fast, short range",
        arch: 0,
        damage: 16.0,
        pellets: 1,
        interval: 4,
        mag: 32,
        reload: 100,
        spread_hip: 0.022,
        spread_ads: 0.009,
        head_mult: 1.5,
        falloff: (14.0, 40.0, 0.45),
        recoil: 0.008,
        ads_fov: 52.0,
        semi: false,
        heat: 12,
        bloom: 0.03,
        mobility: 1.04,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Ranger",
        blurb: "Self-loader - 2.5x optic, marks targets",
        arch: 2,
        damage: 46.0,
        pellets: 1,
        interval: 15,
        mag: 10,
        reload: 140,
        spread_hip: 0.035,
        spread_ads: 0.0012,
        head_mult: 2.0,
        falloff: (120.0, 220.0, 0.75),
        recoil: 0.03,
        ads_fov: 30.0,
        semi: true,
        heat: 40,
        bloom: 0.02,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Magnum",
        blurb: "Revolver - six heavy rounds",
        arch: 4,
        damage: 52.0,
        pellets: 1,
        interval: 24,
        mag: 6,
        reload: 130,
        spread_hip: 0.016,
        spread_ads: 0.003,
        head_mult: 2.0,
        falloff: (30.0, 70.0, 0.6),
        recoil: 0.05,
        ads_fov: 54.0,
        semi: true,
        heat: 60,
        bloom: 0.03,
        mobility: 1.04,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Stinger",
        blurb: "Machine pistol - automatic, 20 rounds",
        arch: 4,
        damage: 13.0,
        pellets: 1,
        interval: 4,
        mag: 20,
        reload: 90,
        spread_hip: 0.03,
        spread_ads: 0.014,
        head_mult: 1.5,
        falloff: (10.0, 30.0, 0.4),
        recoil: 0.012,
        ads_fov: 56.0,
        semi: false,
        heat: 16,
        bloom: 0.04,
        mobility: 1.06,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Lancer",
        blurb: "Burst rifle - three rounds a pull, long reach",
        arch: 0,
        damage: 24.0,
        pellets: 1,
        interval: 4,
        mag: 30,
        reload: 125,
        spread_hip: 0.024,
        spread_ads: 0.003,
        head_mult: 1.8,
        falloff: (50.0, 110.0, 0.6),
        recoil: 0.012,
        ads_fov: 46.0,
        semi: false,
        heat: 14,
        bloom: 0.02,
        mobility: 1.0,
        burst: 3,
        burst_gap: 16,
    },
    WeaponDef {
        name: "Slugger",
        blurb: "Slug shotgun - one heavy round, mid range",
        arch: 1,
        damage: 85.0,
        pellets: 1,
        interval: 50,
        mag: 5,
        reload: 150,
        spread_hip: 0.03,
        spread_ads: 0.006,
        head_mult: 1.5,
        falloff: (30.0, 70.0, 0.4),
        recoil: 0.055,
        ads_fov: 52.0,
        semi: false,
        heat: 0,
        bloom: 0.0,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Drummer",
        blurb: "Automatic shotgun - 12-round drum, wide",
        arch: 1,
        damage: 8.0,
        pellets: 6,
        interval: 15,
        mag: 12,
        reload: 200,
        spread_hip: 0.085,
        spread_ads: 0.07,
        head_mult: 1.2,
        falloff: (7.0, 22.0, 0.2),
        recoil: 0.03,
        ads_fov: 58.0,
        semi: false,
        heat: 0,
        bloom: 0.0,
        mobility: 0.95,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Hammerhead",
        blurb: "Heavy rifle - three huge rounds, very slow",
        arch: 2,
        damage: 115.0,
        pellets: 1,
        interval: 90,
        mag: 3,
        reload: 200,
        spread_hip: 0.07,
        spread_ads: 0.0004,
        head_mult: 2.0,
        falloff: (300.0, 400.0, 0.9),
        recoil: 0.08,
        ads_fov: 22.0,
        semi: false,
        heat: 0,
        bloom: 0.0,
        mobility: 0.9,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Deuce",
        blurb: "Sawn-off - two barrels, point blank only",
        arch: 4,
        damage: 11.0,
        pellets: 7,
        interval: 12,
        mag: 2,
        reload: 110,
        spread_hip: 0.09,
        spread_ads: 0.08,
        head_mult: 1.2,
        falloff: (5.0, 16.0, 0.15),
        recoil: 0.07,
        ads_fov: 56.0,
        semi: true,
        heat: 0,
        bloom: 0.0,
        mobility: 1.04,
        burst: 0,
        burst_gap: 0,
    },
    WeaponDef {
        name: "Melee",
        blurb: "Close-quarters strike",
        arch: 0,
        damage: MELEE_DAMAGE,
        pellets: 1,
        interval: MELEE_TICKS,
        mag: 1,
        reload: 1,
        spread_hip: 0.0,
        spread_ads: 0.0,
        head_mult: 1.0,
        falloff: (1.0, 2.0, 1.0),
        recoil: 0.0,
        ads_fov: 60.0,
        semi: true,
        heat: 0,
        bloom: 0.0,
        mobility: 1.0,
        burst: 0,
        burst_gap: 0,
    },
];

pub fn weapon_def(wid: u8) -> &'static WeaponDef {
    &WEAPONS[wid as usize % WEAPONS.len()]
}

/// Weapon id currently in the player's hands.
pub fn wid(s: &PState) -> u8 {
    if s.slot == 1 {
        s.side
    } else {
        s.prim
    }
}

pub fn kit(k: u8) -> &'static KitDef {
    &KITS[(k as usize) % KITS.len()]
}

/// Clamp a requested loadout to something the class may carry.
pub fn fix_loadout(k: u8, prim: u8, side: u8) -> (u8, u8, u8) {
    let k = k % KITS.len() as u8;
    let kd = kit(k);
    let prim = if kd.prims.contains(&prim) { prim } else { kd.prims[0] };
    let side = if SIDEARMS.contains(&side) { side } else { SIDEARM_ID };
    (k, prim, side)
}

/// The predicted part of a player. Everything `step` reads or writes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct PState {
    pub pos: Vec3,
    pub vel: Vec3,
    pub on_ground: bool,
    /// Rounds in the weapon in hand.
    pub ammo: u8,
    /// Rounds in the holstered weapon.
    pub ammo_alt: u8,
    /// 0 = primary, 1 = sidearm.
    pub slot: u8,
    /// Ticks left on the draw animation; cannot fire until 0.
    pub swap: u8,
    /// Fire button state last tick (for semi-automatic weapons).
    pub trig: bool,
    pub cooldown: u8,
    pub reload: u8,
    pub grenades: u8,
    pub gren_cd: u8,
    pub kit: u8,
    /// Weapon ids of the primary and the sidearm carried.
    pub prim: u8,
    pub side: u8,
    pub surge: bool,
    pub crouch: bool,
    /// Ticks left in a slide (sprint, then crouch).
    pub slide: u8,
    /// Ticks until another slide is allowed.
    pub slide_cd: u8,
    /// Barrel heat, 0..=HEAT_MAX. Widens the cone; cools between shots.
    pub heat: u8,
    /// Rounds still to come in the burst being fired.
    pub burst: u8,
    /// Ticks left of a melee swing; no shooting or reloading until 0.
    pub melee: u8,
    /// On a ladder: hands on the rungs, so no gravity and no shooting.
    pub climbing: bool,
}

impl PState {
    pub fn spawn(pos: Vec3, k: u8) -> PState {
        PState::spawn_with(pos, k, 255, 255)
    }

    pub fn spawn_with(pos: Vec3, k: u8, prim: u8, side: u8) -> PState {
        let (k, prim, side) = fix_loadout(k, prim, side);
        let kd = kit(k);
        PState {
            pos,
            vel: Vec3::ZERO,
            on_ground: false,
            ammo: weapon_def(prim).mag,
            ammo_alt: weapon_def(side).mag,
            slot: 0,
            swap: 0,
            trig: false,
            cooldown: 0,
            reload: 0,
            grenades: kd.grenades,
            gren_cd: 0,
            kit: k,
            prim,
            side,
            surge: false,
            crouch: false,
            slide: 0,
            slide_cd: 0,
            heat: 0,
            burst: 0,
            melee: 0,
            climbing: false,
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct StepOut {
    pub fired: bool,
    pub threw: bool,
    pub jumped: bool,
    pub landed: bool,
    pub reload_started: bool,
    pub swapped: bool,
    pub slid: bool,
    pub mantle: bool,
    /// A melee swing started this tick.
    pub melee: bool,
    /// Barrel heat the shot was fired with (before this shot heated it).
    pub heat: u8,
}

pub fn view_dir(yaw: f32, pitch: f32) -> Vec3 {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    vec3(cp * sy, sp, cp * cy)
}

pub fn right_of(yaw: f32) -> Vec3 {
    let (sy, cy) = yaw.sin_cos();
    vec3(-cy, 0.0, sy)
}

fn overlaps(b: &Block, p: Vec3, h: f32) -> bool {
    p.x + P_R > b.min.x && p.x - P_R < b.max.x && p.z + P_R > b.min.z && p.z - P_R < b.max.z && p.y + h > b.min.y && p.y < b.max.y
}

fn blocked_at(w: &World, p: Vec3, dmask: u64, h: f32) -> bool {
    w.blocks.iter().enumerate().any(|(i, b)| overlaps(b, p, h) && w.solid(i, dmask))
}

/// A ledge directly ahead that can be climbed: something solid in the way
/// whose top is within reach, with room to stand on it.
fn mantle_top(s: &PState, w: &World, dmask: u64, dir: Vec3) -> Option<f32> {
    let h = height_of(s.crouch);
    let probe = s.pos + dir * 0.45;
    let mut top: Option<f32> = None;
    for (i, b) in w.blocks.iter().enumerate() {
        if !overlaps(b, probe, h) || !w.solid(i, dmask) {
            continue;
        }
        let rise = b.max.y - s.pos.y;
        if rise > MANTLE_REACH {
            return None;
        }
        if rise > 0.0 {
            top = Some(top.map_or(b.max.y, |t: f32| t.max(b.max.y)));
        }
    }
    let ty = top?;
    if blocked_at(w, vec3(probe.x, ty + 0.02, probe.z), dmask, h) {
        return None;
    }
    Some(ty)
}

fn move_axis(s: &mut PState, w: &World, dmask: u64, axis: usize, was_ground: bool) {
    let delta = s.vel[axis] * DT;
    if delta == 0.0 {
        return;
    }
    let h = height_of(s.crouch);
    let mut np = s.pos;
    np[axis] += delta;
    let mut step_to: Option<f32> = None;
    for (i, b) in w.blocks.iter().enumerate() {
        if !overlaps(b, np, h) || !w.solid(i, dmask) {
            continue;
        }
        // Low ledge: step onto it instead of stopping (stairs, kerbs).
        if was_ground && b.max.y - np.y <= STEP_H {
            let up = vec3(np.x, b.max.y + 0.001, np.z);
            if !blocked_at(w, up, dmask, h) {
                step_to = Some(step_to.map_or(up.y, |y: f32| y.max(up.y)));
                continue;
            }
        }
        np[axis] = if delta > 0.0 { b.min[axis] - P_R - 1e-3 } else { b.max[axis] + P_R + 1e-3 };
        s.vel[axis] = 0.0;
    }
    if let Some(y) = step_to {
        np.y = np.y.max(y);
    }
    if blocked_at(w, np, dmask, h) || w.in_rock(np, P_R, h) {
        s.vel[axis] = 0.0;
        return;
    }
    s.pos = np;
}

fn move_collide(s: &mut PState, w: &World, dmask: u64, jumped: bool, was_ground: bool) {
    let h = height_of(s.crouch);
    let embedded = blocked_at(w, s.pos, dmask, h);
    if embedded {
        // Never trap a player inside geometry: let them walk out.
        s.pos.x += s.vel.x * DT;
        s.pos.z += s.vel.z * DT;
    } else {
        move_axis(s, w, dmask, 0, was_ground);
        move_axis(s, w, dmask, 2, was_ground);
    }
    s.pos.x = s.pos.x.clamp(-HALF_X + 0.5, HALF_X - 0.5);
    s.pos.z = s.pos.z.clamp(-HALF_Z + 0.5, HALF_Z - 0.5);

    let mut np = s.pos;
    np.y += s.vel.y * DT;
    let g = w.ground(np.x, np.z, s.pos.y);
    let mut on_ground = false;
    // A tunnel roof stops a jump.
    if let Some(roof) = w.roof(np.x, np.z, s.pos.y) {
        if np.y + h > roof {
            np.y = (roof - h).max(g);
            s.vel.y = s.vel.y.min(0.0);
        }
    }
    if was_ground && !jumped && s.vel.y <= 0.0 && np.y - g < 0.4 {
        np.y = g; // stay glued to downhill slopes
    }
    if np.y <= g {
        np.y = g;
        s.vel.y = s.vel.y.max(0.0);
        on_ground = true;
    }
    for (i, b) in w.blocks.iter().enumerate() {
        if !overlaps(b, np, h) || !w.solid(i, dmask) {
            continue;
        }
        if s.vel.y <= 0.0 && s.pos.y >= b.max.y - 0.02 {
            np.y = b.max.y;
            s.vel.y = 0.0;
            on_ground = true;
        } else if s.vel.y > 0.0 && s.pos.y + h <= b.min.y + 0.02 {
            np.y = b.min.y - h - 1e-3;
            s.vel.y = 0.0;
        } else if !embedded && b.max.y - np.y < 0.6 {
            np.y = b.max.y;
            s.vel.y = s.vel.y.min(0.0);
            on_ground = true;
        }
    }
    s.pos = np;
    s.on_ground = on_ground;
}

/// The ladder a player at `p` is in front of, close enough to hold: within
/// reach of the rungs, between its foot and the deck it arrives at.
pub fn ladder_at(w: &World, p: Vec3) -> Option<Ladder> {
    w.ladders.iter().copied().find(|l| {
        let (rx, rz) = (p.x - l.at.x, p.z - l.at.z);
        let depth = -(rx * l.face.x + rz * l.face.z);
        let side = rx * l.face.z - rz * l.face.x;
        (-0.05..=LADDER_REACH).contains(&depth) && side.abs() <= LADDER_GRIP && p.y >= l.at.y - 0.5 && p.y < l.top
    })
}

/// One tick on a ladder: no gravity, forward climbs, back descends and
/// crouch slides down. Reaching the ground or a deck below lets go; at the
/// top the climber steps forward off the rungs onto the deck.
fn climb(s: &mut PState, l: &Ladder, mz: f32, slide: bool, w: &World, dmask: u64) {
    let across = vec3(l.face.z, 0.0, -l.face.x);
    let (rx, rz) = (s.pos.x - l.at.x, s.pos.z - l.at.z);
    let depth = -(rx * l.face.x + rz * l.face.z);
    let side = rx * across.x + rz * across.z;
    // Settle onto the centre line just off the rungs, through collision.
    let mut pull = l.face * (depth - CLIMB_HOLD) - across * side;
    let most = CLIMB_PULL * DT;
    if pull.length() > most {
        pull *= most / pull.length();
    }
    s.vel = pull / DT;
    s.vel.y = if slide { -CLIMB_SLIDE } else { mz * CLIMB_SPEED };
    if s.vel.y > 0.0 && s.pos.y + s.vel.y * DT >= l.top {
        let off = vec3(s.pos.x, l.top + 0.001, s.pos.z) + l.face * (depth + CLIMB_STEP);
        if !blocked_at(w, off, dmask, height_of(s.crouch)) {
            s.pos = off;
            s.vel = l.face * (CLIMB_SPEED * 0.5);
            s.on_ground = true;
            s.climbing = false;
            return;
        }
        // Something stands on the hatch: wait just below it.
        s.vel.y = ((l.top - 0.002 - s.pos.y) / DT).max(0.0);
    }
    move_collide(s, w, dmask, false, false);
    if s.on_ground {
        s.climbing = false;
    }
}

/// Advance one player by one command (one 60 Hz tick).
pub fn step(s: &mut PState, c: &Cmd, w: &World, dmask: u64) -> StepOut {
    let kd = kit(s.kit);
    let mut wd = weapon_def(wid(s));
    let mut out = StepOut::default();
    let fire_btn = c.buttons & btn::FIRE != 0;
    let ads = c.buttons & btn::ADS != 0;
    let (mx, mz) = c.movement();
    // Crouch: lower profile, steadier aim, slower. Standing back up needs
    // headroom, so you cannot pop up inside something.
    let hv = vec3(s.vel.x, 0.0, s.vel.z);
    let hs = hv.length();
    let crouch_btn = c.buttons & btn::CROUCH != 0;
    s.slide_cd = s.slide_cd.saturating_sub(1);
    // Slide: hit crouch at a sprint and you keep the speed for a moment,
    // low to the ground. Jumping or running out of speed ends it.
    if crouch_btn && !s.crouch && s.on_ground && s.slide == 0 && s.slide_cd == 0 && hs > WALK * kd.speed * 1.25 {
        s.slide = SLIDE_TICKS;
        s.slide_cd = SLIDE_TICKS + 36;
        let boost = (hs * 1.12).min(WALK * 2.0) / hs;
        s.vel.x *= boost;
        s.vel.z *= boost;
        out.slid = true;
    }
    if s.slide > 0 {
        s.slide -= 1;
        if hs < 3.0 || (!crouch_btn && s.slide < SLIDE_TICKS - 10) {
            s.slide = 0;
        }
    }
    if crouch_btn || s.slide > 0 {
        s.crouch = true;
    } else if s.crouch && !blocked_at(w, s.pos, dmask, P_H) {
        s.crouch = false;
    }
    let sliding = s.slide > 0;
    let sprint = c.buttons & btn::SPRINT != 0 && mz > 0.0 && !fire_btn && !ads && !s.crouch;

    let (sy, cy) = c.yaw.sin_cos();
    let fwd = vec3(sy, 0.0, cy);
    let right = vec3(-cy, 0.0, sy);
    let mut wish = fwd * mz + right * mx;
    if wish.length_squared() > 1.0 {
        wish = wish.normalize();
    }
    let mut speed = WALK * kd.speed;
    if sprint {
        speed *= SPRINT_MULT;
    }
    if ads {
        speed *= 0.55;
    }
    if s.crouch {
        speed *= 0.5;
    }
    speed *= wd.mobility; // a handgun is lighter on your feet, a machine gun is not
    if s.surge {
        speed *= 1.18;
    }
    let was_ground = s.on_ground;
    let jump_btn = c.buttons & btn::JUMP != 0;
    // Ladders: look at the rungs and push forward to take hold (or back,
    // dropping onto one from above). Jump lets go.
    let ladder = ladder_at(w, s.pos);
    match ladder {
        None => s.climbing = false,
        Some(l) if s.climbing && jump_btn => {
            s.climbing = false;
            s.vel = -l.face * LET_GO;
        }
        Some(l) if !s.climbing && !jump_btn && !sliding && fwd.dot(l.face) > LADDER_LOOK => {
            s.climbing = (mz > 0.0 && !crouch_btn) || (mz < 0.0 && !was_ground);
        }
        _ => {}
    }
    let hv = vec3(s.vel.x, 0.0, s.vel.z);
    if s.climbing {
        // Hands on the rungs; `climb` moves the player.
    } else if sliding {
        // Friction only, plus a little steering.
        let dv = wish * 2.0 * DT - hv.normalize_or_zero() * (if was_ground { 5.5 } else { 0.5 }) * DT;
        s.vel.x += dv.x;
        s.vel.z += dv.z;
    } else if was_ground || wish != Vec3::ZERO {
        // Quicker to stop than to start: crisp, but with a little weight.
        let accel = if !was_ground {
            16.0
        } else if wish == Vec3::ZERO {
            60.0
        } else {
            52.0
        };
        let dv = wish * speed - hv;
        let max = accel * DT;
        let dv = if dv.length() > max { dv.normalize() * max } else { dv };
        s.vel.x += dv.x;
        s.vel.z += dv.z;
    }
    if was_ground && jump_btn {
        s.vel.y = JUMP_V;
        s.slide = 0;
        out.jumped = true;
    }
    // Mantle: hold jump while pushing into a ledge to climb onto it.
    if jump_btn && mz > 0.0 && !sliding && s.vel.y < MANTLE_V && mantle_top(s, w, dmask, fwd).is_some() {
        s.vel.y = MANTLE_V + GRAVITY * DT;
        out.mantle = true;
    }
    if let (true, Some(l)) = (s.climbing, ladder) {
        climb(s, &l, mz, crouch_btn, w, dmask);
    } else {
        s.vel.y -= GRAVITY * DT;
        let fall = s.vel.y;
        move_collide(s, w, dmask, out.jumped, was_ground);
        out.landed = s.on_ground && !was_ground && fall < -5.0;
    }

    // Weapon timing.
    s.cooldown = s.cooldown.saturating_sub(1);
    s.gren_cd = s.gren_cd.saturating_sub(1);
    s.swap = s.swap.saturating_sub(1);
    // The barrel cools quickly once you are off the trigger.
    s.heat = s.heat.saturating_sub(if fire_btn && s.cooldown > 0 { 1 } else { 3 });
    if c.buttons & btn::SWAP != 0 && s.swap == 0 {
        // Drawing the other weapon is faster than reloading; it cancels a
        // reload in progress.
        s.slot ^= 1;
        std::mem::swap(&mut s.ammo, &mut s.ammo_alt);
        s.reload = 0;
        s.cooldown = 0;
        s.heat = 0;
        s.burst = 0;
        s.swap = if s.slot == 1 { 16 } else { 26 };
        out.swapped = true;
        wd = weapon_def(wid(s));
    }
    // Melee: a quick strike with the weapon in hand. It interrupts a
    // reload, and nothing else can be done until the swing is over.
    s.melee = s.melee.saturating_sub(1);
    if c.more & btn2::MELEE != 0 && s.melee == 0 && s.swap == 0 && !s.climbing {
        s.melee = MELEE_TICKS;
        s.reload = 0;
        s.burst = 0;
        out.melee = true;
    }
    if s.reload > 0 {
        s.reload -= 1;
        if s.reload == 0 {
            s.ammo = wd.mag;
        }
    }
    let want_reload = (c.buttons & btn::RELOAD != 0 && s.ammo < wd.mag) || s.ammo == 0;
    if s.reload == 0 && want_reload && s.melee == 0 {
        s.reload = if s.surge { wd.reload / 2 } else { wd.reload };
        out.reload_started = true;
    }
    if s.reload > 0 || s.ammo == 0 || s.climbing {
        s.burst = 0;
    }
    // A burst, once started, finishes on its own.
    let pull = (fire_btn && (!wd.semi || !s.trig)) || s.burst > 0;
    if pull && s.reload == 0 && s.cooldown == 0 && s.swap == 0 && s.melee == 0 && s.ammo > 0 && !s.climbing {
        s.ammo -= 1;
        s.cooldown = wd.interval;
        if wd.burst > 0 {
            s.burst = if s.burst > 0 { s.burst - 1 } else { wd.burst - 1 };
            if s.burst == 0 {
                s.cooldown = wd.burst_gap;
            }
        }
        out.fired = true;
        out.heat = s.heat;
        s.heat = (s.heat + wd.heat).min(HEAT_MAX);
    }
    s.trig = fire_btn;
    if c.buttons & btn::GRENADE != 0 && s.grenades > 0 && s.gren_cd == 0 && !s.climbing {
        s.grenades -= 1;
        s.gren_cd = 50;
        out.threw = true;
    }
    out
}

/// Current cone half-angle. Shared so the crosshair shows the real spread.
pub fn spread(k: u8, ads: bool, hspeed: f32, airborne: bool, crouch: bool, heat: u8) -> f32 {
    let wd = weapon_def(k);
    let steady = if crouch { 0.6 } else { 1.0 };
    let base = if ads { wd.spread_ads } else { wd.spread_hip } * steady;
    let mv = (hspeed / WALK).min(1.6) * if ads { 0.006 } else { 0.016 };
    let bloom = wd.bloom * (heat.min(HEAT_MAX) as f32 / HEAT_MAX as f32) * steady * if ads { 0.55 } else { 1.0 };
    base + mv + bloom + if airborne { 0.04 } else { 0.0 }
}

/// Pellet directions for a shot. Seeded by the command sequence so the
/// client's predicted tracers match what the server resolves.
#[allow(clippy::too_many_arguments)]
pub fn shot_dirs(k: u8, seq: u32, yaw: f32, pitch: f32, ads: bool, hspeed: f32, airborne: bool, crouch: bool, heat: u8) -> Vec<Vec3> {
    let wd = weapon_def(k);
    let cone = spread(k, ads, hspeed, airborne, crouch, heat);
    let f = view_dir(yaw, pitch);
    let r = right_of(yaw);
    let u = r.cross(f);
    let mut rng = Rng::new(seq as u64 * 7919 + k as u64);
    (0..wd.pellets)
        .map(|_| {
            let rad = rng.f32().sqrt() * cone;
            let a = rng.f32() * std::f32::consts::TAU;
            (f + r * (rad * a.cos()) + u * (rad * a.sin())).normalize()
        })
        .collect()
}

pub fn falloff(k: u8, dist: f32) -> f32 {
    let (a, b, m) = weapon_def(k).falloff;
    if dist <= a {
        1.0
    } else if dist >= b {
        m
    } else {
        1.0 + (m - 1.0) * (dist - a) / (b - a)
    }
}

/// Ray vs a standing player at `pos` (feet). Returns distance and headshot.
pub fn ray_player(o: Vec3, d: Vec3, pos: Vec3, crouch: bool) -> Option<(f32, bool)> {
    let (body_top, head_y) = if crouch { (0.88, 1.04) } else { (1.45, 1.63) };
    let mut best: Option<(f32, bool)> = None;
    let min = pos + vec3(-0.38, 0.0, -0.38);
    let max = pos + vec3(0.38, body_top, 0.38);
    if let Some((t, _)) = crate::map::ray_aabb(o, d, min, max) {
        best = Some((t, false));
    }
    let c = pos + vec3(0.0, head_y, 0.0);
    let oc = o - c;
    let b = oc.dot(d);
    let disc = b * b - (oc.length_squared() - 0.24 * 0.24);
    if disc >= 0.0 {
        let t = -b - disc.sqrt();
        if t > 0.0 && best.is_none_or(|(bt, _)| t < bt + 0.3) {
            best = Some((t, true));
        }
    }
    best
}

#[cfg(test)]
mod finish_tests {
    use super::*;

    #[test]
    fn finish_ids_and_palettes_are_valid() {
        // These indices are persisted in settings and sent over the wire.
        // New finishes may be appended; existing ids must stay stable.
        let names = ["FACTORY", "OXIDE", "ARCTIC", "WOODLAND", "MIDNIGHT", "HAZARD", "SANDSTORM"];
        assert!(WEAPON_SKINS.len() <= u8::MAX as usize);
        for (id, name) in names.into_iter().enumerate() {
            assert_eq!(WEAPON_SKINS[id].name, name);
        }
        let factory = weapon_skin(0);
        assert_eq!(factory.pattern, WeaponPattern::Plain);
        assert!([
            factory.body,
            factory.furniture,
            factory.accent,
            factory.detail,
            factory.pattern_color,
            factory.pattern_alt,
            factory.edge,
            factory.hardware
        ]
        .iter()
        .all(Option::is_none));
        assert!(factory.sheen == 0.0 && factory.glow == 0.0, "the factory gun is not polished or glowing");
        for (id, skin) in WEAPON_SKINS.iter().enumerate() {
            assert!(!skin.name.is_empty() && !skin.blurb.is_empty());
            assert!(!WEAPON_SKINS[..id].iter().any(|other| other.name == skin.name), "duplicate finish {}", skin.name);
            let colors =
                [skin.body, skin.furniture, skin.accent, skin.detail, skin.pattern_color, skin.pattern_alt, skin.edge, skin.hardware];
            for color in colors.into_iter().flatten() {
                assert!(color.into_iter().all(|v| v.is_finite() && (0.0..=1.0).contains(&v)), "{} has an invalid colour", skin.name);
            }
            assert_eq!(skin.pattern == WeaponPattern::Plain, skin.pattern_color.is_none(), "{} has incomplete pattern data", skin.name);
            assert!((0.0..=1.0).contains(&skin.sheen) && (0.0..=1.0).contains(&skin.glow), "{} has an invalid sheen or glow", skin.name);
            // Each finish reads as its own at a glance: no shared pattern.
            if skin.pattern != WeaponPattern::Plain {
                assert!(!WEAPON_SKINS[..id].iter().any(|other| other.pattern == skin.pattern), "{} repeats a pattern", skin.name);
            }
            if let Some(detail) = skin.detail {
                assert!(detail.into_iter().all(|v| v <= 0.25), "{} needs dark iron sights and small parts", skin.name);
            }
            if let Some(pattern) = skin.pattern_color {
                let body = skin.body.expect("patterned finishes need a base coat");
                let contrast: f32 = body.into_iter().zip(pattern).map(|(a, b)| (a - b).abs()).sum();
                assert!(contrast >= 0.2, "{} needs a visible secondary paint colour", skin.name);
            }
        }
    }

    #[test]
    fn every_carried_gun_accepts_all_shipped_finishes_and_rejects_unknown_ids() {
        let ids: Vec<u8> = (0..WEAPON_SKINS.len() as u8).collect();
        for wid in KITS.iter().flat_map(|kit| kit.prims.iter()).chain(SIDEARMS.iter()).copied() {
            assert_eq!(weapon_skins_for(wid), ids, "{} is missing a shipped finish", weapon_def(wid).name);
            for &id in &ids {
                assert_eq!(fix_weapon_skin(wid, id), id);
            }
            for id in WEAPON_SKINS.len() as u16..=u8::MAX as u16 {
                assert_eq!(fix_weapon_skin(wid, id as u8), 0, "unknown finish {id} must fall back to Factory");
            }
        }
    }
}
