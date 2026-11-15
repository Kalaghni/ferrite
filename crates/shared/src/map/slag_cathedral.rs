//! Slag Cathedral: an open casting nave, furnace courts and overhead galleries.
//! Shared anchors keep decorative steelwork aligned with authoritative collision.

use super::{shift, BlockKind, World, SPREAD};

pub const MAP_ID: u8 = 6;
pub const SECTOR_NAMES: [&str; 5] = ["WEST STOCKYARD", "BOILER COURT", "CRUCIBLE NAVE", "BOILER COURT", "EAST STOCKYARD"];
/// The nave, its galleries, furnaces and lava stand where they were first
/// built; the courts, yards and bases beyond them moved out with the spread.
pub const POINTS: [(f32, f32); 5] = [(-117.0, -27.0), (-60.0, 33.0), (0.0, 0.0), (60.0, -33.0), (117.0, 27.0)];
pub const NAVE_HALF_X: f32 = 34.0;
pub const NAVE_HALF_Z: f32 = 22.0;
pub const NAVE_RIB_X: [f32; 6] = [-30.0, -18.0, -6.0, 6.0, 18.0, 30.0];
pub const NAVE_RIB_Z: f32 = 18.0;
pub const NAVE_EAVE_Y: f32 = 24.0;
pub const NAVE_RIDGE_Y: f32 = 30.0;
pub const RIB_FOOT_HALF_X: f32 = 1.4;
pub const RIB_FOOT_HALF_Z: f32 = 1.5;
pub const RIB_COLUMN_HALF_X: f32 = 0.8;
pub const RIB_COLUMN_HALF_Z: f32 = 0.9;
pub const FURNACE_ANCHORS: [(f32, f32); 2] = [(-18.0, -32.0), (18.0, 32.0)];
pub const FURNACE_HEIGHT: f32 = 38.0;
pub const FURNACE_HALF: f32 = 4.8;
pub const FURNACE_SHAFT_HALF: f32 = 4.1;
pub const FURNACE_BASE_Y: f32 = 7.0;
pub const GALLERY_HALF_X: f32 = 26.0;
pub const GALLERY_Z: f32 = 15.0;
pub const GALLERY_HALF_Z: f32 = 1.9;
pub const GALLERY_Y: f32 = 3.6;
pub const GALLERY_BOTTOM: f32 = 3.25;
pub const GALLERY_RAIL_INNER_Z: f32 = 13.2;
pub const GALLERY_RAIL_OUTER_Z: f32 = 16.8;
pub const GALLERY_RAIL_HALF: f32 = 0.055;
pub const GALLERY_RAIL_HEIGHTS: [f32; 2] = [GALLERY_Y + 0.5, GALLERY_Y + 1.0];
/// Open inner gates beside the ribs; all four stair-end entries stay clear.
pub const GALLERY_INNER_RAIL_SPANS: [(f32, f32); 5] =
    [(-26.0, -19.2), (-16.8, -7.2), (-4.8, 4.8), (7.2, 16.8), (19.2, 26.0)];
pub const STAIR_START_X: f32 = 34.4;
pub const STAIR_TREAD: f32 = 0.7;
pub const STAIR_RISE: f32 = 0.3;
pub const STAIR_STEPS: usize = 12;
pub const STAIR_HALF_Z: f32 = 1.7;
pub const SERVICE_Z: f32 = 39.0;
pub const SERVICE_HALF_Z: f32 = 3.0;
pub const SERVICE_HALF_X: f32 = 48.0;
pub const SERVICE_ROOF_Y: f32 = 4.8;
pub const SERVICE_ROOF_BOTTOM: f32 = 4.5;
pub const PIPE_YARD_Z: f32 = 50.0 * SPREAD;
/// Uprights of the exterior pipe racks, west half.
pub const PIPE_YARD_X: [f32; 6] = [-132.0, -108.0, -84.0, -60.0, -36.0, -12.0];
pub const PIPE_RACK_Y: f32 = 6.2;
/// West-authored casting machinery, mirrored by the builder.
pub const MACHINE_ANCHORS: [(f32, f32); 5] = [(-22.0, -6.0), (-10.0, 6.5), (-70.5, -12.0), (-96.0, 18.0), (-132.0, 7.5)];
/// (centre x, half width): repeated side-wall piers leave wide lateral doors.
pub const SIDE_WALL_SEGMENTS: [(f32, f32); 3] = [(-30.0, 4.0), (-18.0, 3.0), (-6.0, 2.0)];
/// The bell yards: a great bell stands in its square casting flask beneath a
/// lifting gantry in the south-west yard, mirrored to the north-east. The
/// flask and the bell's body are stacked boxes; broken mould halves lie about.
pub const BELL_YARD: (f32, f32) = (-126.0, -66.0);
pub const BELL_FLASK_HALF: f32 = 2.2;
pub const BELL_FLASK_H: f32 = 1.6;
pub const BELL_BODY_HALF: f32 = 1.5;
pub const BELL_H: f32 = 5.0;
pub const BELL_GANTRY_HALF: f32 = 3.9;
pub const BELL_GANTRY_Y: f32 = 8.4;
pub const BELL_GANTRY_BEAM: f32 = 0.6;
/// (centre x, centre z, half x, half z, height) of the west yard's mould halves.
pub const MOULD_FRAGMENTS: [(f32, f32, f32, f32, f32); 3] =
    [(-118.5, -62.5, 1.6, 0.5, 0.9), (-133.0, -63.0, 0.5, 1.7, 1.1), (-120.4, -68.9, 1.4, 0.5, 0.7)];
/// Exterior service blocks (west half): low cover in the yards.
pub const SERVICE_BLOCKS: [(f32, f32); 4] = [(-89.0 * SPREAD, 35.0 * SPREAD), (-67.0 * SPREAD, -44.0 * SPREAD), (-52.0 * SPREAD, 48.0 * SPREAD), (-6.0 * SPREAD, -52.0 * SPREAD)];
/// How far the west base's buildings, the stockyard at A and the boiler
/// court at B moved when the map was spread out.
pub const BASE_SHIFT: (f32, f32) = shift(-106.0, 0.0);
pub const STOCKYARD_SHIFT: (f32, f32) = shift(-78.0, -18.0);
pub const COURT_SHIFT: (f32, f32) = shift(-40.0, 22.0);
/// Molten metal is tapped from each furnace into an overhead launder that
/// crosses the gallery and pours into a lava channel here (and at the mirror).
pub const LAVAFALL: (f32, f32) = (-23.6, -LAVA_Z);
/// Two open lava channels run the length of the casting floor at z = +-LAVA_Z.
/// The spans are the molten stretches; between them the channel is bridged by
/// steel grating and is safe to walk. The set is its own mirror image, so the
/// same spans serve both channels under the map's half-turn symmetry.
pub const LAVA_Z: f32 = 8.6;
pub const LAVA_HALF_W: f32 = 0.65;
pub const LAVA_SPANS: [(f32, f32); 4] = [(-28.0, -15.5), (-7.5, -4.0), (4.0, 7.5), (15.5, 28.0)];
/// Feet this close to the floor are in the melt; a jump clears a channel.
pub const LAVA_REACH: f32 = 0.3;
/// Health burned away per second while standing in a channel.
pub const LAVA_DPS: f32 = 40.0;

/// Whether (x, z) lies in molten lava, with the footprint grown by `margin`.
pub fn in_lava(x: f32, z: f32, margin: f32) -> bool {
    (z.abs() - LAVA_Z).abs() < LAVA_HALF_W + margin
        && LAVA_SPANS.iter().any(|&(x0, x1)| x > x0 - margin && x < x1 + margin)
}

pub(super) fn build(w: &mut World, rng: &mut crate::rng::Rng) {
    use BlockKind::*;

    // Rear walls sit beyond the entire eight-metre spawn disc. Two open
    // staging bays shelter each base without funnelling it into one exit.
    let (ox, oz) = BASE_SHIFT;
    w.sym(-115.5 + ox, oz, 0.6, 4.5, 0.0, 5.0, Concrete, 1);
    w.sym(-115.5 + ox, oz, 0.22, 0.22, 5.0, 7.0, Metal, 1);
    for z in [-15.0f32, 15.0] {
        w.sym(-98.0 + ox, z + oz, 4.0, 1.0, 0.0, 2.6, Concrete, 2);
        w.sym(-94.5 + ox, z + oz, 0.5, 1.25, 2.6, 2.8, Metal, 0);
        w.sym(-101.0 + ox, z + oz, 0.3, 0.3, 2.6, 2.8, Metal, 0);
        w.sym(-98.0 + ox, z + oz, 4.4, 1.5, 5.4, 0.25, Metal, 1);
    }

    // A / E: open stockyard portals. The capture disc stays on bare ground.
    let (ox, oz) = STOCKYARD_SHIFT;
    w.sym(-78.0 + ox, -24.5 + oz, 5.5, 0.5, 0.0, 2.4, Concrete, 2);
    w.sym(-80.5 + ox, -11.5 + oz, 3.0, 0.5, 0.0, 1.15, Concrete, 1);
    w.sym(-70.5 + ox, -20.0 + oz, 0.45, 3.0, 0.0, 1.1, Sandbag, 0);
    for x in [-85.0f32, -71.0] {
        w.sym(x + ox, -18.0 + oz, 0.55, 0.55, 0.0, 6.0, Metal, 0);
    }
    w.sym(-78.0 + ox, -18.0 + oz, 7.6, 0.6, 6.0, 0.65, Metal, 1);
    w.crates(rng, -85.0 + ox, -25.0 + oz, 3);

    // B / D: boiler courts beside the nave. Offset shelter walls leave
    // several entrances, including the approach to the gallery stairs.
    let (ox, oz) = COURT_SHIFT;
    w.sym(-47.0 + ox, 22.0 + oz, 0.5, 5.0, 0.0, 3.6, Concrete, 1);
    w.sym(-40.0 + ox, 29.0 + oz, 6.0, 0.5, 0.0, 3.6, Concrete, 2);
    w.sym(-34.0 + ox, 24.0 + oz, 0.45, 2.0, 0.0, 1.1, Sandbag, 0);
    w.sym(-40.0 + ox, 16.0 + oz, 3.5, 0.45, 0.0, 1.1, Concrete, 2);
    w.sym(-45.0 + ox, 26.0 + oz, 1.2, 1.0, 0.0, 1.8, Metal, 1);
    w.sym(-40.0 + ox, 27.5 + oz, 6.0, 2.0, 5.7, 0.35, Metal, 0);

    // Monumental rib bases and columns. Above shoulder height the nave
    // remains completely open; the roof's angled steelwork is visual mesh.
    for x in NAVE_RIB_X {
        for z in [-NAVE_RIB_Z, NAVE_RIB_Z] {
            w.add_abs(x, z, RIB_FOOT_HALF_X, RIB_FOOT_HALF_Z, -0.25, 1.2, Concrete, 1);
            w.add_abs(x, z, RIB_COLUMN_HALF_X, RIB_COLUMN_HALF_Z, 1.2, NAVE_EAVE_Y, Metal, 0);
        }
    }
    for z in [-NAVE_HALF_Z, NAVE_HALF_Z] {
        for (x, hx) in SIDE_WALL_SEGMENTS {
            w.sym_abs(x, z, hx, 0.6, -0.25, 9.0, Concrete, 1);
        }
    }
    w.sym_abs(0.0, NAVE_HALF_Z, NAVE_HALF_X, 0.6, 9.0, NAVE_EAVE_Y, Concrete, 2);
    // Grand end portals: clear 22m-wide centre gates and open side entries.
    w.sym_abs(-NAVE_HALF_X, 11.8, 0.9, 0.8, -0.25, 12.0, Concrete, 2);
    w.sym_abs(-NAVE_HALF_X, -11.8, 0.9, 0.8, -0.25, 12.0, Concrete, 2);
    w.sym_abs(-NAVE_HALF_X, 0.0, 0.8, 12.6, 12.0, 15.0, Metal, 0);

    // Human-accessible side galleries. Twelve 30cm rises are comfortably
    // within auto-step reach; bots have a 3.25m-clear ground route beneath.
    w.sym_abs(0.0, GALLERY_Z, GALLERY_HALF_X, GALLERY_HALF_Z, GALLERY_BOTTOM, GALLERY_Y, Metal, 1);
    for z in [-GALLERY_Z, GALLERY_Z] {
        for i in 0..STAIR_STEPS {
            let x = -STAIR_START_X + (i as f32 + 0.5) * STAIR_TREAD;
            w.sym_abs(x, z, STAIR_TREAD * 0.5, STAIR_HALF_Z, -0.2, STAIR_RISE * (i + 1) as f32, Step, (i % 2) as u8);
        }
    }
    // Physical mid/top rail beams, never a full invisible wall. Gates in
    // the inner edge line up with the visible openings beside the ribs.
    for y in GALLERY_RAIL_HEIGHTS {
        w.sym_abs(0.0, GALLERY_RAIL_OUTER_Z, GALLERY_HALF_X, GALLERY_RAIL_HALF,
            y - GALLERY_RAIL_HALF, y + GALLERY_RAIL_HALF, Metal, 0);
        for (x0, x1) in GALLERY_INNER_RAIL_SPANS {
            w.sym_abs((x0 + x1) * 0.5, GALLERY_RAIL_INNER_Z, (x1 - x0) * 0.5, GALLERY_RAIL_HALF,
                y - GALLERY_RAIL_HALF, y + GALLERY_RAIL_HALF, Metal, 0);
        }
    }
    // Gallery supports lie on the outside edge, preserving the aisle below.
    for x in [-24.0f32, -12.0, 0.0, 12.0, 24.0] {
        w.sym_abs(x, GALLERY_Z + GALLERY_HALF_Z - 0.15, 0.15, 0.15, -0.1, GALLERY_BOTTOM, Metal, 0);
    }

    // Two furnace towers flank the cathedral, each high enough to orient
    // players from a base. Base and shaft match the renderer's dimensions.
    let (fx, fz) = FURNACE_ANCHORS[0];
    w.sym_abs(fx, fz, FURNACE_HALF, FURNACE_HALF, -0.3, FURNACE_BASE_Y, Concrete, 0);
    w.sym_abs(fx, fz, FURNACE_SHAFT_HALF, FURNACE_SHAFT_HALF, FURNACE_BASE_Y, FURNACE_HEIGHT, Metal, 0);
    w.sym_abs(fx, fz, FURNACE_SHAFT_HALF + 0.35, FURNACE_SHAFT_HALF + 0.35, FURNACE_HEIGHT - 0.5, FURNACE_HEIGHT, Metal, 1);

    // Mould beds and heavy presses interrupt the long casting aisle. None
    // sits inside a capture pad; low edges can be climbed or crouched behind.
    for (i, (x, z)) in MACHINE_ANCHORS.into_iter().enumerate() {
        w.sym(x, z, 2.2, 1.2, 0.0, 0.95, Concrete, (i % 3) as u8);
        w.sym(x - 1.0, z, 0.9, 1.0, 0.95, 1.35, Metal, 0);
        w.sym(x + 1.5, z, 0.45, 1.3, 0.0, 1.25, Metal, 1);
    }
    w.sym(-5.5, -7.0, 1.8, 0.45, 0.0, 1.0, Sandbag, 0);
    w.crates(rng, -11.5, -9.0, 3);
    w.crates(rng, -87.0, -9.0, 4);

    // Roofed service passages stay open along their full inner side.
    // Furnace shoulders intrude slightly into one edge, leaving >4m clear.
    w.sym_abs(0.0, SERVICE_Z, SERVICE_HALF_X, SERVICE_HALF_Z, SERVICE_ROOF_BOTTOM, SERVICE_ROOF_Y, Metal, 1);
    for z in [-SERVICE_Z, SERVICE_Z] {
        for x in [-44.0f32, -28.0, -12.0] {
            w.sym_abs(x, z + z.signum() * SERVICE_HALF_Z, 0.22, 0.22, -0.15, SERVICE_ROOF_BOTTOM, Metal, 0);
        }
    }

    // Exterior pipe yards offer a real third lane rather than a dead end.
    // Most pipework is overhead; service blocks provide spaced ground cover.
    for x in PIPE_YARD_X {
        w.sym(x, PIPE_YARD_Z, 0.3, 0.3, 0.0, PIPE_RACK_Y, Metal, 0);
        w.sym(x, PIPE_YARD_Z, 0.3, 2.4, PIPE_RACK_Y, 0.4, Metal, 1);
    }
    let (x0, x1) = (PIPE_YARD_X[0], PIPE_YARD_X[PIPE_YARD_X.len() - 1]);
    w.sym((x0 + x1) * 0.5, PIPE_YARD_Z, (x1 - x0) * 0.5 + 0.6, 0.4, PIPE_RACK_Y + 0.4, 0.4, Metal, 0);
    for (x, z) in SERVICE_BLOCKS {
        w.sym(x, z, 2.4, 1.1, 0.0, 1.2, Concrete, 1);
        w.sym(x, z, 1.25, 0.85, 1.2, 1.0, Metal, 0);
    }
    w.crates(rng, -102.0, 55.5, 3);

    // Bell yards fill the two quiet corners with a landmark and hard cover.
    // Added after every crate cluster so earlier placements are unchanged.
    let (bx, bz) = BELL_YARD;
    w.sym(bx, bz, BELL_FLASK_HALF, BELL_FLASK_HALF, 0.0, BELL_FLASK_H, Concrete, 3);
    w.sym(bx, bz, BELL_BODY_HALF, BELL_BODY_HALF, BELL_FLASK_H, BELL_H - BELL_FLASK_H, Metal, 2);
    for dx in [-BELL_GANTRY_HALF, BELL_GANTRY_HALF] {
        w.sym(bx + dx, bz, 0.3, 0.3, 0.0, BELL_GANTRY_Y, Metal, 0);
    }
    w.sym(bx, bz, BELL_GANTRY_HALF + 0.5, 0.4, BELL_GANTRY_Y, BELL_GANTRY_BEAM, Metal, 1);
    for (x, z, hx, hz, h) in MOULD_FRAGMENTS {
        w.sym(x, z, hx, hz, 0.0, h, Concrete, 0);
    }

    // The works beyond the cathedral: ingot stores and pattern shops,
    // open casting sheds, gas holders and slag-heap lookouts.
    w.scatter_buildings(rng, 5, &[(7.5, 5.0, 5.2), (5.0, 3.5, 4.2)], Metal);
    w.scatter_sheds(rng, 4);
    w.scatter_tanks(rng, 5);
    w.scatter_lookouts(rng, 2, 4.2);
    w.forest(rng, 0, 130, 1.0);
}
