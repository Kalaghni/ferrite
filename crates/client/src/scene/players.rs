//! Soldier silhouettes, equipment and movement poses.
//!
//! The shape language follows the launcher art: heavy, broad-shouldered
//! troopers built from faceted plates (every edge cut back, flat shaded),
//! pale armour layered over a dark undersuit, a domed helmet with a
//! glowing visor, big shoulder plates and a pack whose lid carries the
//! team colour and two pale recognition bars.

use super::team_col;
use super::weapons::{gun_grips, gun_model_pose};
use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use ferrite_shared::proto::flag;

#[derive(Clone, Copy)]
pub struct PlayerView {
    pub id: u8,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,
    pub flags2: u8,
    pub hp: u8,
    pub team: u8,
    pub kit: u8,
    /// Family of the primary carried (0 rifle, 1 shotgun, 2 long rifle).
    pub gun: u8,
    /// Exact weapon in hand, shared with first-person and loadout geometry.
    pub wid: u8,
    /// Reload progress; -1 when not reloading.
    pub reload_phase: f32,
    /// Working-action progress: 1 immediately after firing, 0 when ready.
    pub cycle: f32,
    /// Look: index into `sim::SKINS`.
    pub skin: u8,
    /// Finish on the weapon in hand: index into `sim::WEAPON_SKINS`.
    pub wskin: u8,
    /// 0..1 through a melee strike.
    pub melee: f32,
    pub phase: f32,
    pub moving: f32,
    /// Smoothed 0..1 pose weights.
    pub crouch: f32,
    pub air: f32,
    pub slide: f32,
    pub reload: f32,
    /// 1 on the frame a shot is fired, decaying.
    pub fire: f32,
    /// 1 on the frame damage is taken, decaying.
    pub hurt: f32,
    /// Velocity in the soldier's own frame (forward, right), m/s.
    pub vel: (f32, f32),
    /// 0 = on their feet, 1 = flat on the ground.
    pub fall: f32,
    /// How far a body has sunk into the ground before it is removed.
    pub sink: f32,
    /// Far enough away that the weapon's finish is drawn in its colours
    /// only, without its pattern.
    pub far: bool,
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            id: 0,
            pos: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            flags: 0,
            flags2: 0,
            hp: 0,
            team: 0,
            kit: 0,
            gun: 0,
            wid: 0,
            reload_phase: -1.0,
            cycle: 0.0,
            skin: 0,
            wskin: 0,
            melee: 0.0,
            phase: 0.0,
            moving: 0.0,
            crouch: 0.0,
            air: 0.0,
            slide: 0.0,
            reload: 0.0,
            fire: 0.0,
            hurt: 0.0,
            vel: (0.0, 0.0),
            fall: 0.0,
            sink: 0.0,
            far: false,
        }
    }
}

/// One flat face, wound so that it faces away from `inside`.
fn face(m: &mut Mesh, p: &[Vec3], inside: Vec3, col: Col) {
    let n = (p[1] - p[0]).cross(p[p.len() - 1] - p[0]);
    let mid = p.iter().copied().sum::<Vec3>() / p.len() as f32;
    let out = n.dot(mid - inside) >= 0.0;
    match (p.len(), out) {
        (3, true) => m.tri(p[0], p[1], p[2], col),
        (3, false) => m.tri(p[0], p[2], p[1], col),
        (_, true) => m.quad(p[0], p[1], p[2], p[3], col),
        (_, false) => m.quad(p[0], p[3], p[2], p[1], col),
    }
}

/// The plate everything is built from: a box of half-size `h` with all
/// twelve edges cut back by `k` (0..1 of each half-size, limited by the
/// longest so long parts do not come to a point), which gives the six
/// faces, twelve bevels and eight corner facets of the art's armour.
/// `top` widens (> 1) or narrows (< 1) the +y end in x and z; every face
/// stays flat under that taper.
fn slab(m: &mut Mesh, g: &Mat4, c: Vec3, h: Vec3, top: f32, k: f32, col: Col) {
    let cap = h.max_element() * 0.6;
    let cut = vec3(h.x.min(cap), h.y.min(cap), h.z.min(cap)) * k;
    let s = [1.0f32, -1.0];
    // Corner (sx, sy, sz) has one vertex for each axis: the one lying on
    // that axis' face.
    let v = |axis: usize, sx: f32, sy: f32, sz: f32| {
        let sg = [sx, sy, sz];
        let mut p = [0.0f32; 3];
        for i in 0..3 {
            let full = h[i];
            p[i] = sg[i] * if i == axis { full } else { full - cut[i] };
        }
        let t = 1.0 + (top - 1.0) * (p[1] / h.y + 1.0) * 0.5;
        g.transform_point3(c + vec3(p[0] * t, p[1], p[2] * t))
    };
    let inside = g.transform_point3(c);
    let darker = shade(col, 0.93);
    // Faces: the four corners of each, in order round it.
    let ring = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for &sa in &s {
        face(m, &ring.map(|(a, b)| v(0, sa, a, b)), inside, col);
        face(m, &ring.map(|(a, b)| v(1, a, sa, b)), inside, if sa > 0.0 { col } else { darker });
        face(m, &ring.map(|(a, b)| v(2, a, b, sa)), inside, col);
    }
    // Bevels along each edge.
    for &a in &s {
        for &b in &s {
            // Edges running along z (between the x and y faces).
            face(m, &[v(0, a, b, -1.0), v(1, a, b, -1.0), v(1, a, b, 1.0), v(0, a, b, 1.0)], inside, col);
            // Along y (between x and z).
            face(m, &[v(0, a, -1.0, b), v(2, a, -1.0, b), v(2, a, 1.0, b), v(0, a, 1.0, b)], inside, col);
            // Along x (between y and z).
            face(m, &[v(1, -1.0, a, b), v(2, -1.0, a, b), v(2, 1.0, a, b), v(1, 1.0, a, b)], inside, col);
        }
    }
    for &x in &s {
        for &y in &s {
            for &z in &s {
                face(m, &[v(0, x, y, z), v(1, x, y, z), v(2, x, y, z)], inside, col);
            }
        }
    }
}

/// Frame along a bone: origin at its middle, +y pointing from `b` to `a`,
/// +z kept as close to the body's forward as the bone allows.
fn bone(g: &Mat4, a: Vec3, b: Vec3) -> Option<(Mat4, f32)> {
    let d = a - b;
    if d.length_squared() < 1e-8 {
        return None;
    }
    let y = d.normalize();
    let guide = if y.z.abs() > 0.9 { Vec3::Y } else { Vec3::Z };
    let x = y.cross(guide).normalize();
    let z = x.cross(y);
    let basis = Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), ((a + b) * 0.5).extend(1.0));
    Some((*g * basis, d.length()))
}

/// An eight-sided limb from `b` up to `a`, wider at `a` by `top`.
fn segment(m: &mut Mesh, g: &Mat4, a: Vec3, b: Vec3, radius: f32, top: f32, col: Col) {
    if let Some((f, len)) = bone(g, a, b) {
        slab(m, &f, Vec3::ZERO, vec3(radius * 0.9, len * 0.5 + radius * 0.35, radius), top, 0.42, col);
    }
}

/// Scale of a gun in another soldier's hands.
const GUN_SCALE: f32 = 0.72;
/// Beyond this many metres a soldier's weapon finish keeps its colours but
/// drops its pattern: the paint is under a pixel there, and its layers sit
/// closer together than the depth buffer can separate.
pub const FINISH_DETAIL_RANGE: f32 = 10.0;
/// How far in front of the chest plate's face (negative: sunk into it) the
/// butt of a shouldered gun rests.
const BUTT_SEAT: f32 = -0.03;

/// Where the front of the chest plate is, in body space.
fn chest_front(bulk: f32) -> f32 {
    0.152 * bulk + 0.032
}

/// How far forward of the spine a gun's origin is held. Pistols are held
/// out at arm's length; anything with a stock is held so that the stock
/// ends at the chest plate and not inside it.
fn gun_reach(wid: u8, bulk: f32) -> f32 {
    let arch = ferrite_shared::sim::weapon_def(wid).arch;
    if arch == 4 {
        return 0.30;
    }
    let butt = if arch == 2 { 0.39 } else { 0.36 } * GUN_SCALE;
    (chest_front(bulk) + BUTT_SEAT + butt).max(0.28)
}

/// Two-bone solve with a stable bend pole. A small amount of reach stretch
/// accommodates extreme aim poses without detaching hands from the gun.
fn joint(a: Vec3, b: Vec3, upper: f32, lower: f32, pole: Vec3) -> Vec3 {
    let d = b - a;
    let len = d.length().max(1e-4);
    let axis = d / len;
    let stretch = (len / ((upper + lower) * 0.995)).max(1.0);
    let (u, l) = (upper * stretch, lower * stretch);
    let along = ((u * u - l * l + len * len) / (2.0 * len)).clamp(0.0, u);
    let bend = (pole - axis * pole.dot(axis)).normalize_or_zero();
    a + axis * along + bend * (u * u - along * along).max(0.0).sqrt()
}

/// Mix a colour toward white: the hot core of a visor's glow.
fn hot(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] + (1.0 - c[0]) * k, c[1] + (1.0 - c[1]) * k, c[2] + (1.0 - c[2]) * k]
}

/// A soldier with planted foot targets and articulated arms. All equipment
/// follows the same body rig while head and weapon retain an independent aim.
pub fn soldier(m: &mut Mesh, alpha: &mut Mesh, w: &World, p: &PlayerView, time: f32, blob: bool) {
    let tc = team_col(p.team);
    let shield = p.flags & flag::SHIELD != 0;
    let glow = if p.flags & flag::SURGE != 0 { 0.35 + 0.25 * (time * 9.0).sin() } else { 0.0 };
    let h = p.hurt.clamp(0.0, 1.0) * 0.6;
    let tint = |c: [f32; 3], e: f32| -> Col { [c[0] + (1.0 - c[0]) * h, c[1] + (1.0 - c[1]) * h, c[2] + (1.0 - c[2]) * h, e.max(h * 0.5)] };
    let main = tint(tc, glow);
    let look = p.skin % 6;
    let rare = p.skin as usize % ferrite_shared::sim::SKINS.len() >= 6;
    // Undersuit, armour, webbing, warning paint and markings. The art
    // keeps pale armour over a dark suit, with team colour reserved for
    // the shoulders, the pack lid and the visor. The armory looks (ids 6
    // and up) are the same six shapes in rarer colours.
    let mut pal = match look {
        1 => ([0.30, 0.31, 0.27], [0.50, 0.49, 0.44], [0.34, 0.25, 0.16], [0.95, 0.72, 0.13], [0.86, 0.9, 0.9]),
        3 => ([0.075, 0.085, 0.105], [0.27, 0.29, 0.31], [0.12, 0.13, 0.14], [0.95, 0.72, 0.13], [0.86, 0.9, 0.9]),
        4 => ([0.27, 0.27, 0.22], [0.60, 0.58, 0.54], [0.34, 0.25, 0.16], [0.80, 0.64, 0.30], [0.86, 0.9, 0.9]),
        5 => ([0.24, 0.26, 0.29], [0.66, 0.65, 0.62], [0.22, 0.21, 0.19], [0.95, 0.72, 0.13], [0.86, 0.9, 0.9]),
        _ => ([0.21, 0.205, 0.215], [0.71, 0.69, 0.67], [0.20, 0.19, 0.18], [0.95, 0.72, 0.13], [0.88, 0.91, 0.92]),
    };
    if rare {
        pal = match look {
            0 => ([0.055, 0.058, 0.066], [0.12, 0.125, 0.135], [0.09, 0.09, 0.10], [0.92, 0.94, 0.96], [0.96, 0.97, 0.98]),
            1 => ([0.58, 0.47, 0.31], [0.47, 0.25, 0.14], [0.30, 0.19, 0.11], [0.86, 0.50, 0.22], [0.93, 0.86, 0.70]),
            2 => ([0.15, 0.16, 0.18], [0.30, 0.33, 0.38], [0.20, 0.17, 0.10], [0.96, 0.76, 0.22], [0.98, 0.82, 0.30]),
            3 => ([0.80, 0.78, 0.72], [0.56, 0.56, 0.54], [0.38, 0.36, 0.33], [0.70, 0.72, 0.74], [0.20, 0.21, 0.23]),
            4 => ([0.43, 0.07, 0.08], [0.63, 0.49, 0.20], [0.21, 0.12, 0.07], [0.85, 0.66, 0.24], [0.95, 0.83, 0.48]),
            _ => ([0.07, 0.09, 0.07], [0.12, 0.14, 0.12], [0.10, 0.12, 0.09], [0.46, 0.96, 0.16], [0.62, 1.0, 0.30]),
        };
    }
    let dark = tint(pal.0, glow * 0.2);
    let armor = tint(pal.1, 0.0);
    let metal = tint([0.13, 0.15, 0.18], 0.0);
    let skin = tint([0.78, 0.65, 0.51], 0.0);
    let glove = tint([0.13, 0.13, 0.14], 0.0);
    let webbing = tint(pal.2, 0.0);
    let hazard = tint(pal.3, 0.0);
    let markings = tint(pal.4, 0.0);
    let boots = tint([0.15, 0.14, 0.14], 0.0);
    let visor = tint([0.06, 0.07, 0.09], 0.0);
    // Blackout's white marks and Juggernaut's gold carry over to the
    // visor; everyone else's glows in their team colour.
    let glass = if rare && matches!(look, 0 | 2) { tint(hot(pal.4, 0.2), 0.9) } else { tint(hot(tc, 0.35), 0.95) };
    // Gear on the pack and belt: a shade between suit and webbing.
    let gear = tint([(pal.0[0] + pal.2[0]) * 0.6, (pal.0[1] + pal.2[1]) * 0.6, (pal.0[2] + pal.2[2]) * 0.6], 0.0);

    let sl = p.slide.clamp(0.0, 1.0);
    let c = p.crouch.clamp(0.0, 1.0).max(sl);
    let kneel = c * (1.0 - sl);
    let air = p.air.clamp(0.0, 1.0) * (1.0 - c);
    let speed = (p.vel.0 * p.vel.0 + p.vel.1 * p.vel.1).sqrt();
    let sprint = ((speed - 6.4) / 1.6).clamp(0.0, 1.0) * (1.0 - c);
    let fall = p.fall.clamp(0.0, 1.0);
    let fall = fall * fall * (3.0 - 2.0 * fall);
    let locomotion = p.moving.clamp(0.0, 1.0) * (1.0 - sl) * (1.0 - air) * (1.0 - fall);
    let root = Mat4::from_translation(p.pos + vec3(0.0, 0.19 * fall - p.sink, 0.0))
        * Mat4::from_rotation_y(p.yaw)
        * Mat4::from_rotation_z((if p.id.is_multiple_of(2) { 0.13 } else { -0.13 }) * fall)
        * Mat4::from_rotation_x(-fall * 1.5);
    let bulk = if p.kit == 1 {
        1.16
    } else if p.kit == 2 {
        0.96
    } else {
        1.0
    };
    // How much plate a look wears: the scout's light rig, the heavy's
    // layered suit.
    let plated: f32 = match look {
        1 => 0.7,
        2 => 1.12,
        3 => 0.85,
        4 => 0.9,
        _ => 1.0,
    };
    let phase = p.phase * 2.2;
    let walk_dir = if speed > 0.1 { vec3(-p.vel.1 / speed, 0.0, p.vel.0 / speed) } else { Vec3::Z };
    let hip_y = 0.84 - 0.44 * c;
    let bounce = (phase * 2.0).cos() * 0.014 * locomotion * (1.0 - c);

    // Feet travel backwards during the planted half of each cycle, then
    // lift through the recovery half. IK preserves the ankle target while
    // knees bend instead of stretching the whole leg into the floor.
    for (sign, offset) in [(1.0f32, 0.0f32), (-1.0, std::f32::consts::PI)] {
        let ph = phase + offset;
        let reach = (0.30 + 0.13 * sprint) * locomotion * (1.0 - 0.65 * kneel);
        let lift = ph.cos().max(0.0).powf(1.5) * (0.11 + 0.045 * sprint) * locomotion;
        let hip = vec3(sign * 0.15, hip_y + bounce, 0.0);
        let mut ankle = vec3(sign * 0.165, 0.10 + lift, 0.0) + walk_dir * (ph.sin() * reach);
        ankle.z += kneel * if sign > 0.0 { 0.28 } else { -0.22 };
        ankle += vec3(sign * 0.025, 0.055, 0.64 - sign * 0.07) * sl;
        ankle += vec3(sign * 0.035, 0.31 + sign * 0.045, -0.16) * air;
        ankle.x += sign * 0.11 * fall;
        if p.flags & flag::GROUND != 0 && c < 0.7 && fall < 0.01 {
            let foot_world = root.transform_point3(ankle);
            let ground = w.height(p.pos.x, p.pos.z);
            if (ground - p.pos.y).abs() < 0.18 {
                ankle.y += (w.height(foot_world.x, foot_world.z) - ground).clamp(-0.10, 0.16);
            }
        }
        let knee = joint(hip, ankle, 0.39, 0.38, Vec3::Z);
        // Thigh: suit, with a plate over its front and outside.
        segment(m, &root, hip, knee, 0.124 * bulk, 1.15, dark);
        if let Some((f, len)) = bone(&root, hip, knee) {
            slab(
                m,
                &f,
                vec3(sign * 0.028, 0.02, 0.05),
                vec3(0.095 * bulk * plated.min(1.0), len * 0.3, 0.06),
                1.12,
                0.4,
                if look == 3 { dark } else { shade(armor, 0.94) },
            );
            if look == 2 {
                // The heavy's tasset hangs over the hip.
                slab(m, &f, vec3(sign * 0.075, len * 0.26, 0.02), vec3(0.06, 0.1, 0.1), 1.1, 0.4, armor);
            }
            if sign < 0.0 && look != 3 {
                // Holster on the right thigh.
                slab(m, &f, vec3(-0.11 * bulk, 0.04, -0.005), vec3(0.03, 0.095, 0.055), 0.9, 0.4, gear);
            }
        }
        // Shin: an armoured greave from knee to boot, and a big knee pad.
        segment(m, &root, knee, ankle, 0.088 * bulk, 1.1, shade(dark, 0.92));
        if let Some((f, len)) = bone(&root, knee, ankle) {
            slab(
                m,
                &f,
                vec3(0.0, -0.01, 0.012),
                vec3(0.095 * bulk, len * 0.36, 0.093 * bulk),
                1.15,
                0.42,
                if look == 3 { shade(dark, 1.25) } else { armor },
            );
            if look == 5 {
                slab(m, &f, vec3(0.0, -len * 0.12, 0.012), vec3(0.104 * bulk, 0.03, 0.102 * bulk), 1.0, 0.3, hazard);
            }
            slab(
                m,
                &f,
                vec3(0.0, len * 0.5, 0.085),
                vec3(0.088 * bulk, 0.088, 0.055),
                0.8,
                0.48,
                if look == 3 { metal } else { shade(armor, 1.06) },
            );
        }
        let boot = root
            * Mat4::from_translation(ankle)
            * Mat4::from_rotation_y((-p.vel.1 / speed.max(1.0)).clamp(-1.0, 1.0) * 0.25 * locomotion)
            * Mat4::from_rotation_x(-ph.sin().max(0.0) * 0.18 * locomotion);
        slab(m, &boot, vec3(0.0, -0.02, 0.04), vec3(0.1, 0.08, 0.155), 0.86, 0.38, boots);
        slab(m, &boot, vec3(0.0, -0.085, 0.045), vec3(0.105, 0.018, 0.163), 1.0, 0.35, shade(boots, 0.6));
        slab(m, &boot, vec3(0.0, 0.02, 0.15), vec3(0.07, 0.035, 0.045), 0.8, 0.45, shade(boots, 1.25));
    }

    // Rotation is about the hips. Weight shifts toward the supporting leg;
    // breathing and counter-rotation disappear as a body comes to rest.
    let idle = (time * 1.7 + p.id as f32).sin() * 0.006 * (1.0 - fall);
    let lean =
        (0.075 * (p.vel.0 / 5.6).clamp(-1.0, 1.0) + 0.18 * sprint) * (1.0 - sl) + 0.17 * kneel - 0.58 * sl + 0.09 * air - 0.04 * p.fire;
    let roll = (p.vel.1 / 5.6).clamp(-1.0, 1.0) * 0.08 * (1.0 - c) + phase.sin() * 0.025 * locomotion * (1.0 - c);
    let body = root
        * Mat4::from_translation(vec3(0.0, idle + bounce - 0.56 * c + 0.84, 0.0))
        * Mat4::from_rotation_x(lean)
        * Mat4::from_rotation_z(roll)
        * Mat4::from_rotation_y(phase.sin() * 0.035 * locomotion * (1.0 - c))
        * Mat4::from_translation(vec3(0.0, -0.84, 0.0));

    // ---- torso: suit, belt, abdomen and the chest plate over it ----
    let b = bulk;
    slab(m, &body, vec3(0.0, 0.92, 0.0), vec3(0.19 * b, 0.11, 0.13 * b), 1.05, 0.4, dark);
    slab(m, &body, vec3(0.0, 1.15, -0.005), vec3(0.215 * b, 0.25, 0.135 * b), 1.16, 0.4, dark);
    slab(m, &body, vec3(0.0, 0.875, 0.0), vec3(0.225 * b, 0.042, 0.152 * b), 1.0, 0.35, webbing);
    slab(m, &body, vec3(0.0, 0.875, 0.152 * b), vec3(0.04, 0.03, 0.012), 1.0, 0.3, metal);
    // Groin plate and the abdomen's two bands.
    slab(m, &body, vec3(0.0, 0.79, 0.115 * b), vec3(0.085, 0.06, 0.03), 0.8, 0.4, shade(armor, 0.86));
    slab(m, &body, vec3(0.0, 1.0, 0.075 * b), vec3(0.175 * b, 0.055, 0.075 * b), 1.04, 0.42, shade(armor, 0.86));
    for sign in [-1.0f32, 1.0] {
        // Pouches round the hips.
        slab(m, &body, vec3(sign * 0.205 * b, 0.84, 0.06), vec3(0.042, 0.065, 0.06), 0.92, 0.4, gear);
        slab(m, &body, vec3(sign * 0.12 * b, 0.84, 0.162 * b), vec3(0.05, 0.055, 0.03), 0.92, 0.4, gear);
    }
    // The cuirass: wide over the chest, cut in at the waist.
    let chest_col = if look == 1 { shade(armor, 0.82) } else { armor };
    slab(m, &body, vec3(0.0, 1.245, 0.02), vec3(0.245 * b * plated.max(0.9), 0.165, 0.132 * b), 1.12, 0.38, chest_col);
    // A team tab on the chest, and a second plate layer for the heavy.
    slab(m, &body, vec3(-0.115 * b, 1.3, chest_front(b) + 0.002), vec3(0.048, 0.03, 0.01), 1.0, 0.3, main);
    if look == 2 {
        slab(m, &body, vec3(0.0, 1.21, chest_front(b) - 0.01), vec3(0.17 * b, 0.1, 0.03), 1.12, 0.4, shade(armor, 1.06));
    }
    match look {
        1 => {
            // The scout's rig: three magazine pouches across the front.
            for x in [-0.11f32, 0.0, 0.11] {
                slab(m, &body, vec3(x * b, 1.13, chest_front(b) + 0.02), vec3(0.048, 0.07, 0.032), 0.92, 0.4, webbing);
            }
        }
        4 => {
            // The veteran's bandolier, shoulder to hip.
            let strap = body * Mat4::from_translation(vec3(0.0, 1.13, chest_front(b) + 0.012)) * Mat4::from_rotation_z(0.72);
            slab(m, &strap, Vec3::ZERO, vec3(0.034, 0.3, 0.014), 1.0, 0.3, webbing);
            for k in -2i32..=2 {
                slab(m, &strap, vec3(0.0, k as f32 * 0.1, 0.018), vec3(0.025, 0.028, 0.012), 1.0, 0.35, hazard);
            }
        }
        5 => {
            for y in [1.17f32, 1.33] {
                slab(m, &body, vec3(0.0, y, chest_front(b) - 0.004), vec3(0.2 * b, 0.02, 0.012), 1.0, 0.3, hazard);
            }
        }
        _ => {}
    }
    // Collar.
    slab(m, &body, vec3(0.0, 1.425, -0.01), vec3(0.135, 0.06, 0.12), 0.8, 0.45, shade(dark, 1.1));
    if look == 2 {
        slab(m, &body, vec3(0.0, 1.415, 0.02), vec3(0.17, 0.05, 0.15), 0.85, 0.42, armor);
    }

    // ---- shoulder plates: big, layered, team colour on the top ----
    let pad = match look {
        1 => 0.72,
        2 => 1.15,
        3 => 0.85,
        _ => 1.0,
    };
    for sign in [-1.0f32, 1.0] {
        let shoulder = body * Mat4::from_translation(vec3(sign * 0.3 * b, 1.35, -0.005)) * Mat4::from_rotation_z(sign * 0.3);
        let pc = if look == 3 { shade(dark, 1.6) } else { armor };
        slab(m, &shoulder, vec3(sign * 0.045, -0.015, 0.0), vec3(0.135, 0.12, 0.17) * pad, 0.7, 0.5, pc);
        slab(m, &shoulder, vec3(sign * 0.055, 0.075 * pad, 0.0), vec3(0.122, 0.045, 0.158) * pad, 0.78, 0.5, main);
        if look == 2 {
            // The heavy's lower lame.
            slab(m, &shoulder, vec3(sign * 0.1, -0.12, 0.0), vec3(0.06, 0.07, 0.15), 0.9, 0.45, shade(armor, 0.9));
        }
    }

    // ---- the pack: dark body, team-coloured lid, two pale bars ----
    let pack_w = match p.kit {
        1 => 0.23,
        2 => 0.19,
        _ => 0.21,
    } * if look == 1 { 0.88 } else { 1.0 };
    let pack_h = if p.kit == 2 { 0.235 } else { 0.21 };
    let back = -0.135 * b;
    slab(m, &body, vec3(0.0, 1.14, back - 0.095), vec3(pack_w, pack_h, 0.095), 1.0, 0.32, gear);
    let lid = body * Mat4::from_translation(vec3(0.0, 1.27, back - 0.19)) * Mat4::from_rotation_x(-0.3);
    slab(m, &lid, Vec3::ZERO, vec3(pack_w * 1.08, 0.14, 0.035), 1.04, 0.32, main);
    for x in [-0.036f32, 0.036] {
        let bar = lid * Mat4::from_translation(vec3(x * 1.15, 0.0, -0.035)) * Mat4::from_rotation_z(-0.36);
        slab(m, &bar, Vec3::ZERO, vec3(0.019, 0.075, 0.006), 1.0, 0.3, markings);
    }
    for sign in [-1.0f32, 1.0] {
        slab(m, &body, vec3(sign * pack_w * 0.5, 0.97, back - 0.17), vec3(pack_w * 0.42, 0.06, 0.05), 0.9, 0.4, shade(gear, 0.85));
        // Shoulder straps over the collar.
        slab(m, &body, vec3(sign * 0.12, 1.36, 0.04), vec3(0.03, 0.08, 0.15), 1.0, 0.3, webbing);
    }

    // ---- head ----
    // Head counter-leans with the torso. The Regular helmet is the art's:
    // a faceted dome, a dark visor band with a glowing slit, ear pods and
    // a jaw guard.
    let head = body
        * Mat4::from_translation(vec3(0.0, 1.445, 0.0))
        * Mat4::from_rotation_x(-p.pitch * 0.65 * (1.0 - fall) - lean * 0.8)
        * Mat4::from_rotation_z(-roll * 0.7);
    slab(m, &head, vec3(0.0, 0.03, 0.0), vec3(0.07, 0.05, 0.07), 1.0, 0.4, dark);
    let face = |m: &mut Mesh| {
        slab(m, &head, vec3(0.0, 0.16, 0.0), vec3(0.1, 0.125, 0.115), 1.14, 0.5, skin);
        slab(m, &head, vec3(0.0, 0.085, 0.035), vec3(0.075, 0.05, 0.085), 1.1, 0.5, shade(skin, 0.94));
        slab(m, &head, vec3(0.0, 0.15, 0.118), vec3(0.016, 0.03, 0.018), 0.7, 0.45, shade(skin, 0.9));
        slab(m, &head, vec3(0.0, 0.2, 0.108), vec3(0.075, 0.012, 0.012), 1.0, 0.4, shade(skin, 0.7));
        for sign in [-1.0f32, 1.0] {
            slab(m, &head, vec3(sign * 0.105, 0.15, -0.01), vec3(0.012, 0.03, 0.022), 1.0, 0.4, shade(skin, 0.92));
        }
    };
    match look {
        1 => {
            // Soft cap with a peak, goggles and a headset.
            face(m);
            slab(m, &head, vec3(0.0, 0.255, -0.01), vec3(0.128, 0.055, 0.135), 0.86, 0.45, shade(main, 0.75));
            slab(m, &head, vec3(0.0, 0.225, 0.14), vec3(0.11, 0.012, 0.06), 0.9, 0.4, shade(main, 0.55));
            slab(m, &head, vec3(0.0, 0.17, 0.12), vec3(0.122, 0.034, 0.02), 1.0, 0.4, metal);
            for x in [-0.055f32, 0.055] {
                slab(m, &head, vec3(x, 0.172, 0.138), vec3(0.042, 0.024, 0.008), 1.0, 0.4, tint([0.38, 0.62, 0.68], 0.25));
            }
            for sign in [-1.0f32, 1.0] {
                slab(m, &head, vec3(sign * 0.125, 0.15, 0.0), vec3(0.025, 0.05, 0.05), 1.0, 0.45, metal);
            }
        }
        2 => {
            // Full-face helmet: a bigger shell, the visor wrapping round,
            // a team crest.
            slab(m, &head, vec3(0.0, 0.185, -0.015), vec3(0.172, 0.165, 0.18), 0.86, 0.5, armor);
            slab(m, &head, vec3(0.0, 0.3, -0.03), vec3(0.13, 0.06, 0.14), 0.72, 0.5, shade(armor, 1.04));
            slab(m, &head, vec3(0.0, 0.33, -0.03), vec3(0.04, 0.03, 0.15), 0.8, 0.4, main);
            slab(m, &head, vec3(0.0, 0.16, 0.1), vec3(0.16, 0.07, 0.08), 1.0, 0.55, visor);
            slab(m, &head, vec3(0.0, 0.17, 0.178), vec3(0.12, 0.015, 0.006), 1.0, 0.3, glass);
            slab(m, &head, vec3(0.0, 0.06, 0.1), vec3(0.12, 0.055, 0.075), 1.12, 0.42, shade(armor, 0.86));
            for sign in [-1.0f32, 1.0] {
                slab(m, &head, vec3(sign * 0.175, 0.16, -0.02), vec3(0.022, 0.06, 0.06), 1.0, 0.45, metal);
            }
        }
        3 => {
            // Hood and a mask with a faint eye slit.
            slab(m, &head, vec3(0.0, 0.175, -0.03), vec3(0.165, 0.18, 0.175), 0.78, 0.45, dark);
            slab(m, &head, vec3(0.0, 0.31, -0.08), vec3(0.09, 0.06, 0.1), 0.6, 0.45, shade(dark, 0.9));
            slab(m, &head, vec3(0.0, 0.13, 0.115), vec3(0.115, 0.085, 0.035), 1.05, 0.42, metal);
            slab(m, &head, vec3(0.0, 0.175, 0.152), vec3(0.085, 0.012, 0.004), 1.0, 0.3, tint(hot(tc, 0.2), 0.5));
            slab(m, &head, vec3(0.0, 0.08, 0.15), vec3(0.05, 0.03, 0.01), 1.0, 0.4, shade(metal, 0.7));
        }
        4 => {
            // Bare head, a tilted beret with a brass badge.
            face(m);
            let tilt = head * Mat4::from_translation(vec3(0.02, 0.27, -0.005)) * Mat4::from_rotation_z(-0.22);
            slab(m, &tilt, Vec3::ZERO, vec3(0.15, 0.035, 0.145), 0.82, 0.5, main);
            slab(m, &tilt, vec3(-0.065, 0.02, 0.11), vec3(0.022, 0.022, 0.01), 1.0, 0.4, hazard);
            slab(m, &head, vec3(0.0, 0.09, 0.128), vec3(0.06, 0.016, 0.012), 1.0, 0.4, tint([0.4, 0.37, 0.33], 0.0));
        }
        5 => {
            // Hard hat with a brim, respirator, ear defenders.
            face(m);
            slab(m, &head, vec3(0.0, 0.27, 0.0), vec3(0.15, 0.07, 0.155), 0.8, 0.45, hazard);
            slab(m, &head, vec3(0.0, 0.22, 0.02), vec3(0.18, 0.012, 0.2), 1.0, 0.4, hazard);
            slab(m, &head, vec3(0.0, 0.34, 0.0), vec3(0.025, 0.012, 0.13), 1.0, 0.4, shade(hazard, 0.7));
            slab(m, &head, vec3(0.0, 0.08, 0.125), vec3(0.075, 0.05, 0.04), 0.9, 0.42, metal);
            for sign in [-1.0f32, 1.0] {
                slab(m, &head, vec3(sign * 0.05, 0.07, 0.165), vec3(0.025, 0.025, 0.02), 1.0, 0.45, shade(metal, 0.7));
                slab(m, &head, vec3(sign * 0.13, 0.15, -0.01), vec3(0.03, 0.06, 0.06), 1.0, 0.45, main);
            }
        }
        _ => {
            slab(m, &head, vec3(0.0, 0.175, -0.02), vec3(0.16, 0.15, 0.17), 0.84, 0.5, armor);
            slab(m, &head, vec3(0.0, 0.29, -0.03), vec3(0.125, 0.06, 0.135), 0.72, 0.5, shade(armor, 1.04));
            slab(m, &head, vec3(0.0, 0.165, 0.09), vec3(0.15, 0.043, 0.075), 1.0, 0.55, visor);
            slab(m, &head, vec3(0.0, 0.167, 0.163), vec3(0.112, 0.013, 0.006), 1.0, 0.3, glass);
            slab(m, &head, vec3(0.0, 0.075, 0.07), vec3(0.11, 0.05, 0.08), 1.12, 0.5, shade(armor, 0.88));
            for sign in [-1.0f32, 1.0] {
                slab(m, &head, vec3(sign * 0.16, 0.15, -0.025), vec3(0.024, 0.055, 0.055), 1.0, 0.5, metal);
                slab(m, &head, vec3(sign * 0.18, 0.15, -0.025), vec3(0.006, 0.028, 0.028), 1.0, 0.4, main);
            }
        }
    }

    // ---- arms and gun ----
    // Third person, first person and the picker share exactly the same
    // gun. Hand targets come from that model rather than a generic rifle.
    let relaxed = (0.5 * sprint + 0.25 * air) * (1.0 - p.fire) * (1.0 - p.melee) + 0.42 * p.reload + 1.0 * fall;
    let aim = -p.pitch * (1.0 - 0.6 * sprint) * (1.0 - fall) - lean + relaxed - 0.065 * p.fire;
    let arms = Mat4::from_translation(vec3(0.0, 1.34, -0.035 * p.fire + 0.2 * p.melee))
        * Mat4::from_rotation_x(aim)
        * Mat4::from_rotation_y(0.38 * sprint * (1.0 - p.fire) * (1.0 - p.melee) + 0.2 * p.reload + 0.55 * p.melee);
    // A long gun is held out far enough that its butt sits against the
    // front of the chest plate, at the shoulder, and not inside it.
    let long = ferrite_shared::sim::weapon_def(p.wid).arch != 4;
    let reach = gun_reach(p.wid, bulk);
    let gun = arms
        * Mat4::from_translation(vec3(if long { -0.135 } else { -0.12 }, -0.06, reach))
        * Mat4::from_rotation_y(std::f32::consts::PI)
        * Mat4::from_scale(Vec3::splat(GUN_SCALE));
    gun_model_pose(m, p.wid, p.team, p.wskin, body * gun, p.reload_phase, p.cycle, !p.far);
    let (mut grip, mut support) = gun_grips(p.wid);
    if long {
        // With the gun that far forward the off hand holds nearer the
        // receiver, where the arm can reach it.
        support.z = support.z.max(-0.25);
    }
    // Hands follow the moving fore-end and bolt as well as the weapon root.
    let action = if p.reload_phase < 0.0 { (smooth(0.22, 0.72, 1.0 - p.cycle) * std::f32::consts::PI).sin() } else { 0.0 };
    if matches!(p.wid, 1 | 13) {
        support.z += action * 0.085;
    }
    if matches!(p.wid, 2 | 15) {
        grip = grip.lerp(vec3(0.07, 0.03, 0.1 + action * 0.07), action);
    }
    let right_hand = gun.transform_point3(grip);
    let mut left_hand = gun.transform_point3(support);
    if p.reload_phase >= 0.0 {
        let r = p.reload_phase.clamp(0.0, 1.0);
        let reach = smooth(0.03, 0.18, r) * (1.0 - smooth(0.72, 0.94, r));
        let mag = gun.transform_point3(vec3(0.0, -0.20 - (r * std::f32::consts::PI).sin() * 0.12, -0.04));
        left_hand = left_hand.lerp(mag, reach);
    }
    for (sign, hand) in [(-1.0f32, right_hand), (1.0, left_hand)] {
        // The off shoulder rolls forward into a long gun.
        let shoulder = vec3(sign * 0.29 * bulk, 1.34, if long && sign > 0.0 { 0.07 } else { 0.0 });
        let elbow = joint(shoulder, hand, 0.29, 0.32, vec3(sign * 0.7, -1.0, 0.1));
        segment(m, &body, shoulder, elbow, 0.078 * bulk, 1.1, dark);
        if let Some((f, len)) = bone(&body, shoulder, elbow) {
            // Upper-arm plate, hanging from under the shoulder plate.
            if look != 4 {
                slab(m, &f, vec3(0.0, -0.03, 0.0), vec3(0.084 * bulk, len * 0.28, 0.084 * bulk), 1.1, 0.45, armor);
            }
            slab(m, &f, vec3(0.0, -len * 0.5, -0.02), vec3(0.06, 0.06, 0.06), 1.0, 0.48, shade(armor, 0.88));
        }
        let rolled = look == 4;
        segment(m, &body, elbow, hand, 0.064 * bulk, 1.12, if rolled { skin } else { shade(dark, 0.92) });
        if let Some((f, len)) = bone(&body, elbow, hand) {
            // Bracer over the forearm.
            if rolled {
                slab(m, &f, vec3(0.0, len * 0.42, 0.0), vec3(0.075 * bulk, 0.04, 0.075 * bulk), 1.0, 0.4, dark);
            } else {
                slab(m, &f, vec3(0.0, -0.02, 0.0), vec3(0.074 * bulk, len * 0.3, 0.074 * bulk), 1.12, 0.42, shade(armor, 0.96));
            }
        }
        slab(m, &body, hand, vec3(0.046, 0.05, 0.055), 0.9, 0.42, glove);
    }
    if shield {
        let a = 0.25 + 0.15 * (time * 12.0).sin();
        alpha.cuboid(p.pos + vec3(-0.5, 0.0, -0.5), p.pos + vec3(0.5, 1.95 - 0.56 * c, 0.5), [1.0, 1.0, 1.0, a * 0.25]);
    }
    if blob {
        let gy = if p.flags & flag::GROUND != 0 { p.pos.y } else { w.height(p.pos.x, p.pos.z) } + 0.04;
        let r = 0.55;
        alpha.quad(
            vec3(p.pos.x - r, gy, p.pos.z - r),
            vec3(p.pos.x - r, gy, p.pos.z + r),
            vec3(p.pos.x + r, gy, p.pos.z + r),
            vec3(p.pos.x + r, gy, p.pos.z - r),
            [0.05, 0.04, 0.06, 0.4],
        );
    }
}

fn smooth(a: f32, b: f32, t: f32) -> f32 {
    let t = ((t - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plates_are_closed_and_face_outward() {
        let mut m = Mesh::default();
        let g = Mat4::from_rotation_y(0.7) * Mat4::from_translation(vec3(1.0, 2.0, 3.0));
        let c = vec3(0.1, -0.2, 0.3);
        slab(&mut m, &g, c, vec3(0.2, 0.05, 0.12), 0.8, 0.4, [1.0; 4]);
        assert_eq!(m.triangles(), 6 * 2 + 12 * 2 + 8);
        let centre = g.transform_point3(c);
        for t in m.i.chunks(3) {
            let (a, b, d) = (m.v[t[0] as usize], m.v[t[1] as usize], m.v[t[2] as usize]);
            let mid = (Vec3::from_array(a.pos) + Vec3::from_array(b.pos) + Vec3::from_array(d.pos)) / 3.0;
            assert!(Vec3::from_array(a.nrm).dot(mid - centre) > 0.0);
        }
    }

    #[test]
    fn no_stock_is_buried_in_the_chest() {
        for wid in 0..ferrite_shared::sim::WEAPONS.len() as u8 {
            if ferrite_shared::sim::weapon_def(wid).arch == 4 {
                continue;
            }
            let mut m = Mesh::default();
            super::super::weapons::gun_model(&mut m, wid, 0, 0, Mat4::IDENTITY);
            let back = m.finish().iter().flat_map(|(v, _)| v.iter().map(|p| p.pos[2])).fold(f32::MIN, f32::max);
            for bulk in [0.9f32, 1.0, 1.1, 1.2] {
                // The gun faces forward, so its +Z end is nearest the body.
                let butt = gun_reach(wid, bulk) - back * GUN_SCALE;
                let sunk = chest_front(bulk) - butt;
                assert!((-0.04..=0.05).contains(&sunk), "weapon {wid}, bulk {bulk}: butt {sunk:.3} m into the chest");
            }
        }
    }

    #[test]
    fn soldier_equipment_and_extreme_poses_have_valid_geometry() {
        let world = World::new(0xFE221);
        // Idle, sprint, strafe, crouch, slide, airborne, reload, fallen.
        for look in 0..12 {
            for wid in [0, 1, 2, 4, 10, 16] {
                for pose in 0..8 {
                    let mut p = PlayerView {
                        pos: world.points[2],
                        hp: 100,
                        skin: look,
                        kit: look % 3,
                        wid,
                        flags: flag::GROUND,
                        pitch: if pose % 2 == 0 { 0.7 } else { -0.7 },
                        phase: 1.6,
                        ..PlayerView::default()
                    };
                    match pose {
                        1 => {
                            p.vel = (8.4, 0.0);
                            p.moving = 1.0;
                        }
                        2 => {
                            p.vel = (0.0, 5.6);
                            p.moving = 1.0;
                        }
                        3 => {
                            p.crouch = 1.0;
                        }
                        4 => {
                            p.slide = 1.0;
                            p.crouch = 1.0;
                        }
                        5 => {
                            p.air = 1.0;
                            p.flags = 0;
                        }
                        6 => {
                            p.reload = 1.0;
                            p.reload_phase = 0.5;
                        }
                        7 => {
                            p.fall = 1.0;
                        }
                        _ => {}
                    }
                    let (mut m, mut alpha) = (Mesh::default(), Mesh::default());
                    soldier(&mut m, &mut alpha, &world, &p, 2.0, true);
                    assert!(!m.v.is_empty());
                    for (v, i) in m.finish().into_iter().chain(alpha.finish()) {
                        assert!(i.iter().all(|&n| (n as usize) < v.len()));
                        for v in v {
                            let pos = Vec3::from_array(v.pos);
                            assert!(pos.is_finite() && Vec3::from_array(v.nrm).is_finite());
                            assert!((pos - p.pos).length() < 3.0, "look {look}, weapon {wid}, pose {pose}: {pos:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn knee_solver_keeps_both_bones_and_the_planted_target() {
        let hip = vec3(0.15, 0.84, 0.0);
        for z in [-0.3f32, 0.0, 0.3] {
            let ankle = vec3(0.15, 0.095, z);
            let knee = joint(hip, ankle, 0.39, 0.38, Vec3::Z);
            let stretch = ((ankle - hip).length() / (0.77 * 0.995)).max(1.0);
            assert!(((knee - hip).length() - 0.39 * stretch).abs() < 1e-4);
            assert!(((ankle - knee).length() - 0.38 * stretch).abs() < 1e-4);
        }
    }
}
