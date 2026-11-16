//! Great landmarks: the relay's cellular backbone and the foundry's volcano.
//! The mast stands on the field and can be climbed; everything a player can
//! touch is drawn from the shared layout in `map::relay`, which builds the
//! matching collision. The volcano is off-field. Moving parts of both are
//! ambient scenery and change neither collision nor damage.
use super::slag_cathedral::art::{annulus, cross, soft, stencil, wall};
use super::slag_cathedral::{beam, pipe};
use super::{hash01, smooth};
use crate::gfx::{shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::relay as layout;
use ferrite_shared::map::World;
use std::f32::consts::{PI, TAU};

pub(crate) const MAST_XZ: (f32, f32) = layout::BACKBONE;
pub(crate) const VOLCANO_XZ: (f32, f32) = (0.0, -160.0);
pub(crate) use ferrite_shared::events::eruption;
const STEEL: Col = [0.36, 0.39, 0.42, 0.0];
const SAFETY: Col = [0.90, 0.66, 0.14, 0.0];
const WHITE: Col = [0.84, 0.85, 0.80, 0.0];
const RED: Col = [0.71, 0.19, 0.13, 0.0];
const DARK: Col = [0.10, 0.12, 0.14, 0.0];
const HOT: Col = [0.95, 0.21, 0.025, 0.9];

pub(crate) fn mast_origin(w: &World) -> Vec3 {
    vec3(MAST_XZ.0, layout::mast_base(w), MAST_XZ.1)
}
pub(crate) fn volcano_origin(w: &World) -> Vec3 {
    vec3(VOLCANO_XZ.0, w.height(VOLCANO_XZ.0, VOLCANO_XZ.1), VOLCANO_XZ.1)
}

// Eruptions (`ferrite_shared::events::eruption`) run on the world clock, the
// same on the server and every client; the lava they send down the runner
// burns (see `World::lava`).

pub fn build(m: &mut Mesh, w: &World) {
    match w.map {
        5 => cellular_mast(m, w),
        6 => volcano(m, w),
        _ => {}
    }
}

/// The parts of `y0..y1`, at least a metre tall, that leave the headroom
/// above the ground and above each deck clear of bracing.
fn braced_bands(y0: f32, y1: f32) -> Vec<(f32, f32)> {
    let mut out = vec![(y0, y1)];
    for floor in [0.0, layout::LANDING, layout::TOP_DECK] {
        let (c0, c1) = (floor, floor + layout::HEADROOM);
        out = out.into_iter().flat_map(|(a, b)| [(a, c0.min(b)), (c1.max(a), b)]).filter(|&(a, b)| b - a >= 1.0).collect();
    }
    out
}

fn cellular_mast(m: &mut Mesh, w: &World) {
    let root = mast_origin(w);
    let at = |x: f32, y: f32, z: f32| root + vec3(x, y, z);
    let corner = |sx: f32, sz: f32, y: f32| at(sx * layout::mast_half(y), y, sz * layout::mast_half(y));
    // Four tapering legs on concrete footings, white and oxide red, with
    // braced bays. This is the whole station's unmistakable backbone. The
    // bracing leaves head room above the ground and both decks, so what a
    // player walks through is open lattice and what stops them is a leg.
    let (foot, pad) = (layout::MAST_FOOT, layout::FOOTING_HALF);
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let (fx, fz) = (sx * foot, sz * foot);
            m.cuboid(at(fx - pad, -0.6, fz - pad), at(fx + pad, layout::FOOTING_H, fz + pad), [0.45, 0.44, 0.40, 0.0]);
            for bay in 0..layout::MAST_BAYS {
                let y0 = bay as f32 * layout::MAST_BAY;
                let y1 = y0 + layout::MAST_BAY;
                let paint = if bay % 3 == 1 { RED } else { WHITE };
                beam(m, corner(sx, sz, y0.max(layout::FOOTING_H)), corner(sx, sz, y1), layout::MAST_LEG, layout::MAST_LEG, paint);
                for (a, b) in braced_bands(y0, y1) {
                    beam(m, corner(sx, sz, a), corner(-sx, sz, b), 0.095, 0.095, shade(paint, 0.8));
                    beam(m, corner(sx, sz, a), corner(sx, -sz, b), 0.095, 0.095, shade(paint, 0.8));
                }
            }
        }
    }
    // Girts round all four faces at every bay and at the edges of the clear
    // bands. Each band's bracing starts and ends on one.
    let mut girts: Vec<f32> = (1..=layout::MAST_BAYS).map(|k| k as f32 * layout::MAST_BAY).collect();
    girts.extend([layout::HEADROOM, layout::LANDING - 0.2, layout::LANDING + layout::HEADROOM]);
    girts.extend([layout::TOP_DECK - 0.2, layout::TOP_DECK + layout::HEADROOM]);
    for y in girts {
        for s in [-1.0f32, 1.0] {
            beam(m, corner(-1.0, s, y), corner(1.0, s, y), 0.16, 0.16, STEEL);
            beam(m, corner(s, -1.0, y), corner(s, 1.0, y), 0.16, 0.16, STEEL);
        }
    }
    mast_decks(m, &at, &corner);
    mast_ladders(m, &at);
    // A dark cable tray beside the first ladder, out of the way of both
    // decks, and on up the mast above head height.
    let tray = |m: &mut Mesh, y0: f32, y1: f32| m.cuboid(at(0.8, y0, -0.55), at(1.15, y1, -0.30), DARK);
    tray(m, 0.4, layout::LANDING - layout::SLAB);
    tray(m, layout::TOP_DECK + layout::HEADROOM, 57.0);
    // Three tiers of directional cellular panels, each visibly joined to
    // its service ring and carrying a paired radio unit behind it. The
    // first ring is the top deck: its panels stand round the rail (they are
    // in the collision) and its ring is cut back to clear the deck.
    for (tier, y) in [layout::PANEL_Y, 48.0, 57.0].into_iter().enumerate() {
        if tier == 0 {
            annulus(m, at(0.0, layout::TOP_DECK - layout::SLAB - 0.05, 0.0), 3.0, 3.9, 20, DARK);
        } else {
            annulus(m, at(0.0, y - 2.25, 0.0), 0.8, 3.9, 20, DARK);
        }
        for k in 0..layout::PANELS {
            let a = k as f32 * TAU / layout::PANELS as f32 + tier as f32 * 0.17;
            let radial = vec3(a.sin(), 0.0, a.cos());
            let f = Mat4::from_translation(at(0.0, y, 0.0) + radial * layout::PANEL_R) * Mat4::from_rotation_y(a);
            if tier > 0 {
                beam(m, at(0.0, y - 1.25, 0.0), at(0.0, y - 1.25, 0.0) + radial * 3.45, 0.14, 0.14, STEEL);
            }
            let (px, py, pz) = layout::PANEL_HALF;
            m.obox(&f, Vec3::ZERO, vec3(px, py, pz), WHITE);
            m.obox(&f, vec3(0.0, 0.0, -0.25), vec3(0.32, 1.5, 0.09), STEEL);
            m.obox(&f, vec3(0.0, -0.55, -0.48), vec3(0.27, 0.48, 0.15), DARK);
            for seam in [-0.82f32, 0.82] {
                m.obox(&f, vec3(0.0, seam, 0.197), vec3(0.39, 0.022, 0.008), shade(WHITE, 0.7));
            }
            let a1 = a + TAU / 6.0;
            let p = |angle: f32, h: f32| at(angle.sin() * 3.8, h, angle.cos() * 3.8);
            beam(m, p(a, y - 2.2), p(a, y - 1.1), 0.055, 0.055, STEEL);
            beam(m, p(a, y - 1.1), p(a1, y - 1.1), 0.06, 0.06, STEEL);
        }
    }
    pipe(m, at(0.0, 55.0, 0.0), at(0.0, 64.0, 0.0), 0.19, STEEL);
    pipe(m, at(0.0, 63.5, 0.0), at(0.0, 67.0, 0.0), 0.055, WHITE);
    // A small maintenance dish and its support bracket on the lower mast.
    let dish = Mat4::from_translation(at(-3.3, 29.0, -3.0)) * Mat4::from_rotation_y(PI);
    m.tube(&dish, Vec3::ZERO, 1.8, 1.5, 0.18, 18, WHITE);
    beam(m, at(-1.0, 28.8, -1.0), at(-3.3, 29.0, -3.0), 0.2, 0.2, STEEL);
    pipe(m, at(-3.3, 29.0, -3.0), at(-3.3, 29.0, -4.3), 0.07, STEEL);
    for a in [0.0f32, TAU / 3.0, TAU * 2.0 / 3.0] {
        let end = at(a.cos() * 1.6 - 3.3, 29.0 + a.sin() * 1.6, -3.0);
        beam(m, end, at(-3.3, 29.0, -4.3), 0.035, 0.035, STEEL);
    }
    // The BACKBONE equipment hut behind the mast, its door facing the
    // ladder, and a cable cover across the yard between them.
    let (x0, z0, x1, z1, h) = layout::HUT;
    let (hut, size, eave) = (at(x0, 0.0, z0), vec3(x1 - x0, h, z1 - z0), layout::HUT_EAVE);
    m.cuboid(hut - Vec3::Y * 0.4, hut + size, [0.42, 0.43, 0.42, 0.0]);
    m.cuboid(hut + vec3(-eave, h, -eave), hut + size + vec3(eave, layout::HUT_ROOF, eave), DARK);
    m.cuboid(hut + vec3(0.5, 0.1, -0.035), hut + vec3(2.1, 2.8, 0.0), STEEL);
    stencil(m, &wall(hut + vec3(4.0, 2.3, -0.045), PI), "BACKBONE", 0.09, WHITE);
    m.cuboid(at(x0 + 1.1, -0.05, 1.0), at(x0 + 1.5, 0.05, z0), DARK);
}

/// The rest platform and the top deck, as `layout::mast` builds them:
/// grating with the hatch cut out, two rails on posts round the edge, the
/// rest platform's plate parapets, and the frame that carries each deck.
fn mast_decks(m: &mut Mesh, at: &impl Fn(f32, f32, f32) -> Vec3, corner: &impl Fn(f32, f32, f32) -> Vec3) {
    const GRATE: Col = [0.22, 0.24, 0.26, 0.0];
    let (rh, slab) = (layout::RAIL_HALF, layout::SLAB);
    let decks = [(layout::LANDING, layout::LANDING_HALF, &layout::LADDERS[0]), (layout::TOP_DECK, layout::TOP_HALF, &layout::LADDERS[1])];
    for (y, half, ladder) in decks {
        let (hx0, hz0, hx1, hz1) = layout::hatch(ladder);
        let box_at = |m: &mut Mesh, a: (f32, f32), b: (f32, f32), y0: f32, y1: f32, col: Col| {
            m.cuboid(at(a.0.min(b.0), y0, a.1.min(b.1)), at(a.0.max(b.0), y1, a.1.max(b.1)), col);
        };
        for (a, b) in [((-half, -half), (half, hz0)), ((-half, hz1), (half, half)), ((-half, hz0), (hx0, hz1)), ((hx1, hz0), (half, hz1))] {
            box_at(m, a, b, y - slab, y, GRATE);
        }
        // A frame under the grating, braced back to the legs.
        let e = half - 2.0 * rh;
        for s in [-1.0f32, 1.0] {
            box_at(m, (-half, s * half), (half, s * e), y - slab - 0.22, y - slab, STEEL);
            box_at(m, (s * half, -half), (s * e, half), y - slab - 0.22, y - slab, STEEL);
            for t in [-1.0f32, 1.0] {
                beam(m, corner(s, t, y - 2.0), at(s * half, y - slab - 0.1, t * half), 0.12, 0.12, STEEL);
            }
        }
        // Rails and their posts, exactly on the rail line.
        for hgt in layout::RAIL_HEIGHTS {
            let (r0, r1) = (y + hgt - rh, y + hgt + rh);
            box_at(m, (-half, -half), (half, -e), r0, r1, SAFETY);
            box_at(m, (-half, e), (half, half), r0, r1, SAFETY);
            box_at(m, (-half, -e), (-e, e), r0, r1, SAFETY);
            box_at(m, (e, -e), (half, e), r0, r1, SAFETY);
        }
        let posts = (half * 2.0 / 1.2).ceil() as usize;
        for k in 0..=posts {
            let t = -half + rh + (half * 2.0 - 2.0 * rh) * k as f32 / posts as f32;
            for p in [(t, -half + rh), (t, half - rh), (-half + rh, t), (half - rh, t)] {
                box_at(m, (p.0 - rh, p.1 - rh), (p.0 + rh, p.1 + rh), y, y + layout::RAIL_HEIGHTS[1] + rh, SAFETY);
            }
        }
    }
    // The rest platform's cage: a plate parapet in the middle of each side,
    // white with a red cap.
    let (p, e, t) = (layout::LANDING_PARAPET_HALF_LEN, layout::LANDING_HALF - 2.0 * rh, layout::PARAPET_T);
    let (y0, y1) = (layout::LANDING, layout::LANDING + layout::LANDING_PARAPET_H);
    for s in [-1.0f32, 1.0] {
        for (a, b) in [((-p, s * e), (p, s * (e - t))), ((s * e, -p), (s * (e - t), p))] {
            let (lo, hi) = (at(a.0.min(b.0), y0, a.1.min(b.1)), at(a.0.max(b.0), y1, a.1.max(b.1)));
            m.cuboid(lo, hi - Vec3::Y * 0.12, WHITE);
            m.cuboid(lo + Vec3::Y * (y1 - y0 - 0.12), hi, RED);
        }
    }
}

/// Both ladders as `layout::LADDERS` places them: rails that stand on as
/// hand holds above the deck, rungs, the dark cable riser behind that is
/// their collision, and a safety cage from head height up. Above the top
/// deck a service ladder carries on up the mast, out of reach.
fn mast_ladders(m: &mut Mesh, at: &impl Fn(f32, f32, f32) -> Vec3) {
    for l in layout::LADDERS {
        let face = vec3(l.face.0, 0.0, l.face.1);
        let across = vec3(l.face.1, 0.0, -l.face.0);
        let plane = |y: f32| at(l.x, y, l.z);
        let rail = layout::LADDER_HALF;
        for s in [-1.0f32, 1.0] {
            beam(m, plane(l.foot) + across * (s * rail), plane(l.top + 1.1) + across * (s * rail), 0.075, 0.075, STEEL);
        }
        let mut y = l.foot + 0.5;
        while y < l.top - 0.05 {
            beam(m, plane(y) - across * (rail - 0.03), plane(y) + across * (rail - 0.03), 0.045, 0.045, STEEL);
            y += layout::RUNG_PITCH;
        }
        let back = (plane(l.foot) - across * (rail + 0.05), plane(l.top) + across * (rail + 0.05) + face * layout::LADDER_BACK);
        m.cuboid(back.0.min(back.1) - Vec3::Y * if l.foot <= 0.0 { 0.3 } else { 0.0 }, back.0.max(back.1), DARK);
        // Cage hoops, open where the climber's hands are.
        let hoop = |k: usize, y: f32| {
            let t = k as f32 / 5.0 * PI;
            plane(y) + across * (0.62 * t.cos()) - face * (0.95 * t.sin())
        };
        let (first, mut last) = (l.foot + layout::HEADROOM, l.foot + layout::HEADROOM);
        let mut y = first;
        while y < l.top - 0.3 {
            for k in 0..5 {
                beam(m, hoop(k, y), hoop(k + 1, y), 0.06, 0.06, STEEL);
            }
            last = y;
            y += 1.2;
        }
        for k in 1..5 {
            beam(m, hoop(k, first), hoop(k, last), 0.05, 0.05, STEEL);
        }
    }
    let l = layout::LADDERS[1];
    let plane = |y: f32| at(l.x, y, l.z);
    let (start, end) = (layout::TOP_DECK + layout::HEADROOM, 56.0);
    for s in [-1.0f32, 1.0] {
        beam(m, plane(start) + Vec3::X * (s * layout::LADDER_HALF), plane(end) + Vec3::X * (s * layout::LADDER_HALF), 0.075, 0.075, STEEL);
    }
    let mut y = start + 0.2;
    while y < end {
        beam(m, plane(y) - Vec3::X * 0.4, plane(y) + Vec3::X * 0.4, 0.045, 0.045, STEEL);
        y += layout::RUNG_PITCH;
    }
}

// The outermost ring is planted into the surrounding ridge; all other rings
// form a closed, faceted cone and the inside of its sunken caldera.
const CONE: [(f32, f32); 7] = [(47.0, -2.0), (39.0, 6.0), (27.0, 20.0), (17.0, 35.0), (10.0, 48.0), (6.8, 47.2), (5.8, 43.2)];
fn cone_point(root: Vec3, ring: usize, angle: f32) -> Vec3 {
    let (r, h) = CONE[ring];
    let jag = (angle * 7.0 + 0.3).sin() * 0.032 + (angle * 11.0).cos() * 0.018;
    root + vec3(angle.cos() * r * (1.0 + jag), h + (angle * 9.0).cos() * if ring == 0 { 0.0 } else { 0.55 }, angle.sin() * r * (1.0 + jag))
}
fn volcano(m: &mut Mesh, w: &World) {
    let root = volcano_origin(w);
    const N: usize = 48;
    for ring in 0..CONE.len() - 1 {
        for k in 0..N {
            let a0 = k as f32 * TAU / N as f32;
            let a1 = (k + 1) as f32 * TAU / N as f32;
            let p = |r, a| {
                let mut p = cone_point(root, r, a);
                if r == 0 {
                    p.y = w.height(p.x, p.z) - 1.0;
                }
                p
            };
            let c = if ring >= 4 { [0.19, 0.115, 0.10, 0.0] } else { [0.25, 0.225, 0.24, 0.0] };
            m.quad(p(ring, a0), p(ring + 1, a0), p(ring + 1, a1), p(ring, a1), shade(c, 0.8 + hash01(k as u32, ring as u32 + 414) * 0.4));
        }
    }
    // Caldera pool and narrow glowing seams pouring over the near rim.
    let lava = root + Vec3::Y * 43.22;
    for k in 0..32 {
        let a = k as f32 * TAU / 32.0;
        let b = (k + 1) as f32 * TAU / 32.0;
        m.tri(lava, lava + vec3(a.cos(), 0.0, a.sin()) * 5.8, lava + vec3(b.cos(), 0.0, b.sin()) * 5.8, HOT);
    }
    for (seam, angle) in [0.53f32, 1.25, 1.84, 2.40, 4.15].into_iter().enumerate() {
        for ring in 1..5 {
            let a = angle + (ring as f32 * 1.8 + seam as f32).sin() * 0.05;
            let b = angle + ((ring + 1) as f32 * 1.8 + seam as f32).sin() * 0.05;
            let width = if ring > 2 { 0.018 } else { 0.012 };
            m.quad(
                cone_point(root, ring, a - width) + Vec3::Y * 0.12,
                cone_point(root, ring + 1, b - width) + Vec3::Y * 0.12,
                cone_point(root, ring + 1, b + width) + Vec3::Y * 0.12,
                cone_point(root, ring, a + width) + Vec3::Y * 0.12,
                shade(HOT, 0.65),
            );
        }
    }
}

pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    if w.map == 5 {
        let root = mast_origin(w);
        let pulse = if time.rem_euclid(2.0) < 0.18 { 1.0 } else { 0.08 };
        // Obstruction lamps: a pair on the legs over the battlefield, then
        // up the axis above the top deck, out of the climbers' faces.
        let low = layout::mast_half(26.0) + 0.25;
        for p in [vec3(-low, 26.0, -low), vec3(low, 26.0, -low), vec3(0.0, 45.0, 0.0), vec3(0.0, 64.0, 0.0)] {
            let p = root + p;
            cross(add, p, 0.23, 0.23, [1.0, 0.10, 0.035, pulse]);
            soft(add, p, 1.2, 1.2, [1.0, 0.08, 0.025, pulse * 0.65]);
        }
        // A rotating microwave scanner between the first two panel rings,
        // above the top deck and clear of the climb.
        let g = Mat4::from_translation(root + vec3(0.0, 43.0, 0.0)) * Mat4::from_rotation_y(time * 0.35);
        m.obox(&g, vec3(0.0, 0.0, 3.4), vec3(1.7, 0.45, 0.14), WHITE);
        m.obox(&g, vec3(0.0, 0.0, 1.7), vec3(0.12, 0.13, 1.7), STEEL);
    } else if w.map == 6 {
        volcano_atmosphere(m, alpha, add, w, time.max(0.0));
    }
}

fn volcano_atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let root = volcano_origin(w);
    let vent = root + Vec3::Y * 47.0;
    let state = eruption(w.seed, time);
    let seed = (w.seed ^ (w.seed >> 32)) as u32;
    // Even a dormant cone breathes. During a warning the column thickens;
    // after the blast it spreads into a large, dark anvil of ash aloft.
    for k in 0..42u32 {
        let age = (time * (0.035 + hash01(k, 744) * 0.014) + hash01(k, seed ^ 745)).fract();
        let h = age * (29.0 + 39.0 * state.ash);
        let width = 2.0 + age * (8.0 + state.ash * 13.0);
        let p = vent + vec3(age * (15.0 + 12.0 * state.ash) + (k as f32 * 1.9).sin() * age * 2.7, h, age * -7.0);
        let opacity = (0.13 + state.warning * 0.14 + state.ash * 0.18) * (1.0 - age).powf(0.6);
        soft(alpha, p, width, width * 0.85, [0.12, 0.105, 0.12, opacity]);
    }
    let flicker = 0.8 + 0.2 * (time * 8.3).sin();
    soft(
        add,
        vent - Vec3::Y * 1.0,
        5.8 + state.fountain * 3.0,
        2.0 + state.fountain * 6.0,
        [1.0, 0.22, 0.025, (0.22 + state.warning * 0.45) * flicker],
    );
    if state.fountain > 0.001 {
        // A pulsing fountain emerges from the crater before the larger bombs
        // arc outward. Every bomb and its tail stays on the distant cone.
        for k in 0..12u32 {
            let a = k as f32 * TAU / 12.0;
            let wave = 0.75 + 0.25 * (time * 4.8 + k as f32 * 1.3).sin();
            let top = vent + vec3(a.cos() * 1.8, 8.0 + 17.0 * state.fountain * wave, a.sin() * 1.8);
            beam(m, vent + vec3(a.cos(), 0.0, a.sin()) * 2.4, top, 0.35 + state.fountain * 0.32, 0.35, [1.0, 0.36, 0.045, 1.0]);
        }
        for k in 0..40u32 {
            let life = 4.8 + hash01(k, 751) * 2.0;
            let age = (time / life + hash01(k, seed ^ state.cycle ^ 752)).fract();
            let secs = age * life;
            let a = hash01(k, 753) * TAU;
            let reach = 2.5 + hash01(k, 754) * 2.8;
            let speed = 16.0 + hash01(k, 755) * 8.0;
            let p = vent + vec3(a.cos() * reach * secs, speed * secs - 4.8 * secs * secs, a.sin() * reach * secs);
            if p.y < root.y + 20.0 {
                continue;
            }
            let size = (0.20 + hash01(k, 756) * 0.34) * state.fountain;
            let velocity = vec3(a.cos() * reach, speed - 9.6 * secs, a.sin() * reach);
            let tail = p - velocity.normalize_or_zero() * (1.2 + age * 1.8);
            beam(m, tail, p, size * 0.65, size * 0.65, [0.8, 0.14, 0.015, 0.95]);
            m.cuboid(p - Vec3::splat(size), p + Vec3::splat(size), [1.0, 0.61, 0.10, 1.0]);
            soft(add, p, size * 3.0, size * 3.0, [1.0, 0.28, 0.025, state.fountain * (1.0 - age) * 0.85]);
        }
    }
    // The crater breaches on its northern flank and the overflow runs down
    // into the runner that carries it onto the field (`events::lava_flow`).
    if state.erupts {
        let phase = state.age + 26.0;
        let glow = smooth(26.5, 30.0, phase)
            * (1.0 - smooth(ferrite_shared::events::LAVA_CRUSTS - 2.0, ferrite_shared::events::LAVA_COOLED, phase));
        if glow > 0.0 {
            let north = PI * 0.5;
            let (sx, sz) = ferrite_shared::map::slag_cathedral::RUNNER[0];
            let path = [
                cone_point(root, 2, north) + vec3(0.0, -0.4, 0.0),
                cone_point(root, 1, north + 0.03),
                cone_point(root, 0, north - 0.02) + Vec3::Y * 0.4,
                vec3(sx, w.height(sx, sz) + 0.1, sz),
            ];
            let hot = [
                HOT[0] * glow + 0.08 * (1.0 - glow),
                HOT[1] * glow + 0.07 * (1.0 - glow),
                HOT[2] * glow + 0.06 * (1.0 - glow),
                HOT[3] * glow,
            ];
            for (k, pair) in path.windows(2).enumerate() {
                beam(m, pair[0] + Vec3::Y * 0.3, pair[1] + Vec3::Y * 0.3, 3.4 - k as f32 * 0.4, 0.6, hot);
                beam(m, pair[0] + Vec3::Y * 0.62, pair[1] + Vec3::Y * 0.62, 1.2, 0.1, [1.0 * glow, 0.62 * glow, 0.18 * glow, glow]);
                for j in 0..4 {
                    let p = pair[0].lerp(pair[1], (j as f32 + 0.5) / 4.0) + Vec3::Y * 1.2;
                    soft(add, p, 3.2, 2.0, [1.0, 0.3, 0.04, 0.22 * glow]);
                }
            }
        }
    }
    // The plume rains fine incandescent fragments after its main fountain.
    for k in 0..55u32 {
        let age = (time * 0.10 + hash01(k, seed ^ 761)).fract();
        let a = hash01(k, 762) * TAU;
        let spread = 3.0 + age * (8.0 + hash01(k, 763) * 15.0);
        let p = vent + vec3(a.cos() * spread + age * 12.0, 5.0 + (1.0 - age) * 32.0, a.sin() * spread);
        cross(add, p, 0.10, 0.24, [1.0, 0.30, 0.04, state.ash * (1.0 - age) * 0.8]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_shared::events::ERUPTION_PERIOD;

    fn finite(mesh: &Mesh) {
        for vertices in mesh.chunks.iter().map(|(v, _)| v).chain(std::iter::once(&mesh.v)) {
            for v in vertices {
                assert!(v.pos.iter().chain(v.nrm.iter()).chain(v.col.iter()).all(|x| x.is_finite()));
            }
        }
    }

    #[test]
    fn great_landmarks_and_all_eruption_stages_are_finite_and_bounded() {
        for id in [5, 6] {
            let w = World::with_map(0xFE221, id);
            let mut static_mesh = Mesh::default();
            build(&mut static_mesh, &w);
            finite(&static_mesh);
            assert!(static_mesh.triangles() > 500 && static_mesh.triangles() < 14000, "{} triangles", static_mesh.triangles());
            // The mast stands on its own levelled site in the field; the
            // volcano stays beyond it.
            let (x0, z0, x1, z1) = layout::MAST_SITE;
            let (mx, mz) = MAST_XZ;
            let ground = mast_origin(&w).y;
            for vertices in static_mesh.chunks.iter().map(|(v, _)| v).chain(std::iter::once(&static_mesh.v)) {
                for v in vertices {
                    let [x, y, z] = v.pos;
                    let ok = if id == 5 {
                        x > mx + x0 && x < mx + x1 && z > mz + z0 && z < mz + z1 && y > ground - 1.0 && y < ground + 70.0
                    } else {
                        z < -110.0
                    };
                    assert!(ok, "landmark strays from its site: {:?}", v.pos);
                }
            }
            for time in [0.0, 22.0, 32.0, 45.0, 62.0, 128.0, 224.0, 10001.0] {
                let (mut m, mut a, mut glow) = (Mesh::default(), Mesh::default(), Mesh::default());
                atmosphere(&mut m, &mut a, &mut glow, &w, time);
                for mesh in [&m, &a, &glow] {
                    finite(mesh);
                }
                assert!(m.triangles() + a.triangles() + glow.triangles() < 5000);
            }
        }
    }

    #[test]
    fn eruption_has_a_real_chance_and_warning_blast_and_cooldown() {
        let seed = 0xFE221;
        let active = (0..64).filter(|&cycle| eruption(seed, cycle as f32 * ERUPTION_PERIOD + 33.0).fountain > 0.5).collect::<Vec<_>>();
        assert!(!active.is_empty() && active.len() < 64, "need both erupting and dormant windows");
        let base = active[0] as f32 * ERUPTION_PERIOD;
        let warning = eruption(seed, base + 23.0);
        assert!(warning.warning > 0.0 && warning.fountain == 0.0);
        let blast = eruption(seed, base + 33.0);
        assert!(blast.fountain > 0.9 && blast.ash > 0.0);
        let cooling = eruption(seed, base + 60.0);
        assert!(cooling.ash > 0.5 && cooling.fountain == 0.0);
        assert_eq!(eruption(seed, base + 33.0), blast);
        eprintln!("Volcano review blast time: {}", base + 33.0);
    }
}
