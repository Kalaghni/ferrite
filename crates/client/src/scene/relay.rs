//! Relay, the art pass.
//!
//! Highland Relay 5 is a listening station, and it is evening after rain.
//! Two exchange towers and two array towers can be climbed; from their decks
//! the far ranges come up over the ridge. A great dish in each half is still
//! listening. Nobody is here, but somebody was a moment ago: the west watch
//! post is tidy, the east one was left in a hurry, and the west tower's
//! beacon is spelling HELLO in Morse. The east tower answers.
//!
//! Tower geometry is read from the shared layout, so what is drawn is what
//! collides. Everything else is small, overhead or painted on.
use super::slag_cathedral::art::{billboard, cross, sky_dir, sky_disc, sky_glow, soft, stencil, wall};
use super::slag_cathedral::{beam, girder, pipe};
use super::{hash01, mix3, smooth};
use crate::gfx::{shade, Col, Light, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::{relay as layout, shift, vnoise, Block, World, HALF_X, HALF_Z, SPREAD};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const ID: u8 = layout::MAP_ID;
pub const FOG: [f32; 4] = [0.86, 0.70, 0.54, 0.0040];
pub const SUN: Vec3 = Vec3::new(0.60, 0.33, 0.42);

const CONCRETE: Col = [0.56, 0.54, 0.50, 0.0];
const SHAFT: Col = [0.66, 0.67, 0.66, 0.0];
const STEEL: Col = [0.30, 0.33, 0.37, 0.0];
const GRATE: Col = [0.20, 0.22, 0.25, 0.0];
const SIGNAL_RED: Col = [0.74, 0.17, 0.12, 0.0];
const SAFETY: Col = [0.90, 0.66, 0.14, 0.0];
const WHITE: Col = [0.84, 0.85, 0.84, 0.0];
const INK: Col = [0.07, 0.08, 0.09, 0.0];
const LAMP: Col = [1.0, 0.82, 0.55, 0.92];
const COOL: Col = [0.74, 0.90, 0.96, 0.9];
const BEACON: Col = [1.0, 0.14, 0.07, 0.9];
const WIND: Vec3 = Vec3::new(0.94, 0.0, 0.34);

/// One tower as it stands in the world.
struct Site {
    x: f32,
    z: f32,
    rot: u8,
    side: f32,
    base: f32,
    flights: usize,
    /// 0 west array, 1 east array, 2 west exchange, 3 east exchange.
    index: usize,
}

impl Site {
    /// The tower's own frame: x and z as in the shared layout, y up from the pad.
    fn frame(&self) -> Mat4 {
        let x = layout::place(0.0, 0.0, self.rot, self.side, 1.0, 0.0);
        let z = layout::place(0.0, 0.0, self.rot, self.side, 0.0, 1.0);
        Mat4::from_cols(
            vec3(x.0, 0.0, x.1).extend(0.0),
            Vec3::Y.extend(0.0),
            vec3(z.0, 0.0, z.1).extend(0.0),
            vec3(self.x, self.base, self.z).extend(1.0),
        )
    }
    fn at(&self, dx: f32, y: f32, dz: f32) -> Vec3 {
        self.frame().transform_point3(vec3(dx, y, dz))
    }
    fn deck(&self) -> f32 {
        layout::deck_height(self.flights)
    }
    fn name(&self) -> &'static str {
        ["WA-1", "EA-4", "WX-2", "EX-3"][self.index]
    }
    fn exchange(&self) -> bool {
        self.index >= 2
    }
}

fn sites(w: &World) -> Vec<Site> {
    let mut out = Vec::new();
    for (t, (x, z, flights, rot)) in layout::TOWERS.into_iter().enumerate() {
        for (k, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let (sx, sz) = (x * side, z * side);
            out.push(Site { x: sx, z: sz, rot, side, base: layout::pad_top(w, sx, sz), flights, index: t * 2 + k });
        }
    }
    out
}

/// Dish pedestals: (x, z, ground height, 1 for west / -1 for east).
fn dishes(w: &World) -> [(f32, f32, f32, f32); 2] {
    let (x, z) = layout::DISH;
    [(x, z, w.height(x, z), 1.0), (-x, -z, w.height(-x, -z), -1.0)]
}

/// Where something first authored at `(x, z)`, in the cluster round
/// objective `k`, stands now: the cluster moved out with its objective when
/// the map was spread, keeping its shape (as `build_relay` places it).
fn beside(w: &World, k: usize, x: f32, z: f32) -> (f32, f32) {
    let p = w.points[k];
    let (ox, oz) = shift(p.x / SPREAD, p.z / SPREAD);
    (x + ox, z + oz)
}

/// The west generator shed at A.
fn generator(w: &World) -> (f32, f32) {
    beside(w, 0, -82.5, 18.5)
}

/// The west communications cabin (a container) at B.
fn comms(w: &World) -> (f32, f32) {
    beside(w, 1, -43.5, -31.5)
}

/// The west switchgear cabinet beside the exchange at C, which did not move.
const SWITCHGEAR: (f32, f32) = (-8.5, -7.0);
/// Roof heights of the shed and the cabinet, as `build_relay` builds them,
/// and the height of the power line on the pole each carries.
const SHED_ROOF: f32 = 1.45;
const SHED_LINE: f32 = 4.4;
const CABINET_ROOF: f32 = 1.4;
const CABINET_LINE: f32 = 2.35;

/// A cable gantry's two posts stand this far either side of its centre, as
/// `build_relay` raises them; `layout::GANTRIES` runs from the exchange out
/// toward A.
const GANTRY_HALF: f32 = 3.8;

/// Surface of revolution about the +Y axis of `g`.
fn lathe_m(m: &mut Mesh, g: &Mat4, profile: &[(f32, f32)], segs: usize, col: impl Fn(usize, usize) -> Col) {
    for (i, pair) in profile.windows(2).enumerate() {
        let ((r0, y0), (r1, y1)) = (pair[0], pair[1]);
        let (dr, dy) = (r1 - r0, y1 - y0);
        let len = (dr * dr + dy * dy).sqrt().max(1e-5);
        let (nr, ny) = (dy / len, -dr / len);
        // `k` stays the caller's facet (colour patterns key on it); each is
        // swept in `ROUND` steps.
        for k in 0..segs * crate::gfx::ROUND {
            let fine = (segs * crate::gfx::ROUND) as f32;
            let a0 = k as f32 / fine * TAU;
            let a1 = (k + 1) as f32 / fine * TAU;
            let k = k / crate::gfx::ROUND;
            let (d0, d1) = (vec3(a0.cos(), 0.0, a0.sin()), vec3(a1.cos(), 0.0, a1.sin()));
            let p = |d: Vec3, r: f32, y: f32| g.transform_point3(d * r + Vec3::Y * y);
            let n = |d: Vec3| g.transform_vector3(d * nr + Vec3::Y * ny).normalize_or_zero();
            m.quad_n([p(d1, r0, y0), p(d0, r0, y0), p(d0, r1, y1), p(d1, r1, y1)], [n(d1), n(d0), n(d0), n(d1)], col(i, k));
        }
    }
}

/// A frame at `at` whose +Y points along `axis`.
fn aim(at: Vec3, axis: Vec3) -> Mat4 {
    let y = axis.normalize_or_zero();
    let x = if y.y.abs() > 0.99 { Vec3::X } else { Vec3::Y.cross(y).normalize_or_zero() };
    let z = x.cross(y);
    Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), at.extend(1.0))
}

/// A cable hanging between two points, with optional marker balls.
fn wire(m: &mut Mesh, a: Vec3, b: Vec3, sag: f32, markers: usize, thick: f32) {
    let at = |t: f32| a.lerp(b, t) - Vec3::Y * (sag * 4.0 * t * (1.0 - t));
    let n = ((a - b).length() / 2.2).ceil().max(4.0) as usize;
    for k in 0..n {
        beam(m, at(k as f32 / n as f32), at((k + 1) as f32 / n as f32), thick, thick, INK);
    }
    for k in 0..markers {
        let p = at((k as f32 + 1.0) / (markers as f32 + 1.0));
        m.cuboid(p - Vec3::splat(0.16), p + Vec3::splat(0.16), if k % 2 == 0 { SIGNAL_RED } else { WHITE });
    }
}

/// The line of flags from a west or east exchange tower's deck rail down to
/// the nearest cable gantry: (top, bottom, sag).
fn flag_line(w: &World, s: &Site) -> (Vec3, Vec3, f32) {
    let top = s.at(layout::RING_HALF, s.deck() + 1.0, -layout::RING_HALF);
    let (x, z) = layout::GANTRIES[0];
    let (gx, gz) = (x * s.side, (z + GANTRY_HALF) * s.side);
    (top, vec3(gx, w.height(x * s.side, z * s.side) + 6.05, gz), 1.1 * SPREAD)
}

// ------------------------------------------------------------------ ground

fn segment_distance(px: f32, pz: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let t = (((px - a.0) * dx + (pz - a.1) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
    ((px - a.0 - dx * t).powi(2) + (pz - a.1 - dz * t).powi(2)).sqrt()
}

/// Gravel service tracks between the buildings, swept pads under the towers
/// and dishes, and heather across the open ground.
pub fn ground_tint(w: &World, mut col: [f32; 3], c: Vec3, n: Vec3) -> [f32; 3] {
    // Base to A, A past the dish to the exchange, A to B, B to its array
    // tower and on to C, and the exchange tower to C.
    let xz = |p: Vec3| (p.x, p.z);
    let (base, a, b, c0) = (xz(w.bases[0]), xz(w.points[0]), xz(w.points[1]), xz(w.points[2]));
    let (array, exchange) = ((layout::TOWERS[0].0, layout::TOWERS[0].1), (layout::TOWERS[1].0, layout::TOWERS[1].1));
    let tracks = [(base, a), (a, layout::DISH), (layout::DISH, exchange), (a, b), (b, array), (b, c0), (exchange, c0)];
    let mut track = f32::MAX;
    for (a, b) in tracks {
        track = track.min(segment_distance(c.x, c.z, a, b)).min(segment_distance(-c.x, -c.z, a, b));
    }
    let wander = (vnoise(c.x / 7.0, c.z / 7.0, 91) - 0.5) * 1.2;
    let on_track = 1.0 - smooth(1.4, 2.8, track + wander);
    // Heather, in drifts, off the tracks and off the steep ground.
    let heather = smooth(0.52, 0.74, vnoise(c.x / 11.0 + 40.0, c.z / 11.0, w.seed as u32 ^ 0x4EA7));
    let flatness = smooth(0.9, 0.98, n.y);
    col = mix3(col, [0.47, 0.31, 0.45], heather * flatness * (1.0 - on_track) * 0.55);
    col = mix3(col, [0.58, 0.52, 0.44], on_track * 0.8);
    let mut pad = f32::MAX;
    for (x, z, _, _) in layout::TOWERS {
        for s in [1.0f32, -1.0] {
            pad = pad.min(((c.x - x * s).powi(2) + (c.z - z * s).powi(2)).sqrt() - 1.5);
        }
    }
    let (dx, dz) = layout::DISH;
    for s in [1.0f32, -1.0] {
        pad = pad.min(((c.x - dx * s).powi(2) + (c.z - dz * s).powi(2)).sqrt());
    }
    mix3(col, [0.60, 0.55, 0.48], (1.0 - smooth(3.2, 6.2, pad)) * 0.85)
}

/// Collision this module draws itself: the towers and the dish pedestals.
pub fn owns(b: &Block) -> bool {
    let c = (b.min + b.max) * 0.5;
    let near = |x: f32, z: f32, r: f32| (c.x - x).abs() < r && (c.z - z).abs() < r;
    let (dx, dz) = layout::DISH;
    layout::TOWERS.iter().any(|&(x, z, _, _)| near(x, z, layout::APRON_HALF + 0.1) || near(-x, -z, layout::APRON_HALF + 0.1))
        || ((near(dx, dz, 0.2) || near(-dx, -dz, 0.2)) && (b.max.x - b.min.x - 2.0 * layout::DISH_PEDESTAL_HALF).abs() < 0.05)
}

// ------------------------------------------------------------------ static

pub fn build(m: &mut Mesh, w: &World) {
    let all = sites(w);
    for s in &all {
        tower(m, s, &all);
        deck_life(m, s);
    }
    for (x, z, g, side) in dishes(w) {
        dish(m, vec3(x, g, z), side);
    }
    cables(m, w, &all);
    station(m, w);
    flowers(m, w);
}

/// Cotton grass, tormentil and thistle through the turf, bright in low sun.
fn flowers(m: &mut Mesh, w: &World) {
    const HEADS: [Col; 3] = [[0.96, 0.95, 0.9, 0.3], [0.97, 0.8, 0.2, 0.3], [0.8, 0.42, 0.66, 0.3]];
    // As thick on the ground as before the map was spread out.
    let count = (1500.0 * SPREAD * SPREAD) as u32;
    for k in 0..count {
        let x = (hash01(k, 901) * 2.0 - 1.0) * (HALF_X - 6.0);
        let z = (hash01(k, 902) * 2.0 - 1.0) * (HALF_Z - 4.0);
        let n = w.terrain_normal(x, z);
        if n.y < 0.95 || w.blocks.iter().any(|b| x > b.min.x - 0.3 && x < b.max.x + 0.3 && z > b.min.z - 0.3 && z < b.max.z + 0.3) {
            continue;
        }
        // Flowers come in drifts rather than evenly.
        if vnoise(x / 6.0, z / 6.0, 77) < 0.45 {
            continue;
        }
        let kind = (hash01(k, 903) * 3.0) as usize % 3;
        let base = vec3(x, w.height(x, z), z);
        for j in 0..3u32 {
            let off = vec3(hash01(k, 910 + j) - 0.5, 0.0, hash01(k, 920 + j) - 0.5) * 0.5;
            let hgt = 0.16 + hash01(k, 930 + j) * 0.16;
            let head = base + off + Vec3::Y * hgt;
            m.tri_n(base + off - Vec3::X * 0.012, base + off + Vec3::X * 0.012, head, n, [0.3, 0.42, 0.24, 0.0]);
            cross(m, head, 0.034, 0.034, HEADS[kind]);
        }
    }
}

/// A rail on posts between two points of a tower's frame.
fn rail(m: &mut Mesh, f: &Mat4, a: Vec3, b: Vec3, heights: &[f32]) {
    let p = |v: Vec3| f.transform_point3(v);
    for (k, &h) in heights.iter().enumerate() {
        let col = if k + 1 == heights.len() { SAFETY } else { STEEL };
        beam(m, p(a + Vec3::Y * h), p(b + Vec3::Y * h), 0.07, 0.07, col);
    }
    let top = heights[heights.len() - 1];
    let n = ((a - b).length() / 1.15).ceil().max(1.0) as usize;
    for k in 0..=n {
        let q = a.lerp(b, k as f32 / n as f32);
        beam(m, p(q), p(q + Vec3::Y * top), 0.045, 0.045, STEEL);
    }
}

fn tower(m: &mut Mesh, s: &Site, all: &[Site]) {
    let f = s.frame();
    let (c, r) = (layout::CORE_HALF, layout::RING_HALF);
    let deck = s.deck();
    let slab = layout::SLAB;
    let p = |x: f32, y: f32, z: f32| f.transform_point3(vec3(x, y, z));

    // Apron and pad: poured concrete with a painted safety margin.
    let (ah, ph, drop) = (layout::APRON_HALF, layout::PAD_HALF, layout::APRON_DROP);
    m.obox(&f, vec3(0.0, -drop - 1.2, 0.0), vec3(ah, 1.2, ah), shade(CONCRETE, 0.82));
    m.obox(&f, vec3(0.0, -drop * 0.5, 0.0), vec3(ph, drop * 0.5, ph), CONCRETE);
    for (dx, dz, hx, hz) in
        [(0.0, ph - 0.12, ph, 0.07), (0.0, -ph + 0.12, ph, 0.07), (ph - 0.12, 0.0, 0.07, ph), (-ph + 0.12, 0.0, 0.07, ph)]
    {
        m.obox(&f, vec3(dx, 0.004, dz), vec3(hx, 0.004, hz), SAFETY);
    }
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        m.obox(&f, vec3(sx * (ah - 0.3), -drop + 0.03, sz * (ah - 0.3)), vec3(0.09, 0.03, 0.09), STEEL);
    }

    // Service core: pale cladding in courses, a red band under the deck.
    let top = deck - slab;
    m.obox(&f, vec3(0.0, top * 0.5, 0.0), vec3(c, top * 0.5, c), SHAFT);
    m.obox(&f, vec3(0.0, top - 0.75, 0.0), vec3(c + 0.012, 0.75, c + 0.012), SIGNAL_RED);
    m.obox(&f, vec3(0.0, 0.3, 0.0), vec3(c + 0.03, 0.3, c + 0.03), shade(STEEL, 0.9));
    let mut y = 1.5;
    while y < top - 1.5 {
        m.obox(&f, vec3(0.0, y, 0.0), vec3(c + 0.016, 0.025, c + 0.016), shade(STEEL, 1.1));
        y += 1.5;
    }
    // The face under the highest flights carries the door, the name, a lit
    // slot window at every storey and the cable ladder.
    let door_side = (layout::start_corner(s.flights) + 2) % 4;
    let (nx, nz, yaw) = [(0.0f32, -1.0f32, PI), (1.0, 0.0, FRAC_PI_2), (0.0, 1.0, 0.0), (-1.0, 0.0, -FRAC_PI_2)][door_side];
    let face = f * wall(vec3(nx * (c + 0.014), 0.0, nz * (c + 0.014)), yaw);
    m.obox(&face, vec3(-0.35, 1.05, 0.0), vec3(0.48, 1.05, 0.02), shade(STEEL, 0.75));
    m.obox(&face, vec3(-0.35, 1.05, 0.02), vec3(0.4, 0.95, 0.012), shade(STEEL, 1.05));
    m.obox(&face, vec3(-0.05, 1.05, 0.04), vec3(0.03, 0.09, 0.02), SAFETY);
    m.obox(&face, vec3(-0.35, 2.25, 0.03), vec3(0.3, 0.06, 0.04), COOL);
    stencil(m, &(face * Mat4::from_translation(vec3(0.0, 2.55, 0.02))), "RELAY 5", 0.05, INK);
    stencil(m, &(face * Mat4::from_translation(vec3(0.0, 3.1, 0.02))), s.name(), 0.085, SIGNAL_RED);
    for (dx, sx) in [(0.72f32, 0.06f32), (1.02, 0.06)] {
        m.obox(&face, vec3(dx, top * 0.5 + 0.3, 0.05), vec3(sx * 0.5, top * 0.5 - 0.3, 0.02), STEEL);
    }
    let mut ry = 0.8;
    while ry < top {
        m.obox(&face, vec3(0.87, ry, 0.05), vec3(0.15, 0.02, 0.02), STEEL);
        ry += 0.45;
    }
    m.obox(&face, vec3(0.87, top * 0.5 + 0.3, 0.035), vec3(0.06, top * 0.5 - 0.3, 0.012), INK);
    for side in 0..4 {
        let (sx, sz, syaw) = [(0.0f32, -1.0f32, PI), (1.0, 0.0, FRAC_PI_2), (0.0, 1.0, 0.0), (-1.0, 0.0, -FRAC_PI_2)][side];
        let g = f * wall(vec3(sx * (c + 0.014), 0.0, sz * (c + 0.014)), syaw);
        let mut wy = 2.2 + 0.375 * side as f32;
        while wy < top - 1.7 {
            m.obox(&g, vec3(-0.45, wy, 0.0), vec3(0.3, 0.09, 0.012), LAMP);
            m.obox(&g, vec3(-0.45, wy + 0.13, 0.01), vec3(0.36, 0.03, 0.03), STEEL);
            wy += 3.0;
        }
    }

    // The stair: grated treads with a bright nosing, stringers both sides,
    // and a handrail on the open side.
    for j in 0..s.flights {
        let (a, _, foot) = layout::flight(s.flights, j);
        let (ca, cb) = (layout::CORNERS[a], layout::CORNERS[(a + 1) % 4]);
        let d = vec3((cb.0 - ca.0) / 3.7, 0.0, (cb.1 - ca.1) / 3.7);
        let out = vec3(d.z, 0.0, -d.x);
        let mid = vec3((ca.0 + cb.0) * 0.5, 0.0, (ca.1 + cb.1) * 0.5);
        if j > 0 {
            let l = vec3(ca.0, foot, ca.1);
            m.obox(&f, l - Vec3::Y * (slab * 0.5), vec3(0.55, slab * 0.5, 0.55), GRATE);
            let (ox, oz) = (ca.0.signum() * (r - layout::RAIL_HALF), ca.1.signum() * (r - layout::RAIL_HALF));
            rail(m, &f, vec3(ox, foot, ca.1 - 0.55), vec3(ox, foot, ca.1 + 0.55), &[1.0]);
            rail(m, &f, vec3(ca.0 - 0.55, foot, oz), vec3(ca.0 + 0.55, foot, oz), &[1.0]);
            // A knee brace back to the core under every landing.
            let inner = vec3(ca.0.signum() * c, foot - 1.1, ca.1.signum() * c);
            beam(m, f.transform_point3(l - Vec3::Y * slab), f.transform_point3(inner), 0.08, 0.1, STEEL);
        }
        for i in 0..layout::STEPS_PER_FLIGHT {
            let ((mx, mz), _, t) = layout::step(a, foot, i);
            let centre = vec3(mx, t - slab * 0.5, mz);
            let half =
                vec3(d.x.abs() * layout::TREAD * 0.5 + d.z.abs() * 0.55, slab * 0.5, d.z.abs() * layout::TREAD * 0.5 + d.x.abs() * 0.55);
            m.obox(&f, centre, half, shade(GRATE, 1.0 + 0.12 * (i % 2) as f32));
            let nose = vec3(mx, t + 0.004, mz) - d * (layout::TREAD * 0.5 - 0.04);
            m.obox(&f, nose, vec3(d.x.abs() * 0.04 + d.z.abs() * 0.55, 0.004, d.z.abs() * 0.04 + d.x.abs() * 0.55), SAFETY);
            let post = vec3(mx, t, mz) + out * (0.55 - layout::RAIL_HALF);
            beam(m, f.transform_point3(post), f.transform_point3(post + Vec3::Y), 0.04, 0.04, STEEL);
        }
        let run = layout::CORE_HALF;
        let (lo, hi) = (foot + layout::RISE * 0.5, foot + layout::RISE * (layout::STEPS_PER_FLIGHT as f32 + 0.5));
        for e in [-1.0f32, 1.0] {
            let off = out * (e * 0.52);
            beam(
                m,
                f.transform_point3(mid - d * run + off + Vec3::Y * (lo - 0.2)),
                f.transform_point3(mid + d * run + off + Vec3::Y * (hi - 0.2)),
                0.06,
                0.24,
                STEEL,
            );
        }
        let hand = out * (0.55 - layout::RAIL_HALF);
        beam(
            m,
            f.transform_point3(mid - d * run + hand + Vec3::Y * (lo + 1.15)),
            f.transform_point3(mid + d * run + hand + Vec3::Y * (hi + 0.85)),
            0.07,
            0.07,
            SAFETY,
        );
    }

    // The deck, its rails and the two plate parapets.
    for (a, b) in [((-c, -r), (r, c)), ((-r, -r), (-c, -c)), ((c, c), (r, r))] {
        let centre = vec3((a.0 + b.0) * 0.5, deck - slab * 0.5, (a.1 + b.1) * 0.5);
        let half = vec3((b.0 - a.0) * 0.5, slab * 0.5, (b.1 - a.1) * 0.5);
        m.obox(&f, centre, half, GRATE);
        m.obox(&f, centre + Vec3::Y * (slab * 0.5 + 0.003), vec3(half.x - 0.08, 0.003, half.z - 0.08), shade(GRATE, 1.35));
    }
    let e = r - layout::RAIL_HALF;
    for (a, b) in [
        ((-r, -e), (r, -e)),
        ((e, -r), (e, r)),
        ((c, e), (r, e)),
        ((c + layout::RAIL_HALF, c), (c + layout::RAIL_HALF, r)),
        ((-c, c - layout::RAIL_HALF), (c, c - layout::RAIL_HALF)),
        ((-c + layout::RAIL_HALF, -c), (-c + layout::RAIL_HALF, c)),
        ((-e, -r), (-e, -c)),
    ] {
        rail(m, &f, vec3(a.0, deck, a.1), vec3(b.0, deck, b.1), &layout::RAIL_HEIGHTS);
    }
    let (ph2, pt) = (layout::PARAPET_HALF_LEN, layout::PARAPET_T);
    for (centre, half, yaw, at) in [
        (
            vec3(0.0, deck + layout::PARAPET_H * 0.5, -r + pt * 0.5),
            vec3(ph2, layout::PARAPET_H * 0.5, pt * 0.5),
            PI,
            vec3(0.0, deck + 0.3, -r - 0.004),
        ),
        (
            vec3(r - pt * 0.5, deck + layout::PARAPET_H * 0.5, 0.0),
            vec3(pt * 0.5, layout::PARAPET_H * 0.5, ph2),
            FRAC_PI_2,
            vec3(r + 0.004, deck + 0.3, 0.0),
        ),
    ] {
        m.obox(&f, centre, half, shade(STEEL, 1.15));
        m.obox(&f, centre + Vec3::Y * (layout::PARAPET_H * 0.5), half * vec3(1.0, 0.0, 1.0) + vec3(0.03, 0.025, 0.03), SAFETY);
        stencil(m, &(f * wall(at, yaw)), s.name(), 0.06, WHITE);
    }

    // The mast: banded red and white, step pegs, a crown of panel antennas.
    let mast = layout::MAST_ABOVE;
    for k in 0..5 {
        let (y0, y1) = (deck + mast * k as f32 / 5.0, deck + mast * (k + 1) as f32 / 5.0);
        let g = f * Mat4::from_translation(vec3(0.0, (y0 + y1) * 0.5, 0.0)) * Mat4::from_rotation_x(FRAC_PI_2);
        m.tube(&g, Vec3::ZERO, layout::MAST_HALF - 0.02, 0.0, (y1 - y0) * 0.5, 10, if k % 2 == 0 { SIGNAL_RED } else { WHITE });
    }
    for k in 0..12 {
        let y = deck + 2.4 + k as f32 * 0.32;
        let sx = if k % 2 == 0 { 1.0 } else { -1.0 };
        beam(m, p(0.0, y, 0.0), p(sx * 0.48, y, 0.0), 0.03, 0.03, STEEL);
    }
    let crown = deck + mast - 1.05;
    for k in 0..3 {
        let a = k as f32 * TAU / 3.0 + 0.5;
        let (dx, dz) = (a.cos(), a.sin());
        beam(m, p(0.0, crown + 0.35, 0.0), p(dx * 0.62, crown + 0.35, dz * 0.62), 0.06, 0.06, STEEL);
        beam(m, p(0.0, crown - 0.35, 0.0), p(dx * 0.62, crown - 0.35, dz * 0.62), 0.06, 0.06, STEEL);
        let g = f * Mat4::from_translation(vec3(dx * 0.68, crown, dz * 0.68)) * Mat4::from_rotation_y(FRAC_PI_2 - a);
        for e in [-0.19f32, 0.19] {
            m.obox(&g, vec3(e, 0.0, 0.0), vec3(0.14, 0.52, 0.05), WHITE);
            m.obox(&g, vec3(e, -0.45, 0.055), vec3(0.1, 0.04, 0.01), SAFETY);
        }
    }
    // Microwave drums look at the other towers: line of sight made visible.
    let here = p(0.0, deck + 3.1, 0.0);
    let mut drum = 0;
    for other in all {
        let there = vec3(other.x, here.y, other.z);
        let gap = (there - here).length();
        if gap < 1.0 || gap > 90.0 || drum >= 2 {
            continue;
        }
        let dir = (there - here) / gap;
        let hub = here + Vec3::Y * (drum as f32 * 1.25 - 0.3);
        let g = aim(hub + dir * 0.3, dir);
        lathe_m(m, &g, &[(0.0, 0.0), (0.42, 0.1), (0.5, 0.26), (0.5, 0.36)], 12, |ring, _| shade(WHITE, 0.82 + 0.08 * ring as f32));
        for k in 0..12 {
            let (a0, a1) = (k as f32 / 12.0 * TAU, (k + 1) as f32 / 12.0 * TAU);
            let q = |a: f32| g.transform_point3(vec3(a.cos() * 0.5, 0.36, a.sin() * 0.5));
            m.tri_n(g.transform_point3(vec3(0.0, 0.39, 0.0)), q(a0), q(a1), dir, shade(WHITE, 1.06));
        }
        beam(m, hub, hub + dir * 0.3, 0.1, 0.1, STEEL);
        drum += 1;
    }
    // Deck floodlight on a mast bracket, and the beacon housing on top.
    beam(m, p(0.0, deck + 2.3, 0.0), p(0.75, deck + 2.3, -0.75), 0.05, 0.05, STEEL);
    m.obox(&f, vec3(0.8, deck + 2.24, -0.8), vec3(0.2, 0.05, 0.2), COOL);
    m.obox(&f, vec3(0.8, deck + 2.31, -0.8), vec3(0.24, 0.025, 0.24), INK);
    if s.exchange() {
        m.obox(&f, vec3(0.0, deck + mast + 0.1, 0.0), vec3(0.2, 0.1, 0.2), STEEL);
        for (dx, dz) in [(0.62f32, 0.62f32), (-0.62, -0.62)] {
            m.obox(&f, vec3(dx, deck + mast - 0.42, dz), vec3(0.07, 0.07, 0.07), BEACON);
        }
    } else {
        beam(m, p(0.0, deck + mast, 0.0), p(0.0, deck + mast + 1.3, 0.0), 0.035, 0.035, STEEL);
        m.obox(&f, vec3(0.0, deck + mast + 0.1, 0.0), vec3(0.1, 0.1, 0.1), BEACON);
        // Weather arm: the anemometer and windsock turn on it at runtime.
        beam(m, p(0.0, deck + 4.2, 0.0), p(-0.95, deck + 4.2, 0.95), 0.05, 0.05, STEEL);
        beam(m, p(-0.95, deck + 4.2, 0.95), p(-0.95, deck + 4.55, 0.95), 0.035, 0.035, STEEL);
        beam(m, p(r - 0.1, deck + 1.0, r - 0.1), p(r - 0.1, deck + 2.9, r - 0.1), 0.045, 0.045, WHITE);
    }
}

/// What was left on the decks. West: a tidy watch. East: left in a hurry.
fn deck_life(m: &mut Mesh, s: &Site) {
    let f = s.frame();
    let deck = s.deck();
    let (r, ph) = (layout::RING_HALF, layout::PARAPET_H);
    let p = |x: f32, y: f32, z: f32| f.transform_point3(vec3(x, y, z));
    let west = s.side > 0.0;
    let canvas: Col = [0.25, 0.36, 0.30, 0.0];
    if s.exchange() {
        // A folding stool in the corner where the two parapets nearly meet.
        let c = vec3(1.85, deck, -1.85);
        if west {
            m.obox(&f, c + vec3(0.0, 0.42, 0.0), vec3(0.2, 0.015, 0.17), canvas);
            for (a, b) in [((-0.2, -0.15), (0.2, 0.15)), ((0.2, -0.15), (-0.2, 0.15))] {
                beam(m, p(c.x + a.0, deck, c.z + a.1), p(c.x + b.0, deck + 0.42, c.z + b.1), 0.025, 0.025, STEEL);
            }
        } else {
            // Kicked over.
            m.obox(
                &(f * Mat4::from_translation(c + vec3(-0.3, 0.17, 0.25)) * Mat4::from_rotation_z(1.45)),
                Vec3::ZERO,
                vec3(0.2, 0.015, 0.17),
                canvas,
            );
            beam(m, p(c.x - 0.5, deck + 0.02, c.z + 0.1), p(c.x - 0.05, deck + 0.02, c.z + 0.42), 0.025, 0.025, STEEL);
            beam(m, p(c.x - 0.5, deck + 0.02, c.z + 0.42), p(c.x - 0.05, deck + 0.02, c.z + 0.1), 0.025, 0.025, STEEL);
        }
        // On the parapet: field glasses and a flask. In the east the mug
        // went over the edge of the ledge and the logbook lies open.
        let top = deck + ph + 0.026;
        let g = f * Mat4::from_translation(vec3(-0.45, top + 0.045, -r + 0.07));
        for e in [-0.045f32, 0.045] {
            m.tube(&g, vec3(e, 0.0, 0.0), 0.038, 0.0, 0.085, 8, INK);
            m.tube(&g, vec3(e, 0.0, -0.09), 0.03, 0.0, 0.012, 8, COOL);
        }
        m.obox(&g, Vec3::ZERO, vec3(0.03, 0.02, 0.03), STEEL);
        let flask = f * Mat4::from_translation(vec3(0.5, top + 0.12, -r + 0.07)) * Mat4::from_rotation_x(FRAC_PI_2);
        if west {
            m.tube(&flask, Vec3::ZERO, 0.045, 0.0, 0.12, 8, [0.16, 0.34, 0.26, 0.0]);
            m.tube(&flask, vec3(0.0, 0.0, -0.135), 0.035, 0.0, 0.018, 8, WHITE);
        } else {
            let fallen = f * Mat4::from_translation(vec3(0.35, deck + 0.045, -r + 0.5)) * Mat4::from_rotation_y(0.7);
            m.tube(&fallen, Vec3::ZERO, 0.045, 0.0, 0.12, 8, [0.16, 0.34, 0.26, 0.0]);
            m.obox(&f, vec3(0.75, deck + 0.004, -r + 0.6), vec3(0.22, 0.004, 0.16), [0.2, 0.14, 0.1, 0.0]);
        }
        // The set: a field radio against the parapet, its lamp lit at runtime.
        m.obox(&f, vec3(-0.75, deck + 0.14, -r + 0.38), vec3(0.2, 0.14, 0.1), [0.24, 0.29, 0.22, 0.0]);
        m.obox(&f, vec3(-0.75, deck + 0.16, -r + 0.485), vec3(0.13, 0.05, 0.006), INK);
        beam(m, p(-0.6, deck + 0.28, -r + 0.36), p(-0.52, deck + 1.25, -r + 0.3), 0.012, 0.012, STEEL);
        // The log. West: closed, pencil squared on top. East: open, face up.
        if west {
            m.obox(&f, vec3(0.05, top + 0.02, -r + 0.07), vec3(0.11, 0.02, 0.055), [0.5, 0.16, 0.14, 0.0]);
            beam(m, p(-0.03, top + 0.047, -r + 0.07), p(0.13, top + 0.047, -r + 0.07), 0.012, 0.012, SAFETY);
        } else {
            for (e, tilt) in [(-0.1f32, 0.12f32), (0.1, -0.12)] {
                let page = f * Mat4::from_translation(vec3(1.2 + e, deck + 0.03, -1.2)) * Mat4::from_rotation_z(tilt);
                m.obox(&page, Vec3::ZERO, vec3(0.1, 0.006, 0.14), [0.86, 0.84, 0.76, 0.0]);
                for k in 0..5 {
                    m.obox(&page, vec3(0.0, 0.008, -0.1 + k as f32 * 0.045), vec3(0.075, 0.001, 0.004), INK);
                }
            }
        }
    } else {
        // Array towers: a spotting telescope on a tripod, aimed over the ridge.
        let c = vec3(1.9, deck, -1.9);
        let head = p(c.x, deck + 1.25, c.z);
        for k in 0..3 {
            let a = k as f32 * TAU / 3.0 + 0.4;
            beam(m, head, p(c.x + a.cos() * 0.32, deck, c.z + a.sin() * 0.32), 0.025, 0.025, INK);
        }
        let look = (f.transform_vector3(vec3(0.9, 0.42, -0.9))).normalize();
        let g = aim(head + look * 0.1, look);
        m.tube(&(g * Mat4::from_rotation_x(FRAC_PI_2)), Vec3::ZERO, 0.055, 0.0, 0.42, 10, WHITE);
        m.tube(&(g * Mat4::from_rotation_x(FRAC_PI_2)), vec3(0.0, 0.0, -0.44), 0.07, 0.05, 0.05, 10, INK);
        // An owl has taken the weather arm on the east tower.
        if !west {
            let o = p(-0.6, deck + 4.225, 0.6);
            let g = Mat4::from_translation(o);
            lathe_m(&mut *m, &g, &[(0.0, 0.0), (0.085, 0.03), (0.1, 0.14), (0.085, 0.24), (0.0, 0.29)], 8, |ring, seg| {
                if seg % 2 == 0 && ring < 3 {
                    [0.42, 0.33, 0.24, 0.0]
                } else {
                    [0.56, 0.47, 0.36, 0.0]
                }
            });
            for e in [-0.05f32, 0.05] {
                m.tri(o + vec3(e - 0.02, 0.27, 0.0), o + vec3(e, 0.34, 0.0), o + vec3(e + 0.02, 0.27, 0.0), [0.36, 0.28, 0.2, 0.0]);
                m.cuboid(o + vec3(e - 0.016, 0.2, 0.085), o + vec3(e + 0.016, 0.232, 0.1), SAFETY);
            }
        }
    }
}

/// The great dish: a concrete pedestal, a yoke, and a tilted reflector whose
/// rim never comes within five metres of the ground.
fn dish(m: &mut Mesh, foot: Vec3, side: f32) {
    let (h, ph) = (layout::DISH_PEDESTAL_H, layout::DISH_PEDESTAL_HALF);
    m.cuboid(foot + vec3(-ph, -0.6, -ph), foot + vec3(ph, h, ph), CONCRETE);
    m.cuboid(foot + vec3(-ph - 0.05, -0.6, -ph - 0.05), foot + vec3(ph + 0.05, 0.45, ph + 0.05), shade(CONCRETE, 0.8));
    m.cuboid(foot + vec3(-ph - 0.04, h - 0.3, -ph - 0.04), foot + vec3(ph + 0.04, h, ph + 0.04), shade(CONCRETE, 0.86));
    for y in [1.6f32, 2.8, 4.0] {
        m.cuboid(foot + vec3(-ph - 0.012, y, -ph - 0.012), foot + vec3(ph + 0.012, y + 0.04, ph + 0.012), shade(CONCRETE, 0.72));
    }
    // Door, lamp, nameplate and a caged ladder to the yoke.
    let face = wall(foot + vec3(0.0, 0.0, side * (ph + 0.012)), if side > 0.0 { 0.0 } else { PI });
    m.obox(&face, vec3(-0.5, 1.05, 0.0), vec3(0.45, 1.05, 0.02), shade(STEEL, 0.8));
    m.obox(&face, vec3(-0.5, 2.3, 0.03), vec3(0.3, 0.06, 0.04), LAMP);
    stencil(m, &(face * Mat4::from_translation(vec3(-0.55, 2.75, 0.01))), if side > 0.0 { "DISH 1" } else { "DISH 2" }, 0.06, INK);
    stencil(m, &(face * Mat4::from_translation(vec3(-0.55, 3.35, 0.01))), "LISTENING", 0.035, SIGNAL_RED);
    for dx in [0.6f32, 0.95] {
        m.obox(&face, vec3(dx, h * 0.5 + 0.2, 0.05), vec3(0.025, h * 0.5 - 0.2, 0.025), STEEL);
    }
    let mut y = 0.6;
    while y < h {
        m.obox(&face, vec3(0.775, y, 0.05), vec3(0.175, 0.018, 0.018), STEEL);
        y += 0.4;
    }
    // Turntable and yoke.
    let cap = foot + Vec3::Y * h;
    let g = Mat4::from_translation(cap + Vec3::Y * 0.2) * Mat4::from_rotation_x(FRAC_PI_2);
    m.tube(&g, Vec3::ZERO, 1.25, 0.0, 0.2, 16, STEEL);
    let hub = cap + Vec3::Y * 2.0;
    let azimuth = vec3(-side, 0.0, side * 0.35).normalize();
    let across = Vec3::Y.cross(azimuth).normalize();
    for e in [-1.0f32, 1.0] {
        girder(m, cap + across * (e * 0.9) + Vec3::Y * 0.4, hub + across * (e * 1.15), 0.3, 0.45, STEEL);
        m.cuboid(hub + across * (e * 1.15) - Vec3::splat(0.22), hub + across * (e * 1.15) + Vec3::splat(0.22), SAFETY);
    }
    pipe(m, hub - across * 1.15, hub + across * 1.15, 0.13, shade(STEEL, 0.8));
    // The reflector: a shallow paraboloid of white petals, ribbed behind.
    let tilt = 0.66f32;
    let axis = (azimuth * tilt.sin() + Vec3::Y * tilt.cos()).normalize();
    let g = aim(hub, axis);
    const RADIUS: f32 = 4.2;
    const FOCAL: f32 = 3.7;
    let profile: Vec<(f32, f32)> = (0..=6).map(|k| k as f32 / 6.0 * RADIUS).map(|r| (r, r * r / (4.0 * FOCAL))).collect();
    lathe_m(m, &g, &profile, 24, |ring, seg| {
        shade(WHITE, 0.86 + 0.07 * ((ring + seg) % 2) as f32 + 0.05 * hash01(seg as u32, ring as u32))
    });
    let rim = RADIUS * RADIUS / (4.0 * FOCAL);
    let q = |a: f32, r: f32, y: f32| g.transform_point3(vec3(a.cos() * r, y, a.sin() * r));
    for k in 0..24 {
        let (a0, a1) = (k as f32 / 24.0 * TAU, (k + 1) as f32 / 24.0 * TAU);
        beam(m, q(a0, RADIUS, rim), q(a1, RADIUS, rim), 0.09, 0.09, STEEL);
        if k % 2 == 0 {
            beam(m, q(a0, 0.5, -0.5), q(a0, RADIUS, rim - 0.06), 0.07, 0.12, shade(STEEL, 0.85));
            beam(m, q(a0, 2.3, rim * 0.3 - 0.25), q(a1 + (a1 - a0), 2.3, rim * 0.3 - 0.25), 0.05, 0.05, shade(STEEL, 0.85));
        }
    }
    m.tube(&(g * Mat4::from_rotation_x(FRAC_PI_2)), vec3(0.0, 0.0, 0.3), 0.55, 0.0, 0.3, 12, shade(STEEL, 0.9));
    // Feed legs to the focus, where the receiver sits.
    let focus = q(0.0, 0.0, FOCAL);
    for k in 0..3 {
        let a = k as f32 * TAU / 3.0 + 0.3;
        beam(m, q(a, RADIUS * 0.94, rim * 0.88), focus, 0.06, 0.06, STEEL);
    }
    m.obox(&g, vec3(0.0, FOCAL - 0.1, 0.0), vec3(0.2, 0.22, 0.2), shade(STEEL, 1.1));
    for k in 0..10 {
        let (a0, a1) = (k as f32 / 10.0 * TAU, (k + 1) as f32 / 10.0 * TAU);
        m.tri_n(q(0.0, 0.0, FOCAL - 0.34), q(a0, 0.36, FOCAL - 0.34), q(a1, 0.36, FOCAL - 0.34), -axis, WHITE);
    }
}

/// One overhead cable: ends, sag at mid-span, marker balls, thickness.
struct Span {
    a: Vec3,
    b: Vec3,
    sag: f32,
    markers: usize,
    thick: f32,
}

impl Span {
    fn new(a: Vec3, b: Vec3, sag: f32, markers: usize, thick: f32) -> Span {
        Span { a, b, sag, markers, thick }
    }
}

/// The top of a gantry beam over post `dz`, in the `side` half.
fn gantry_top(w: &World, (x, z): (f32, f32), dz: f32, side: f32) -> Vec3 {
    vec3(x * side, w.height(x * side, z * side) + 6.1, (z + dz) * side)
}

/// Overhead cable: tower to tower, tower to dish, the flag lines, and power
/// from the generator sheds along the gantries to the exchange.
fn spans(w: &World, all: &[Site]) -> Vec<Span> {
    let crown = |s: &Site| s.at(0.0, s.deck() + layout::MAST_ABOVE - 1.4, 0.0);
    let (wa, ea, wx, ex) = (&all[0], &all[1], &all[2], &all[3]);
    // The signal line between the two exchange towers passes over C.
    let mut out = vec![
        Span::new(crown(wx), crown(ex), 2.4, 5, 0.05),
        Span::new(crown(wx), crown(wa), 3.2, 3, 0.04),
        Span::new(crown(ex), crown(ea), 3.2, 3, 0.04),
    ];
    for (x, z, g, side) in dishes(w) {
        let near = if side > 0.0 { wx } else { ex };
        let pedestal = vec3(x, g + layout::DISH_PEDESTAL_H + 0.3, z);
        out.push(Span::new(crown(near), pedestal, 1.6, 0, 0.035));
    }
    for s in [wx, ex] {
        let (a, b, sag) = flag_line(w, s);
        out.push(Span::new(a, b, sag, 0, 0.02));
    }
    let [inner, outer] = layout::GANTRIES;
    let (gx, gz) = generator(w);
    let (sx, sz) = SWITCHGEAR;
    for side in [1.0f32, -1.0] {
        for dz in [-GANTRY_HALF, GANTRY_HALF] {
            let top = |gantry| gantry_top(w, gantry, dz, side);
            let shed = vec3(gx * side, w.height(gx * side, gz * side) + SHED_LINE, (gz + dz * 0.2) * side);
            let cabinet = vec3(sx * side, w.height(sx * side, sz * side) + CABINET_LINE, (sz + dz * 0.2) * side);
            // Drawn taut enough to clear the ruin wall round the shed.
            out.push(Span::new(shed, top(outer), 1.0, 0, 0.03));
            out.push(Span::new(top(outer), top(inner), 1.5, 0, 0.03));
            out.push(Span::new(top(inner), cabinet, 0.8, 0, 0.03));
        }
    }
    out
}

fn cables(m: &mut Mesh, w: &World, all: &[Site]) {
    for s in spans(w, all) {
        wire(m, s.a, s.b, s.sag, s.markers, s.thick);
    }
    let insulators = |m: &mut Mesh, p: Vec3| {
        for k in 0..3 {
            let y = k as f32 * 0.09;
            m.cuboid(p + vec3(-0.09, y - 0.14, -0.09), p + vec3(0.09, y - 0.09, 0.09), WHITE);
        }
    };
    for side in [1.0f32, -1.0] {
        // Insulator stacks on the gantry beams.
        for dz in [-GANTRY_HALF, GANTRY_HALF] {
            for gantry in layout::GANTRIES {
                insulators(m, gantry_top(w, gantry, dz, side) - Vec3::Y * 0.1);
            }
        }
        // A pole and cross-arm on the shed and on the cabinet take the line.
        for ((x, z), roof, line) in [(generator(w), SHED_ROOF, SHED_LINE), (SWITCHGEAR, CABINET_ROOF, CABINET_LINE)] {
            let (x, z) = (x * side, z * side);
            let g = w.height(x, z);
            let lead = GANTRY_HALF * 0.2;
            beam(m, vec3(x, g + roof, z), vec3(x, g + line - 0.1, z), 0.1, 0.1, STEEL);
            beam(m, vec3(x, g + line - 0.14, z - lead - 0.15), vec3(x, g + line - 0.14, z + lead + 0.15), 0.05, 0.08, STEEL);
            for e in [-lead, lead] {
                insulators(m, vec3(x, g + line + 0.04, z + e));
            }
        }
    }
}

/// Doors, lamps, vents and names for the cabins, sheds and switchgear.
fn station(m: &mut Mesh, w: &World) {
    for side in [1.0f32, -1.0] {
        let yaw = |toward_plus_z: bool| {
            if toward_plus_z == (side > 0.0) {
                0.0
            } else {
                PI
            }
        };
        // Communications cabin by B / D: door and lit window toward the point.
        let (cx, cz) = comms(w);
        let (cx, cz) = (cx * side, cz * side);
        let g = w.height(cx, cz);
        let face = wall(vec3(cx, g, cz + side * 1.512), yaw(true));
        m.obox(&face, vec3(-2.6, 1.0, 0.0), vec3(0.45, 1.0, 0.02), shade(STEEL, 0.7));
        m.obox(&face, vec3(-2.25, 1.0, 0.03), vec3(0.03, 0.08, 0.02), SAFETY);
        // Everything sits between the container's ribs, two metres apart.
        m.obox(&face, vec3(1.0, 1.5, 0.0), vec3(0.8, 0.42, 0.015), LAMP);
        for dx in [0.2f32, 1.0, 1.8] {
            m.obox(&face, vec3(dx, 1.5, 0.02), vec3(0.03, 0.46, 0.02), INK);
        }
        for dy in [-0.44f32, 0.0, 0.44] {
            m.obox(&face, vec3(1.0, 1.5 + dy, 0.02), vec3(0.83, 0.025, 0.02), INK);
        }
        // The operator's chair is empty, the desk lamp still on.
        m.obox(&face, vec3(1.45, 1.27, 0.018), vec3(0.14, 0.19, 0.004), INK);
        m.obox(&face, vec3(0.5, 1.25, 0.018), vec3(0.03, 0.17, 0.004), INK);
        m.obox(&face, vec3(0.58, 1.42, 0.018), vec3(0.1, 0.03, 0.004), INK);
        stencil(m, &(face * Mat4::from_translation(vec3(-2.95, 2.15, 0.01))), "COMMS", 0.042, WHITE);
        stencil(m, &(face * Mat4::from_translation(vec3(1.0, 2.3, 0.01))), "IF IT ANSWERS", 0.022, SAFETY);
        stencil(m, &(face * Mat4::from_translation(vec3(1.0, 2.08, 0.01))), "LOG IT", 0.022, SAFETY);
        m.obox(&face, vec3(3.1, 1.9, 0.2), vec3(0.4, 0.3, 0.2), shade(STEEL, 1.2));
        m.obox(&face, vec3(3.1, 1.9, 0.405), vec3(0.3, 0.2, 0.004), INK);
        let roof = g + 2.7;
        for (dx, hgt) in [(-3.2f32, 2.6f32), (-2.6, 1.7), (2.9, 3.4)] {
            beam(m, vec3(cx + dx * side, roof, cz), vec3(cx + dx * side, roof + hgt, cz), 0.03, 0.03, STEEL);
        }
        m.cuboid(vec3(cx - 0.5, roof, cz - 0.5), vec3(cx + 0.5, roof + 0.35, cz + 0.5), shade(STEEL, 1.1));

        // Exchange switchgear at C: louvred doors and a nameplate.
        let (sx, sz) = (SWITCHGEAR.0 * side, SWITCHGEAR.1 * side);
        let g = w.height(sx, sz);
        for plus in [true, false] {
            let n = if plus { 1.0 } else { -1.0 };
            let face = wall(vec3(sx, g, sz + n * 1.012), if plus { 0.0 } else { PI });
            for k in 0..4 {
                let dx = -1.5 + k as f32;
                m.obox(&face, vec3(dx, 0.7, 0.0), vec3(0.44, 0.6, 0.012), shade(STEEL, 1.1 + 0.08 * (k % 2) as f32));
                for l in 0..4 {
                    m.obox(&face, vec3(dx, 0.35 + l as f32 * 0.12, 0.016), vec3(0.3, 0.02, 0.006), INK);
                }
                m.obox(&face, vec3(dx + 0.34, 0.95, 0.02), vec3(0.02, 0.06, 0.01), SAFETY);
            }
            stencil(m, &(face * Mat4::from_translation(vec3(0.0, 1.14, 0.014))), "EXCHANGE", 0.028, WHITE);
        }

        // Generator shed at A / E: exhaust stack, vents and a status lamp.
        let (gx, gz) = generator(w);
        let (gx, gz) = (gx * side, gz * side);
        let g = w.height(gx, gz);
        pipe(m, vec3(gx - 0.8 * side, g + 1.45, gz), vec3(gx - 0.8 * side, g + 2.7, gz), 0.09, INK);
        m.cuboid(vec3(gx - 0.8 * side - 0.14, g + 2.7, gz - 0.14), vec3(gx - 0.8 * side + 0.14, g + 2.76, gz + 0.14), INK);
        for plus in [true, false] {
            let n = if plus { 1.0 } else { -1.0 };
            let face = wall(vec3(gx, g, gz + n * 1.012), if plus { 0.0 } else { PI });
            for l in 0..6 {
                m.obox(&face, vec3(-0.45, 0.35 + l as f32 * 0.13, 0.0), vec3(0.55, 0.025, 0.012), INK);
            }
            m.obox(&face, vec3(0.75, 1.1, 0.0), vec3(0.07, 0.07, 0.012), [0.25, 0.95, 0.4, 0.9]);
            stencil(m, &(face * Mat4::from_translation(vec3(0.7, 0.45, 0.012))), if side > 0.0 { "GEN 1" } else { "GEN 2" }, 0.032, SAFETY);
        }
    }
}

// --------------------------------------------------------------------- sky

/// Clear evening after rain: gold at the horizon, rose opposite the sun,
/// deep blue overhead; snow ranges beyond the ridge, cirrus, moon and stars.
pub fn sky() -> Mesh {
    let mut m = Mesh::default();
    let sun = SUN.normalize();
    let (sun_el, sun_az) = (sun.y.asin(), sun.z.atan2(sun.x));
    let r = 420.0;
    let colour = |e: f32, a: f32| -> Col {
        let warm = (a - sun_az).cos();
        let low = mix3([0.97, 0.77, 0.52], [0.80, 0.63, 0.68], smooth(0.0, 0.5, e));
        let mut c = mix3(low, [0.20, 0.33, 0.61], smooth(0.2, 1.25, e));
        let glow = warm.max(0.0).powi(3) * (1.0 - smooth(0.0, 0.8, e));
        c = [c[0] + 0.08 * glow, c[1] + 0.05 * glow, c[2] - 0.06 * glow];
        // The belt of Venus, opposite the sun.
        c = mix3(c, [0.74, 0.60, 0.76], (-warm).max(0.0).powi(2) * (1.0 - smooth(0.02, 0.5, e)) * 0.45);
        // Below the horizon the dome matches the fog, so terrain fades into it.
        c = mix3([FOG[0], FOG[1], FOG[2]], c, smooth(-0.02, 0.07, e));
        [c[0].min(1.0), c[1].min(1.0), c[2].clamp(0.0, 1.0), 1.0]
    };
    for row in 0..16 {
        let e0 = -0.25 + 1.82 * row as f32 / 16.0;
        let e1 = -0.25 + 1.82 * (row + 1) as f32 / 16.0;
        for k in 0..48 {
            let a = k as f32 * TAU / 48.0;
            let b = (k + 1) as f32 * TAU / 48.0;
            m.quad_c(
                sky_dir(e0, a) * r,
                sky_dir(e0, b) * r,
                sky_dir(e1, b) * r,
                sky_dir(e1, a) * r,
                [colour(e0, a), colour(e0, b), colour(e1, b), colour(e1, a)],
            );
        }
    }
    // First stars overhead and a pale moon opposite the sun.
    for k in 0..70u32 {
        let e = 0.85 + hash01(k, 601) * 0.7;
        let s = 0.4 + hash01(k, 603) * 0.5;
        billboard(&mut m, sky_dir(e, hash01(k, 602) * TAU), 412.0, s, s, [0.95, 0.96, 1.0, 0.15 + hash01(k, 604) * 0.35]);
    }
    let moon = sky_dir(0.62, sun_az + PI + 0.45);
    sky_glow(&mut m, moon, 410.0, 40.0, [0.9, 0.9, 1.0, 0.12]);
    sky_disc(&mut m, moon, 408.0, 11.0, [0.93, 0.92, 0.9, 0.7]);
    sky_disc(&mut m, sky_dir(0.632, sun_az + PI + 0.468), 407.0, 9.5, colour(0.632, sun_az + PI + 0.468));
    // The sun, low, with a wide soft glow.
    sky_glow(&mut m, sun, 404.0, 210.0, [1.0, 0.78, 0.45, 0.28]);
    sky_glow(&mut m, sun, 403.0, 62.0, [1.0, 0.88, 0.6, 0.55]);
    sky_disc(&mut m, sun, 402.0, 13.0, [1.0, 0.96, 0.84, 1.0]);
    // Cirrus: long combed streaks, gold toward the sun and lilac away.
    for k in 0..22u32 {
        let a = hash01(k, 611) * TAU;
        let e = 0.3 + hash01(k, 612) * 0.6;
        let d = sky_dir(e, a);
        let rt = d.cross(Vec3::Y).normalize_or_zero();
        let up = rt.cross(d);
        let lean = up * 0.18 + rt;
        let c = d * 392.0;
        let (hw, hh) = (50.0 + hash01(k, 613) * 90.0, 1.0 + hash01(k, 614) * 2.2);
        let warm = ((a - sun_az).cos() * 0.5 + 0.5).powi(2);
        let tint = mix3([0.80, 0.74, 0.90], [1.0, 0.80, 0.62], warm);
        for layer in 0..3 {
            let cc = c + up * (layer as f32 * hh * 2.4) + rt * (layer as f32 * hw * 0.22);
            let core = [tint[0], tint[1], tint[2], 0.42 - 0.1 * layer as f32];
            let rim = [tint[0], tint[1], tint[2], 0.0];
            m.quad_c(cc - lean * hw - up * hh, cc - up * hh, cc + up * hh, cc - lean * hw + up * hh, [rim, core, core, rim]);
            m.quad_c(cc - up * hh, cc + lean * hw - up * hh, cc + lean * hw + up * hh, cc + up * hh, [core, rim, rim, core]);
        }
    }
    // A single contrail, catching the last of the light.
    let (a0, a1) = (sun_az + 1.9, sun_az + 2.5);
    for k in 0..14 {
        let t0 = k as f32 / 14.0;
        let t1 = (k + 1) as f32 / 14.0;
        let p = |t: f32, off: f32| sky_dir(0.55 + t * 0.2 + off, a0 + (a1 - a0) * t) * 396.0;
        let c = |t: f32| [1.0, 0.86, 0.74, 0.55 * t * t];
        m.quad_c(p(t0, -0.0025), p(t1, -0.0025), p(t1, 0.0025), p(t0, 0.0025), [c(t0), c(t1), c(t1), c(t0)]);
    }
    billboard(&mut m, sky_dir(0.75, a1), 395.0, 0.9, 0.9, [1.0, 1.0, 1.0, 0.95]);
    // Three ranges, each paler and higher than the one in front, their
    // snowfields lit rose on the side that faces the sun.
    for (layer, (base, amp, freq, rock)) in [
        (0.19f32, 0.115f32, 5.0f32, [0.58f32, 0.56, 0.72]),
        (0.15, 0.085, 8.0, [0.45, 0.45, 0.63]),
        (0.105, 0.06, 13.0, [0.34, 0.37, 0.52]),
    ]
    .into_iter()
    .enumerate()
    {
        let dist = 384.0 - layer as f32 * 4.0;
        let ridge = |a: f32| {
            let u = a / TAU * freq * 6.0;
            let wrap = |x: f32, seed: u32| {
                // Tile the noise so the range closes on itself.
                let period = freq * 6.0;
                let t = (x / period).fract();
                vnoise(x, 0.5, seed) * (1.0 - t) + vnoise(x - period, 0.5, seed) * t
            };
            let n = wrap(u, 700 + layer as u32) * 0.65 + wrap(u * 2.7, 710 + layer as u32) * 0.35;
            base + amp * (n * n * 1.6 - 0.15)
        };
        let steps = 160;
        for k in 0..steps {
            let (a, b) = (k as f32 / steps as f32 * TAU, (k + 1) as f32 / steps as f32 * TAU);
            let (ea, eb) = (ridge(a), ridge(b));
            let foot = [FOG[0], FOG[1], FOG[2], 0.0];
            let body = [rock[0], rock[1], rock[2], 1.0];
            m.quad_c(
                sky_dir(0.03, a) * dist,
                sky_dir(0.03, b) * dist,
                sky_dir(eb, b) * dist,
                sky_dir(ea, a) * dist,
                [foot, foot, body, body],
            );
            // Snow above the snowline, deeper on the higher peaks.
            let line = base + amp * 0.42;
            if ea.max(eb) > line {
                let lit = ((a + b) * 0.5 - sun_az).cos();
                let snow = mix3([0.72, 0.76, 0.92], [1.0, 0.80, 0.70], smooth(-0.3, 0.6, lit));
                let col = [snow[0], snow[1], snow[2], 1.0];
                m.quad(
                    sky_dir(ea.min(line.max(ea - 0.045)), a) * (dist - 1.0),
                    sky_dir(eb.min(line.max(eb - 0.045)), b) * (dist - 1.0),
                    sky_dir(eb, b) * (dist - 1.0),
                    sky_dir(ea, a) * (dist - 1.0),
                    col,
                );
            }
        }
    }
    let _ = sun_el;
    m
}

// ------------------------------------------------------------------ lights

pub fn lights(w: &World, eye: Vec3, existing: [Light; 4]) -> [Light; 4] {
    let mut all: Vec<Light> = Vec::new();
    for s in sites(w) {
        all.push(Light { pos: s.at(0.8, s.deck() + 2.0, -0.8), radius: 9.0, col: [0.72, 0.88, 0.96], power: 0.55 });
    }
    for (x, z, g, side) in dishes(w) {
        all.push(Light { pos: vec3(x - 0.5 * side, g + 2.1, z + side * 2.2), radius: 8.0, col: [1.0, 0.8, 0.55], power: 0.6 });
    }
    for side in [1.0f32, -1.0] {
        let (cx, cz) = comms(w);
        let (cx, cz) = (cx * side, cz * side);
        all.push(Light {
            pos: vec3(cx + 0.6 * side, w.height(cx, cz) + 1.6, cz + side * 2.6),
            radius: 9.0,
            col: [1.0, 0.8, 0.55],
            power: 0.7,
        });
    }
    all.sort_by(|a, b| (a.pos - eye).length_squared().total_cmp(&(b.pos - eye).length_squared()));
    let mut result = existing;
    let mut n = 0;
    for light in &mut result {
        if (light.power <= 0.0 || light.radius <= 0.0) && n < all.len() {
            *light = all[n];
            n += 1;
        }
    }
    result
}

// -------------------------------------------------------------- atmosphere

/// Whether a Morse lamp sending `code` (dots, dashes, spaces between
/// letters) is lit `t` seconds into its message. Returns false once done.
fn morse(code: &str, t: f32) -> bool {
    const UNIT: f32 = 0.17;
    let mut at = 0.0;
    for c in code.chars() {
        let (on, gap) = match c {
            '.' => (1.0, 1.0),
            '-' => (3.0, 1.0),
            _ => (0.0, 2.0),
        };
        if t >= at && t < at + on * UNIT {
            return true;
        }
        at += (on + gap) * UNIT;
    }
    false
}
/// HELLO.
const HELLO: &str = ".... . .-.. .-.. ---";
const MESSAGE_SECONDS: f32 = 11.0;

/// Everything that moves: the Morse beacons and their answer, a turning
/// radar head on each exchange tower, anemometers and windsocks, the flag
/// lines, a hawk, seed fluff on the wind, exhaust from the generators and
/// the lamps on the field radios and switchgear.
pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32) {
    let all = sites(w);
    let turn = (time / MESSAGE_SECONDS).floor() as u32;
    let into = time % MESSAGE_SECONDS;
    for s in &all {
        let f = s.frame();
        let deck = s.deck();
        let mast = deck + layout::MAST_ABOVE;
        if s.exchange() {
            // West calls on even turns; east answers on odd ones.
            let mine = (turn % 2 == 0) == (s.side > 0.0);
            let lit = mine && morse(HELLO, into - 0.6);
            for (dx, dz) in [(0.62f32, 0.62f32), (-0.62, -0.62)] {
                let p = s.at(dx, mast - 0.42, dz);
                cross(add, p, 0.1, 0.1, [1.0, 0.14, 0.06, if lit { 1.0 } else { 0.12 }]);
                if lit {
                    soft(add, p, 1.1, 1.1, [1.0, 0.12, 0.05, 0.65]);
                }
            }
            // The radar head turns steadily on top of the mast.
            let g = f * Mat4::from_translation(vec3(0.0, mast + 0.45, 0.0)) * Mat4::from_rotation_y(time * 1.3 + s.index as f32);
            m.obox(&g, Vec3::ZERO, vec3(0.9, 0.2, 0.05), WHITE);
            m.obox(&g, vec3(0.0, 0.0, -0.07), vec3(0.86, 0.03, 0.03), STEEL);
            m.obox(&g, vec3(0.0, -0.22, 0.0), vec3(0.07, 0.1, 0.07), STEEL);
            for e in [-0.6f32, 0.0, 0.6] {
                m.obox(&g, vec3(e, 0.0, -0.06), vec3(0.025, 0.2, 0.03), STEEL);
            }
            // The field radio's lamp: steady in the west, frantic in the east.
            let lamp = s.at(-0.62, deck + 0.2, -layout::RING_HALF + 0.49);
            let on = if s.side > 0.0 { (time * 0.8).sin() > -0.6 } else { (time * 9.0).sin() > 0.0 };
            cross(add, lamp, 0.03, 0.03, [0.3, 1.0, 0.45, if on { 0.9 } else { 0.05 }]);
            // Flags down to the gantry, snapping in the wind.
            let (a, b, sag) = flag_line(w, s);
            let n = ((a - b).length() / 0.95) as usize;
            let along = (b - a).normalize_or_zero();
            let side = along.cross(Vec3::Y).normalize_or_zero();
            const FLAGS: [[f32; 3]; 5] = [[0.2, 0.42, 0.8], [0.92, 0.92, 0.88], [0.82, 0.2, 0.16], [0.2, 0.6, 0.32], [0.93, 0.74, 0.2]];
            for k in 1..n {
                let t = k as f32 / n as f32;
                let top = a.lerp(b, t) - Vec3::Y * (sag * 4.0 * t * (1.0 - t));
                let flap = (time * 7.0 + k as f32 * 1.7).sin() * 0.14 + (time * 2.3 + k as f32).sin() * 0.08;
                let c = FLAGS[k % 5];
                let low = top - Vec3::Y * 0.42 + side * (0.18 + flap) + WIND * 0.1;
                alpha.quad(top - along * 0.2, top + along * 0.2, low + along * 0.2, low - along * 0.2, [c[0], c[1], c[2], 0.92]);
            }
        } else {
            let p = s.at(0.0, mast + 0.1, 0.0);
            let pulse = (time * 1.1 + s.index as f32 * 1.6).sin().max(0.0).powi(6);
            cross(add, p, 0.12, 0.12, [1.0, 0.14, 0.06, 0.25 + 0.75 * pulse]);
            if pulse > 0.05 {
                soft(add, p, 1.0, 1.0, [1.0, 0.12, 0.05, 0.6 * pulse]);
            }
            // Three-cup anemometer on the weather arm.
            let hub = s.at(-0.95, deck + 4.58, 0.95);
            let spin = time * (6.0 + 2.0 * (time * 0.31).sin());
            for k in 0..3 {
                let a = spin + k as f32 * TAU / 3.0;
                let tip = hub + vec3(a.cos(), 0.0, a.sin()) * 0.26;
                beam(m, hub, tip, 0.018, 0.018, STEEL);
                m.cuboid(tip - Vec3::splat(0.045), tip + Vec3::splat(0.045), WHITE);
            }
            // Windsock on the corner pole, four tapering hoops of cloth.
            let root = s.at(layout::RING_HALF - 0.1, deck + 2.8, layout::RING_HALF - 0.1);
            let up = Vec3::Y;
            let across = WIND.cross(up).normalize();
            let mut prev = root;
            for k in 0..4 {
                let t = (k + 1) as f32 / 4.0;
                let sway = (time * 5.0 - k as f32 * 0.9).sin() * 0.05 * t + (time * 1.7).sin() * 0.06 * t;
                let next = root + WIND * (0.34 * (k + 1) as f32) + across * sway - up * (0.09 * t * t * 4.0);
                let (r0, r1) = (0.15 - 0.022 * k as f32, 0.15 - 0.022 * (k + 1) as f32);
                let col = if k % 2 == 0 { [0.95, 0.42, 0.12, 0.95] } else { [0.95, 0.94, 0.9, 0.95] };
                alpha.quad(prev - up * r0, prev + up * r0, next + up * r1, next - up * r1, col);
                alpha.quad(prev - across * r0, prev + across * r0, next + across * r1, next - across * r1, col);
                prev = next;
            }
        }
    }

    // The dishes' receivers glow as they integrate, slowly.
    for (x, z, g, side) in dishes(w) {
        let hub = vec3(x, g + layout::DISH_PEDESTAL_H + 2.0, z);
        let azimuth = vec3(-side, 0.0, side * 0.35).normalize();
        let axis = (azimuth * 0.66f32.sin() + Vec3::Y * 0.66f32.cos()).normalize();
        let breathe = 0.5 + 0.5 * (time * 0.7 + side).sin();
        soft(add, hub + axis * 3.3, 0.5, 0.5, [0.35, 1.0, 0.6, 0.25 + 0.4 * breathe]);
    }

    // Switchgear lamps chatter; generator exhaust drifts off downwind.
    for side in [1.0f32, -1.0] {
        let (sx, sz) = (SWITCHGEAR.0 * side, SWITCHGEAR.1 * side);
        let g = w.height(sx, sz);
        for k in 0..12u32 {
            let face = if k % 2 == 0 { 1.0 } else { -1.0 };
            let p = vec3(sx - 1.7 + (k / 2) as f32 * 0.62, g + 1.02, sz + face * 1.03);
            let rate = 0.7 + hash01(k, 801) * 3.0;
            let on = ((time * rate + hash01(k, 802) * 9.0).fract() < 0.55) as u32 as f32;
            let col = if k % 3 == 0 { [1.0, 0.6, 0.15] } else { [0.3, 1.0, 0.45] };
            cross(add, p, 0.022, 0.022, [col[0], col[1], col[2], 0.1 + 0.8 * on]);
        }
        let (gx, gz) = generator(w);
        let (gx, gz) = (gx * side, gz * side);
        let stack = vec3(gx - 0.8 * side, w.height(gx, gz) + 2.8, gz);
        for k in 0..5u32 {
            let age = (time * 0.35 + k as f32 / 5.0).fract();
            let p = stack + WIND * (age * 3.5) + Vec3::Y * (age * 1.6);
            let s = 0.18 + age * 0.9;
            soft(alpha, p, s, s, [0.4, 0.4, 0.42, (1.0 - age) * 0.28]);
        }
    }

    // A hawk rides the ridge lift in wide, lazy circles.
    for k in 0..2u32 {
        let a = time * 0.11 * if k == 0 { 1.0 } else { -0.8 } + k as f32 * 2.4;
        let (rx, rz) = ((58.0 - k as f32 * 20.0) * SPREAD, (34.0 - k as f32 * 8.0) * SPREAD);
        let p = vec3(a.cos() * rx, 30.0 + k as f32 * 9.0 + (time * 0.23).sin() * 2.0, a.sin() * rz);
        let fwd = vec3(-a.sin() * rx, 0.0, a.cos() * rz).normalize_or_zero() * if k == 0 { 1.0 } else { -1.0 };
        let right = fwd.cross(Vec3::Y);
        let bank = Vec3::Y * 0.28;
        let col = [0.16, 0.12, 0.1, 0.95];
        for e in [-1.0f32, 1.0] {
            let tip = p + right * (e * 1.25) + bank * e - fwd * 0.2;
            alpha.tri(p + fwd * 0.42, p - fwd * 0.3, tip, col);
            alpha.tri(p - fwd * 0.3, tip, tip - fwd * 0.34, col);
        }
        alpha.tri(p - fwd * 0.3, p - fwd * 0.72 + right * 0.18, p - fwd * 0.72 - right * 0.18, col);
    }

    // Seed fluff and midges carried across the plateau, gold in the low sun.
    // As many to the square metre as before the map was spread out.
    let fluff = (70.0 * SPREAD * SPREAD) as u32;
    for k in 0..fluff {
        let life = 14.0 + hash01(k, 811) * 10.0;
        let age = (time / life + hash01(k, 812)).fract();
        let start =
            vec3((hash01(k, 813) * 2.0 - 1.0) * (HALF_X - 18.0 * SPREAD), 0.0, (hash01(k, 814) * 2.0 - 1.0) * (HALF_Z - 12.0 * SPREAD));
        let mut p = start + WIND * (age * 34.0 - 17.0);
        p.x = p.x.clamp(-HALF_X + 4.0, HALF_X - 4.0);
        p.z = p.z.clamp(-HALF_Z + 4.0, HALF_Z - 4.0);
        p.y = w.height(p.x, p.z) + 1.0 + hash01(k, 815) * 4.5 + (time * 0.6 + k as f32).sin() * 0.5;
        let fade = (age * (1.0 - age) * 6.0).min(1.0);
        cross(add, p, 0.025, 0.025, [1.0, 0.9, 0.62, 0.5 * fade]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_dressing_is_finite_bounded_and_within_budget() {
        let w = World::with_map(0xFE221, ID);
        let mut mesh = Mesh::default();
        build(&mut mesh, &w);
        eprintln!("Relay static dressing: {} triangles", mesh.triangles());
        for s in sites(&w) {
            eprintln!("  tower {} at {},{} pad {:.2} deck {:.2}", s.name(), s.x, s.z, s.base, s.base + s.deck());
        }
        assert!(mesh.triangles() > 8_000 && mesh.triangles() < 132_000, "static dressing budget");
        let mut top = 0.0f32;
        for (v, indices) in mesh.finish() {
            assert!(indices.iter().all(|&i| (i as usize) < v.len()));
            for v in v {
                let p = Vec3::from_array(v.pos);
                assert!(p.is_finite() && Vec3::from_array(v.nrm).is_finite());
                assert!(p.x.abs() < HALF_X && p.z.abs() < HALF_Z && p.y > -8.0 && p.y < 34.0, "detail outside bounds: {p:?}");
                top = top.max(p.y);
            }
        }
        assert!(top > 16.0, "the exchange masts should be the tallest things on the map");
        // Every tower block, and both dish pedestals, are drawn here and
        // nowhere else; nothing else on the map is claimed.
        let owned = w.blocks.iter().filter(|b| owns(b)).count();
        assert!(owned > 200 && owned < w.blocks.len(), "{owned} of {} blocks owned", w.blocks.len());
        assert_eq!(w.blocks.iter().filter(|b| owns(b) && (b.max.x - b.min.x - 3.0).abs() < 0.01 && b.max.y - b.min.y > 5.0).count(), 2);
    }

    #[test]
    fn relay_sky_and_animation_stay_finite_and_small() {
        let w = World::with_map(0xFE221, ID);
        let mut vertices = 0;
        for (v, indices) in sky().finish() {
            vertices += v.len();
            assert!(indices.iter().all(|&i| (i as usize) < v.len()));
            for v in v {
                assert!(Vec3::from_array(v.pos).is_finite());
                assert!(v.col.into_iter().all(|c| c.is_finite() && (0.0..=1.0).contains(&c)), "sky colour out of range: {:?}", v.col);
            }
        }
        assert!(vertices > 4_000 && vertices < 20_000, "sky budget: {vertices} vertices");
        for time in [0.0f32, 0.9, 4.2, 9.5, 61.0, 500.0] {
            let (mut solid, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
            atmosphere(&mut solid, &mut alpha, &mut add, &w, time);
            assert!(solid.triangles() + alpha.triangles() + add.triangles() < 2_500, "animation budget");
            for (v, indices) in solid.finish().into_iter().chain(alpha.finish()).chain(add.finish()) {
                assert!(indices.iter().all(|&i| (i as usize) < v.len()));
                for v in v {
                    let p = Vec3::from_array(v.pos);
                    assert!(
                        p.is_finite() && p.x.abs() < HALF_X + 7.0 && p.z.abs() < HALF_Z + 4.0 && p.y > -8.0 && p.y < 60.0,
                        "animated detail astray: {p:?}"
                    );
                    assert!(v.col.into_iter().all(f32::is_finite));
                }
            }
        }
    }

    #[test]
    fn relay_cables_clear_the_ground_and_the_buildings() {
        // On the reference map the cables clear everything. The huts, tanks
        // and lookouts scattered over the plateau know nothing of the cable
        // runs, so on other seeds only the station's own buildings count.
        cables_clear(&World::with_map(0xFE221, ID), true);
        for seed in 0..32 {
            cables_clear(&World::with_map(seed, ID), false);
        }
    }

    fn cables_clear(w: &World, everything: bool) {
        let station = |b: &Block| {
            let c = (b.min + b.max) * 0.5;
            let near = |x: f32, z: f32, r: f32| (c.x - x).hypot(c.z - z) < r || (c.x + x).hypot(c.z + z) < r;
            everything
                || owns(b)
                || w.points.iter().any(|p| near(p.x, p.z, 16.0))
                || layout::GANTRIES.iter().any(|&(x, z)| near(x, z, GANTRY_HALF + 1.0))
        };
        let all = sites(w);
        let mut runs = spans(w, &all);
        // The flag line's flags hang below it.
        for r in runs.iter_mut().filter(|r| r.thick < 0.025) {
            r.a.y -= 0.45;
            r.b.y -= 0.45;
        }
        for r in &runs {
            for k in 1..40 {
                let t = k as f32 / 40.0;
                let p = r.a.lerp(r.b, t) - Vec3::Y * (r.sag * 4.0 * t * (1.0 - t));
                let flat = |q: Vec3| vec3(q.x - p.x, 0.0, q.z - p.z).length();
                // Head room for a player walking underneath, except where
                // the cable comes down to a shed or a cabinet.
                let low = |q: Vec3| q.y - w.height(q.x, q.z) < 3.0 && flat(q) < 8.0;
                let need = if low(r.a) || low(r.b) { 0.8 } else { 2.0 };
                assert!(p.y > w.height(p.x, p.z) + need, "seed {}: cable {:?} -> {:?} sags to the ground at {p:?}", w.seed, r.a, r.b);
                // Clear of the collision, but for the mast crowns it hangs from.
                let near_end = flat(r.a).min(flat(r.b)) < layout::CROWN_HALF + 0.3;
                let hit = w
                    .blocks
                    .iter()
                    .filter(|b| station(b))
                    .find(|b| (p.cmpgt(b.min - Vec3::splat(0.1)) & p.cmplt(b.max + Vec3::splat(0.1))).all());
                assert!(
                    near_end || hit.is_none(),
                    "seed {}: cable {:?} -> {:?} passes through {:?} at {p:?}",
                    w.seed,
                    r.a,
                    r.b,
                    hit.map(|b| (b.min, b.max))
                );
            }
        }
        // The cabins, sheds and switchgear the dressing is pinned to stand
        // where the shared layout built them.
        for (x, z) in [comms(w), generator(w), SWITCHGEAR] {
            assert!(
                w.blocks.iter().any(|b| { ((b.min.x + b.max.x) * 0.5 - x).abs() < 0.05 && ((b.min.z + b.max.z) * 0.5 - z).abs() < 0.05 }),
                "no building at {x},{z} (seed {})",
                w.seed
            );
        }
    }

    #[test]
    fn the_beacons_spell_hello_and_take_turns() {
        // H: four dots. Lit at the start of each, dark between.
        assert!(morse(HELLO, 0.01) && !morse(HELLO, 0.18) && morse(HELLO, 0.35));
        // O ends the word with three dashes; then silence.
        let units: f32 = HELLO
            .chars()
            .map(|c| match c {
                '.' => 2.0,
                '-' => 4.0,
                _ => 2.0,
            })
            .sum();
        assert!(units * 0.17 + 0.6 < MESSAGE_SECONDS, "the message must fit its turn");
        assert!(!morse(HELLO, units * 0.17 + 0.2) && !morse(HELLO, -0.1));
        let lights = lights(&World::with_map(0xFE221, ID), Vec3::ZERO, [Light::default(); 4]);
        assert!(lights.iter().all(|l| l.radius > 0.0 && l.power > 0.0));
    }
}
