//! Foundry Valley: the Broken Viaduct. A riveted steel lattice arch carries
//! the old ore line 44 m over the battlefield, across the valley from ridge
//! to ridge along x = 0. Beyond the north fence the next span has fallen onto
//! the slope; the freight locomotive that was crossing hangs nose-down over
//! the gap, held only by its coupling to the tender. Abandoned ore cars
//! stand on the deck above the field.
use super::{bar, block, drum, ground_range, jit, roll, ticks, Cue};
use crate::gfx::{shade, Col, Mesh};
use crate::scene::slag_cathedral::art::{soft, stencil, wall};
use crate::scene::slag_cathedral::{beam, girder};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use std::f32::consts::{PI, TAU};

pub const MAP: u8 = 0;

const RUST: Col = [0.47, 0.25, 0.15, 0.0];
const RUST_DARK: Col = [0.30, 0.17, 0.11, 0.0];
const RUST_PALE: Col = [0.60, 0.36, 0.21, 0.0];
const STONE: Col = [0.56, 0.52, 0.46, 0.0];
const STONE_DARK: Col = [0.40, 0.37, 0.33, 0.0];
const IRON: Col = [0.10, 0.10, 0.11, 0.0];
const TIMBER: Col = [0.30, 0.22, 0.15, 0.0];
const RAIL: Col = [0.36, 0.33, 0.31, 0.0];
const BUFFER: Col = [0.52, 0.13, 0.08, 0.0];
const ORE: Col = [0.52, 0.31, 0.19, 0.0];
const TUNNEL: Col = [0.05, 0.05, 0.06, 0.0];

/// Rail level above the valley floor at C.
const DECK: f32 = 44.0;
/// The arch springs from footings this far either side of the valley's middle.
const SPAN: f32 = 116.0;
/// Deck girder depth below rail level.
const GIRDER: f32 = 2.8;
/// The north approach span is gone between these two piers.
const BREAK: (f32, f32) = (127.0, 145.0);
/// Towers carrying the approaches (z of each), north then south.
const TRESTLES: [f32; 3] = [147.5, -131.0, -144.0];
/// Abutments where the line enters the hills.
const ABUTMENT: f32 = 153.0;

fn floor(w: &World) -> f32 {
    w.points[2].y
}
fn deck_y(w: &World) -> f32 {
    floor(w) + DECK
}
/// Height of the arch footings: just above the ground at both springings.
fn spring_y(w: &World) -> f32 {
    let mut hi = f32::MIN;
    for z in [SPAN, -SPAN] {
        for x in [-9.0, 9.0] {
            hi = hi.max(w.height(x, z));
        }
    }
    hi + 1.0
}
/// Half the distance between the two arch ribs: they splay at the footings.
fn rib_x(z: f32) -> f32 {
    4.4 + 4.0 * (z / SPAN).powi(2)
}
/// Depth of each rib's lattice.
fn rib_depth(z: f32) -> f32 {
    3.6 + 3.4 * (z / SPAN).powi(2)
}
/// Underside (intrados) of the arch at z: a parabola from the footings.
fn intrados(w: &World, z: f32) -> f32 {
    let s = spring_y(w);
    let crown = deck_y(w) - GIRDER - rib_depth(0.0) - 0.2;
    s + (crown - s) * (1.0 - (z / SPAN).powi(2))
}

pub fn covers(x: f32, z: f32, r: f32) -> bool {
    let near = |cz: f32, hz: f32, hx: f32| x.abs() < hx + r && (z - cz).abs() < hz + r;
    // Footings, towers, the fallen span and the abutments; the arch itself
    // passes high over everything.
    near(SPAN + 4.0, 9.0, 13.0)
        || near(-SPAN - 4.0, 9.0, 13.0)
        || near(137.0, 10.0, 13.0)
        || TRESTLES.iter().any(|&t| near(t, 5.0, 9.0))
        || near(ABUTMENT + 6.0, 10.0, 12.0)
        || near(-ABUTMENT - 6.0, 10.0, 12.0)
}

#[allow(dead_code)] // the asset_review example's cameras
pub fn view(w: &World, view: &str) -> Option<(Vec3, Vec3)> {
    let g = floor(w);
    let d = deck_y(w);
    Some(match view {
        // From the field under the arch, looking north at the hanging train.
        "viaduct" => (vec3(62.0, w.height(62.0, 82.0) + 1.8, 82.0), vec3(0.0, d - 2.0, 128.0)),
        // From above the break, beside the locomotive.
        "train" => (vec3(30.0, d + 18.0, 138.0), vec3(0.0, d + 4.0, 129.0)),
        // The whole crossing from a base, as a soldier sees it.
        "crossing" => (vec3(-150.0, w.height(-150.0, -20.0) + 1.8, -20.0), vec3(0.0, g + 26.0, 10.0)),
        _ => return None,
    })
}

pub fn build(m: &mut Mesh, w: &World) {
    arch(m, w);
    deck(m, w, -ABUTMENT, BREAK.0);
    deck(m, w, BREAK.1, ABUTMENT);
    torn_ends(m, w);
    for (k, &z) in TRESTLES.iter().enumerate() {
        trestle(m, w, z, k as u32);
    }
    for side in [1.0f32, -1.0] {
        footing(m, w, side);
        abutment(m, w, side);
    }
    fallen_span(m, w);
    // Abandoned ore cars over the field, nose to tail behind the tender.
    let d = deck_y(w);
    for k in 0..5 {
        let z = BREAK.0 - 13.8 - k as f32 * 9.4;
        ore_car(m, &Mat4::from_translation(vec3(0.0, d, z)), 31 + k);
    }
}

// ----------------------------------------------------------------- arch ----

fn arch(m: &mut Mesh, w: &World) {
    const N: usize = 32;
    let node = |k: usize, side: f32, outer: bool| {
        let z = -SPAN + 2.0 * SPAN * k as f32 / N as f32;
        let y = intrados(w, z) + if outer { rib_depth(z) } else { 0.0 };
        vec3(side * rib_x(z), y, z)
    };
    for side in [-1.0f32, 1.0] {
        for k in 0..N {
            let s = (k as u32) * 7 + if side > 0.0 { 3 } else { 0 };
            for outer in [false, true] {
                bar(m, node(k, side, outer), node(k + 1, side, outer), 1.05, jit(RUST, s + outer as u32));
            }
            bar(m, node(k, side, false), node(k, side, true), 0.55, jit(RUST_DARK, s + 2));
            // Pratt diagonals lean toward the crown from both footings.
            let (a, b) =
                if k < N / 2 { (node(k, side, false), node(k + 1, side, true)) } else { (node(k, side, true), node(k + 1, side, false)) };
            bar(m, a, b, 0.42, jit(RUST_DARK, s + 4));
        }
        bar(m, node(N, side, false), node(N, side, true), 0.55, RUST_DARK);
    }
    // Lateral bracing between the ribs: struts at every node, crosses in
    // alternate bays along the underside.
    for k in 0..=N {
        bar(m, node(k, -1.0, true), node(k, 1.0, true), 0.38, jit(RUST_DARK, k as u32 + 400));
        if k % 2 == 0 && k < N {
            bar(m, node(k, -1.0, false), node(k + 2, 1.0, false), 0.32, RUST_DARK);
            bar(m, node(k, 1.0, false), node(k + 2, -1.0, false), 0.32, RUST_DARK);
        }
    }
    // Spandrel bents carry the deck from the arch, every second node.
    let d = deck_y(w) - GIRDER;
    for k in (2..N - 1).step_by(2) {
        let z = -SPAN + 2.0 * SPAN * k as f32 / N as f32;
        let foot_y = intrados(w, z) + rib_depth(z);
        if d - foot_y < 1.2 {
            continue;
        }
        let (fx, tx) = (rib_x(z), 3.3);
        for side in [-1.0f32, 1.0] {
            bar(m, vec3(side * fx, foot_y, z), vec3(side * tx, d, z), 0.62, jit(RUST, k as u32 + 500));
        }
        let rungs = ((d - foot_y) / 6.5).ceil().max(1.0) as usize;
        for r in 0..=rungs {
            let t = r as f32 / rungs as f32;
            let y = foot_y + (d - foot_y) * t;
            let x = fx + (tx - fx) * t;
            bar(m, vec3(-x, y, z), vec3(x, y, z), 0.34, RUST_DARK);
            if r < rungs {
                let t1 = (r + 1) as f32 / rungs as f32;
                let (y1, x1) = (foot_y + (d - foot_y) * t1, fx + (tx - fx) * t1);
                let flip = if r % 2 == 0 { 1.0 } else { -1.0 };
                bar(m, vec3(-x * flip, y, z), vec3(x1 * flip, y1, z), 0.26, RUST_DARK);
            }
        }
    }
}

/// Concrete footing and the tall braced pier that rises from it to the deck.
fn footing(m: &mut Mesh, w: &World, side: f32) {
    let z = side * SPAN;
    let c = vec3(0.0, 0.0, z + side * 3.0);
    let (lo, _) = ground_range(w, c, 11.0);
    let top = spring_y(w);
    m.cuboid(vec3(-12.5, lo - 3.0, c.z - 8.0), vec3(12.5, top, c.z + 8.0), jit(STONE, 9));
    m.cuboid(vec3(-13.1, top - 1.4, c.z - 8.6), vec3(13.1, top - 0.6, c.z + 8.6), STONE_DARK);
    // Bearing shoes under each rib.
    for x in [-1.0f32, 1.0] {
        let p = vec3(x * rib_x(z), top, z);
        m.cuboid(p - vec3(1.6, 0.0, 1.6), p + vec3(1.6, 1.2, 1.6), IRON);
    }
    let deck = deck_y(w) - GIRDER;
    tower(m, vec3(0.0, deck, z + side * 3.6), top + 1.0, 5.4, 2.6, 0.05, 0.03, 61 + side as u32);
    // Knee braces carry the deck out past the tower to the next span.
    for x in [-3.3f32, 3.3] {
        bar(m, vec3(x, deck - 9.0, z + side * 6.0), vec3(x, deck - 0.2, z + side * 11.0), 0.6, RUST);
        bar(m, vec3(x, deck - 4.0, z + side * 6.4), vec3(x, deck - 0.2, z + side * 6.4), 0.4, RUST_DARK);
    }
}

fn trestle(m: &mut Mesh, w: &World, z: f32, seed: u32) {
    let (lo, hi) = ground_range(w, vec3(0.0, 0.0, z), 6.0);
    let top = deck_y(w) - GIRDER;
    m.cuboid(vec3(-7.5, lo - 2.5, z - 4.0), vec3(7.5, hi + 1.0, z + 4.0), jit(STONE, seed + 20));
    tower(m, vec3(0.0, top, z), hi + 1.0, 4.6, 2.2, 0.08, 0.04, seed);
}

/// A riveted lattice tower: four legs splaying toward the ground, braced in
/// bays of about six metres on every face.
#[allow(clippy::too_many_arguments)]
fn tower(m: &mut Mesh, top: Vec3, foot_y: f32, hx: f32, hz: f32, splay_x: f32, splay_z: f32, seed: u32) {
    let corner = |sx: f32, sz: f32, y: f32| {
        let drop = top.y - y;
        top + vec3(sx * (hx + splay_x * drop), y - top.y, sz * (hz + splay_z * drop))
    };
    let bays = ((top.y - foot_y) / 6.0).ceil().max(1.0) as usize;
    let at = |b: usize| top.y - (top.y - foot_y) * b as f32 / bays as f32;
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        bar(m, corner(sx, sz, top.y), corner(sx, sz, foot_y), 0.85, jit(RUST, seed * 13 + (sx + sz * 2.0 + 4.0) as u32));
    }
    for b in 0..=bays {
        let y = at(b);
        let col = jit(RUST_DARK, seed * 31 + b as u32);
        bar(m, corner(-1.0, -1.0, y), corner(1.0, -1.0, y), 0.36, col);
        bar(m, corner(-1.0, 1.0, y), corner(1.0, 1.0, y), 0.36, col);
        bar(m, corner(-1.0, -1.0, y), corner(-1.0, 1.0, y), 0.36, col);
        bar(m, corner(1.0, -1.0, y), corner(1.0, 1.0, y), 0.36, col);
        if b < bays {
            let y1 = at(b + 1);
            let f = if b % 2 == 0 { 1.0 } else { -1.0 };
            for sz in [-1.0f32, 1.0] {
                bar(m, corner(-f, sz, y), corner(f, sz, y1), 0.3, col);
            }
            for sx in [-1.0f32, 1.0] {
                bar(m, corner(sx, -f, y), corner(sx, f, y1), 0.3, col);
            }
        }
    }
}

// ----------------------------------------------------------------- deck ----

/// Plate-girder deck with ballast, sleepers, rails and a parapet, from z0
/// to z1 at rail level.
fn deck(m: &mut Mesh, w: &World, z0: f32, z1: f32) {
    let d = deck_y(w);
    let g = Mat4::from_translation(vec3(0.0, d, 0.0));
    deck_section(m, &g, z0, z1, true);
}

/// The deck in frame `g` (rail level at y = 0, running along z).
fn deck_section(m: &mut Mesh, g: &Mat4, z0: f32, z1: f32, sleepers: bool) {
    let p = |x: f32, y: f32, z: f32| g.transform_point3(vec3(x, y, z));
    let segs = ((z1 - z0) / 14.0).ceil().max(1.0) as usize;
    for s in 0..segs {
        let a = z0 + (z1 - z0) * s as f32 / segs as f32;
        let b = z0 + (z1 - z0) * (s + 1) as f32 / segs as f32;
        let col = jit(RUST, s as u32 + (z0.abs() as u32) * 3);
        for x in [-3.3f32, 3.3] {
            girder(m, p(x, -GIRDER * 0.5 - 0.35, a), p(x, -GIRDER * 0.5 - 0.35, b), 0.9, GIRDER, col);
        }
        // Deck plate and ballast bed.
        block(m, g, vec3(-4.4, -0.75, a), vec3(4.4, -0.4, b), shade(RUST_DARK, 0.9));
        block(m, g, vec3(-1.9, -0.4, a), vec3(1.9, -0.05, b), STONE_DARK);
        for x in [-0.72f32, 0.72] {
            beam(m, p(x, 0.0, a), p(x, 0.0, b), 0.14, 0.18, RAIL);
        }
        // Parapet: a top rail on stanchions every seven metres.
        for x in [-4.25f32, 4.25] {
            beam(m, p(x, 1.05, a), p(x, 1.05, b), 0.12, 0.12, RUST_DARK);
            beam(m, p(x, 0.5, a), p(x, 0.5, b), 0.08, 0.08, RUST_DARK);
            bar(m, p(x, -0.4, a), p(x, 1.1, a), 0.14, RUST_DARK);
        }
        // Cross girders under the plate.
        bar(m, p(-3.3, -0.9, a), p(3.3, -0.9, a), 0.5, RUST_DARK);
    }
    if sleepers {
        let n = ((z1 - z0) / 2.4) as usize;
        for k in 0..n {
            let z = z0 + 1.2 + k as f32 * 2.4;
            block(m, g, vec3(-1.35, -0.12, z - 0.16), vec3(1.35, 0.02, z + 0.16), TIMBER);
        }
    }
}

/// Where the deck tore: bent girders and rails over the gap, both sides.
fn torn_ends(m: &mut Mesh, w: &World) {
    let d = deck_y(w);
    // South lip of the gap, under the hanging train: the girders buckle
    // downward and the rails splay outward and droop.
    let z = BREAK.0;
    for (x, droop, len) in [(-3.3f32, 0.9f32, 5.5f32), (3.3, 0.55, 3.8)] {
        let a = vec3(x, d - GIRDER * 0.5 - 0.35, z);
        let mid = a + vec3(x * 0.05, -len * 0.35, len * 0.8);
        let tip = mid + vec3(x * 0.12, -len * droop, len * 0.35);
        girder(m, a, mid, 0.9, GIRDER * 0.9, RUST);
        girder(m, mid, tip, 0.8, GIRDER * 0.7, RUST_DARK);
    }
    for (x, out) in [(-0.72f32, -1.0f32), (0.72, 1.0)] {
        let mut p = vec3(x, d, z - 0.3);
        for k in 0..5 {
            let q = p + vec3(out * (0.35 + k as f32 * 0.25), -0.25 - k as f32 * 0.55, 1.1 - k as f32 * 0.12);
            beam(m, p, q, 0.14, 0.18, RAIL);
            p = q;
        }
    }
    // North lip: a short stub of the fallen span still bolted to its tower.
    let z = BREAK.1;
    for x in [-3.3f32, 3.3] {
        let a = vec3(x, d - GIRDER * 0.5 - 0.35, z);
        let tip = a + vec3(0.0, -2.6, -4.6);
        girder(m, a, tip, 0.9, GIRDER * 0.85, RUST_DARK);
    }
    for x in [-0.72f32, 0.72] {
        beam(m, vec3(x, d, z), vec3(x * 1.4, d - 0.8, z - 3.5), 0.14, 0.18, RAIL);
    }
    m.cuboid(vec3(-4.4, d - 0.75, z - 1.6), vec3(4.4, d - 0.4, z), shade(RUST_DARK, 0.85));
    // A signal gantry post at the break, bent toward the gap.
    let foot = vec3(4.3, d - 0.4, BREAK.0 - 2.5);
    let knee = foot + vec3(0.0, 4.2, 0.0);
    let head = knee + vec3(-0.7, 1.6, 2.4);
    bar(m, foot, knee, 0.28, RUST_DARK);
    bar(m, knee, head, 0.24, RUST_DARK);
    m.cuboid(head - vec3(0.4, 0.6, 0.15), head + vec3(0.4, 0.6, 0.15), IRON);
}

/// The fallen approach span lies broken in two on the north slope, with an
/// ore car that went over with it.
fn fallen_span(m: &mut Mesh, w: &World) {
    let ground = |x: f32, z: f32| w.height(x, z);
    let piece = |m: &mut Mesh, a: Vec3, b: Vec3, roll: f32| {
        let (g, len) = super::super::slag_cathedral::basis(a, b);
        // `basis` puts local z along a -> b; turn the section upright and roll it.
        let g = g * Mat4::from_rotation_z(roll) * Mat4::from_translation(vec3(0.0, 0.0, -len * 0.5));
        deck_section(m, &g, 0.0, len, true);
    };
    // The south half fell almost flat; its far end lifts off a rock.
    let a = vec3(-1.5, ground(-1.5, 128.5) + 1.2, 128.5);
    let b = vec3(1.8, ground(1.8, 136.5) + 3.4, 136.5);
    piece(m, a, b, 0.32);
    // The north half leans against the tower it fell from.
    let c = vec3(3.5, ground(3.5, 138.0) + 0.8, 138.0);
    let e = vec3(1.0, w.height(1.0, 144.0) + 13.0, 144.0);
    piece(m, c, e, -0.42);
    // Twisted rail lengths and a capsized ore car in the scree.
    for (k, (x, z)) in [(-6.0f32, 130.0f32), (7.0, 133.0), (-4.0, 139.0)].into_iter().enumerate() {
        let a = vec3(x, ground(x, z) + 0.2, z);
        let b = a + vec3(2.5 + k as f32, 1.0, 5.0 - k as f32 * 2.0);
        let c = b + vec3(3.0, -1.0 + ground(b.x + 3.0, b.z + 1.0) - b.y + 0.2, 1.0);
        beam(m, a, b, 0.14, 0.18, RAIL);
        beam(m, b, c, 0.14, 0.18, RAIL);
    }
    let p = vec3(10.0, ground(10.0, 132.0) + 1.5, 132.0);
    let g =
        Mat4::from_translation(p) * Mat4::from_rotation_y(0.7) * Mat4::from_rotation_z(1.9) * Mat4::from_translation(vec3(0.0, -2.0, 0.0));
    ore_car(m, &g, 77);
    for k in 0..7u32 {
        let (x, z) = (-9.0 + k as f32 * 3.1, 128.0 + (k * 5 % 7) as f32 * 2.4);
        let p = vec3(x, ground(x, z), z);
        let s = 0.8 + 1.1 * crate::scene::hash01(k, 0x0E1);
        m.cuboid(p - vec3(s, 0.3, s * 0.8), p + vec3(s, s * 0.9, s * 0.8), jit(ORE, k));
    }
}

/// Stone abutment where the line goes into the hillside through a portal.
fn abutment(m: &mut Mesh, w: &World, side: f32) {
    let d = deck_y(w);
    let z0 = side * ABUTMENT;
    let (lo, _) = ground_range(w, vec3(0.0, 0.0, z0 + side * 6.0), 10.0);
    let near = z0;
    let far = z0 + side * 14.0;
    let (za, zb) = (near.min(far), near.max(far));
    // Stepped masonry, battered inward as it rises.
    for (k, (inset, y0, y1)) in [(0.0f32, lo - 3.0, d - 9.0), (0.9, d - 9.0, d - 0.5)].into_iter().enumerate() {
        m.cuboid(vec3(-8.0 + inset, y0, za + inset * 0.5), vec3(8.0 - inset, y1, zb - inset * 0.5), jit(STONE, 40 + k as u32));
    }
    m.cuboid(vec3(-7.6, d - 0.5, za), vec3(7.6, d + 0.4, zb), STONE_DARK);
    // The hill the line burrows into, and the portal.
    let hill = vec3(0.0, d, far + side * 4.0);
    let rock = |m: &mut Mesh, c: Vec3, r: f32, h: f32, seed: u32| {
        const N: usize = 7;
        let top = c + Vec3::Y * h;
        for k in 0..N {
            let a0 = k as f32 / N as f32 * TAU + seed as f32;
            let a1 = (k + 1) as f32 / N as f32 * TAU + seed as f32;
            let j = |a: f32| 0.82 + 0.3 * crate::scene::hash01((a * 100.0) as u32, seed);
            let p0 = c + vec3(a0.cos() * r * j(a0), -4.0 - h * 0.5, a0.sin() * r * j(a0));
            let p1 = c + vec3(a1.cos() * r * j(a1), -4.0 - h * 0.5, a1.sin() * r * j(a1));
            let q0 = c + vec3(a0.cos() * r * 0.62, h * 0.86, a0.sin() * r * 0.62);
            let q1 = c + vec3(a1.cos() * r * 0.62, h * 0.86, a1.sin() * r * 0.62);
            m.quad(p0, q0, q1, p1, jit(STONE_DARK, seed * 9 + k as u32));
            m.tri(q0, top, q1, jit(STONE, seed * 11 + k as u32));
        }
    };
    let lo_h = w.height(hill.x, hill.z);
    rock(m, vec3(0.0, lo_h, hill.z + side * 8.0), 24.0, d - lo_h + 7.0, 3 + side as u32);
    let portal = wall(vec3(0.0, d, far - side * 0.2), if side > 0.0 { PI } else { 0.0 });
    m.obox(&portal, vec3(0.0, 3.6, 0.3), vec3(3.4, 3.6, 0.6), TUNNEL);
    m.obox(&portal, vec3(0.0, 8.0, 0.1), vec3(5.2, 0.9, 0.9), STONE);
    for x in [-4.4f32, 4.4] {
        m.obox(&portal, vec3(x, 3.6, 0.1), vec3(0.9, 3.8, 0.9), STONE);
    }
    stencil(
        m,
        &(portal * Mat4::from_translation(vec3(0.0, 7.55, 1.02))),
        if side > 0.0 { "1907" } else { "ORE LINE" },
        0.16,
        shade(STONE, 0.6),
    );
}

// ---------------------------------------------------------- rolling stock --

fn wheel(m: &mut Mesh, g: &Mat4, c: Vec3, r: f32, segs: usize, col: Col) {
    drum(m, g.transform_point3(c - Vec3::X * 0.09), g.transform_point3(c + Vec3::X * 0.09), r, segs, col);
}

/// A hopper ore car, rail level at y = 0 of `g`, centred on its origin.
fn ore_car(m: &mut Mesh, g: &Mat4, seed: u32) {
    let col = jit(RUST, seed);
    let p = |x: f32, y: f32, z: f32| g.transform_point3(vec3(x, y, z));
    // Hopper: wide at the top, pinched to its doors.
    m.corners(
        [
            p(-1.0, 1.1, -2.6),
            p(1.0, 1.1, -2.6),
            p(-1.55, 3.3, -4.0),
            p(1.55, 3.3, -4.0),
            p(-1.0, 1.1, 2.6),
            p(1.0, 1.1, 2.6),
            p(-1.55, 3.3, 4.0),
            p(1.55, 3.3, 4.0),
        ],
        col,
    );
    // Its load of ore, heaped above the rim.
    m.corners(
        [
            p(-1.45, 3.25, -3.8),
            p(1.45, 3.25, -3.8),
            p(-0.6, 3.95, -2.6),
            p(0.6, 3.95, -2.6),
            p(-1.45, 3.25, 3.8),
            p(1.45, 3.25, 3.8),
            p(-0.6, 3.95, 2.6),
            p(0.6, 3.95, 2.6),
        ],
        jit(ORE, seed + 1),
    );
    for z in [-3.6f32, -1.2, 1.2, 3.6] {
        block(m, g, vec3(-1.62, 1.2, z - 0.1), vec3(1.62, 3.35, z + 0.1), shade(col, 0.7));
    }
    block(m, g, vec3(-1.0, 0.75, -4.3), vec3(1.0, 1.15, 4.3), IRON);
    for z in [-2.9f32, 2.9] {
        block(m, g, vec3(-1.15, 0.3, z - 1.0), vec3(1.15, 0.8, z + 1.0), IRON);
        for x in [-0.82f32, 0.82] {
            for dz in [-0.7f32, 0.7] {
                block(m, g, vec3(x - 0.08, 0.0, z + dz - 0.42), vec3(x + 0.08, 0.84, z + dz + 0.42), RUST_DARK);
            }
        }
    }
}

/// The tender, rail level at y = 0, its rear at z = 0, 7.6 m long.
fn tender(m: &mut Mesh, g: &Mat4) {
    block(m, g, vec3(-1.5, 1.05, 0.2), vec3(1.5, 3.4, 7.4), IRON);
    block(m, g, vec3(-1.56, 3.2, 0.15), vec3(1.56, 3.45, 7.45), shade(RUST_DARK, 0.8));
    // Coal heaped in the bunker.
    let p = |x: f32, y: f32, z: f32| g.transform_point3(vec3(x, y, z));
    m.corners(
        [
            p(-1.4, 3.3, 2.2),
            p(1.4, 3.3, 2.2),
            p(-0.7, 4.0, 3.2),
            p(0.7, 4.0, 3.2),
            p(-1.4, 3.3, 7.2),
            p(1.4, 3.3, 7.2),
            p(-0.7, 4.1, 6.4),
            p(0.7, 4.1, 6.4),
        ],
        [0.07, 0.07, 0.08, 0.0],
    );
    block(m, g, vec3(-1.25, 0.65, 0.0), vec3(1.25, 1.05, 7.6), IRON);
    for z in [1.5f32, 6.1] {
        for dz in [-0.7f32, 0.7] {
            for x in [-1.0f32, 1.0] {
                wheel(m, g, vec3(x, 0.45, z + dz), 0.45, 5, RUST_DARK);
            }
        }
    }
    block(m, g, vec3(-1.5, 0.75, -0.1), vec3(1.5, 1.2, 0.25), BUFFER);
    // Faded running number on both flanks.
    for x in [-1.52f32, 1.52] {
        let side = *g * wall(vec3(x, 2.0, 3.8), if x > 0.0 { PI * 0.5 } else { -PI * 0.5 });
        stencil(m, &side, "FV 07", 0.11, [0.70, 0.62, 0.42, 0.0]);
    }
}

/// An eight-coupled freight engine, rail level at y = 0, rear coupler at
/// the origin, chimney toward +z.
fn locomotive(m: &mut Mesh, g: &Mat4) {
    let p = |x: f32, y: f32, z: f32| g.transform_point3(vec3(x, y, z));
    block(m, g, vec3(-1.5, 0.75, -0.1), vec3(1.5, 1.25, 0.3), BUFFER);
    block(m, g, vec3(-1.15, 0.8, 0.2), vec3(1.15, 1.35, 16.0), IRON);
    block(m, g, vec3(-1.6, 1.35, 4.2), vec3(1.6, 1.47, 16.0), shade(RUST_DARK, 0.7));
    // Cab: side sheets with windows, front sheet, roof.
    for x in [-1.5f32, 1.5] {
        block(m, g, vec3(x - 0.06, 1.35, 0.6), vec3(x + 0.06, 3.0, 4.2), IRON);
        block(m, g, vec3(x - 0.06, 3.0, 0.6), vec3(x + 0.06, 4.05, 1.4), IRON);
        block(m, g, vec3(x - 0.06, 3.0, 3.3), vec3(x + 0.06, 4.05, 4.2), IRON);
    }
    block(m, g, vec3(-1.5, 1.35, 4.1), vec3(1.5, 4.05, 4.25), IRON);
    block(m, g, vec3(-1.7, 4.05, 0.3), vec3(1.7, 4.3, 4.5), shade(RUST_DARK, 0.75));
    // Boiler, bands, smokebox and its door.
    drum(m, p(0.0, 2.55, 4.2), p(0.0, 2.55, 14.2), 1.0, 9, IRON);
    for z in [6.5f32, 9.5, 12.5] {
        drum(m, p(0.0, 2.55, z - 0.08), p(0.0, 2.55, z + 0.08), 1.04, 9, RUST);
    }
    drum(m, p(0.0, 2.55, 14.2), p(0.0, 2.55, 15.8), 1.07, 9, [0.06, 0.06, 0.07, 0.0]);
    drum(m, p(0.0, 2.55, 15.8), p(0.0, 2.55, 15.92), 0.82, 9, RUST_DARK);
    drum(m, p(0.0, 3.3, 15.0), p(0.0, 4.65, 15.0), 0.33, 6, IRON);
    drum(m, p(0.0, 4.5, 15.0), p(0.0, 4.75, 15.0), 0.43, 6, RUST_DARK);
    for (z, h) in [(11.0f32, 0.62f32), (8.0, 0.5)] {
        drum(m, p(0.0, 3.2, z), p(0.0, 3.4 + h, z), 0.48, 6, RUST);
    }
    // Rusted streaks where the boiler lagging failed.
    for (z, a) in [(5.6f32, 0.6f32), (10.4, -0.9), (13.2, 0.3)] {
        let n = vec3(a.sin(), a.cos(), 0.0);
        block(m, &(*g * Mat4::from_translation(vec3(0.0, 2.55, z) + n * 0.98)), vec3(-0.3, -0.05, -0.7), vec3(0.3, 0.05, 0.7), RUST_PALE);
    }
    // Cylinders, wheels and coupling rods.
    for x in [-1.3f32, 1.3] {
        drum(m, p(x, 1.4, 13.6), p(x, 1.4, 15.5), 0.45, 6, IRON);
        for z in [5.6f32, 7.4, 9.2, 11.0] {
            wheel(m, g, vec3(x * 0.82, 0.75, z), 0.74, 6, RUST_DARK);
        }
        wheel(m, g, vec3(x * 0.82, 0.45, 14.4), 0.45, 5, RUST_DARK);
        beam(m, p(x * 0.97, 0.75, 5.6), p(x * 0.97, 0.75, 11.0), 0.1, 0.2, RAIL);
        beam(m, p(x * 0.97, 0.75, 9.2), p(x * 0.97, 1.3, 14.0), 0.1, 0.18, RAIL);
    }
    // Buffer beam, pilot and a dead headlamp.
    block(m, g, vec3(-1.55, 0.7, 16.0), vec3(1.55, 1.35, 16.35), BUFFER);
    m.corners(
        [
            p(-1.4, 0.15, 16.3),
            p(1.4, 0.15, 16.3),
            p(-1.4, 0.75, 16.3),
            p(1.4, 0.75, 16.3),
            p(-0.9, 0.15, 17.3),
            p(0.9, 0.15, 17.3),
            p(-1.0, 0.75, 16.6),
            p(1.0, 0.75, 16.6),
        ],
        RUST_DARK,
    );
    block(m, g, vec3(-0.32, 3.55, 15.4), vec3(0.32, 4.05, 15.95), IRON);
    block(m, g, vec3(-0.22, 3.65, 15.95), vec3(0.22, 3.95, 16.0), [0.55, 0.52, 0.42, 0.0]);
    for x in [-1.5f32, 1.5] {
        let side = *g * wall(vec3(x * 1.01, 2.0, 2.4), if x > 0.0 { PI * 0.5 } else { -PI * 0.5 });
        stencil(m, &side, "4417", 0.1, [0.72, 0.60, 0.38, 0.0]);
    }
}

// ----------------------------------------------------------------- motion --

/// How far the hanging engine swings (radians) at `time`: a slow pendulum
/// stirred by gusts.
fn swing(w: &World, time: f32) -> (f32, f32) {
    let phase = roll(w, 1, 0x7A1) * TAU;
    let gust = 0.6 + 0.4 * (time * 0.11 + phase).sin();
    let pitch = (time * TAU / 9.0 + phase).sin() * 0.026 * gust + (time * TAU / 3.7).sin() * 0.006;
    let sway = (time * TAU / 6.3 + phase * 0.5).sin() * 0.018 * gust;
    (pitch, sway)
}

pub fn atmosphere(m: &mut Mesh, _alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let d = deck_y(w);
    let (pitch, sway) = swing(w, time);
    // The tender, rocking on its springs as the engine below drags at it.
    let tender_at = Mat4::from_translation(vec3(0.0, d, BREAK.0 - 8.9))
        * Mat4::from_translation(vec3(0.0, 0.0, 1.4))
        * Mat4::from_rotation_x(pitch * 0.12 + 0.02)
        * Mat4::from_rotation_z(sway * 0.35)
        * Mat4::from_translation(vec3(0.0, 0.0, -1.4));
    tender(m, &tender_at);
    // The engine: hinged at its rear coupler on the lip, hanging nose-down.
    let pivot = vec3(0.0, d + 1.0, BREAK.0 - 0.2);
    let hang = Mat4::from_translation(pivot)
        * Mat4::from_rotation_y(sway * 0.6)
        * Mat4::from_rotation_x(1.02 + pitch)
        * Mat4::from_rotation_z(sway)
        * Mat4::from_translation(vec3(0.0, -1.0, 0.0));
    locomotive(m, &hang);
    // Drawbar and safety chains between them, pulled taut.
    let hook = tender_at.transform_point3(vec3(0.0, 1.0, 7.6));
    let eye = hang.transform_point3(vec3(0.0, 1.0, 0.0));
    bar(m, hook, eye, 0.2, IRON);
    for x in [-0.5f32, 0.5] {
        beam(m, tender_at.transform_point3(vec3(x, 0.95, 7.5)), hang.transform_point3(vec3(x * 0.8, 0.9, 0.3)), 0.07, 0.07, RAIL);
    }
    // The signal lamp dangles by its cable under the lip and still blinks.
    let anchor = vec3(4.0, d - 0.6, BREAK.0 - 0.3);
    let a = (time * TAU / 4.9).sin() * 0.16 + (time * TAU / 1.7 + 0.8).sin() * 0.05;
    let b = (time * TAU / 6.1).cos() * 0.09;
    let lamp = anchor + vec3(b.sin() * 6.5, -6.5 * a.cos() * b.cos(), a.sin() * 6.5);
    beam(m, anchor, lamp + Vec3::Y * 0.45, 0.05, 0.05, IRON);
    let on = (time / 1.6).fract() < 0.45;
    let lg = Mat4::from_translation(lamp) * Mat4::from_rotation_x(a) * Mat4::from_rotation_z(b);
    block(m, &lg, vec3(-0.32, -0.45, -0.3), vec3(0.32, 0.45, 0.3), IRON);
    block(m, &lg, vec3(-0.18, -0.1, 0.3), vec3(0.18, 0.26, 0.36), if on { [1.0, 0.12, 0.05, 1.0] } else { [0.25, 0.05, 0.04, 0.0] });
    if on {
        soft(add, lg.transform_point3(vec3(0.0, 0.08, 0.5)), 1.3, 1.3, [1.0, 0.16, 0.06, 0.75]);
        soft(add, lg.transform_point3(vec3(0.0, 0.08, 0.5)), 4.0, 4.0, [1.0, 0.12, 0.05, 0.18]);
    }
}

/// The carriage creaks as the engine reaches the end of each slow swing,
/// on most swings.
pub fn cues(w: &World, from: f32, to: f32, out: &mut Vec<(f32, Cue, Vec3)>) {
    let phase = roll(w, 1, 0x7A1) * TAU;
    // sin(time * TAU / 9 + phase) peaks when time = (k + 1/4) * 9 - phase * 9 / TAU.
    let offset = (0.25 - phase / TAU).rem_euclid(1.0) * 9.0;
    let at = vec3(0.0, deck_y(w), BREAK.0 - 3.0);
    for t in ticks(from, to, 9.0, offset, |k| roll(w, k, 0xC4EE) < 0.6) {
        out.push((t, Cue::Creak, at));
    }
}
