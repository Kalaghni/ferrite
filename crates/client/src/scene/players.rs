//! Soldier silhouettes, equipment and movement poses.

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
        }
    }
}

/// Eight-sided rings give armor a cut silhouette without smoothing away
/// the deliberate facets. `top` narrows helmets and widens chest plates.
fn plate(m: &mut Mesh, g: &Mat4, c: Vec3, h: Vec3, top: f32, col: Col) {
    let bevel = 0.28;
    let ring = [
        (-1.0 + bevel, -1.0),
        (1.0 - bevel, -1.0),
        (1.0, -1.0 + bevel),
        (1.0, 1.0 - bevel),
        (1.0 - bevel, 1.0),
        (-1.0 + bevel, 1.0),
        (-1.0, 1.0 - bevel),
        (-1.0, -1.0 + bevel),
    ];
    let at = |i: usize, upper: bool| {
        let (x, z) = ring[i % 8];
        let scale = if upper { top } else { 1.0 };
        g.transform_point3(
            c + vec3(
                x * h.x * scale,
                if upper { h.y } else { -h.y },
                z * h.z * scale,
            ),
        )
    };
    for k in 0..8 {
        m.quad(
            at(k, false),
            at(k, true),
            at(k + 1, true),
            at(k + 1, false),
            col,
        );
    }
    // Convex caps triangulated from a ring vertex avoid degenerate faces.
    for k in 1..7 {
        m.tri(at(0, true), at(k + 1, true), at(k, true), col);
        m.tri(
            at(0, false),
            at(k, false),
            at(k + 1, false),
            shade(col, 0.9),
        );
    }
}

fn segment(m: &mut Mesh, g: &Mat4, a: Vec3, b: Vec3, radius: f32, col: Col) {
    let d = a - b;
    if d.length_squared() < 1e-8 {
        return;
    }
    let y = d.normalize();
    let guide = if y.z.abs() > 0.9 { Vec3::Y } else { Vec3::Z };
    let x = y.cross(guide).normalize();
    let z = x.cross(y);
    let basis = Mat4::from_cols(
        x.extend(0.0),
        y.extend(0.0),
        z.extend(0.0),
        ((a + b) * 0.5).extend(1.0),
    );
    plate(
        m,
        &(*g * basis),
        Vec3::ZERO,
        vec3(radius * 0.86, d.length() * 0.5, radius),
        1.12,
        col,
    );
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

/// A soldier with planted foot targets and articulated arms. All equipment
/// follows the same body rig while head and weapon retain an independent aim.
pub fn soldier(m: &mut Mesh, alpha: &mut Mesh, w: &World, p: &PlayerView, time: f32, blob: bool) {
    let tc = team_col(p.team);
    let shield = p.flags & flag::SHIELD != 0;
    let glow = if p.flags & flag::SURGE != 0 {
        0.35 + 0.25 * (time * 9.0).sin()
    } else {
        0.0
    };
    let h = p.hurt.clamp(0.0, 1.0) * 0.6;
    let tint = |c: [f32; 3], e: f32| -> Col {
        [
            c[0] + (1.0 - c[0]) * h,
            c[1] + (1.0 - c[1]) * h,
            c[2] + (1.0 - c[2]) * h,
            e.max(h * 0.5),
        ]
    };
    let main = tint(tc, glow);
    let look = p.skin % 6;
    // The key art separates muted cloth from bright team armor. Keeping
    // that value contrast makes silhouettes readable against the terrain.
    let fatigues = match look {
        3 => [0.075, 0.085, 0.105],
        1 => [0.34, 0.36, 0.34],
        _ => [0.25, 0.27, 0.28],
    };
    let dark = tint(fatigues, glow * 0.2);
    let armor = tint([0.43, 0.45, 0.46], 0.0);
    let metal = tint([0.13, 0.15, 0.18], 0.0);
    let skin = tint([0.78, 0.65, 0.51], 0.0);
    let glove = tint([0.15, 0.16, 0.17], 0.0);
    let leather = tint([0.34, 0.25, 0.16], 0.0);
    let hazard = tint([0.95, 0.72, 0.13], 0.0);
    let markings = tint([0.86, 0.9, 0.9], 0.0);

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
        * Mat4::from_rotation_z((if p.id % 2 == 0 { 0.13 } else { -0.13 }) * fall)
        * Mat4::from_rotation_x(-fall * 1.5);
    let bulk = if p.kit == 1 {
        1.16
    } else if p.kit == 2 {
        0.96
    } else {
        1.0
    };
    let phase = p.phase * 2.2;
    let walk_dir = if speed > 0.1 {
        vec3(-p.vel.1 / speed, 0.0, p.vel.0 / speed)
    } else {
        Vec3::Z
    };
    let hip_y = 0.84 - 0.44 * c;
    let bounce = (phase * 2.0).cos() * 0.014 * locomotion * (1.0 - c);

    // Feet travel backwards during the planted half of each cycle, then
    // lift through the recovery half. IK preserves the ankle target while
    // knees bend instead of stretching the whole leg into the floor.
    for (sign, offset) in [(1.0f32, 0.0f32), (-1.0, std::f32::consts::PI)] {
        let ph = phase + offset;
        let reach = (0.30 + 0.13 * sprint) * locomotion * (1.0 - 0.65 * kneel);
        let lift = ph.cos().max(0.0).powf(1.5) * (0.11 + 0.045 * sprint) * locomotion;
        let hip = vec3(sign * 0.145, hip_y + bounce, 0.0);
        let mut ankle = vec3(sign * 0.155, 0.095 + lift, 0.0) + walk_dir * (ph.sin() * reach);
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
        segment(m, &root, hip, knee, 0.101 * bulk, dark);
        segment(m, &root, knee, ankle, 0.077 * bulk, shade(dark, 0.9));
        plate(
            m,
            &root,
            knee + vec3(0.0, 0.0, 0.055),
            vec3(0.082 * bulk, 0.082, 0.056),
            0.82,
            armor,
        );
        let boot = root
            * Mat4::from_translation(ankle)
            * Mat4::from_rotation_y(
                (-p.vel.1 / speed.max(1.0)).clamp(-1.0, 1.0) * 0.25 * locomotion,
            )
            * Mat4::from_rotation_x(-ph.sin().max(0.0) * 0.18 * locomotion);
        plate(
            m,
            &boot,
            vec3(0.0, -0.025, 0.045),
            vec3(0.087, 0.065, 0.145),
            0.83,
            metal,
        );
        m.obox(
            &boot,
            vec3(0.0, -0.076, 0.045),
            vec3(0.09, 0.014, 0.148),
            shade(metal, 0.7),
        );
        if look == 2 {
            plate(
                m,
                &root,
                hip.lerp(knee, 0.44) + vec3(sign * 0.035, 0.0, 0.06),
                vec3(0.085, 0.12, 0.04),
                1.05,
                armor,
            );
        } else if look == 5 {
            segment(
                m,
                &root,
                knee.lerp(ankle, 0.56),
                knee.lerp(ankle, 0.7),
                0.083 * bulk,
                hazard,
            );
        }
    }

    // Rotation is about the hips. Weight shifts toward the supporting leg;
    // breathing and counter-rotation disappear as a body comes to rest.
    let idle = (time * 1.7 + p.id as f32).sin() * 0.006 * (1.0 - fall);
    let lean = (0.075 * (p.vel.0 / 5.6).clamp(-1.0, 1.0) + 0.18 * sprint) * (1.0 - sl)
        + 0.17 * kneel
        - 0.58 * sl
        + 0.09 * air
        - 0.04 * p.fire;
    let roll = (p.vel.1 / 5.6).clamp(-1.0, 1.0) * 0.08 * (1.0 - c)
        + phase.sin() * 0.025 * locomotion * (1.0 - c);
    let body = root
        * Mat4::from_translation(vec3(0.0, idle + bounce - 0.56 * c + 0.84, 0.0))
        * Mat4::from_rotation_x(lean)
        * Mat4::from_rotation_z(roll)
        * Mat4::from_rotation_y(phase.sin() * 0.035 * locomotion * (1.0 - c))
        * Mat4::from_translation(vec3(0.0, -0.84, 0.0));
    plate(
        m,
        &body,
        vec3(0.0, 1.12, 0.0),
        vec3(0.225 * bulk, 0.28, 0.145 * bulk),
        1.14,
        dark,
    );
    plate(
        m,
        &body,
        vec3(0.0, 1.21, 0.138 * bulk),
        vec3(0.215 * bulk, 0.19, 0.055),
        1.12,
        if look == 3 { shade(main, 0.65) } else { main },
    );
    plate(
        m,
        &body,
        vec3(0.0, 0.85, 0.0),
        vec3(0.25 * bulk, 0.065, 0.155 * bulk),
        0.92,
        metal,
    );
    m.obox(
        &body,
        vec3(0.0, 0.855, 0.163 * bulk),
        vec3(0.035, 0.035, 0.018),
        armor,
    );
    for sign in [-1.0f32, 1.0] {
        let shoulder = body
            * Mat4::from_translation(vec3(sign * 0.28 * bulk, 1.35, 0.0))
            * Mat4::from_rotation_z(sign * 0.3);
        plate(
            m,
            &shoulder,
            vec3(sign * 0.035, 0.015, 0.0),
            vec3(0.115, 0.11, 0.16),
            0.76,
            armor,
        );
        plate(
            m,
            &shoulder,
            vec3(sign * 0.05, 0.065, 0.0),
            vec3(0.116, 0.047, 0.15),
            0.85,
            shade(main, 0.92),
        );
        m.obox(
            &body,
            vec3(sign * 0.135, 0.96, 0.186 * bulk),
            vec3(0.055, 0.075, 0.04),
            metal,
        );
        m.obox(
            &body,
            vec3(sign * 0.247 * bulk, 0.87, 0.035),
            vec3(0.046, 0.082, 0.078),
            dark,
        );
    }
    plate(
        m,
        &body,
        vec3(0.0, 1.45, 0.0),
        vec3(0.135, 0.035, 0.125),
        0.85,
        metal,
    );
    m.obox(&body, vec3(0.0, 1.48, 0.0), vec3(0.065, 0.04, 0.065), dark);

    // The compact pack and inset team plate are the strongest repeated
    // shape in the launcher art, including its two pale recognition bars.
    let pack_w = match p.kit {
        1 => 0.215,
        2 => 0.165,
        _ => 0.185,
    };
    let pack_h = if p.kit == 2 { 0.245 } else { 0.20 };
    plate(
        m,
        &body,
        vec3(0.0, 1.19, -0.185 * bulk),
        vec3(pack_w, pack_h, 0.095),
        0.9,
        metal,
    );
    let pack_panel =
        body * Mat4::from_translation(vec3(0.0, 1.24, -0.282 * bulk)) * Mat4::from_rotation_x(0.12);
    plate(
        m,
        &pack_panel,
        Vec3::ZERO,
        vec3(pack_w * 0.87, 0.125, 0.027),
        1.0,
        main,
    );
    for x in [-0.038f32, 0.038] {
        let stripe = pack_panel
            * Mat4::from_translation(vec3(x, 0.015, -0.03))
            * Mat4::from_rotation_z(-0.12);
        m.obox(&stripe, Vec3::ZERO, vec3(0.017, 0.065, 0.004), markings);
    }
    m.obox(
        &body,
        vec3(0.0, 1.0, -0.23 * bulk),
        vec3(pack_w * 0.78, 0.06, 0.07),
        dark,
    );
    match look {
        1 => {
            for x in [-0.14f32, 0.0, 0.14] {
                plate(
                    m,
                    &body,
                    vec3(x * bulk, 1.04, 0.204 * bulk),
                    vec3(0.051, 0.08, 0.033),
                    0.92,
                    leather,
                );
            }
        }
        2 => {
            for sign in [-1.0f32, 1.0] {
                let pauldron = body
                    * Mat4::from_translation(vec3(sign * 0.36 * bulk, 1.34, -0.015))
                    * Mat4::from_rotation_z(sign * 0.22);
                plate(
                    m,
                    &pauldron,
                    Vec3::ZERO,
                    vec3(0.11, 0.13, 0.185),
                    0.75,
                    armor,
                );
            }
            plate(
                m,
                &body,
                vec3(0.0, 1.43, 0.0),
                vec3(0.19, 0.055, 0.175),
                0.9,
                armor,
            );
        }
        3 => m.obox(
            &body,
            vec3(0.31 * bulk, 1.24, 0.0),
            vec3(0.055, 0.03, 0.142),
            main,
        ),
        4 => {
            let strap =
                body * Mat4::from_translation(vec3(0.0, 1.16, 0.203)) * Mat4::from_rotation_z(0.7);
            m.obox(&strap, Vec3::ZERO, vec3(0.035, 0.28, 0.019), leather);
            for k in -2i32..=2 {
                m.obox(
                    &strap,
                    vec3(0.0, k as f32 * 0.095, 0.023),
                    vec3(0.025, 0.027, 0.012),
                    hazard,
                );
            }
        }
        5 => {
            for y in [1.08f32, 1.32] {
                m.obox(
                    &body,
                    vec3(0.0, y, 0.2),
                    vec3(0.205 * bulk, 0.021, 0.008),
                    hazard,
                );
            }
        }
        _ => {}
    }

    // Head counter-leans with the torso. The closed Regular helmet has a
    // sloped crown, cheek guards and a recessed face aperture as in the art.
    let head = body
        * Mat4::from_translation(vec3(0.0, 1.48, 0.0))
        * Mat4::from_rotation_x(-p.pitch * 0.65 * (1.0 - fall) - lean * 0.8)
        * Mat4::from_rotation_z(-roll * 0.7);
    plate(
        m,
        &head,
        vec3(0.0, 0.145, 0.0),
        vec3(0.125, 0.135, 0.13),
        0.87,
        skin,
    );
    match look {
        1 => {
            plate(
                m,
                &head,
                vec3(0.0, 0.27, -0.015),
                vec3(0.154, 0.047, 0.16),
                0.82,
                shade(main, 0.8),
            );
            m.obox(
                &head,
                vec3(0.0, 0.242, 0.16),
                vec3(0.125, 0.014, 0.065),
                shade(main, 0.6),
            );
            m.obox(
                &head,
                vec3(0.0, 0.171, 0.13),
                vec3(0.137, 0.037, 0.021),
                metal,
            );
            for x in [-0.066f32, 0.066] {
                m.obox(
                    &head,
                    vec3(x, 0.177, 0.153),
                    vec3(0.05, 0.024, 0.006),
                    tint([0.38, 0.57, 0.62], 0.06),
                );
            }
        }
        2 => {
            plate(
                m,
                &head,
                vec3(0.0, 0.165, 0.0),
                vec3(0.175, 0.17, 0.18),
                0.85,
                armor,
            );
            plate(
                m,
                &head,
                vec3(0.0, 0.285, -0.01),
                vec3(0.17, 0.055, 0.175),
                0.8,
                main,
            );
            m.obox(
                &head,
                vec3(0.0, 0.185, 0.178),
                vec3(0.129, 0.032, 0.012),
                metal,
            );
            m.obox(
                &head,
                vec3(0.0, 0.19, 0.191),
                vec3(0.105, 0.012, 0.004),
                tint(tc, 0.15),
            );
            plate(
                m,
                &head,
                vec3(0.0, 0.055, 0.15),
                vec3(0.085, 0.055, 0.055),
                0.8,
                metal,
            );
        }
        3 => {
            plate(
                m,
                &head,
                vec3(0.0, 0.17, -0.035),
                vec3(0.173, 0.18, 0.18),
                0.77,
                dark,
            );
            plate(
                m,
                &head,
                vec3(0.0, 0.10, 0.13),
                vec3(0.122, 0.07, 0.022),
                1.05,
                metal,
            );
            m.obox(
                &head,
                vec3(0.0, 0.197, 0.148),
                vec3(0.109, 0.019, 0.006),
                tint([0.5, 0.61, 0.65], 0.03),
            );
        }
        4 => {
            let tilt = head
                * Mat4::from_translation(vec3(0.018, 0.275, 0.0))
                * Mat4::from_rotation_z(-0.22);
            plate(m, &tilt, Vec3::ZERO, vec3(0.181, 0.039, 0.166), 0.79, main);
            m.obox(
                &tilt,
                vec3(0.09, 0.041, 0.09),
                vec3(0.023, 0.021, 0.025),
                hazard,
            );
            m.obox(
                &head,
                vec3(0.0, 0.079, 0.13),
                vec3(0.058, 0.015, 0.012),
                tint([0.55, 0.52, 0.46], 0.0),
            );
        }
        5 => {
            plate(
                m,
                &head,
                vec3(0.0, 0.265, 0.0),
                vec3(0.163, 0.07, 0.163),
                0.77,
                hazard,
            );
            plate(
                m,
                &head,
                vec3(0.0, 0.214, 0.025),
                vec3(0.186, 0.012, 0.204),
                1.0,
                hazard,
            );
            m.obox(
                &head,
                vec3(0.0, 0.332, 0.0),
                vec3(0.022, 0.012, 0.135),
                shade(hazard, 0.7),
            );
            for sign in [-1.0f32, 1.0] {
                plate(
                    m,
                    &head,
                    vec3(sign * 0.15, 0.135, -0.01),
                    vec3(0.029, 0.063, 0.06),
                    0.9,
                    main,
                );
            }
        }
        _ => {
            plate(
                m,
                &head,
                vec3(0.0, 0.215, -0.015),
                vec3(0.168, 0.115, 0.18),
                0.75,
                armor,
            );
            plate(
                m,
                &head,
                vec3(0.0, 0.26, -0.025),
                vec3(0.16, 0.065, 0.17),
                0.78,
                shade(main, 0.9),
            );
            m.obox(
                &head,
                vec3(0.0, 0.17, 0.158),
                vec3(0.121, 0.048, 0.014),
                metal,
            );
            m.obox(
                &head,
                vec3(0.0, 0.188, 0.174),
                vec3(0.101, 0.022, 0.004),
                tint([0.2, 0.28, 0.3], 0.0),
            );
            plate(
                m,
                &head,
                vec3(0.0, 0.067, 0.131),
                vec3(0.106, 0.05, 0.052),
                0.9,
                armor,
            );
            for sign in [-1.0f32, 1.0] {
                plate(
                    m,
                    &head,
                    vec3(sign * 0.143, 0.123, -0.005),
                    vec3(0.032, 0.08, 0.095),
                    0.94,
                    shade(armor, 0.85),
                );
                m.obox(
                    &head,
                    vec3(sign * 0.174, 0.15, -0.015),
                    vec3(0.007, 0.033, 0.038),
                    metal,
                );
            }
        }
    }

    // Third person, first person and the picker share exactly the same
    // gun. Hand targets come from that model rather than a generic rifle.
    let relaxed = (0.5 * sprint + 0.25 * air) * (1.0 - p.fire) * (1.0 - p.melee)
        + 0.42 * p.reload
        + 1.0 * fall;
    let aim = -p.pitch * (1.0 - 0.6 * sprint) * (1.0 - fall) - lean + relaxed - 0.065 * p.fire;
    let arms = Mat4::from_translation(vec3(0.0, 1.34, -0.035 * p.fire + 0.2 * p.melee))
        * Mat4::from_rotation_x(aim)
        * Mat4::from_rotation_y(
            0.38 * sprint * (1.0 - p.fire) * (1.0 - p.melee) + 0.2 * p.reload + 0.55 * p.melee,
        );
    let gun = arms
        * Mat4::from_translation(vec3(-0.12, -0.06, 0.28))
        * Mat4::from_rotation_y(std::f32::consts::PI)
        * Mat4::from_scale(Vec3::splat(0.72));
    gun_model_pose(
        m,
        p.wid,
        p.team,
        p.wskin,
        body * gun,
        p.reload_phase,
        p.cycle,
    );
    let (mut grip, mut support) = gun_grips(p.wid);
    // Hands follow the moving fore-end and bolt as well as the weapon root.
    let action = if p.reload_phase < 0.0 {
        (smooth(0.22, 0.72, 1.0 - p.cycle) * std::f32::consts::PI).sin()
    } else {
        0.0
    };
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
        let mag = gun.transform_point3(vec3(
            0.0,
            -0.20 - (r * std::f32::consts::PI).sin() * 0.12,
            -0.04,
        ));
        left_hand = left_hand.lerp(mag, reach);
    }
    for (sign, hand) in [(-1.0f32, right_hand), (1.0, left_hand)] {
        let shoulder = vec3(sign * 0.285 * bulk, 1.34, 0.0);
        let elbow = joint(shoulder, hand, 0.29, 0.32, vec3(sign * 0.7, -1.0, 0.1));
        segment(m, &body, shoulder, elbow, 0.078 * bulk, dark);
        segment(
            m,
            &body,
            elbow,
            hand,
            0.061 * bulk,
            if look == 4 { skin } else { shade(dark, 0.9) },
        );
        plate(
            m,
            &body,
            elbow + vec3(sign * 0.025, 0.0, -0.015),
            vec3(0.065, 0.064, 0.065),
            0.9,
            armor,
        );
        plate(m, &body, hand, vec3(0.043, 0.046, 0.05), 0.9, glove);
    }
    if shield {
        let a = 0.25 + 0.15 * (time * 12.0).sin();
        alpha.cuboid(
            p.pos + vec3(-0.5, 0.0, -0.5),
            p.pos + vec3(0.5, 1.95 - 0.56 * c, 0.5),
            [1.0, 1.0, 1.0, a * 0.25],
        );
    }
    if blob {
        let gy = if p.flags & flag::GROUND != 0 {
            p.pos.y
        } else {
            w.height(p.pos.x, p.pos.z)
        } + 0.04;
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
    fn soldier_equipment_and_extreme_poses_have_valid_geometry() {
        let world = World::new(0xFE221);
        // Idle, sprint, strafe, crouch, slide, airborne, reload, fallen.
        for look in 0..6 {
            for wid in [0, 1, 2, 4, 10, 16] {
                for pose in 0..8 {
                    let mut p = PlayerView {
                        pos: world.points[2],
                        hp: 100,
                        skin: look,
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
                            assert!(
                                (pos - p.pos).length() < 3.0,
                                "look {look}, weapon {wid}, pose {pose}: {pos:?}"
                            );
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
