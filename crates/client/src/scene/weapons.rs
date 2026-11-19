//! Shared weapon geometry for first person and loadout previews.

use super::{hash01, limb, team_col};
use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::sim::{self, weapon_def};

#[path = "sights.rs"]
mod sights;
use sights::{front_sight, optic, rear_sight};
// The game and the review example consume different parts of this facade.
#[allow(unused_imports)]
pub use sights::{sight_fov, sight_model, sights_for, HOLO, IRON, OPTIC, RED_DOT, SCOPE4, SCOPE8, SIGHT_NAMES};

#[path = "finishes.rs"]
mod finishes;

pub struct ViewModel {
    pub sight: u8,
    /// Weapon family (0 rifle, 1 shotgun, 2 long rifle, 4 handgun).
    pub kit: u8,
    /// The exact weapon, for the details that set it apart.
    pub wid: u8,
    pub team: u8,
    pub ads: f32,
    pub recoil: f32,
    pub bob: f32,
    pub bob_amt: f32,
    /// Reload progress 0..1, negative when not reloading.
    pub reload: f32,
    /// 1 as the weapon starts coming up, 0 once it is ready.
    pub draw: f32,
    /// 1 just after a shot, 0 when the next round is ready (pump, bolt).
    pub cycle: f32,
    pub sway: (f32, f32),
    pub flash: f32,
    pub sprint: f32,
    /// Landing dip, strafe lean (-1..1), slide, airborne, climbing: 0..1.
    pub land: f32,
    pub strafe: f32,
    pub slide: f32,
    pub air: f32,
    pub mantle: f32,
    pub time: f32,
    /// Changes every shot; picks the muzzle flash shape.
    pub seq: u32,
    /// Draw only the weapon, placed by this matrix (no arms, no motion).
    pub preview: Option<Mat4>,
    /// Finish: index into `sim::WEAPON_SKINS`.
    pub skin: u8,
    /// Paint the finish's pattern; off for distant soldiers, who keep only
    /// its colours.
    pub detail: bool,
    /// Melee strike: 1 as it starts, falling to 0 when it is over.
    pub melee: f32,
}

/// Hand contact points shared by first- and third-person poses.
/// Gun space is in metres, with the muzzle toward -Z and +Y up.
pub fn gun_grips(wid: u8) -> (Vec3, Vec3) {
    match weapon_def(wid).arch {
        1 => (vec3(0.0, -0.06, 0.1), vec3(0.0, -0.075, if wid == 14 { -0.26 } else { -0.32 })),
        2 => (vec3(0.0, -0.065, 0.13), vec3(0.0, -0.06, -0.3)),
        4 => (vec3(0.0, -0.085, 0.02), if wid == 11 { vec3(0.0, -0.075, -0.17) } else { vec3(-0.03, -0.1, 0.005) }),
        _ => (vec3(0.0, -0.085, 0.1), vec3(0.0, -0.05, if wid == 8 { -0.25 } else { -0.36 })),
    }
}

/// A weapon on its own, placed by `g`. Gun space: muzzle toward -Z, +Y up.
pub fn gun_model(m: &mut Mesh, wid: u8, team: u8, skin: u8, g: Mat4) {
    gun_model_pose(m, wid, team, skin, g, -1.0, 0.0, true);
}

/// The same geometry in another soldier's hands, including magazine and
/// action. `detail` off leaves out the finish's pattern (far away).
#[allow(clippy::too_many_arguments)]
pub fn gun_model_pose(m: &mut Mesh, wid: u8, team: u8, skin: u8, g: Mat4, reload: f32, cycle: f32, detail: bool) {
    gun_model_state(m, wid, team, skin, sights_for(wid)[0], g, reload, cycle, detail);
}

/// Review a supported attachment using the same model as first person.
pub fn gun_model_sighted(m: &mut Mesh, wid: u8, team: u8, skin: u8, sight: u8, g: Mat4) {
    let sight = if sights_for(wid).contains(&sight) { sight } else { sights_for(wid)[0] };
    if sight == sights_for(wid)[0] {
        gun_model(m, wid, team, skin, g);
    } else {
        gun_model_state(m, wid, team, skin, sight, g, -1.0, 0.0, true);
    }
}

#[allow(clippy::too_many_arguments)]
fn gun_model_state(m: &mut Mesh, wid: u8, team: u8, skin: u8, sight: u8, g: Mat4, reload: f32, cycle: f32, detail: bool) {
    let mut add = Mesh::default();
    let v = ViewModel {
        sight,
        kit: weapon_def(wid).arch,
        wid,
        team,
        ads: 0.0,
        recoil: 0.0,
        bob: 0.0,
        bob_amt: 0.0,
        reload,
        draw: 0.0,
        cycle,
        sway: (0.0, 0.0),
        flash: 0.0,
        sprint: 0.0,
        land: 0.0,
        strafe: 0.0,
        slide: 0.0,
        air: 0.0,
        mantle: 0.0,
        time: 0.0,
        seq: 0,
        preview: Some(g),
        skin,
        detail,
        melee: 0.0,
    };
    viewmodel(m, &mut add, &v);
}

/// A weapon for the HUD, in pixels with +Y up: seen from the side, turned
/// by `turn` radians, and scaled so its profile fits a `w` x `h` box
/// centred on (`cx`, `cy`).
#[allow(clippy::too_many_arguments)]
pub fn gun_icon(m: &mut Mesh, wid: u8, team: u8, skin: u8, cx: f32, cy: f32, w: f32, h: f32, turn: f32) {
    gun_icon_sighted(m, wid, team, skin, sights_for(wid)[0], cx, cy, w, h, turn);
}

#[allow(clippy::too_many_arguments)]
pub fn gun_icon_sighted(m: &mut Mesh, wid: u8, team: u8, skin: u8, sight: u8, cx: f32, cy: f32, w: f32, h: f32, turn: f32) {
    let mut probe = Mesh::default();
    gun_model_sighted(&mut probe, wid, team, skin, sight, Mat4::IDENTITY);
    let rotation = Mat4::from_rotation_x(0.14) * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2 + turn);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for v in &probe.v {
        let p = rotation.transform_point3(Vec3::from(v.pos));
        lo = lo.min(p);
        hi = hi.max(p);
    }
    if probe.v.is_empty() {
        return;
    }
    let s = (w / (hi.x - lo.x).max(0.01)).min(h / (hi.y - lo.y).max(0.01));
    let g =
        Mat4::from_translation(vec3(cx, cy, 0.0)) * Mat4::from_scale(Vec3::splat(s)) * Mat4::from_translation(-(lo + hi) * 0.5) * rotation;
    gun_model_sighted(m, wid, team, skin, sight, g);
}

/// First-person weapon and arms, built in camera space (camera looks down
/// -Z, +X is right, +Y is up). With `detail` the gun is built fine:
/// chamfered parts, rounder barrels, rounded receivers and small hardware.
pub fn viewmodel(m: &mut Mesh, add: &mut Mesh, v: &ViewModel) {
    let was = m.fine;
    m.fine = v.detail;
    build(m, add, v);
    m.fine = was;
}

fn build(m: &mut Mesh, add: &mut Mesh, v: &ViewModel) {
    use std::f32::consts::PI;
    let tc = team_col(v.team);
    let kit = weapon_def(v.wid).arch;
    debug_assert_eq!(v.kit, kit, "viewmodel family must match its weapon");
    // A finish replaces whichever of the weapon's own colours it names.
    let fin = sim::weapon_skin(sim::fix_weapon_skin(v.wid, v.skin));
    let pick = |own: Col, with: Option<[f32; 3]>| with.map_or(own, |c| rgb(c[0], c[1], c[2]));
    // A glowing finish lights its accent strip and front sight too.
    let mut accent = pick(rgb(tc[0], tc[1], tc[2]), fin.accent);
    accent[3] = fin.glow * 0.6;
    let metal = pick(rgb(0.2, 0.2, 0.23), fin.body);
    // Handles, muzzles, guards and baseplates: plated or anodised.
    let hw = pick(metal, fin.hardware);
    let dark = pick(rgb(0.11, 0.11, 0.13), fin.detail);
    // The self-loaders wear polymer where the older guns wear wood.
    let wood = pick(if matches!(v.wid, 7 | 9 | 14) { rgb(0.17, 0.18, 0.17) } else { rgb(0.45, 0.3, 0.17) }, fin.furniture);
    let grip_wood = pick(rgb(0.45, 0.3, 0.17), fin.furniture);
    // A part is metal, furniture or a dark detail by the colour it is given.
    let receiver = |m: &mut Mesh, g: &Mat4, center: Vec3, half: Vec3, col: Col| {
        let part = if col == metal {
            finishes::Part::Body
        } else if col == wood || col == grip_wood {
            finishes::Part::Furniture
        } else {
            finishes::Part::Detail
        };
        finishes::receiver(m, g, center, half, col, part, fin, v.detail);
    };
    let sleeve = rgb(tc[0] * 0.45, tc[1] * 0.45, tc[2] * 0.48);
    let cuff = rgb(tc[0] * 0.3, tc[1] * 0.3, tc[2] * 0.33);
    let glove = rgb(0.42, 0.34, 0.25);
    let hip = if kit == 4 { vec3(0.15, -0.13, -0.5) } else { vec3(0.19, -0.165, -0.7) };
    let iron = v.sight == IRON;
    let scope = v.sight >= SCOPE4;
    let sight_y = sights::sight_axis(v.wid, v.sight).y;
    let aim = if iron || scope {
        vec3(0.0, -sight_y, -0.5)
    } else {
        // bring the optic's window up to the eye, a little closer in
        vec3(0.0, -sight_y, if kit == 4 { -0.34 } else { -0.47 })
    };
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    let seg = |t: f32, a: f32, b: f32| ease(((t - a) / (b - a)).clamp(0.0, 1.0));
    // Fine detail, for guns seen up close (see `viewmodel`).
    let fine = m.fine;
    // A cross pin through a part `half_x` wide, its heads standing proud.
    let pin = |m: &mut Mesh, g: &Mat4, at: Vec3, half_x: f32, col: Col| {
        let t = *g * Mat4::from_translation(at) * Mat4::from_rotation_y(PI / 2.0);
        m.tube(&t, Vec3::ZERO, 0.005, 0.0, half_x + 0.0015, 6, shade(col, 1.45));
    };
    // Rail teeth across a strip `half_x` wide, from z0 to z1, on top at y.
    let rail = |m: &mut Mesh, g: &Mat4, y: f32, half_x: f32, z0: f32, z1: f32, col: Col| {
        let n = ((z1 - z0) / 0.0145).floor() as usize;
        for k in 0..n {
            let z = z0 + 0.007 + k as f32 * 0.0145;
            m.obox(g, vec3(0.0, y + 0.0014, z), vec3(half_x, 0.0014, 0.0035), col);
        }
    };
    // Rubber ribs across the back of a butt plate whose back face is at z.
    let butt = |m: &mut Mesh, g: &Mat4, y0: f32, y1: f32, half_x: f32, z: f32, col: Col| {
        let mut y = y0 + 0.008;
        while y < y1 - 0.004 {
            m.obox(g, vec3(0.0, y, z + 0.0015), vec3(half_x, 0.0028, 0.0015), col);
            y += 0.016;
        }
    };

    // Reload choreography: tilt in, magazine out, magazine in, settle.
    let r = v.reload.clamp(-1.0, 1.0);
    let reloading = r >= 0.0;
    let (tilt, mag_out, to_mag, seat) = if reloading {
        (
            seg(r, 0.0, 0.14) * (1.0 - seg(r, 0.84, 1.0)),
            seg(r, 0.14, 0.32) * (1.0 - seg(r, 0.5, 0.68)),
            seg(r, 0.04, 0.18) * (1.0 - seg(r, 0.72, 0.9)),
            seg(r, 0.62, 0.68) * (1.0 - seg(r, 0.68, 0.78)),
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };
    // Working the action between shots.
    let cyc = if reloading { 0.0 } else { (seg(1.0 - v.cycle, 0.22, 0.72) * PI).sin() };
    let pump = if matches!(v.wid, 1 | 13) { cyc } else { 0.0 };
    let bolt = if matches!(v.wid, 2 | 15) { cyc } else { 0.0 };
    let draw = ease(v.draw.clamp(0.0, 1.0));
    // Melee: the weapon is driven forward and across, then brought back.
    let strike = if v.melee > 0.0 { ((1.0 - v.melee) / 0.62).clamp(0.0, 1.0) } else { 1.0 };
    let jab = (seg(strike, 0.0, 0.3) * (1.0 - seg(strike, 0.36, 1.0))).min(1.0);

    let k = 1.0 - v.ads * 0.85;
    let mut pos = hip + (aim - hip) * v.ads;
    pos += vec3((v.bob).sin() * 0.012, v.bob.sin().abs() * -0.014, 0.0) * v.bob_amt * (1.0 + 0.6 * v.sprint) * k;
    pos += vec3(v.sway.0, v.sway.1, 0.0) * k;
    pos += vec3((v.time * 0.9).sin() * 0.0018, (v.time * 1.7).sin() * 0.003, 0.0) * k;
    pos.z += v.recoil * 0.07 + pump * 0.012;
    pos.x -= tilt * 0.05;
    pos.y -= -tilt * 0.06 + v.sprint * 0.06 + draw * 0.34 + v.land * 0.05 * k + v.mantle * 0.3 - seat * 0.014 - v.air * 0.012 * k;
    pos.x += (v.slide * 0.03 - bolt * 0.012) * k;
    pos += vec3(-0.1, 0.1, -0.3) * jab;
    let rot = Mat4::from_rotation_x(
        v.recoil * 0.09 + tilt * 0.36 - v.sprint * 0.25 * (1.0 - jab) - draw * 0.9 - v.mantle * 0.8 - v.land * 0.05 * k + pump * 0.03
            - jab * 0.25,
    ) * Mat4::from_rotation_y(v.sprint * 0.5 * (1.0 - jab) + tilt * 0.22 + draw * 0.3 + bolt * 0.04 + jab * 1.15)
        * Mat4::from_rotation_z(-tilt * 0.55 - v.strafe * 0.05 * k + v.slide * 0.28 * k - bolt * 0.16 * k + jab * 0.7);
    let g = v.preview.unwrap_or(Mat4::from_translation(pos) * rot);

    // Different feed systems keep their own reload instead of a universal box.
    let (grip, support) = gun_grips(v.wid);
    let (mut mag_c, mut mag_h) = match kit {
        1 => (Vec3::ZERO, Vec3::ZERO),
        2 => (vec3(0.0, -0.065, -0.06), vec3(0.016, 0.028, 0.04)),
        4 => (vec3(0.0, -0.085, 0.022), vec3(0.013, 0.05, 0.02)),
        _ => (vec3(0.0, -0.1, -0.06), vec3(0.022, 0.07, 0.04)),
    };
    match v.wid {
        6 => (mag_c, mag_h) = (vec3(0.0, -0.095, -0.1), vec3(0.05, 0.05, 0.065)),
        8 => (mag_c, mag_h) = (vec3(0.0, -0.13, -0.06), vec3(0.014, 0.1, 0.024)),
        9 => mag_h = vec3(0.018, 0.05, 0.045),
        10 | 16 => mag_h = Vec3::ZERO,
        11 => (mag_c, mag_h) = (vec3(0.0, -0.11, 0.022), vec3(0.013, 0.08, 0.02)),
        14 => (mag_c, mag_h) = (vec3(0.0, -0.11, -0.06), vec3(0.055, 0.055, 0.035)),
        _ => {}
    }
    let mag_off = vec3(0.0, -0.3 * mag_out, 0.04 * mag_out);
    let slide_back = vec3(0.0, 0.0, pump * 0.085);
    let muzzle_z;
    match kit {
        1 => {
            let barrel_end = match v.wid {
                7 => -0.59,
                13 => -0.74,
                14 => -0.55,
                _ => -0.66,
            };
            let barrel_back = -0.19;
            receiver(m, &g, vec3(0.0, 0.0, -0.04), vec3(0.038, 0.05, 0.19), metal);
            // One bore above the tube magazine, not two overlapping side-by-side barrels.
            m.tube(&g, vec3(0.0, 0.014, (barrel_end + barrel_back) * 0.5), 0.021, 0.014, (barrel_back - barrel_end) * 0.5, 16, dark);
            m.tube(&g, vec3(0.0, 0.014, barrel_end + 0.012), 0.023, 0.014, 0.012, 16, hw);
            if v.wid != 14 {
                m.tube(&g, vec3(0.0, -0.038, -0.375), 0.014, 0.0, 0.17, 12, metal);
            }
            // The Rattler's fixed handguard does not pump; the Slugger does.
            let fore_len = if v.wid == 14 { 0.075 } else { 0.1 };
            let fore_z = if v.wid == 14 { -0.26 } else { -0.32 };
            receiver(m, &g, vec3(0.0, -0.045, fore_z) + slide_back, vec3(0.035, 0.03, fore_len), wood);
            for dz in [-0.06f32, 0.0, 0.06] {
                m.obox(&g, vec3(0.0, -0.045, fore_z + dz) + slide_back, vec3(0.036, 0.025, 0.005), shade(wood, 0.72));
            }
            receiver(m, &g, vec3(0.0, -0.03, 0.22), vec3(0.03, 0.05, 0.1), wood);
            if fine {
                // The stock's underside sloping down to the toe of the butt.
                m.loft(&g, &[(0.13, 0.0, -0.075, 0.027, 0.007), (0.32, 0.0, -0.09, 0.029, 0.022)], 0.5, wood);
            }
            m.obox(&g, vec3(0.0, -0.045, 0.34), vec3(0.031, 0.065, 0.02), dark);
            m.obox(&g, vec3(0.0, -0.065, 0.06), vec3(0.006, 0.02, 0.05), dark);
            m.obox(&g, vec3(0.0, 0.055, -0.1), vec3(0.036, 0.008, 0.06), accent);
            m.obox(&g, vec3(0.0385, 0.01, -0.02), vec3(0.002, 0.016, 0.045), dark);
            m.obox(&g, vec3(0.045, 0.008, 0.0 + v.recoil * 0.025), vec3(0.01, 0.006, 0.012), hw);
            if iron {
                front_sight(m, &g, barrel_end + 0.035, 0.034, 0.078, accent, dark);
                rear_sight(m, &g, 0.1, 0.05, 0.078, 0.012, dark);
            }
            if v.wid == 13 {
                // Slugger's vented heat shield and protected front post.
                m.obox(&g, vec3(0.0, 0.04, -0.42), vec3(0.026, 0.005, 0.16), dark);
                for z in [-0.53f32, -0.47, -0.41, -0.35] {
                    m.obox(&g, vec3(0.0, 0.046, z), vec3(0.018, 0.001, 0.009), hw);
                }
            }
            if v.wid == 14 {
                m.obox(&g, vec3(0.0, -0.055, -0.06), vec3(0.025, 0.018, 0.027), metal);
            }
            if fine {
                // Vent rib teeth on the top strip, pins through the receiver,
                // and a ribbed recoil pad.
                rail(m, &g, 0.063, 0.03, -0.16, -0.04, shade(accent, 0.7));
                for z in [0.06f32, -0.13] {
                    pin(m, &g, vec3(0.0, -0.022, z), 0.038, hw);
                }
                butt(m, &g, -0.11, 0.02, 0.029, 0.36, shade(dark, 1.6));
            }
            muzzle_z = barrel_end - 0.005;
        }
        4 => {
            // Shared grip/guard; each action owns its receiver and barrels.
            let grip_col = if matches!(v.wid, 10 | 16) { grip_wood } else { dark };
            if fine {
                // A grip that swells for the palm and narrows to the frame,
                // hanging down from under the slide.
                let down =
                    g * Mat4::from_rotation_x(-0.25) * Mat4::from_translation(vec3(0.0, -0.015, 0.0)) * Mat4::from_rotation_x(PI / 2.0);
                m.loft(
                    &down,
                    &[
                        (0.0, 0.0, 0.0, 0.02, 0.028),
                        (0.045, 0.0, 0.003, 0.0225, 0.032),
                        (0.1, 0.0, 0.004, 0.022, 0.031),
                        (0.12, 0.0, 0.002, 0.0205, 0.028),
                    ],
                    0.55,
                    grip_col,
                );
            } else {
                receiver(m, &(g * Mat4::from_rotation_x(-0.25)), vec3(0.0, -0.075, 0.0), vec3(0.022, 0.06, 0.031), grip_col);
            }
            if fine {
                // Grip ribs down both panels.
                let grip_g = g * Mat4::from_rotation_x(-0.25);
                for k in 0..7 {
                    let y = -0.122 + k as f32 * 0.013;
                    for x in [-0.0226f32, 0.0226] {
                        m.obox(&grip_g, vec3(x, y, 0.0), vec3(0.0012, 0.0026, 0.025), shade(grip_col, 0.72));
                    }
                }
            }
            m.obox(&g, vec3(0.0, -0.055, -0.07), vec3(0.008, 0.004, 0.04), hw);
            m.obox(&g, vec3(0.0, -0.038, -0.108), vec3(0.008, 0.02, 0.004), hw);
            m.obox(&g, vec3(0.0, -0.033, -0.045), vec3(0.004, 0.014, 0.004), dark);
            match v.wid {
                10 => {
                    // Swing-out cylinder: a revolver never inherits the pistol slide.
                    receiver(m, &g, vec3(0.0, -0.01, -0.025), vec3(0.025, 0.025, 0.07), metal);
                    m.obox(&g, vec3(0.0, 0.051, -0.035), vec3(0.012, 0.007, 0.06), accent);
                    let cylinder = g
                        * Mat4::from_translation(vec3(-0.065 * to_mag, 0.014, -0.04))
                        * Mat4::from_rotation_z(v.cycle * std::f32::consts::TAU / 6.0);
                    m.tube(&cylinder, Vec3::ZERO, 0.033, 0.0, 0.041, 12, shade(metal, 1.15));
                    for chamber in 0..6 {
                        let angle = chamber as f32 * std::f32::consts::TAU / 6.0;
                        let c = vec3(angle.cos() * 0.022, angle.sin() * 0.022, 0.043);
                        m.tube(&cylinder, c, 0.007, 0.0, 0.0015, 8, if mag_out > 0.5 { dark } else { grip_wood });
                    }
                    m.tube(&g, vec3(0.0, 0.027, -0.23), 0.017, 0.008, 0.145, 12, metal);
                    m.obox(&g, vec3(0.0, 0.048, -0.235), vec3(0.008, 0.008, 0.14), dark);
                    m.obox(&g, vec3(0.0, 0.034, 0.04), vec3(0.007, 0.018, 0.012), dark);
                    if iron {
                        front_sight(m, &g, -0.355, 0.055, 0.064, accent, dark);
                        rear_sight(m, &g, 0.005, 0.052, 0.064, 0.013, dark);
                    }
                    muzzle_z = -0.38;
                }
                16 => {
                    // Two real bores on a hinged breech, with shells visible while open.
                    receiver(m, &g, vec3(0.0, 0.012, -0.025), vec3(0.037, 0.028, 0.048), metal);
                    let hinge = vec3(0.0, -0.015, -0.075);
                    let barrels = g * Mat4::from_translation(hinge) * Mat4::from_rotation_x(-0.7 * to_mag) * Mat4::from_translation(-hinge);
                    for x in [-0.019f32, 0.019] {
                        m.tube(&barrels, vec3(x, 0.026, -0.2), 0.018, 0.012, 0.12, 12, metal);
                        if to_mag > 0.25 && mag_out < 0.6 {
                            m.tube(&barrels, vec3(x, 0.026, -0.075 + mag_out * 0.035), 0.011, 0.0, 0.025, 8, rgb(0.63, 0.2, 0.12));
                        }
                    }
                    receiver(m, &barrels, vec3(0.0, -0.007, -0.16), vec3(0.035, 0.018, 0.055), grip_wood);
                    m.obox(&g, vec3(0.0, 0.046, -0.025), vec3(0.012, 0.005, 0.014), accent);
                    if iron {
                        front_sight(m, &barrels, -0.3, 0.043, 0.064, accent, dark);
                        rear_sight(m, &g, -0.025, 0.05, 0.064, 0.013, dark);
                    }
                    muzzle_z = -0.325;
                }
                _ => {
                    let kick = vec3(0.0, 0.0, v.recoil * 0.035);
                    receiver(m, &g, vec3(0.0, 0.025, -0.12) + kick, vec3(0.022, 0.026, 0.13), metal);
                    if fine {
                        // Fine cocking serrations, the ejection port and the
                        // slide stop.
                        for k in 0..8 {
                            let z = -0.012 - k as f32 * 0.0065;
                            m.obox(&g, vec3(0.0, 0.025, z) + kick, vec3(0.0228, 0.018, 0.0016), dark);
                        }
                        m.obox(&g, vec3(0.006, 0.0512, -0.085) + kick, vec3(0.012, 0.0014, 0.022), dark);
                        m.obox(&g, vec3(-0.0232, -0.004, -0.08), vec3(0.0016, 0.004, 0.012), hw);
                    } else {
                        for z in [-0.02f32, -0.035, -0.05] {
                            m.obox(&g, vec3(0.0, 0.025, z) + kick, vec3(0.023, 0.019, 0.003), dark);
                        }
                    }
                    m.obox(&g, vec3(0.0, -0.012, -0.1), vec3(0.02, 0.014, 0.11), dark);
                    m.tube(&g, vec3(0.0, 0.03, -0.252), 0.012, 0.007, 0.006, 12, dark);
                    if iron {
                        let slide = g * Mat4::from_translation(kick);
                        front_sight(m, &slide, -0.23, 0.05, 0.064, accent, dark);
                        rear_sight(m, &slide, -0.01, 0.05, 0.064, 0.013, dark);
                    }
                    m.obox(&g, vec3(0.0, 0.025, 0.0) + kick, vec3(0.024, 0.02, 0.012), accent);
                    if v.wid == 11 {
                        receiver(m, &g, vec3(0.0, 0.028, -0.275), vec3(0.025, 0.028, 0.022), dark);
                        m.obox(&g, vec3(0.0, -0.065, -0.17), vec3(0.014, 0.035, 0.016), dark);
                    }
                    muzzle_z = if v.wid == 11 { -0.3 } else { -0.26 };
                }
            }
        }
        2 => {
            receiver(m, &g, vec3(0.0, -0.008, -0.1), vec3(0.03, 0.037, 0.3), wood);
            receiver(m, &g, vec3(0.0, 0.022, -0.02), vec3(0.026, 0.026, 0.15), metal);
            m.obox(&g, vec3(0.0, 0.05, -0.12), vec3(0.022, 0.003, 0.15), dark);
            let heavy = v.wid == 15;
            let barrel_end = if heavy {
                -0.89
            } else if v.wid == 9 {
                -0.77
            } else {
                -0.87
            };
            m.tube(
                &g,
                vec3(0.0, 0.015, (barrel_end - 0.32) * 0.5),
                if heavy { 0.02 } else { 0.014 },
                0.008,
                (-0.32 - barrel_end) * 0.5,
                16,
                dark,
            );
            if v.wid == 9 {
                // Ranger: a self-loading action, fixed handguard and charging handle.
                m.obox(&g, vec3(0.029, 0.024, 0.02), vec3(0.002, 0.013, 0.055), dark);
                m.obox(&g, vec3(0.038, 0.025, 0.055 + v.recoil * 0.04), vec3(0.014, 0.007, 0.009), hw);
                for z in [-0.35f32, -0.3, -0.25] {
                    m.obox(&g, vec3(0.0, 0.008, z), vec3(0.031, 0.004, 0.008), dark);
                }
            } else {
                let bz = 0.07 + bolt * 0.075;
                m.tube(&g, vec3(0.0, 0.022, bz), 0.011, 0.0, 0.05, 12, shade(hw, 1.3));
                let handle = g * Mat4::from_translation(vec3(0.026, 0.022, bz + 0.03)) * Mat4::from_rotation_z(-0.9 + bolt * 0.9);
                m.obox(&handle, vec3(0.022, 0.0, 0.0), vec3(0.022, 0.005, 0.005), hw);
                m.obox(&handle, vec3(0.046, 0.0, 0.0), vec3(0.009, 0.009, 0.009), dark);
            }
            m.obox(&g, vec3(0.0, -0.055, 0.03), vec3(0.006, 0.016, 0.04), dark);
            receiver(m, &g, vec3(0.0, -0.03, 0.25), vec3(0.028, 0.055, 0.1), wood);
            if fine {
                // A deeper stock, its underside sloping down to the toe.
                m.loft(&g, &[(0.155, 0.0, -0.078, 0.025, 0.007), (0.352, 0.0, -0.1, 0.027, 0.02)], 0.5, wood);
                m.obox(&g, vec3(0.0, -0.045, 0.37), vec3(0.03, 0.075, 0.02), dark);
            } else {
                m.obox(&g, vec3(0.0, -0.035, 0.37), vec3(0.03, 0.065, 0.02), dark);
            }
            if heavy {
                receiver(m, &g, vec3(0.0, 0.015, -0.92), vec3(0.037, 0.022, 0.035), hw);
                for z in [-0.935f32, -0.905] {
                    m.obox(&g, vec3(0.0, 0.015, z), vec3(0.038, 0.012, 0.006), dark);
                }
            } else {
                m.tube(&g, vec3(0.0, 0.015, barrel_end), 0.019, 0.008, 0.018, 16, hw);
            }
            if iron {
                front_sight(m, &g, barrel_end + 0.02, if heavy { 0.034 } else { 0.028 }, 0.071, accent, dark);
                rear_sight(m, &g, 0.1, 0.048, 0.071, 0.014, dark);
            }
            if fine {
                // Teeth on the scope rail, pins through the action, and a pad.
                rail(m, &g, 0.053, 0.02, -0.26, 0.02, shade(dark, 1.5));
                for z in [-0.12f32, 0.06] {
                    pin(m, &g, vec3(0.0, 0.008, z), 0.026, hw);
                }
                butt(m, &g, -0.115, 0.025, 0.028, 0.39, shade(dark, 1.6));
            }
            muzzle_z = if heavy { -0.96 } else { barrel_end - 0.022 };
        }
        _ => {
            receiver(m, &g, vec3(0.0, 0.0, -0.06), vec3(0.032, 0.05, if v.wid == 8 { 0.2 } else { 0.22 }), metal);
            let front = if v.wid == 8 { -0.29 } else { -0.35 };
            let handguard = if v.wid == 5 { wood } else { dark };
            receiver(m, &g, vec3(0.0, 0.0, front), vec3(0.029, 0.033, if v.wid == 8 { 0.055 } else { 0.085 }), handguard);
            for z in [front - 0.045, front, front + 0.045] {
                m.obox(&g, vec3(0.03, 0.01, z), vec3(0.0015, 0.01, 0.009), hw);
            }
            if v.wid == 8 {
                // Wasp: short receiver and a continuous suppressor around its bore.
                m.tube(&g, vec3(0.0, 0.012, -0.405), 0.027, 0.007, 0.085, 16, dark);
                m.tube(&g, vec3(0.0, 0.012, -0.327), 0.029, 0.007, 0.007, 16, hw);
            } else {
                // Separate barrel, gas block and muzzle pieces avoid coincident surfaces.
                let end = match v.wid {
                    5 => -0.69,
                    12 => -0.67,
                    _ => -0.59,
                };
                m.tube(
                    &g,
                    vec3(0.0, 0.012, (-0.37 + end) * 0.5),
                    if v.wid == 6 { 0.019 } else { 0.014 },
                    0.007,
                    (-0.37 - end) * 0.5,
                    16,
                    dark,
                );
                m.tube(&g, vec3(0.0, 0.012, -0.43), 0.022, 0.014, 0.018, 12, hw);
                m.tube(&g, vec3(0.0, 0.012, end), 0.019, 0.007, 0.012, 16, hw);
            }
            match v.wid {
                5 => {
                    // Warden: continuous wooden furniture and a long exposed barrel.
                    receiver(m, &g, vec3(0.0, -0.02, 0.245), vec3(0.03, 0.047, 0.093), wood);
                    if fine {
                        m.loft(&g, &[(0.16, 0.0, -0.062, 0.028, 0.006), (0.334, 0.0, -0.075, 0.029, 0.016)], 0.5, wood);
                    }
                }
                12 => {
                    // Lancer's ribbed fore-end and angular gas block.
                    m.obox(&g, vec3(0.0, 0.034, -0.37), vec3(0.027, 0.005, 0.075), accent);
                    for z in [-0.41f32, -0.37, -0.33] {
                        m.obox(&g, vec3(0.0, -0.032, z), vec3(0.03, 0.005, 0.008), hw);
                    }
                }
                6 => {
                    // Bulwark: carry handle with supports, bipod and a visible feed cover.
                    m.obox(&g, vec3(0.0, 0.097, -0.2), vec3(0.008, 0.007, 0.068), dark);
                    for z in [-0.26f32, -0.14] {
                        m.obox(&g, vec3(0.0, 0.073, z), vec3(0.008, 0.024, 0.008), metal);
                    }
                    m.obox(&g, vec3(0.0, 0.052, -0.1), vec3(0.035, 0.005, 0.07), metal);
                    for x in [-0.027f32, 0.027] {
                        m.obox(&g, vec3(x, -0.025, -0.48), vec3(0.005, 0.005, 0.1), dark);
                    }
                }
                _ => {}
            }
            if fine {
                let down =
                    g * Mat4::from_rotation_x(-0.3) * Mat4::from_translation(vec3(0.0, -0.04, 0.07)) * Mat4::from_rotation_x(PI / 2.0);
                m.loft(
                    &down,
                    &[
                        (0.0, 0.0, 0.0, 0.018, 0.024),
                        (0.045, 0.0, 0.003, 0.0205, 0.028),
                        (0.092, 0.0, 0.003, 0.02, 0.026),
                        (0.1, 0.0, 0.001, 0.019, 0.023),
                    ],
                    0.6,
                    dark,
                );
            } else {
                receiver(m, &(g * Mat4::from_rotation_x(-0.3)), vec3(0.0, -0.09, 0.07), vec3(0.02, 0.05, 0.026), dark);
            }
            m.obox(&g, vec3(0.0, -0.062, 0.04), vec3(0.005, 0.004, 0.045), dark);
            if v.wid != 5 {
                // Open gap below the stock strut reads clearly from the side.
                m.obox(&g, vec3(0.0, 0.002, 0.23), vec3(0.018, 0.018, 0.08), metal);
                if fine {
                    // The stock deepens toward the butt, its underside sloping.
                    m.loft(
                        &g,
                        &[(0.25, 0.0, -0.014, 0.022, 0.022), (0.3, 0.0, -0.028, 0.025, 0.038), (0.334, 0.0, -0.032, 0.027, 0.054)],
                        0.45,
                        dark,
                    );
                } else {
                    receiver(m, &g, vec3(0.0, -0.035, 0.3), vec3(0.026, 0.042, 0.036), dark);
                }
            }
            m.obox(&g, vec3(0.0, -0.03, 0.34), vec3(0.03, 0.06, 0.012), dark);
            m.obox(&g, vec3(0.0325, 0.012, -0.03), vec3(0.0015, 0.014, 0.04), dark);
            m.obox(&g, vec3(0.0, 0.052, 0.13 + v.recoil * 0.03), vec3(0.014, 0.006, 0.02), dark);
            m.obox(&g, vec3(0.0, 0.058, -0.02), vec3(0.031, 0.008, 0.1), accent);
            if iron {
                front_sight(m, &g, front - 0.05, if matches!(v.wid, 8 | 12) { 0.038 } else { 0.032 }, 0.087, accent, dark);
                rear_sight(m, &g, 0.08, 0.064, 0.087, 0.018, dark);
            }
            if fine {
                // Rail teeth along the top strip, pins through the receiver,
                // a trigger in its guard, the selector, slots down the
                // handguard and a ribbed butt.
                rail(m, &g, 0.066, 0.029, -0.115, 0.075, shade(accent, 0.7));
                for z in [0.02f32, -0.17] {
                    pin(m, &g, vec3(0.0, -0.026, z), 0.032, hw);
                }
                let trigger = g * Mat4::from_translation(vec3(0.0, -0.055, 0.03)) * Mat4::from_rotation_x(0.3);
                m.obox(&trigger, Vec3::ZERO, vec3(0.003, 0.008, 0.0025), hw);
                let selector = g * Mat4::from_translation(vec3(-0.033, -0.022, 0.08)) * Mat4::from_rotation_y(PI / 2.0);
                m.tube(&selector, Vec3::ZERO, 0.007, 0.0, 0.0015, 8, hw);
                m.obox(&selector, vec3(0.0, 0.006, -0.002), vec3(0.002, 0.006, 0.0012), hw);
                if v.wid != 5 {
                    let reach = if v.wid == 8 { 0.03 } else { 0.055 };
                    let mut z = front - reach;
                    while z < front + reach - 0.02 {
                        for x in [-0.0294f32, 0.0294] {
                            m.obox(&g, vec3(x, -0.014, z + 0.01), vec3(0.0008, 0.0045, 0.011), shade(dark, 0.6));
                        }
                        z += 0.03;
                    }
                }
                butt(m, &g, -0.088, 0.028, 0.028, 0.352, shade(dark, 1.6));
            }
            muzzle_z = match v.wid {
                5 => -0.705,
                12 => -0.685,
                8 => -0.495,
                _ => -0.605,
            };
        }
    }
    if mag_h != Vec3::ZERO && mag_out < 0.97 {
        if v.wid == 14 {
            let drum = g * Mat4::from_translation(mag_c + mag_off) * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);
            m.tube(&drum, Vec3::ZERO, 0.057, 0.0, 0.035, 16, dark);
            m.tube(&drum, vec3(0.0, 0.0, 0.036), 0.03, 0.0, 0.002, 12, hw);
        } else if fine && matches!(v.wid, 0 | 12) {
            // A curved box magazine, sweeping forward as it goes down.
            let top = g * Mat4::from_translation(mag_c + mag_off + vec3(0.0, mag_h.y, 0.0)) * Mat4::from_rotation_x(PI / 2.0);
            let depth = mag_h.y * 2.0;
            let secs: Vec<(f32, f32, f32, f32, f32)> = (0..6)
                .map(|k| {
                    let t = k as f32 / 5.0;
                    (depth * t, 0.0, -0.032 * t.powf(1.6), mag_h.x, mag_h.z * (1.0 + 0.06 * t))
                })
                .collect();
            m.loft(&top, &secs, 0.35, dark);
            let base = top * Mat4::from_translation(vec3(0.0, -0.032, depth + 0.004)) * Mat4::from_rotation_x(-0.25);
            m.obox(&base, Vec3::ZERO, vec3(mag_h.x * 1.08, mag_h.z * 1.12, 0.005), hw);
        } else {
            receiver(m, &g, mag_c + mag_off, mag_h, dark);
            m.obox(&g, mag_c + mag_off - vec3(0.0, mag_h.y, 0.0), vec3(mag_h.x * 1.05, 0.005, mag_h.z * 1.02), hw);
            if kit == 0 && v.wid != 6 {
                for x in [-1.0f32, 1.0] {
                    m.obox(&g, mag_c + mag_off + vec3(x * (mag_h.x + 0.0008), 0.0, 0.0), vec3(0.001, mag_h.y * 0.7, mag_h.z * 0.3), hw);
                }
            }
        }
    }
    if !iron {
        optic(m, &g, v.wid, v.sight, accent, dark, metal);
    }
    if v.preview.is_some() {
        return;
    }

    // ---- arms: elbows hang in camera space, hands stay on the weapon ----
    let follow = (pos - hip) * 0.55;
    let r_el = vec3(0.36, -0.56, -0.16) + follow;
    let l_el = vec3(-0.16, -0.56, -0.3) + follow;
    // Right hand holds the grip, or reaches up to work the bolt.
    let r_gun = grip + (vec3(0.07, 0.03, 0.1 + bolt * 0.07) - grip) * bolt.min(1.0);
    let r_hand = g.transform_point3(r_gun);
    // Left hand supports the fore-end, rides the pump, or handles the magazine.
    let l_gun = if v.wid == 10 && reloading {
        support + (vec3(-0.065 * to_mag, 0.0, 0.035) - support) * to_mag
    } else if v.wid == 16 && reloading {
        support + (vec3(0.0, -0.015, -0.065) - support) * to_mag
    } else if kit == 1 && v.wid != 14 && reloading {
        // feeding shells one at a time
        let push = (r * 4.0 * PI).sin().abs();
        let port = vec3(0.0, -0.075 - 0.07 * (1.0 - push), -0.06);
        support + (port - support) * to_mag
    } else {
        let at_mag = mag_c + mag_off - vec3(0.0, mag_h.y + 0.02, 0.0);
        support + slide_back + (at_mag - support) * to_mag
    };
    let l_hand = g.transform_point3(l_gun);
    let cam = Mat4::IDENTITY;
    for (el, hand, thumb_x) in [(r_el, r_hand, -1.0f32), (l_el, l_hand, 1.0)] {
        let d = (hand - el).normalize_or_zero();
        let wrist = hand - d * 0.075;
        limb(m, &cam, el, wrist, 0.043, true, sleeve);
        limb(m, &cam, wrist - d * 0.035, wrist, 0.047, true, cuff);
        limb(m, &cam, wrist, hand + d * 0.03, 0.036, false, glove);
        // thumb wrapped over the top
        let side = d.cross(Vec3::Y).normalize_or_zero();
        m.obox(&cam, hand + side * (0.034 * thumb_x) + vec3(0.0, 0.022, 0.0), vec3(0.012, 0.013, 0.03), shade(glove, 1.2));
    }
    if kit == 1 && v.wid != 14 && reloading && to_mag > 0.6 {
        // the shell going in
        m.obox(&g, l_gun + vec3(0.0, 0.03, -0.01), vec3(0.011, 0.011, 0.028), rgb(0.75, 0.2, 0.15));
    }

    if v.flash > 0.0 {
        let c = g.transform_point3(vec3(0.0, 0.015, muzzle_z));
        let s = (0.012 + 0.02 * v.flash) * if kit == 1 { 1.5 } else { 1.0 };
        let col = [1.0, 0.8, 0.4, v.flash];
        let a0 = hash01(v.seq, 5) * PI;
        add.quad(
            c + vec3(-s, -s, 0.0) * 0.7,
            c + vec3(s, -s, 0.0) * 0.7,
            c + vec3(s, s, 0.0) * 0.7,
            c + vec3(-s, s, 0.0) * 0.7,
            [1.0, 0.95, 0.75, v.flash],
        );
        for j in 0..3u32 {
            let a = a0 + j as f32 * PI / 3.0;
            let (u, w2) = (vec3(a.cos(), a.sin(), 0.0), vec3(-a.sin(), a.cos(), 0.0));
            let len = s * (1.7 + 0.8 * hash01(v.seq, 9 + j));
            add.quad(c - u * len, c - w2 * s * 0.35, c + u * len, c + w2 * s * 0.35, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carried() -> impl Iterator<Item = u8> {
        (0..sim::WEAPONS.len() as u8).filter(|&wid| wid != sim::GRENADE_WEAPON && wid != sim::MELEE_WEAPON)
    }

    #[test]
    fn every_carried_weapon_has_valid_geometry_through_its_action_and_reload() {
        let mut largest = (0, 0);
        for wid in carried() {
            for skin in sim::weapon_skins_for(wid) {
                for (reload, cycle) in [(-1.0, 0.0), (-1.0, 0.5), (0.25, 0.0), (0.6, 0.0), (1.0, 0.0)] {
                    let mut mesh = Mesh::default();
                    gun_model_pose(&mut mesh, wid, 0, skin, Mat4::IDENTITY, reload, cycle, true);
                    assert!(!mesh.v.is_empty(), "{} has no model", weapon_def(wid).name);
                    assert!(mesh.chunks.is_empty(), "one weapon exceeded a mesh page");
                    assert!(mesh.i.iter().all(|&i| (i as usize) < mesh.v.len()));
                    assert!(mesh.v.iter().all(|v| v.pos.iter().chain(&v.nrm).chain(&v.col).all(|x| x.is_finite())));
                    assert!(
                        mesh.v.iter().all(|v| Vec3::from(v.pos).abs().max_element() < 1.1),
                        "{} outside weapon bounds",
                        weapon_def(wid).name
                    );
                    // Fine guns are for first person and soldiers within a few
                    // metres; the far gun is what everyone else carries.
                    assert!(mesh.v.len() < 30000, "{}: {} vertices", weapon_def(wid).name, mesh.v.len());
                    let mut far = Mesh::default();
                    gun_model_pose(&mut far, wid, 0, skin, Mat4::IDENTITY, reload, cycle, false);
                    assert!(far.v.len() < 13000, "{} far: {} vertices", weapon_def(wid).name, far.v.len());
                    assert!(mesh.triangles() > far.triangles() * 3 / 2, "{} is hardly finer up close", weapon_def(wid).name);
                    assert!(!far.fine && !mesh.fine, "fine mode is left on");
                    if mesh.v.len() > largest.1 {
                        largest = (wid, mesh.v.len());
                    }
                }
            }
        }
        eprintln!("largest weapon: {} ({} vertices)", weapon_def(largest.0).name, largest.1);
    }

    /// Triangles each finish adds over the factory gun, worst weapon first.
    #[test]
    fn finishes_stay_within_the_triangle_budget() {
        let tris = |wid: u8, skin: u8| {
            let mut mesh = Mesh::default();
            gun_model(&mut mesh, wid, 0, skin, Mat4::IDENTITY);
            mesh.triangles()
        };
        let mut worst = (0, 0, 0);
        for skin in 0..sim::WEAPON_SKINS.len() as u8 {
            let mut row = format!("{:>13}", sim::weapon_skin(skin).name);
            for wid in carried() {
                let (base, with) = (tris(wid, 0), tris(wid, skin));
                row += &format!(" {with:>5}");
                if with > worst.2 {
                    worst = (wid, skin, with);
                }
                if skin > 0 {
                    assert!(with <= base + 900, "{} on {}: {with} triangles", sim::weapon_skin(skin).name, weapon_def(wid).name);
                }
                // A distant soldier's gun costs the same in every finish.
                let far_tris = |skin: u8| {
                    let mut far = Mesh::default();
                    gun_model_pose(&mut far, wid, 0, skin, Mat4::IDENTITY, -1.0, 0.0, false);
                    far.triangles()
                };
                assert_eq!(far_tris(skin), far_tris(0), "{} adds triangles far away", sim::weapon_skin(skin).name);
            }
            eprintln!("{row}");
        }
        eprintln!("heaviest: {} in {} ({} triangles)", weapon_def(worst.0).name, sim::weapon_skin(worst.1).name, worst.2);
    }

    #[test]
    fn turning_loadout_models_fit_their_cards() {
        for wid in carried() {
            for turn in [0.0, 0.65, 1.4, 2.1] {
                let mut mesh = Mesh::default();
                gun_icon(&mut mesh, wid, 1, 0, 100.0, 50.0, 180.0, 64.0, turn);
                assert!(
                    mesh.v.iter().all(|v| (v.pos[0] - 100.0).abs() <= 90.01 && (v.pos[1] - 50.0).abs() <= 32.01),
                    "{} clips its rotating preview card",
                    weapon_def(wid).name
                );
            }
        }
    }
}
