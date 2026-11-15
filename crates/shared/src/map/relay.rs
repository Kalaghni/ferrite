//! Relay: the signal towers. Each is a square service core wrapped by an
//! open steel stair that climbs, a quarter turn per flight, to a railed deck.
//! A slim antenna mast stands on the deck. Steps rise 30 cm, so a player
//! walks up without jumping; bots stay on the ground.
//!
//! Everything is authored in a tower's own frame and placed with a quarter
//! turn `rot`, then mirrored by the map's half turn.

use super::{BlockKind, World};

pub const MAP_ID: u8 = 5;

/// West-authored towers: (x, z, flights climbed, quarter turns).
/// The array tower south of B has one full turn of stair; the exchange tower
/// beside C climbs a turn and a half and is the tallest thing on the map.
pub const TOWERS: [(f32, f32, usize, u8); 2] = [(-39.0, -38.0, 4, 1), (-9.5, 9.0, 6, 0)];

/// West-authored listening dish: a concrete pedestal, mirrored like the rest.
pub const DISH: (f32, f32) = (-33.0, 15.0);
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
