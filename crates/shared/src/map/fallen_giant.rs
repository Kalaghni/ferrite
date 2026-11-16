//! The Fallen Giant's cargo hold spans Ridgeline's central saddle.
//!
//! The aircraft ploughed a furrow through the saddle and broke in three.
//! Everything a player can touch - the shell, the buckled frames, cargo and
//! the wreckage strewn outside - is collision balanced under a half turn
//! ((x, z) -> (-x, -z)). The art may differ between the two halves only
//! above head height or inside these shared volumes.
use super::{smooth, Block, BlockKind, World};

pub const MAP_ID: u8 = 2;
pub const HALF_LENGTH: f32 = 42.0;
pub const HALF_WIDTH: f32 = 6.0;
pub const WALL_THICKNESS: f32 = 0.35;
pub const ROOF_HEIGHT: f32 = 8.0;
pub const DOOR_HEIGHT: f32 = 4.0;
pub const DOOR_HALF_WIDTH: f32 = 3.0;
pub const SIDE_DOORS: [f32; 3] = [-20.0, 0.0, 20.0];
/// Hull walls and roof.
pub const SHELL_TINT: u8 = 40;
/// Cargo cases and containers inside the hold.
pub const CARGO_TINT: u8 = 41;
/// Buckled frames and the collapsed roof heap inside the hold.
pub const FRAME_TINT: u8 = 42;
/// Wreckage strewn outside the hull.
pub const DEBRIS_TINT: u8 = 43;
/// Trees snapped along the slide path (trunk collision).
pub const STUMP_TINT: u8 = 44;

/// How far the hold floor is cut below the natural saddle: the hull lies
/// in its own furrow instead of on a raised pad.
pub const SINK: f32 = 1.8;
/// Level ground under the hull and in front of every opening.
pub const FLAT_X: f32 = HALF_LENGTH + 4.0;
pub const FLAT_Z: f32 = HALF_WIDTH + 2.0;
/// Length of the slide furrow, bow waves and snapped trees from the centre.
pub const SCAR_LENGTH: f32 = 94.0;

/// The hull broke at both ends of the centre section. A buckled frame
/// stands across each break and blocks most of the hold, so there is no
/// straight line along the hold: the aft frame (at -x) leaves a gap on the
/// +z side, the forward one on the -z side.
pub const BULKHEAD_X: f32 = 10.0;
pub const BULKHEAD_HALF: f32 = 0.3;
/// Edge of each frame's walk-around gap (|z| beyond this is open).
pub const BULKHEAD_GAP: f32 = 1.5;
/// The roof is torn open between these |x| beside each break, on the side
/// the frame blocks; the fallen skin lies in a heap under the tear.
pub const ROOF_TEAR: [f32; 2] = [12.0, 17.0];
pub const ROOF_TEAR_EDGE: f32 = 1.0;

/// A piece of wreckage outside the hull, authored on the aft (-x) half and
/// placed with its half-turn mirror. The ground under it is levelled so the
/// collision box sits flush.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub x: f32,
    pub z: f32,
    pub hx: f32,
    pub hz: f32,
    pub height: f32,
}

/// A main landing-gear bogie torn from the belly.
pub const BOGIE: Piece = Piece { x: -27.0, z: -12.0, hx: 1.3, hz: 0.75, height: 2.0 };
/// An outboard engine nacelle, lying half buried under the wing.
pub const NACELLE: Piece = Piece { x: -33.0, z: 16.5, hx: 3.7, hz: 1.9, height: 2.9 };
/// A burst cargo container thrown out of the hold.
pub const CONTAINER: Piece = Piece { x: -40.0, z: -14.5, hx: 2.9, hz: 1.25, height: 2.6 };
pub const PIECES: [Piece; 3] = [BOGIE, NACELLE, CONTAINER];
/// Snapped trees (x, z, height of the broken trunk) along the furrow.
pub const STUMPS: [(f32, f32, f32); 5] = [(-61.0, 7.0, 1.6), (-68.0, -7.5, 2.3), (-75.0, 8.5, 1.2), (-83.0, -6.5, 1.9), (-90.0, 7.5, 1.4)];

pub fn floor(w: &World) -> f32 {
    w.height(0.0, 0.0)
}

pub fn owns(block: &Block) -> bool {
    match block.kind {
        BlockKind::Metal => matches!(block.tint, SHELL_TINT | CARGO_TINT | FRAME_TINT | DEBRIS_TINT),
        BlockKind::Trunk => block.tint == STUMP_TINT,
        _ => false,
    }
}

/// A smooth hump: 1 at `centre`, 0 from `half` away.
fn hump(v: f32, centre: f32, half: f32) -> f32 {
    1.0 - smooth(0.0, half, (v - centre).abs())
}

/// The crash scar before the wreckage pads: a level furrow floor under the
/// hull, a long rounded skirt back to the hillside, spoil banks along the
/// flanks, bow waves of earth beside both broken ends and the slide furrow
/// running on past them. Every term depends on |x| and |z| only.
fn scar(x: f32, z: f32, h: f32, floor: f32) -> f32 {
    let (ax, az) = (x.abs(), z.abs());
    let dx = (ax - FLAT_X).max(0.0) / 26.0;
    let dz = (az - FLAT_Z).max(0.0) / 20.0;
    // Mostly linear, so the skirt has an even grade with rounded ends
    // rather than a steep middle.
    let t = (dx * dx + dz * dz).sqrt().min(1.0);
    let blend = 1.0 - (t * 0.6 + smooth(0.0, 1.0, t) * 0.4);
    let mut out = h + (floor - h) * blend;
    out += 0.6 * hump(az, 13.0, 5.0) * (1.0 - smooth(30.0, 46.0, ax));
    out += 1.2 * hump(az, 10.5, 6.5) * hump(ax, 58.0, 9.0);
    out -= 0.6 * hump(az, 0.0, 5.0) * smooth(46.0, 54.0, ax) * (1.0 - smooth(80.0, SCAR_LENGTH, ax));
    out
}

/// Called while sampling terrain, after the objective and landmark pads.
/// `natural` is the map's unlevelled shape.
pub(super) fn settle(x: f32, z: f32, h: f32, natural: impl Fn(f32, f32) -> f32) -> f32 {
    let floor = natural(0.0, 0.0) - SINK;
    let mut out = scar(x, z, h, floor);
    // Each piece of wreckage beds into a level pad at the mean height of
    // the ground round it, cutting the high side and filling the low. Pads
    // never reach the level deck and its aprons.
    let apron = smooth(0.0, 2.0, (x.abs() - FLAT_X).max(z.abs() - FLAT_Z));
    for p in PIECES {
        for (px, pz) in [(p.x, p.z), (-p.x, -p.z)] {
            let d = ((x - px).abs() - p.hx).max((z - pz).abs() - p.hz);
            let w = (1.0 - smooth(1.5, 5.5, d)) * apron;
            if w > 0.0 {
                let ground = |sx: f32, sz: f32| {
                    let (gx, gz) = (px + sx * (p.hx + 1.5), pz + sz * (p.hz + 1.5));
                    scar(gx, gz, natural(gx, gz), floor)
                };
                let level = (ground(0.0, 0.0) * 2.0 + ground(-1.0, -1.0) + ground(1.0, -1.0) + ground(-1.0, 1.0) + ground(1.0, 1.0)) / 6.0;
                out += (level - out) * w;
            }
        }
    }
    out
}

pub(super) fn reserved(map: u8, x: f32, z: f32, hx: f32, hz: f32) -> bool {
    if map != MAP_ID {
        return false;
    }
    let near = |px: f32, pz: f32, rx: f32, rz: f32| (x - px).abs() < rx + hx && (z - pz).abs() < rz + hz;
    near(0.0, 0.0, SCAR_LENGTH, HALF_WIDTH + 9.5)
        || PIECES.iter().any(|p| near(p.x, p.z, p.hx + 4.0, p.hz + 4.0) || near(-p.x, -p.z, p.hx + 4.0, p.hz + 4.0))
}

pub(super) fn build(w: &mut World) {
    use BlockKind::*;
    let g = floor(w);
    // Sidewall segments between the paired tears. End-on entry has no
    // blocking ramp lip.
    for z in [-HALF_WIDTH - WALL_THICKNESS * 0.5, HALF_WIDTH + WALL_THICKNESS * 0.5] {
        let mut start = -HALF_LENGTH;
        for end in SIDE_DOORS.map(|x| x - DOOR_HALF_WIDTH).into_iter().chain([HALF_LENGTH]) {
            let center = (start + end) * 0.5;
            w.add_abs(center, z, (end - start) * 0.5, WALL_THICKNESS * 0.5, g - 0.6, g + ROOF_HEIGHT, Metal, SHELL_TINT);
            start = end + DOOR_HALF_WIDTH * 2.0;
        }
        for x in SIDE_DOORS {
            w.add_abs(x, z, DOOR_HALF_WIDTH, WALL_THICKNESS * 0.5, g + DOOR_HEIGHT, g + ROOF_HEIGHT, Metal, SHELL_TINT);
        }
    }
    // Roof: an intact run over each section and a torn bay beside each break.
    let (y0, y1) = (g + ROOF_HEIGHT, g + ROOF_HEIGHT + WALL_THICKNESS);
    let edge = HALF_WIDTH + WALL_THICKNESS;
    let [t0, t1] = ROOF_TEAR;
    w.add_abs(0.0, 0.0, t0, edge, y0, y1, Metal, SHELL_TINT);
    w.sym_abs(-(t1 + HALF_LENGTH) * 0.5, 0.0, (HALF_LENGTH - t1) * 0.5, edge, y0, y1, Metal, SHELL_TINT);
    w.sym_abs(-(t0 + t1) * 0.5, (ROOF_TEAR_EDGE + edge) * 0.5, (t1 - t0) * 0.5, (edge - ROOF_TEAR_EDGE) * 0.5, y0, y1, Metal, SHELL_TINT);
    // Buckled frames across the breaks, floor to roof.
    let half = (BULKHEAD_GAP + HALF_WIDTH) * 0.5;
    w.sym_abs(-BULKHEAD_X, BULKHEAD_GAP - half, BULKHEAD_HALF, half, g, y0, Metal, FRAME_TINT);
    // The roof skin that fell through the tear, a low heap a player can
    // vault, leaving a nook against the frame.
    w.sym_abs(-(t0 + t1) * 0.5, -4.4, 1.9, 1.5, g - 0.3, g + 1.15, Metal, FRAME_TINT);
    // Cargo: low cases at each end, a toppled container in each end
    // section and a pair of pallets beside C.
    w.sym_abs(-30.0, -3.8, 2.8, 0.8, g, g + 1.2, Metal, CARGO_TINT);
    w.sym_abs(-36.0, 3.3, 2.4, 1.25, g, g + 2.4, Metal, CARGO_TINT);
    w.sym_abs(-5.2, -3.9, 1.5, 1.1, g, g + 1.3, Metal, CARGO_TINT);
    // Wreckage outside, each on its own levelled pad.
    for p in PIECES {
        let ground = w.height(p.x, p.z);
        w.sym_abs(p.x, p.z, p.hx, p.hz, ground - 0.4, ground + p.height, Metal, DEBRIS_TINT);
    }
    for (x, z, h) in STUMPS {
        let low = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .into_iter()
            .map(|(sx, sz)| w.height(x + sx * 0.28, z + sz * 0.28))
            .fold(f32::MAX, f32::min);
        w.sym_abs(x, z, 0.28, 0.28, low - 0.6, w.height(x, z) + h, Trunk, STUMP_TINT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glam::{vec3, Vec3};

    const SEEDS: [u64; 4] = [0, 17, 0xFE221, u64::MAX];

    #[test]
    fn wreck_is_balanced_and_has_grounded_clear_openings() {
        for seed in SEEDS {
            let world = World::with_map(seed, MAP_ID);
            let g = floor(&world);
            let wreck: Vec<_> = world.blocks.iter().filter(|b| owns(b)).collect();
            assert_eq!(wreck.len(), 45);
            for b in &wreck {
                assert!(b.min.is_finite() && b.max.is_finite() && b.min.cmplt(b.max).all());
                assert!(
                    wreck.iter().any(|q| {
                        (q.min.x + b.max.x).abs() < 0.001
                            && (q.max.x + b.min.x).abs() < 0.001
                            && (q.min.z + b.max.z).abs() < 0.001
                            && (q.max.z + b.min.z).abs() < 0.001
                            && (q.min.y - b.min.y).abs() < 0.001
                            && (q.max.y - b.max.y).abs() < 0.001
                            && q.tint == b.tint
                            && q.kind == b.kind
                    }),
                    "seed {seed:x}: unbalanced wreck block at {:?}",
                    b.min
                );
            }
            // The terrain itself is mirrored, so the furrow, banks and
            // bow waves give neither team an edge.
            for (x, z) in [(30.0, 12.0), (57.0, 9.0), (70.0, -2.0), (-12.0, 18.0), (35.0, 16.0)] {
                assert!((world.height(x, z) - world.height(-x, -z)).abs() < 0.001);
            }
            // A level deck and level ground in every opening.
            for x in [-FLAT_X, -30.0, 0.0, 30.0, FLAT_X] {
                for z in [-FLAT_Z, 0.0, FLAT_Z] {
                    assert!((world.height(x, z) - g).abs() < 0.01, "seed {seed:x}: uneven deck at {x},{z}");
                }
            }
            for x in SIDE_DOORS {
                assert!(world.los(vec3(x, g + 1.7, -FLAT_Z), vec3(x, g + 1.7, FLAT_Z), 0));
            }
            let roof = world.raycast(vec3(0.0, g + 1.7, 0.0), Vec3::Y, 10.0, 0).unwrap();
            assert!((roof.t - (ROOF_HEIGHT - 1.7)).abs() < 0.01);
            // The hull sits in its furrow: the saddle stands above the deck
            // beside the centre tears.
            assert!(world.height(0.0, 16.0) > g + 1.0 && world.height(0.0, -16.0) > g + 1.0, "seed {seed:x}");
        }
    }

    #[test]
    fn crash_scar_blends_into_the_ridge_without_a_mesa() {
        // The old pad ended in ~36 degree banks within 8 m. Both broken ends
        // now run out along an even grade; the flanks are a gentler skirt
        // except where the furrow cuts through the ridge spine itself.
        for seed in SEEDS {
            let world = World::with_map(seed, MAP_ID);
            let steepest = |xs: (f32, f32), zs: (f32, f32)| {
                let mut worst: (f32, f32, f32) = (0.0, 0.0, 0.0);
                let mut x = xs.0;
                while x <= xs.1 {
                    let mut z = zs.0;
                    while z <= zs.1 {
                        for (x, z) in [(x, z), (-x, -z)] {
                            let n = world.terrain_normal(x, z);
                            let slope = n.y.acos().to_degrees();
                            if slope > worst.0 {
                                worst = (slope, x, z);
                            }
                        }
                        z += 1.0;
                    }
                    x += 1.0;
                }
                worst
            };
            let ends = steepest((FLAT_X, SCAR_LENGTH), (-16.0, 16.0));
            let flanks = steepest((-FLAT_X, FLAT_X), (FLAT_Z, 20.0));
            println!("seed {seed:x}: ends {ends:?} flanks {flanks:?}");
            assert!(ends.0 < 26.0, "seed {seed:x}: {:.1} degree slope at the end {},{}", ends.0, ends.1, ends.2);
            assert!(flanks.0 < 36.0, "seed {seed:x}: {:.1} degree slope at the flank {},{}", flanks.0, flanks.1, flanks.2);
        }
    }
}
