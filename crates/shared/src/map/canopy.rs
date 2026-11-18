//! Canopy: a botanical reserve in the roots and lower boughs of a colossal
//! fig. C is the root well between four trunks; an 18 m bough circuit rings
//! it, reached by stairs behind B and D and by ladders, with inward branches
//! that drop back into the well. Designed with Codex; see docs/maps/canopy.md.
//!
//! Every gameplay surface is authored on the west and placed again by the
//! map's half turn, (x, z) -> (-x, -z).

use super::{BlockKind, World};
use crate::rng::Rng;
use glam::Vec3;

pub const MAP_ID: u8 = 7;
pub const SECTOR_NAMES: [&str; 5] = ["RECEIVING GROVE", "COLLECTOR COURT", "ROOT WELL", "COLLECTOR COURT", "RECEIVING GROVE"];
pub const POINTS: [(f32, f32); 5] = [(-118.0, -24.0), (-60.0, 42.0), (0.0, 0.0), (60.0, -42.0), (118.0, 24.0)];

/// Pad heights at A, B and C (D and E mirror B and A) and at the bases.
pub const PAD_Y: [f32; 3] = [2.4, 6.0, 1.2];
pub const BASE_Y: f32 = 2.4;
/// The west nursery terrace B stands on: x0, z0, x1, z1, top.
pub const TERRACE: (f32, f32, f32, f32, f32) = (-100.0, 26.0, -42.0, 64.0, 6.0);
const TERRACE_BLEND: f32 = 28.0;

/// Walking surface of the boughs, and the thickness of a bough slab.
pub const BOUGH_Y: f32 = 18.0;
pub const SLAB: f32 = 0.6;
pub const BOUGH_HALF: f32 = 2.0;
/// The bough circuit's corners, a closed loop round the root well.
pub const CIRCUIT: [(f32, f32); 12] = [
    (-42.0, 24.0),
    (-18.0, 24.0),
    (-18.0, 48.0),
    (18.0, 48.0),
    (18.0, 24.0),
    (42.0, 24.0),
    (42.0, -24.0),
    (18.0, -24.0),
    (18.0, -48.0),
    (-18.0, -48.0),
    (-18.0, -24.0),
    (-42.0, -24.0),
];
/// West inward branch: from the circuit at (-18, 24) south to a drop over
/// the well at (-18, 0). Its open end is the way down.
pub const BRANCH: ((f32, f32), (f32, f32)) = ((-18.0, 24.0), (-18.0, 0.0));
/// West access: a connector from the circuit corner west along z = 24, then
/// north to the head of the stair behind B.
pub const CONNECTOR: [(f32, f32); 3] = [(-42.0, 24.0), (-78.0, 24.0), (-78.0, 44.0)];

/// Stair behind B: flight one climbs east along z = 60 from the terrace to a
/// landing, flight two climbs south along x = -78 to the connector.
pub const STAIR_RISE: f32 = 0.3;
pub const STAIR_TREAD: f32 = 0.7;
pub const STAIR_STEPS: usize = 20;
pub const STAIR_HALF: f32 = 1.4;
pub const FLIGHT_ONE: (f32, f32) = (-94.0, 60.0);
pub const LANDING: (f32, f32, f32) = (-78.0, 60.0, 2.0);
pub const FLIGHT_TWO: (f32, f32) = (-78.0, 58.0);

/// West ladder: rungs on the outer face of the west circuit leg, climbed
/// facing east. (x of the rung plane, z of its middle.)
pub const LADDER: (f32, f32) = (-44.0, -12.0);
pub const LADDER_HALF: f32 = 0.43;
pub const LADDER_BACK: f32 = 0.1;

/// The four trunks of the hollow tree: west-authored (x, z, half x, half z),
/// mirrored, from the ground to `TRUNK_TOP`.
pub const TRUNKS: [(f32, f32, f32, f32); 2] = [(-28.0, 0.0, 6.0, 8.0), (0.0, 32.0, 6.0, 6.0)];
pub const TRUNK_TOP: f32 = 36.0;

/// The outer buttress east of A, with two passages under it.
pub const BUTTRESS: (f32, f32, f32, f32) = (-96.0, 0.0, 6.0, 30.0);
pub const BUTTRESS_H: f32 = 12.0;
pub const PASSAGES: [f32; 2] = [-20.0, 20.0];
pub const PASSAGE_HALF: f32 = 3.0;
pub const PASSAGE_H: f32 = 4.0;

/// Seedlines, west-authored player-foot paths, mirrored to six. Each starts
/// and ends at a dock with a short climb, so riders clear rails and lips.
/// The court line's ground dock is set on the ground when the map is built.
pub const LINES: [&[(f32, f32, f32)]; 3] = [
    // From the collector court behind B up to the circuit's west corner.
    &[(-78.0, 6.05, 42.0), (-78.0, 8.4, 42.0), (-48.0, 22.0, 28.0), (-42.0, 22.0, 24.0), (-42.0, BOUGH_Y + 0.05, 24.0)],
    // Across the north of the well, west corner to the middle of the north edge.
    &[(-42.0, BOUGH_Y + 0.05, 24.0), (-42.0, 22.0, 24.0), (0.0, 22.0, 48.0), (0.0, BOUGH_Y + 0.05, 48.0)],
    // On from there to the east corner.
    &[(0.0, BOUGH_Y + 0.05, 48.0), (0.0, 22.0, 48.0), (42.0, 22.0, 24.0), (42.0, BOUGH_Y + 0.05, 24.0)],
];
/// How near a dock a player must stand to take a line.
pub const DOCK_REACH: f32 = 1.5;

/// Shade curtains round the root well, west bank: cloth hung across the
/// plane x = `SHADE_X`, over these stretches of z, from a drum at
/// `SHADE_TOP` down to `SHADE_TOP - SHADE_DROP` when fully let down. The
/// east bank is the half-turn mirror. Cloth stops sight, never bullets.
pub const SHADE_X: f32 = -18.0;
pub const SHADE_SPANS: [(f32, f32); 2] = [(8.0, 24.0), (-24.0, -8.0)];
pub const SHADE_TOP: f32 = 7.4;
pub const SHADE_DROP: f32 = 6.4;
/// Seconds for a curtain to roll up or down.
pub const SHADE_SECS: f32 = 0.6;
/// The west bank's crank, on the inward face of the west trunk, and how
/// near (and facing it) a player works it; holding Use this long toggles
/// the bank, which then rests this long before it can be worked again.
pub const CRANK: (f32, f32) = (-21.4, 0.0);
pub const CRANK_REACH: f32 = 2.2;
pub const CRANK_HOLD_TICKS: u16 = 72;
pub const CRANK_REST_TICKS: u32 = 360;

/// The crank of bank `k` (0 west, 1 east), at its height.
pub fn crank(w: &World, k: usize) -> Vec3 {
    let s = if k == 0 { 1.0 } else { -1.0 };
    let (x, z) = (CRANK.0 * s, CRANK.1 * s);
    Vec3::new(x, w.height(x, z) + 1.1, z)
}

/// The bank whose crank a player at `p` facing `fwd` (horizontal) can work.
pub fn crank_at(w: &World, p: Vec3, fwd: Vec3) -> Option<usize> {
    (0..2).find(|&k| {
        let c = crank(w, k);
        let d = Vec3::new(c.x - p.x, 0.0, c.z - p.z);
        d.length() < CRANK_REACH && (c.y - p.y).abs() < 2.0 && d.normalize_or_zero().dot(fwd) > 0.4
    })
}

/// The cloth panels of bank `k`: (x of the plane, z0, z1).
pub fn shade_panels(k: usize) -> [(f32, f32, f32); 2] {
    let s = if k == 0 { 1.0 } else { -1.0 };
    SHADE_SPANS.map(|(z0, z1)| {
        let (a, b) = (z0 * s, z1 * s);
        (SHADE_X * s, a.min(b), a.max(b))
    })
}

/// Whether the sight line a-b passes through let-down cloth, with the banks
/// let down by `shades` (0 up, 1 down).
pub fn screened(shades: [f32; 2], a: Vec3, b: Vec3) -> bool {
    for (k, &down) in shades.iter().enumerate() {
        if down < 0.02 {
            continue;
        }
        let bottom = SHADE_TOP - SHADE_DROP * down.min(1.0);
        for (x, z0, z1) in shade_panels(k) {
            let dx = b.x - a.x;
            if dx.abs() < 1e-5 {
                continue;
            }
            let t = (x - a.x) / dx;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let p = a + (b - a) * t;
            if p.z >= z0 && p.z <= z1 && p.y >= bottom && p.y <= SHADE_TOP {
                return true;
            }
        }
    }
    false
}

/// Open potting shelters: roofs on four posts, cover from the boughs for
/// the ground fight. West-authored centres; each is 8 x 6 m.
pub const SHELTERS: [(f32, f32); 4] = [(-108.0, -48.0), (-56.0, -6.0), (-34.0, 62.0), (-50.0, -62.0)];
pub const SHELTER_HALF: (f32, f32) = (4.0, 3.0);
pub const SHELTER_ROOF: f32 = 3.2;

/// Rails along the bough edges: two bars, as on Relay's towers.
pub const RAIL_HALF: f32 = 0.055;
pub const RAIL_HEIGHTS: [f32; 2] = [0.5, 1.0];
/// Bark lips at the circuit's corners: chest-high cover.
pub const LIP_H: f32 = 0.95;
pub const LIP_T: f32 = 0.35;

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Distance from (x, z) to the rectangle x0..x1, z0..z1 (0 inside).
fn rect_dist(x: f32, z: f32, x0: f32, z0: f32, x1: f32, z1: f32) -> f32 {
    let dx = (x0 - x).max(x - x1).max(0.0);
    let dz = (z0 - z).max(z - z1).max(0.0);
    (dx * dx + dz * dz).sqrt()
}

/// The ground before objectives and bases are levelled: a shallow basin at
/// C between raised groves, the B and D terraces, and a pad at every
/// objective. `noise` is the map's gentle roughness.
pub fn shape(x: f32, z: f32, noise: f32) -> f32 {
    let mut h = 1.2 + 1.2 * smooth(72.0, 132.0, x.abs()) + noise;
    let (x0, z0, x1, z1, top) = TERRACE;
    for (px, pz) in [(x, z), (-x, -z)] {
        let w = 1.0 - smooth(0.0, TERRACE_BLEND, rect_dist(px, pz, x0, z0, x1, z1));
        h += (top - h) * w;
    }
    for (k, &(cx, cz)) in POINTS.iter().enumerate() {
        let y = PAD_Y[if k < 3 { k } else { 4 - k }];
        let d = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
        let w = 1.0 - smooth(10.0, 18.0, d);
        h += (y - h) * w;
    }
    for cx in [-159.0f32, 159.0] {
        let d = ((x - cx).powi(2) + z * z).sqrt();
        let w = 1.0 - smooth(super::BASE_SPAWN_RADIUS, super::BASE_SPAWN_RADIUS + 10.0, d);
        h += (BASE_Y - h) * w;
    }
    h
}

/// The bough circuit, the branches and the connectors as walkable slabs:
/// (x0, z0, x1, z1), west-authored and mirrored by the caller.
pub fn slabs() -> Vec<(f32, f32, f32, f32)> {
    let mut out = Vec::new();
    let seg = |a: (f32, f32), b: (f32, f32)| {
        (a.0.min(b.0) - BOUGH_HALF, a.1.min(b.1) - BOUGH_HALF, a.0.max(b.0) + BOUGH_HALF, a.1.max(b.1) + BOUGH_HALF)
    };
    // Half the loop, west-authored; the other half is its mirror.
    for k in 0..6 {
        out.push(seg(CIRCUIT[k], CIRCUIT[k + 1]));
    }
    // The branch stops short of its corner square and its open end.
    let (a, b) = BRANCH;
    out.push((a.0 - BOUGH_HALF, b.1, a.0 + BOUGH_HALF, a.1 - BOUGH_HALF));
    // The connector west, then north to the head of the stair, ending
    // where the top step does so a climber has headroom.
    out.push(seg(CONNECTOR[0], CONNECTOR[1]));
    let (c1, c2) = (CONNECTOR[1], CONNECTOR[2]);
    out.push((c1.0 - BOUGH_HALF, c1.1 - BOUGH_HALF, c1.0 + BOUGH_HALF, c2.1));
    out
}

/// Whether (x, z) is on a bough slab (either half).
pub fn on_bough(x: f32, z: f32) -> bool {
    slabs().iter().any(|&(x0, z0, x1, z1)| (x >= x0 && x <= x1 && z >= z0 && z <= z1) || (-x >= x0 && -x <= x1 && -z >= z0 && -z <= z1))
}

/// Ground objectives count soldiers standing near the pad, not ones on the
/// boughs, the stairs or riding over it.
pub fn on_pad(w: &World, k: usize, pos: Vec3) -> bool {
    let dy = pos.y - w.points[k].y;
    (-0.6..2.4).contains(&dy)
}

/// Where ground scatter must not go: trunks, roots, stairs, the well.
pub fn reserved(map: u8, x: f32, z: f32, rx: f32, rz: f32) -> bool {
    if map != MAP_ID {
        return false;
    }
    let near = |x: f32, z: f32| {
        TRUNKS.iter().any(|&(cx, cz, hx, hz)| (x - cx).abs() < hx + rx + 2.0 && (z - cz).abs() < hz + rz + 2.0)
            || (x.abs() < 50.0 && z.abs() < 56.0)
            || ((x - BUTTRESS.0).abs() < BUTTRESS.2 + rx + 2.0 && (z - BUTTRESS.1).abs() < BUTTRESS.3 + rz + 2.0)
            || ((x + 86.0).abs() < 12.0 + rx && (z - 52.0).abs() < 14.0 + rz)
    };
    near(x, z) || near(-x, -z)
}

/// Build Canopy's collision: everything below is placed twice, the second
/// time turned half round the centre.
pub fn build(w: &mut World, _rng: &mut Rng) {
    use BlockKind::{Concrete, Metal, Step, Trunk};
    let top = BOUGH_Y;
    let bottom = BOUGH_Y - SLAB;

    // ---- Ground: ground-relative blocks (centre, full size, height).
    let ground: [(f32, f32, f32, f32, f32, BlockKind, u8); 15] = [
        (-132.0, 0.0, 4.0, 24.0, 4.2, Trunk, 1),      // base root shield
        (-118.0, -38.0, 12.0, 2.0, 1.1, Concrete, 0), // A planting bins
        (-118.0, -10.0, 8.0, 2.0, 1.1, Concrete, 0),
        (-118.0, 56.0, 14.0, 12.0, 5.4, Concrete, 2), // northern seed vault
        (-64.0, 10.0, 18.0, 10.0, 4.8, Trunk, 1),     // middle root knuckle
        (-44.0, -30.0, 12.0, 18.0, 6.0, Trunk, 1),    // southern inner root
        (-76.0, -64.0, 20.0, 12.0, 6.0, Trunk, 1),    // southern root mass
        (-26.0, -78.0, 18.0, 10.0, 6.0, Trunk, 1),    // outer return root
        (-74.0, 42.0, 2.0, 12.0, 2.4, Metal, 3),      // B cabinets
        (-46.0, 42.0, 2.0, 8.0, 2.4, Metal, 3),
        (-70.0, 28.0, 6.0, 2.0, 1.1, Concrete, 0), // B bins
        (-50.0, 28.0, 6.0, 2.0, 1.1, Concrete, 0),
        (-60.0, 56.0, 8.0, 2.0, 1.1, Concrete, 0), // B bench
        (-12.0, 14.0, 6.0, 2.0, 1.1, Metal, 3),    // C seed bench
        (-70.0, -84.0, 10.0, 6.0, 2.4, Metal, 3),  // outer seed cabinet
    ];
    for (cx, cz, sx, sz, h, kind, tint) in ground {
        w.sym(cx, cz, sx * 0.5, sz * 0.5, 0.0, h, kind, tint);
    }

    // ---- Potting shelters: a thin roof on four posts.
    let (shx, shz) = SHELTER_HALF;
    for (cx, cz) in SHELTERS {
        for s in [1.0f32, -1.0] {
            let (x, z) = (cx * s, cz * s);
            let roof = low_ground(w, x, z, shx, shz).max(w.height(x, z)) + SHELTER_ROOF;
            w.add_abs(x, z, shx, shz, roof, roof + 0.3, Metal, 4);
            for (px, pz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let (qx, qz) = (x + px * (shx - 0.3), z + pz * (shz - 0.3));
                w.add_abs(qx, qz, 0.2, 0.2, w.height(qx, qz) - 0.6, roof, Metal, 4);
            }
        }
    }

    // ---- The trunks: solid from below the ground to high in the crown.
    for (cx, cz, hx, hz) in TRUNKS {
        for s in [1.0f32, -1.0] {
            let (x, z) = (cx * s, cz * s);
            let low = low_ground(w, x, z, hx, hz) - 0.6;
            w.add_abs(x, z, hx, hz, low, TRUNK_TOP, Trunk, 0);
        }
    }

    // ---- The outer buttress: piers between two passages, a beam over each.
    let (bx, bz, bhx, bhz) = BUTTRESS;
    for s in [1.0f32, -1.0] {
        let x = bx * s;
        let g = w.height(x, bz);
        let low = low_ground(w, x, bz, bhx, bhz) - 0.6;
        let roof = g + BUTTRESS_H;
        let mut z = bz - bhz;
        let mut cuts: Vec<f32> = PASSAGES.iter().map(|p| p * s).collect();
        cuts.sort_by(|a, b| a.total_cmp(b));
        for c in cuts {
            let (z0, z1) = (c - PASSAGE_HALF, c + PASSAGE_HALF);
            w.add_abs(x, (z + z0) * 0.5, bhx, (z0 - z) * 0.5, low, roof, Trunk, 0);
            let floor = low_ground(w, x, c, bhx, PASSAGE_HALF).max(w.height(x, c));
            w.add_abs(x, c, bhx, PASSAGE_HALF, floor + PASSAGE_H, roof, Trunk, 0);
            z = z1;
        }
        let end = bz + bhz;
        w.add_abs(x, (z + end) * 0.5, bhx, (end - z) * 0.5, low, roof, Trunk, 0);
    }

    // ---- The boughs: slabs, rails along their sides, lips at the corners.
    for (x0, z0, x1, z1) in slabs() {
        w.sym_abs((x0 + x1) * 0.5, (z0 + z1) * 0.5, (x1 - x0) * 0.5, (z1 - z0) * 0.5, bottom, top, Trunk, 2);
    }
    for (a, b, gaps) in rail_runs() {
        for (p, q) in split(a, b, &gaps) {
            for h in RAIL_HEIGHTS {
                let (cx, cz) = ((p.0 + q.0) * 0.5, (p.1 + q.1) * 0.5);
                let (hx, hz) =
                    if (p.0 - q.0).abs() < 1e-3 { (RAIL_HALF, (q.1 - p.1).abs() * 0.5) } else { ((q.0 - p.0).abs() * 0.5, RAIL_HALF) };
                w.sym_abs(cx, cz, hx, hz, top + h - RAIL_HALF, top + h + RAIL_HALF, Metal, 0);
            }
        }
    }
    for (cx, cz, hx, hz) in lips() {
        w.sym_abs(cx, cz, hx, hz, top, top + LIP_H, Trunk, 2);
    }

    // ---- The stair behind B, its landing and the head of the connector.
    for s in [1.0f32, -1.0] {
        let put = |w: &mut World, x0: f32, z0: f32, x1: f32, z1: f32, y1: f32, kind: BlockKind| {
            let (x0, x1) = if s > 0.0 { (x0, x1) } else { (-x1, -x0) };
            let (z0, z1) = if s > 0.0 { (z0, z1) } else { (-z1, -z0) };
            let (cx, cz, hx, hz) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5, (x1 - x0) * 0.5, (z1 - z0) * 0.5);
            let low = low_ground(w, cx, cz, hx, hz) - 0.6;
            w.add_abs(cx, cz, hx, hz, low, y1, kind, 1);
        };
        let base = TERRACE.4;
        let (fx, fz) = FLIGHT_ONE;
        for i in 0..STAIR_STEPS {
            let x0 = fx + i as f32 * STAIR_TREAD;
            put(w, x0, fz - STAIR_HALF, x0 + STAIR_TREAD, fz + STAIR_HALF, base + (i + 1) as f32 * STAIR_RISE, Step);
        }
        let mid = base + STAIR_STEPS as f32 * STAIR_RISE;
        let (lx, lz, lh) = LANDING;
        put(w, lx - lh, lz - lh, lx + lh, lz + lh, mid, Trunk);
        let (tx, tz) = FLIGHT_TWO;
        for i in 0..STAIR_STEPS {
            let z1 = tz - i as f32 * STAIR_TREAD;
            put(w, tx - STAIR_HALF, z1 - STAIR_TREAD, tx + STAIR_HALF, z1, mid + (i + 1) as f32 * STAIR_RISE, Step);
        }
    }

    // ---- Seedlines: the first three as authored, then their mirrors.
    for s in [1.0f32, -1.0] {
        for (k, line) in LINES.iter().enumerate() {
            let mut pts: Vec<Vec3> = line.iter().map(|&(x, y, z)| Vec3::new(x * s, y, z * s)).collect();
            if k == 0 {
                let g = w.height(pts[0].x, pts[0].z) + 0.05;
                let lift = pts[1].y - pts[0].y;
                pts[0].y = g;
                pts[1].y = g + lift;
            }
            w.lines.push(super::Seedline::new(pts));
        }
    }

    // ---- The ladders: solid behind the rungs, climbable in front.
    let (lx, lz) = LADDER;
    for s in [1.0f32, -1.0] {
        let (x, z) = (lx * s, lz * s);
        let face = Vec3::new(s, 0.0, 0.0);
        let g = w.height(x, z);
        let back = x + s * LADDER_BACK;
        w.add_abs((x + back) * 0.5, z, LADDER_BACK * 0.5, LADDER_HALF + 0.05, g - 0.6, bottom, Metal, 0);
        w.ladders.push(super::Ladder { at: Vec3::new(x, g, z), face, top });
    }
}

/// Lowest ground under a rectangle.
fn low_ground(w: &World, cx: f32, cz: f32, hx: f32, hz: f32) -> f32 {
    let mut low = w.height(cx, cz);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
        low = low.min(w.height(cx + sx * hx, cz + sz * hz));
    }
    low
}

/// Rails, west-authored: each run from a to b along one side of a slab, and
/// the stretches along it left open (junctions, the ladder, docks).
/// A rail run: from, to, and the stretches along it left open.
type Run = ((f32, f32), (f32, f32), Vec<(f32, f32)>);

fn rail_runs() -> Vec<Run> {
    let e = BOUGH_HALF - RAIL_HALF;
    let h = BOUGH_HALF;
    let mut runs = Vec::new();
    // Each circuit edge, between its corner squares, on both sides. Edge 5
    // is the east leg; its mirror is the west leg, whose outer side opens
    // for the ladder (authored here as the east leg's, at the mirrored z).
    let (_, lz) = LADDER;
    for k in 0..6 {
        let (a, b) = (CIRCUIT[k], CIRCUIT[k + 1]);
        if (a.1 - b.1).abs() < 1e-3 {
            let (x0, x1) = (a.0.min(b.0) + h, a.0.max(b.0) - h);
            for side in [-e, e] {
                runs.push(((x0, a.1 + side), (x1, a.1 + side), Vec::new()));
            }
        } else {
            let (z0, z1) = (a.1.min(b.1) + h, a.1.max(b.1) - h);
            for side in [-e, e] {
                let gaps = if k == 5 && side > 0.0 { vec![(-lz - LADDER_HALF - 0.2, -lz + LADDER_HALF + 0.2)] } else { Vec::new() };
                runs.push(((a.0 + side, z0), (a.0 + side, z1), gaps));
            }
        }
    }
    // The branch, down both sides to its open end.
    let (a, b) = BRANCH;
    for side in [-e, e] {
        runs.push(((a.0 + side, b.1), (a.0 + side, a.1 - h), Vec::new()));
    }
    // The connector west along z = 24 (its north side is open where it
    // turns north), and north along x = -78.
    let (c0, c1, c2) = (CONNECTOR[0], CONNECTOR[1], CONNECTOR[2]);
    runs.push(((c1.0 - h, c0.1 - e), (c0.0 - h, c0.1 - e), Vec::new()));
    runs.push(((c1.0 + h, c0.1 + e), (c0.0 - h, c0.1 + e), Vec::new()));
    runs.push(((c1.0 - e, c1.1 - h), (c1.0 - e, c2.1), Vec::new()));
    runs.push(((c1.0 + e, c1.1 + h), (c1.0 + e, c2.1), Vec::new()));
    runs
}

/// The run a..b with the stretches in `gaps` (along its length) left out.
fn split(a: (f32, f32), b: (f32, f32), gaps: &[(f32, f32)]) -> Vec<((f32, f32), (f32, f32))> {
    let along_z = (a.0 - b.0).abs() < 1e-3;
    let (lo, hi) = if along_z { (a.1.min(b.1), a.1.max(b.1)) } else { (a.0.min(b.0), a.0.max(b.0)) };
    let fixed = if along_z { a.0 } else { a.1 };
    let mut out = Vec::new();
    let mut at = lo;
    let mut gaps = gaps.to_vec();
    gaps.sort_by(|p, q| p.0.total_cmp(&q.0));
    for (g0, g1) in gaps {
        if g0 > at {
            out.push((at, g0.min(hi)));
        }
        at = at.max(g1);
    }
    if hi > at {
        out.push((at, hi));
    }
    out.into_iter()
        .filter(|(p, q)| q - p > 0.05)
        .map(|(p, q)| if along_z { ((fixed, p), (fixed, q)) } else { ((p, fixed), (q, fixed)) })
        .collect()
}

/// Bark lips on the closed sides of the circuit's corner squares (each is
/// open toward the edges, branch and connector that meet there),
/// west-authored: (centre x, centre z, half x, half z).
fn lips() -> Vec<(f32, f32, f32, f32)> {
    let h = BOUGH_HALF;
    let t = LIP_T * 0.5;
    let mut out = Vec::new();
    // Corners 0..6 are west-authored; the rest are their mirrors. Corner 0
    // also meets the west leg (from corner 11) and the connector.
    for k in 0..6 {
        let c = CIRCUIT[k];
        let prev = if k == 0 { CIRCUIT[11] } else { CIRCUIT[k - 1] };
        let next = CIRCUIT[k + 1];
        let mut open = vec![dir(c, prev), dir(c, next)];
        if k == 0 {
            open.push((-1, 0)); // the connector
        }
        if c == BRANCH.0 {
            open.push((0, -1)); // the branch
        }
        for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if open.contains(&d) {
                continue;
            }
            let (dx, dz) = (d.0 as f32, d.1 as f32);
            let (cx, cz) = (c.0 + dx * (h - t), c.1 + dz * (h - t));
            let (hx, hz) = if d.0 != 0 { (t, h) } else { (h, t) };
            out.push((cx, cz, hx, hz));
        }
    }
    out
}

fn dir(from: (f32, f32), to: (f32, f32)) -> (i32, i32) {
    (
        (to.0 - from.0).signum() as i32 * ((to.0 - from.0).abs() > 1e-3) as i32,
        (to.1 - from.1).signum() as i32 * ((to.1 - from.1).abs() > 1e-3) as i32,
    )
}
