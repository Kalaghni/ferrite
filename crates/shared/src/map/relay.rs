//! Relay: the signal towers. Each is a square service core wrapped by an
//! open steel stair that climbs, a quarter turn per flight, to a railed deck.
//! A slim antenna mast stands on the deck. Steps rise 30 cm, so a player
//! walks up without jumping; bots stay on the ground.
//!
//! Everything is authored in a tower's own frame and placed with a quarter
//! turn `rot`, then mirrored by the map's half turn.
//!
//! BACKBONE, the great cellular mast on the north flank, is climbed by
//! ladder (see `mast` and `sim::step`).

use super::{BlockKind, World};
use glam::Vec3;

pub const MAP_ID: u8 = 5;

/// West-authored towers: (x, z, flights climbed, quarter turns).
/// The array tower south of B has one full turn of stair; the exchange tower
/// beside C climbs a turn and a half and is the tallest thing on the map.
/// The array tower keeps its place 12 m south of B, which has moved out with
/// the map's spread; the exchange tower beside C has not moved.
pub const TOWERS: [(f32, f32, usize, u8); 2] = [(-58.0, -51.0, 4, 1), (-9.5, 9.0, 6, 0)];

/// West-authored listening dish: a concrete pedestal, mirrored like the rest.
pub const DISH: (f32, f32) = (-33.0 * super::SPREAD, 15.0 * super::SPREAD);
/// West-authored cable gantries, from the exchange out toward A.
pub const GANTRIES: [(f32, f32); 2] = [(-23.0 * super::SPREAD, -8.0 * super::SPREAD), (-58.0 * super::SPREAD, 2.0 * super::SPREAD)];
pub const DISH_PEDESTAL_HALF: f32 = 1.5;
pub const DISH_PEDESTAL_H: f32 = 5.0;

pub const PAD_HALF: f32 = 2.7;
/// A lower, wider apron round the pad halves the lip on sloping ground.
pub const APRON_HALF: f32 = 3.4;
pub const APRON_DROP: f32 = 0.3;
/// The pad stands this far proud of the highest ground beneath it.
pub const PAD_LIFT: f32 = 0.34;
/// Ground is levelled within about half this radius of each tower.
pub const SITE_RADIUS: f32 = 10.0;
pub const CORE_HALF: f32 = 1.3;
pub const RING_HALF: f32 = 2.4;
pub const STEPS_PER_FLIGHT: usize = 4;
pub const RISE: f32 = 0.3;
pub const TREAD: f32 = 0.65;
/// Each flight ends one more rise up, on a corner landing.
pub const FLIGHT_RISE: f32 = RISE * (STEPS_PER_FLIGHT as f32 + 1.0);
pub const SLAB: f32 = 0.15;
pub const RAIL_HALF: f32 = 0.055;
pub const RAIL_HEIGHTS: [f32; 2] = [0.5, 1.0];
pub const PARAPET_H: f32 = 0.9;
pub const PARAPET_HALF_LEN: f32 = 1.0;
pub const PARAPET_T: f32 = 0.14;
pub const MAST_HALF: f32 = 0.3;
pub const MAST_ABOVE: f32 = 6.5;
pub const CROWN_HALF: f32 = 0.85;
/// Corner landings of the stair ring, anticlockwise from the arrival corner.
pub const CORNERS: [(f32, f32); 4] = [(-1.85, -1.85), (1.85, -1.85), (1.85, 1.85), (-1.85, 1.85)];

/// Height of a tower's deck above its foundation pad.
pub fn deck_height(flights: usize) -> f32 {
    flights as f32 * FLIGHT_RISE
}

/// The ring corner the stair starts from, so that it always arrives at
/// corner 0 whatever the number of flights.
pub fn start_corner(flights: usize) -> usize {
    (4 - flights % 4) % 4
}

/// A point in a tower's frame, in world coordinates. `side` is 1 for a
/// west-authored tower and -1 for its mirror.
pub fn place(x: f32, z: f32, rot: u8, side: f32, dx: f32, dz: f32) -> (f32, f32) {
    let (rx, rz) = match rot % 4 {
        0 => (dx, dz),
        1 => (-dz, dx),
        2 => (-dx, -dz),
        _ => (dz, -dx),
    };
    (x + side * rx, z + side * rz)
}

/// Top of the foundation pad: just proud of the highest ground under the
/// footprint, so the pad is never buried. Sites are levelled, which keeps the
/// apron and the pad each one walkable step.
pub fn pad_top(w: &World, x: f32, z: f32) -> f32 {
    let mut top = f32::NEG_INFINITY;
    for dx in [-PAD_HALF, 0.0, PAD_HALF] {
        for dz in [-PAD_HALF, 0.0, PAD_HALF] {
            top = top.max(w.height(x + dx, z + dz));
        }
    }
    top + PAD_LIFT
}

/// Where flight `j` runs: (start corner, end corner, height of its foot).
pub fn flight(flights: usize, j: usize) -> (usize, usize, f32) {
    let a = (start_corner(flights) + j) % 4;
    (a, (a + 1) % 4, j as f32 * FLIGHT_RISE)
}

/// Centre, travel direction and top height (above the pad) of step `i` of a
/// flight from corner `a`.
pub fn step(a: usize, foot: f32, i: usize) -> ((f32, f32), (f32, f32), f32) {
    let (ca, cb) = (CORNERS[a], CORNERS[(a + 1) % 4]);
    let d = ((cb.0 - ca.0) / 3.7, (cb.1 - ca.1) / 3.7);
    let mid = ((ca.0 + cb.0) * 0.5, (ca.1 + cb.1) * 0.5);
    let along = -CORE_HALF + (i as f32 + 0.5) * TREAD;
    ((mid.0 + d.0 * along, mid.1 + d.1 * along), d, foot + (i as f32 + 1.0) * RISE)
}

struct Site {
    x: f32,
    z: f32,
    rot: u8,
    side: f32,
    base: f32,
}

impl Site {
    /// Box between two opposite corners in the tower's frame.
    #[allow(clippy::too_many_arguments)]
    fn put(&self, w: &mut World, a: (f32, f32), b: (f32, f32), y0: f32, y1: f32, kind: BlockKind, tint: u8) {
        let p = place(self.x, self.z, self.rot, self.side, a.0, a.1);
        let q = place(self.x, self.z, self.rot, self.side, b.0, b.1);
        let (cx, cz) = ((p.0 + q.0) * 0.5, (p.1 + q.1) * 0.5);
        let (hx, hz) = ((p.0 - q.0).abs() * 0.5, (p.1 - q.1).abs() * 0.5);
        w.add_abs(cx, cz, hx, hz, self.base + y0, self.base + y1, kind, tint);
    }

    /// Two thin rails along the segment a..b at standing height above `floor`.
    fn rail(&self, w: &mut World, a: (f32, f32), b: (f32, f32), floor: f32, heights: &[f32]) {
        let grow = |p: (f32, f32), q: (f32, f32)| {
            if (p.0 - q.0).abs() < 1e-4 {
                ((p.0 - RAIL_HALF, p.1), (q.0 + RAIL_HALF, q.1))
            } else {
                ((p.0, p.1 - RAIL_HALF), (q.0, q.1 + RAIL_HALF))
            }
        };
        let (a, b) = grow(a, b);
        for &h in heights {
            self.put(w, a, b, floor + h - RAIL_HALF, floor + h + RAIL_HALF, BlockKind::Metal, 0);
        }
    }
}

/// Build one tower and its half-turn mirror.
pub(super) fn tower(w: &mut World, x: f32, z: f32, flights: usize, rot: u8) {
    use BlockKind::*;
    let deck = deck_height(flights);
    let (c, r) = (CORE_HALF, RING_HALF);
    for side in [1.0f32, -1.0] {
        let (sx, sz) = (x * side, z * side);
        let base = pad_top(w, sx, sz);
        let s = Site { x: sx, z: sz, rot, side, base };
        let mut low = base;
        for dx in [-APRON_HALF, APRON_HALF] {
            for dz in [-APRON_HALF, APRON_HALF] {
                low = low.min(w.height(sx + dx, sz + dz));
            }
        }
        // Apron, foundation pad and the service core the stair wraps round.
        s.put(w, (-APRON_HALF, -APRON_HALF), (APRON_HALF, APRON_HALF), low - base - 0.6, -APRON_DROP, Concrete, 1);
        s.put(w, (-PAD_HALF, -PAD_HALF), (PAD_HALF, PAD_HALF), -APRON_DROP, 0.0, Concrete, 2);
        s.put(w, (-c, -c), (c, c), 0.0, deck - SLAB, Metal, 1);

        // The stair: floating treads with a rail on the open side, and a
        // landing at every corner.
        for j in 0..flights {
            let (a, _, foot) = flight(flights, j);
            if j > 0 {
                let (cx, cz) = CORNERS[a];
                s.put(w, (cx - 0.55, cz - 0.55), (cx + 0.55, cz + 0.55), foot - SLAB, foot, Step, 1);
                let (ox, oz) = (cx.signum() * (r - RAIL_HALF), cz.signum() * (r - RAIL_HALF));
                s.rail(w, (ox, cz - 0.55), (ox, cz + 0.55), foot, &RAIL_HEIGHTS[1..]);
                s.rail(w, (cx - 0.55, oz), (cx + 0.55, oz), foot, &RAIL_HEIGHTS[1..]);
            }
            for i in 0..STEPS_PER_FLIGHT {
                let ((mx, mz), (dx, dz), top) = step(a, foot, i);
                let (hx, hz) = (dx.abs() * TREAD * 0.5 + dz.abs() * 0.55, dz.abs() * TREAD * 0.5 + dx.abs() * 0.55);
                s.put(w, (mx - hx, mz - hz), (mx + hx, mz + hz), top - SLAB, top, Step, (i % 2) as u8);
                // Outward is a quarter turn clockwise from the direction of travel.
                let (nx, nz) = (dz, -dx);
                let (ex, ez) = (mx + nx * (0.55 - RAIL_HALF), mz + nz * (0.55 - RAIL_HALF));
                let h = TREAD * 0.5;
                s.rail(w, (ex - dx.abs() * h, ez - dz.abs() * h), (ex + dx.abs() * h, ez + dz.abs() * h), top, &RAIL_HEIGHTS[1..]);
            }
        }

        // The deck: the core's roof plus the two sides of the ring with
        // headroom to spare. The last flight and a half stay open to the sky.
        s.put(w, (-c, -r), (r, c), deck - SLAB, deck, Metal, 2);
        s.put(w, (-r, -r), (-c, -c), deck - SLAB, deck, Metal, 2);
        s.put(w, (c, c), (r, r), deck - SLAB, deck, Metal, 2);
        let e = r - RAIL_HALF;
        for (a, b) in [
            ((-r, -e), (r, -e)),
            ((e, -r), (e, r)),
            ((c, e), (r, e)),
            ((c + RAIL_HALF, c), (c + RAIL_HALF, r)),
            ((-c, c - RAIL_HALF), (c, c - RAIL_HALF)),
            ((-c + RAIL_HALF, -c), (-c + RAIL_HALF, c)),
            ((-e, -r), (-e, -c)),
        ] {
            s.rail(w, a, b, deck, &RAIL_HEIGHTS);
        }
        // Solid parapets on the two outward edges give a marksman cover.
        let p = PARAPET_HALF_LEN;
        s.put(w, (-p, -r), (p, -r + PARAPET_T), deck, deck + PARAPET_H, Metal, 3);
        s.put(w, (r - PARAPET_T, -p), (r, p), deck, deck + PARAPET_H, Metal, 3);

        // Antenna mast and its crown, well above head height.
        s.put(w, (-MAST_HALF, -MAST_HALF), (MAST_HALF, MAST_HALF), deck, deck + MAST_ABOVE, Metal, 0);
        s.put(w, (-CROWN_HALF, -CROWN_HALF), (CROWN_HALF, CROWN_HALF), deck + MAST_ABOVE - 1.6, deck + MAST_ABOVE - 0.5, Metal, 1);
    }
}

// ------------------------------------------------------------ BACKBONE

/// BACKBONE, the station's great cellular mast, on the north flank. It
/// stands on the centre line, as far from one spawn as from the other, and
/// everything built round it is mirrored across x = 0 rather than by the
/// map's half turn: a landmark this size cannot be doubled. Its parts are
/// authored in the mast's own frame (unrotated, measured up from its
/// levelled ground) and shared with the renderer. (`MAST_HALF` and
/// `MAST_ABOVE` above are the towers' antenna masts.)
pub const BACKBONE: (f32, f32) = (0.0, 84.0);
/// Leg centres at the ground, either side of the mast's axis.
pub const MAST_FOOT: f32 = 4.3;
/// The legs lean in this much per metre of height.
pub const MAST_TAPER: f32 = 0.057;
pub const MAST_BAY: f32 = 5.7;
pub const MAST_BAYS: usize = 10;
/// Square section of a leg.
pub const MAST_LEG: f32 = 0.30;
/// Collision follows each slanted leg in short straight pieces, so a box
/// is never much fatter than the steel it stands for. Pieces are taller
/// than a jump and a mantle together, so the steps between them cannot
/// be climbed.
pub const LEG_PIECES_PER_BAY: usize = 2;
pub const FOOTING_HALF: f32 = 0.8;
/// Footings are one walkable step high.
pub const FOOTING_H: f32 = 0.45;

/// The rest platform partway up, and the top deck inside the first ring
/// of panels. The rest platform is caged by plate parapets; the top deck
/// has only its rails and the panels themselves for cover.
pub const LANDING: f32 = 20.0;
pub const TOP_DECK: f32 = 36.75;
pub const LANDING_HALF: f32 = 3.6;
pub const TOP_HALF: f32 = 2.6;
pub const LANDING_PARAPET_H: f32 = 1.1;
pub const LANDING_PARAPET_HALF_LEN: f32 = 1.6;
/// No bracing crosses this band above the ground or a deck, so nobody
/// walks through steel the renderer draws.
pub const HEADROOM: f32 = 2.4;
/// The ring of panels round the top deck: centre height, radius from the
/// axis, half size (across, up, through) and count.
pub const PANEL_Y: f32 = 39.0;
pub const PANEL_R: f32 = 3.5;
pub const PANEL_HALF: (f32, f32, f32) = (0.43, 1.85, 0.19);
pub const PANELS: usize = 6;

/// One ladder, in the mast's frame: where its rungs are, which way a
/// climber faces, and the heights it runs between.
#[derive(Clone, Copy, Debug)]
pub struct Rungs {
    /// Middle of the rung plane.
    pub x: f32,
    pub z: f32,
    /// Unit direction a climber faces: into the rungs.
    pub face: (f32, f32),
    pub foot: f32,
    pub top: f32,
}

/// The first ladder climbs the face toward the battlefield to the rest
/// platform; the second starts on the platform, on the far side of the
/// core, and climbs to the top deck. With offset ladders a climber steps
/// off at each level, and a fall down the top hatch lands on the platform.
pub const LADDERS: [Rungs; 2] = [
    Rungs { x: 0.0, z: -0.5, face: (0.0, 1.0), foot: 0.0, top: LANDING },
    Rungs { x: 0.0, z: 0.5, face: (0.0, -1.0), foot: LANDING, top: TOP_DECK },
];
/// Ladder rails either side of the centre line.
pub const LADDER_HALF: f32 = 0.43;
pub const RUNG_PITCH: f32 = 0.49;
/// Steel behind the rungs: a ladder cannot be walked through.
pub const LADDER_BACK: f32 = 0.1;
/// Each ladder climbs through a hatch this wide and this deep, open on
/// the climber's side of the rungs.
pub const HATCH_HALF: f32 = 0.75;
pub const HATCH_DEPTH: f32 = 1.1;

/// The BACKBONE equipment hut behind the mast: (x0, z0, x1, z1, height).
pub const HUT: (f32, f32, f32, f32, f32) = (-3.0, 7.5, 3.0, 12.5, 3.8);
pub const HUT_ROOF: f32 = 0.25;
pub const HUT_EAVE: f32 = 0.3;

/// The levelled mast site in its own frame: (x0, z0, x1, z1).
pub const MAST_SITE: (f32, f32, f32, f32) = (-6.5, -6.5, 6.5, 13.5);

/// Half width of the lattice at height `y`: leg centre to axis.
pub fn mast_half(y: f32) -> f32 {
    MAST_FOOT - y * MAST_TAPER
}

/// Ground level at the foot of the mast.
pub fn mast_base(w: &World) -> f32 {
    w.height(BACKBONE.0, BACKBONE.1)
}

/// Rectangle (x0, z0, x1, z1), in the mast frame, of the hatch a ladder
/// climbs through: the well on the climber's side of the rungs.
pub fn hatch(l: &Rungs) -> (f32, f32, f32, f32) {
    let (fx, fz) = l.face;
    // Back corner on one side, and the opposite corner at the rung plane.
    let a = (l.x - fx * HATCH_DEPTH + fz * HATCH_HALF, l.z - fz * HATCH_DEPTH - fx * HATCH_HALF);
    let b = (l.x - fz * HATCH_HALF, l.z + fx * HATCH_HALF);
    (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

/// Whether a block belongs to the mast. These are the only blocks on
/// Relay without a half-turn twin; they are mirrored across x = 0 instead.
pub fn mast_owns(b: &super::Block) -> bool {
    let c = (b.min + b.max) * 0.5;
    let (x0, z0, x1, z1) = MAST_SITE;
    let (mx, mz) = BACKBONE;
    c.x > mx + x0 && c.x < mx + x1 && c.z > mz + z0 && c.z < mz + z1
}

/// One level pad under the legs and the hut, blending into the hillside.
/// Called while sampling terrain, after the towers' sites are levelled.
pub(super) fn settle(x: f32, z: f32, h: f32, natural: impl Fn(f32, f32) -> f32) -> f32 {
    let (x0, z0, x1, z1) = MAST_SITE;
    let (mx, mz) = BACKBONE;
    let (cx, cz) = (mx + (x0 + x1) * 0.5, mz + (z0 + z1) * 0.5);
    let d = ((x - cx).abs() - (x1 - x0) * 0.5).max((z - cz).abs() - (z1 - z0) * 0.5).max(0.0);
    let w = 1.0 - super::smooth(1.0, 8.0, d);
    if w > 0.0 {
        h + (natural(mx, mz) - h) * w
    } else {
        h
    }
}

/// Keep random cover, huts and trees off the mast site and off its
/// half-turn mirror, so the open ground on both flanks stays the same.
pub(super) fn reserved(map: u8, x: f32, z: f32, hx: f32, hz: f32) -> bool {
    if map != MAP_ID {
        return false;
    }
    let (x0, z0, x1, z1) = MAST_SITE;
    let (mx, mz) = BACKBONE;
    let m = 4.0;
    [(x, z), (-x, -z)]
        .iter()
        .any(|&(px, pz)| px + hx > mx + x0 - m && px - hx < mx + x1 + m && pz + hz > mz + z0 - m && pz - hz < mz + z1 + m)
}

/// Build the mast's collision: legs, footings, both decks with their
/// hatches, rails and parapets, the panels, the ladders' backs and the
/// hut. The ladders go into `World::ladders` for `sim::step`.
pub(super) fn mast(w: &mut World) {
    use BlockKind::*;
    let base = mast_base(w);
    let (mx, mz) = BACKBONE;
    // A box between two corners in the mast frame, `y0..y1` above its ground.
    #[allow(clippy::too_many_arguments)]
    fn put(w: &mut World, base: f32, a: (f32, f32), b: (f32, f32), y0: f32, y1: f32, kind: BlockKind, tint: u8) {
        let (mx, mz) = BACKBONE;
        let (cx, cz) = (mx + (a.0 + b.0) * 0.5, mz + (a.1 + b.1) * 0.5);
        w.add_abs(cx, cz, (a.0 - b.0).abs() * 0.5, (a.1 - b.1).abs() * 0.5, base + y0, base + y1, kind, tint);
    }
    let t = MAST_LEG * 0.5;
    let piece = MAST_BAY / LEG_PIECES_PER_BAY as f32;
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let f = (sx * MAST_FOOT, sz * MAST_FOOT);
            let mut low = base;
            for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                low = low.min(w.height(mx + f.0 + dx * FOOTING_HALF, mz + f.1 + dz * FOOTING_HALF));
            }
            let (a, b) = ((f.0 - FOOTING_HALF, f.1 - FOOTING_HALF), (f.0 + FOOTING_HALF, f.1 + FOOTING_HALF));
            put(w, base, a, b, low - base - 0.6, FOOTING_H, Concrete, 2);
            for k in 0..MAST_BAYS * LEG_PIECES_PER_BAY {
                let (y0, y1) = (k as f32 * piece, (k + 1) as f32 * piece);
                // Over the piece the leg's centre runs from mast_half(y0) in to mast_half(y1).
                let (outer, inner) = (mast_half(y0) + t, mast_half(y1) - t);
                put(w, base, (sx * inner, sz * inner), (sx * outer, sz * outer), y0.max(FOOTING_H), y1, Metal, 0);
            }
        }
    }

    // A deck of half size `half` whose top is at `y`, with a ladder's hatch
    // cut out, and two rails round its edge.
    let deck = |w: &mut World, half: f32, y: f32, hole: (f32, f32, f32, f32)| {
        let (hx0, hz0, hx1, hz1) = hole;
        let y0 = y - SLAB;
        put(w, base, (-half, -half), (half, hz0), y0, y, Metal, 2);
        put(w, base, (-half, hz1), (half, half), y0, y, Metal, 2);
        put(w, base, (-half, hz0), (hx0, hz1), y0, y, Metal, 2);
        put(w, base, (hx1, hz0), (half, hz1), y0, y, Metal, 2);
        let e = half - 2.0 * RAIL_HALF;
        for h in RAIL_HEIGHTS {
            let (r0, r1) = (y + h - RAIL_HALF, y + h + RAIL_HALF);
            put(w, base, (-half, -half), (half, -e), r0, r1, Metal, 0);
            put(w, base, (-half, e), (half, half), r0, r1, Metal, 0);
            put(w, base, (-half, -e), (-e, e), r0, r1, Metal, 0);
            put(w, base, (e, -e), (half, e), r0, r1, Metal, 0);
        }
    };
    deck(w, LANDING_HALF, LANDING, hatch(&LADDERS[0]));
    deck(w, TOP_HALF, TOP_DECK, hatch(&LADDERS[1]));
    // The rest platform is caged: a plate parapet inside the rails in the
    // middle of every side, as high as a crouching player.
    let (p, e) = (LANDING_PARAPET_HALF_LEN, LANDING_HALF - 2.0 * RAIL_HALF);
    let (y0, y1) = (LANDING, LANDING + LANDING_PARAPET_H);
    for s in [-1.0f32, 1.0] {
        put(w, base, (-p, s * e), (p, s * (e - PARAPET_T)), y0, y1, Metal, 3);
        put(w, base, (s * e, -p), (s * (e - PARAPET_T), p), y0, y1, Metal, 3);
    }
    // The panels round the top deck stop bullets. Most stand at an angle,
    // so each is boxed by the middle of its footprint and none reaches in
    // over the rail.
    for k in 0..PANELS {
        let (s, c) = (k as f32 * std::f32::consts::TAU / PANELS as f32).sin_cos();
        let (cx, cz) = (s * PANEL_R, c * PANEL_R);
        let (across, through) = (PANEL_HALF.0, PANEL_HALF.2);
        let (hx, hz) = ((across * c.abs() + through * s.abs()) * 0.8, (across * s.abs() + through * c.abs()) * 0.8);
        put(w, base, (cx - hx, cz - hz), (cx + hx, cz + hz), PANEL_Y - PANEL_HALF.1, PANEL_Y + PANEL_HALF.1, Metal, 1);
    }

    // The ladders: solid behind the rungs, climbable in front.
    for l in LADDERS {
        let (fx, fz) = l.face;
        let (bx, bz) = (l.x + fx * LADDER_BACK, l.z + fz * LADDER_BACK);
        let side = (fz.abs() * (LADDER_HALF + 0.05), fx.abs() * (LADDER_HALF + 0.05));
        let floor = if l.foot <= 0.0 { w.height(mx + l.x, mz + l.z).min(base) - base - 0.6 } else { l.foot };
        put(w, base, (l.x.min(bx) - side.0, l.z.min(bz) - side.1), (l.x.max(bx) + side.0, l.z.max(bz) + side.1), floor, l.top, Metal, 0);
        w.ladders.push(super::Ladder { at: Vec3::new(mx + l.x, base + l.foot, mz + l.z), face: Vec3::new(fx, 0.0, fz), top: base + l.top });
    }

    // The BACKBONE hut, with an overhanging roof.
    let (x0, z0, x1, z1, h) = HUT;
    let mut low = base;
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        low = low.min(w.height(mx + x, mz + z));
    }
    put(w, base, (x0, z0), (x1, z1), low - base - 0.6, h, Metal, 1);
    let r = HUT_EAVE;
    put(w, base, (x0 - r, z0 - r), (x1 + r, z1 + r), h, h + HUT_ROOF, Metal, 0);
}
