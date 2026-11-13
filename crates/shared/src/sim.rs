//! Deterministic per-command player simulation: movement, collision and
//! weapon timing. `step` is the single source of truth - the server runs it
//! on arrival of each command, the client runs it for prediction and again
//! when replaying unacknowledged commands after a snapshot.

use crate::map::{Block, World, HALF_X, HALF_Z};
use crate::proto::{btn, Cmd};
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
pub const GRENADE_WEAPON: u8 = 3;

pub struct WeaponDef {
    pub name: &'static str,
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
}

pub struct KitDef {
    pub name: &'static str,
    pub blurb: &'static str,
    pub weapon: WeaponDef,
    pub hp: f32,
    pub grenades: u8,
    pub speed: f32,
}

pub static KITS: [KitDef; 3] = [
    KitDef {
        name: "VANGUARD",
        blurb: "Carbine - 2 frags - all-round line fighter",
        weapon: WeaponDef {
            name: "Carbine",
            damage: 20.0,
            pellets: 1,
            interval: 6,
            mag: 30,
            reload: 120,
            spread_hip: 0.020,
            spread_ads: 0.005,
            head_mult: 1.7,
            falloff: (35.0, 90.0, 0.55),
            recoil: 0.011,
            ads_fov: 48.0,
            semi: false,
        },
        hp: 100.0,
        grenades: 2,
        speed: 1.0,
    },
    KitDef {
        name: "BREACHER",
        blurb: "Scattergun - heavy armour - clears points",
        weapon: WeaponDef {
            name: "Scattergun",
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
        },
        hp: 130.0,
        grenades: 1,
        speed: 1.05,
    },
    KitDef {
        name: "MARKSMAN",
        blurb: "Longrifle - hits mark targets for your team",
        weapon: WeaponDef {
            name: "Longrifle",
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
        },
        hp: 90.0,
        grenades: 1,
        speed: 1.0,
    },
];

/// Weapon id of the sidearm every kit carries (kit primaries are 0..=2,
/// 3 is the frag grenade).
pub const SIDEARM_ID: u8 = 4;

pub static SIDEARM: WeaponDef = WeaponDef {
    name: "Sidearm",
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
};

pub fn weapon_def(wid: u8) -> &'static WeaponDef {
    if wid == SIDEARM_ID {
        &SIDEARM
    } else {
        &kit(wid).weapon
    }
}

/// Weapon id currently in the player's hands.
pub fn wid(s: &PState) -> u8 {
    if s.slot == 1 {
        SIDEARM_ID
    } else {
        s.kit
    }
}

pub fn kit(k: u8) -> &'static KitDef {
    &KITS[(k as usize) % KITS.len()]
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
    pub surge: bool,
    pub crouch: bool,
}

impl PState {
    pub fn spawn(pos: Vec3, k: u8) -> PState {
        let kd = kit(k);
        PState {
            pos,
            vel: Vec3::ZERO,
            on_ground: false,
            ammo: kd.weapon.mag,
            ammo_alt: SIDEARM.mag,
            slot: 0,
            swap: 0,
            trig: false,
            cooldown: 0,
            reload: 0,
            grenades: kd.grenades,
            gren_cd: 0,
            kit: k % 3,
            surge: false,
            crouch: false,
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
    p.x + P_R > b.min.x
        && p.x - P_R < b.max.x
        && p.z + P_R > b.min.z
        && p.z - P_R < b.max.z
        && p.y + h > b.min.y
        && p.y < b.max.y
}

fn blocked_at(w: &World, p: Vec3, dmask: u64, h: f32) -> bool {
    w.blocks.iter().enumerate().any(|(i, b)| overlaps(b, p, h) && w.solid(i, dmask))
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
    if blocked_at(w, np, dmask, h) {
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
    let g = w.height(np.x, np.z);
    let mut on_ground = false;
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

/// Advance one player by one command (one 60 Hz tick).
pub fn step(s: &mut PState, c: &Cmd, w: &World, dmask: u64) -> StepOut {
    let kd = kit(s.kit);
    let mut wd = weapon_def(wid(s));
    let mut out = StepOut::default();
    let fire_btn = c.buttons & btn::FIRE != 0;
    let ads = c.buttons & btn::ADS != 0;
    let mz = c.mz.clamp(-1, 1) as f32;
    let mx = c.mx.clamp(-1, 1) as f32;
    // Crouch: lower profile, steadier aim, slower. Standing back up needs
    // headroom, so you cannot pop up inside something.
    if c.buttons & btn::CROUCH != 0 {
        s.crouch = true;
    } else if s.crouch && !blocked_at(w, s.pos, dmask, P_H) {
        s.crouch = false;
    }
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
    if s.slot == 1 {
        speed *= 1.06; // lighter on your feet with the sidearm out
    }
    if s.surge {
        speed *= 1.18;
    }
    let was_ground = s.on_ground;
    let hv = vec3(s.vel.x, 0.0, s.vel.z);
    if was_ground || wish != Vec3::ZERO {
        let accel = if was_ground { 70.0 } else { 16.0 };
        let dv = wish * speed - hv;
        let max = accel * DT;
        let dv = if dv.length() > max { dv.normalize() * max } else { dv };
        s.vel.x += dv.x;
        s.vel.z += dv.z;
    }
    if was_ground && c.buttons & btn::JUMP != 0 {
        s.vel.y = JUMP_V;
        out.jumped = true;
    }
    s.vel.y -= GRAVITY * DT;
    let fall = s.vel.y;
    move_collide(s, w, dmask, out.jumped, was_ground);
    out.landed = s.on_ground && !was_ground && fall < -5.0;

    // Weapon timing.
    s.cooldown = s.cooldown.saturating_sub(1);
    s.gren_cd = s.gren_cd.saturating_sub(1);
    s.swap = s.swap.saturating_sub(1);
    if c.buttons & btn::SWAP != 0 && s.swap == 0 {
        // Drawing the other weapon is faster than reloading; it cancels a
        // reload in progress.
        s.slot ^= 1;
        std::mem::swap(&mut s.ammo, &mut s.ammo_alt);
        s.reload = 0;
        s.cooldown = 0;
        s.swap = if s.slot == 1 { 16 } else { 26 };
        out.swapped = true;
        wd = weapon_def(wid(s));
    }
    if s.reload > 0 {
        s.reload -= 1;
        if s.reload == 0 {
            s.ammo = wd.mag;
        }
    }
    let want_reload = (c.buttons & btn::RELOAD != 0 && s.ammo < wd.mag) || s.ammo == 0;
    if s.reload == 0 && want_reload {
        s.reload = if s.surge { wd.reload / 2 } else { wd.reload };
        out.reload_started = true;
    }
    if fire_btn && s.reload == 0 && s.cooldown == 0 && s.swap == 0 && s.ammo > 0 && (!wd.semi || !s.trig) {
        s.ammo -= 1;
        s.cooldown = wd.interval;
        out.fired = true;
    }
    s.trig = fire_btn;
    if c.buttons & btn::GRENADE != 0 && s.grenades > 0 && s.gren_cd == 0 {
        s.grenades -= 1;
        s.gren_cd = 50;
        out.threw = true;
    }
    out
}

/// Current cone half-angle. Shared so the crosshair shows the real spread.
pub fn spread(k: u8, ads: bool, hspeed: f32, airborne: bool, crouch: bool) -> f32 {
    let wd = weapon_def(k);
    let base = if ads { wd.spread_ads } else { wd.spread_hip } * if crouch { 0.6 } else { 1.0 };
    let mv = (hspeed / WALK).min(1.6) * if ads { 0.006 } else { 0.016 };
    base + mv + if airborne { 0.04 } else { 0.0 }
}

/// Pellet directions for a shot. Seeded by the command sequence so the
/// client's predicted tracers match what the server resolves.
#[allow(clippy::too_many_arguments)]
pub fn shot_dirs(k: u8, seq: u32, yaw: f32, pitch: f32, ads: bool, hspeed: f32, airborne: bool, crouch: bool) -> Vec<Vec3> {
    let wd = weapon_def(k);
    let cone = spread(k, ads, hspeed, airborne, crouch);
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
        if t > 0.0 && best.map_or(true, |(bt, _)| t < bt + 0.3) {
            best = Some((t, true));
        }
    }
    best
}
