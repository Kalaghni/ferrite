//! Spillway: drained waterworks along an east-west channel, below the Great
//! Dam. When the dam's sirens are tested its outlet gates open and water
//! surges out into the channel at C, spreads along it both ways, holds a
//! while and drains (`crate::events::flood`). Deep water sweeps players off
//! their feet: it hurts, and kills those who stay in it.
//!
//! C stands above the water on the gate deck: a concrete platform on piers
//! in the channel, reached by a bridge from each bank and a stair up from
//! the channel floor at each end. The bridges land on the headwalls of the
//! two outfalls the flood pours from. Everything here is mirrored under a
//! half turn, and is the same distance from both bases.

use super::{Block, BlockKind, World};
use crate::events::{smooth, Flood};
use glam::{vec2, vec3, Vec2, Vec3};

pub const MAP_ID: u8 = 4;
/// Full depth of the flood over the middle of the channel's floor.
pub const DEPTH: f32 = 1.9;
/// The water thins out to nothing this far along the channel either way
/// from C, well short of the bases.
pub const FLOOD_X: f32 = 98.0;
/// The channel is never wider than this either side of its centre line.
pub const FLOOD_HALF_Z: f32 = 26.0;
/// Water this shallow round someone's feet does them no harm.
pub const WADE: f32 = 0.3;
/// Health a strong flood takes per second from someone standing in it.
pub const WATER_DPS: f32 = 30.0;

/// The gate deck is a square this far either side of C, which covers the
/// whole capture disc.
pub const DECK_HALF: f32 = 10.0;
/// Thickness of the deck and bridge slabs.
pub const SLAB: f32 = 0.35;
/// The bridges run north and south along x = 0, this wide either side.
pub const BRIDGE_HALF: f32 = 1.6;
/// The stairs run east and west along z = 0, this wide either side.
pub const STAIR_HALF: f32 = 1.4;
pub const RISE: f32 = 0.3;
pub const TREAD: f32 = 0.55;
/// Rails, like Slag Cathedral's galleries: two thin beams, at these
/// heights above the walking surface.
pub const RAIL_HALF: f32 = 0.055;
pub const RAIL_HEIGHTS: [f32; 2] = [0.5, 1.0];
/// Rails stand this far in from the edge they guard.
pub const RAIL_INSET: f32 = 0.15;
/// Each outfall's headwall: half its length along the bank, thickness,
/// and height above the channel floor.
pub const HEADWALL_HALF: f32 = 7.5;
pub const HEADWALL_T: f32 = 0.8;
pub const HEADWALL_H: f32 = 2.6;
/// Two culvert mouths in each headwall, either side of the bridge: centre
/// (|x|), half width and height. Their sluice gates are scenery that lifts
/// as the flood is released; solid concrete fills the culvert behind.
pub const CULVERT_X: f32 = 4.6;
pub const CULVERT_HALF: f32 = 1.3;
pub const CULVERT_H: f32 = 1.8;
/// Columns under the deck's corners stand at (+-COLUMN, +-COLUMN).
pub const COLUMN: f32 = 8.6;
/// The old floodgate hangs lifted in a portal frame over the deck, this
/// far above it: head clearance everywhere under the frame.
pub const GATE_CLEAR: f32 = 3.3;
/// The portal frame's legs stand at (+-x, +-z) on the deck.
pub const PORTAL: (f32, f32) = (4.5, 9.3);

/// Where C's structure meets this seed's ground. A pure function of the
/// terrain, so the server, the client's prediction and its art agree.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The channel floor at C.
    pub floor: f32,
    /// Top of the deck and bridges: about level with the bank tops, so the
    /// bridges meet the banks, and always well above a full flood.
    pub deck: f32,
    /// |z| of the outfall headwalls' faces, where the banks start to rise.
    pub headwall: f32,
    /// Top of the headwalls and the terraces filled in behind them.
    pub terrace: f32,
    /// |z| where each terrace meets its bank.
    pub terrace_end: f32,
    /// |z| where each bridge meets its bank.
    pub bridge_end: f32,
    /// |z| where the bridge rails end: the bank beside the bridge is
    /// within a short climb of it from there.
    pub rails_end: f32,
    /// Tops of the stair steps, from the deck edge outward; step `j`
    /// spans |x| from `DECK_HALF + j * TREAD` to `DECK_HALF + (j + 1) * TREAD`.
    pub steps: Vec<f32>,
}

impl Layout {
    /// |x| where the stairs reach the channel floor.
    pub fn stair_end(&self) -> f32 {
        DECK_HALF + self.steps.len() as f32 * TREAD
    }
}

/// Smallest |z| at or past `from` (in quarter metres) where `f` holds.
fn first(from: f32, f: impl Fn(f32) -> bool) -> f32 {
    let mut z = from;
    while z < 60.0 && !f(z) {
        z += 0.25;
    }
    z
}

pub fn layout(w: &World) -> Layout {
    let floor = w.height(0.0, 0.0);
    let bank = (0..=48).map(|k| w.height(0.0, 24.0 + k as f32 * 0.5)).fold(f32::MIN, f32::max);
    let deck = floor + (bank - floor - 0.15).clamp(3.0, 4.0);
    let headwall = first(14.0, |z| w.height(0.0, z) >= floor + 0.25);
    let terrace = (floor + HEADWALL_H).min(deck - SLAB);
    let terrace_end = first(headwall + HEADWALL_T, |z| [-HEADWALL_HALF, 0.0, HEADWALL_HALF].iter().all(|&x| w.height(x, z) >= terrace));
    let bridge_end = first(headwall, |z| w.height(0.0, z) >= deck - 0.3);
    let side = BRIDGE_HALF + 0.65;
    let rails_end = first(headwall + HEADWALL_T, |z| w.height(side, z).min(w.height(-side, z)) >= deck - 1.0).min(bridge_end);
    let mut steps = Vec::new();
    for j in 0..20 {
        let top = deck - RISE * (j + 1) as f32;
        let (x0, x1) = (DECK_HALF + j as f32 * TREAD, DECK_HALF + (j + 1) as f32 * TREAD);
        let ground = [x0, x1].iter().flat_map(|&x| [-STAIR_HALF, STAIR_HALF].map(|z| w.height(x, z))).fold(f32::MIN, f32::max);
        if top <= ground + 0.02 {
            break;
        }
        steps.push(top);
    }
    Layout { floor, deck, headwall, terrace, terrace_end, bridge_end, rails_end, steps }
}

/// The surface someone on C's deck, a bridge or a stair at (x, z) stands
/// on, if (x, z) is on one of them. The stairs report the highest step
/// within a stride, so the step ahead never looks like a wall to the bots'
/// navigation.
pub fn walkway(w: &World, x: f32, z: f32) -> Option<f32> {
    let (ax, az) = (x.abs(), z.abs());
    // Everything lies within the deck, the bridges or the stairs.
    if ax > DECK_HALF + 20.0 * TREAD || az > 60.0 || (ax > DECK_HALF && az > DECK_HALF) {
        return None;
    }
    if ax <= DECK_HALF && az <= DECK_HALF {
        return Some(w.points[2].y);
    }
    let l = layout(w);
    if ax <= BRIDGE_HALF && az <= l.bridge_end {
        return Some(l.deck.max(w.height(x, z)));
    }
    if az <= STAIR_HALF && ax <= l.stair_end() {
        let near = ax - 0.65 - DECK_HALF;
        let top = if near < 0.0 { l.deck } else { l.steps[((near / TREAD) as usize).min(l.steps.len() - 1)] };
        return Some(top.max(w.height(x, z)));
    }
    None
}

/// The direction a walker under the deck or a bridge span should go to get
/// out from under it (the bots' flow field describes the deck above), or
/// `want` unchanged anywhere else: toward open floor beside a stair, or
/// sideways from under a bridge.
pub fn steer(w: &World, pos: Vec3, want: Vec2) -> Vec2 {
    if !under(w, pos) {
        return want;
    }
    let sx = if pos.x < 0.0 { -1.0 } else { 1.0 };
    let sz = if pos.z < 0.0 { -1.0 } else { 1.0 };
    let to = if pos.z.abs() > DECK_HALF { vec2(sx, 0.0) } else { vec2(sx * (DECK_HALF + 3.5), sz * 3.0) - vec2(pos.x, pos.z) };
    to.normalize_or_zero() * want.length().max(1e-3)
}

/// Whether feet at `pos` are on the channel floor under the deck or a
/// bridge span, rather than on top.
pub fn under(w: &World, pos: Vec3) -> bool {
    w.map == MAP_ID && walkway(w, pos.x, pos.z).is_some_and(|top| pos.y < top - 1.2)
}

/// Whether feet at `pos` stand on the deck (or close above it), which is
/// what holding C on Spillway takes; the channel floor under it does not
/// count.
pub fn on_deck(w: &World, pos: Vec3) -> bool {
    let d = pos.y - w.points[2].y;
    pos.x.abs() <= DECK_HALF + 0.5 && pos.z.abs() <= DECK_HALF + 0.5 && d > -0.6 && d < 7.0
}

/// Water as deep as it ever gets: what the bots' flood routes avoid.
pub const FULL: Flood = Flood { gates: 1.0, level: 1.0, reach: 1.0e4, age: 0.0 };

/// The channel's floor along its centre line at `x`, smoothed so the water
/// surface runs evenly over the bumps.
pub fn channel_floor(w: &World, x: f32) -> f32 {
    (w.height(x - 3.0, 0.0) + w.height(x, 0.0) + w.height(x + 3.0, 0.0)) / 3.0
}

/// Height of the water surface over (x, z) in flood `f`, if the water has
/// reached that far along the channel. Says nothing about whether the ground
/// there is under it (see `World::water`).
pub fn water_surface(w: &World, x: f32, z: f32, f: Flood) -> Option<f32> {
    if f.level <= 0.0 || z.abs() > FLOOD_HALF_Z {
        return None;
    }
    let ax = x.abs();
    // Shallow at the leading edge of the surge and towards the ends.
    let depth = DEPTH * f.level * smooth(-2.0, 10.0, f.reach - ax) * (1.0 - smooth(FLOOD_X - 24.0, FLOOD_X, ax));
    (depth > 0.02).then(|| channel_floor(w, x) + depth)
}

/// C used to be a lifted floodgate on two piers. Its footprint stands in
/// while the rest of the map is scattered, so everything scattered stays
/// where it always was; `build` then puts the gate deck in its place.
/// Returns the stand-ins.
pub(super) fn reserve(w: &mut World) -> std::ops::Range<usize> {
    use BlockKind::*;
    let start = w.blocks.len();
    let gate = w.height(0.0, 0.0) + 7.8;
    for z in [-11.0f32 * super::SPREAD, 11.0 * super::SPREAD] {
        w.add_abs(0.0, z, 2.0, 1.3, w.height(0.0, z) - 0.6, gate, Concrete, 1);
        w.add_abs(0.0, z, 2.3, 1.55, gate, gate + 0.5, Concrete, 2);
        w.add_abs(0.0, z, 1.1, 1.0, gate + 0.5, gate + 1.4, Metal, 1);
    }
    w.add_abs(0.0, 0.0, 1.7, 11.0 * super::SPREAD + 1.6, gate - 0.4, gate + 0.25, Metal, 0);
    w.add_abs(0.0, 0.0, 0.35, 9.8 * super::SPREAD, gate - 2.3, gate - 0.4, Metal, 1);
    start..w.blocks.len()
}

/// C: the gate deck, its bridges, stairs and outfalls, in place of the
/// stand-ins `reserve` left. The few scattered things (crates, rocks, cover,
/// trees) that would stand in the structure or its approaches give way.
/// Everything here is mirrored, so whatever gives way does so with its
/// mirror.
pub(super) fn build(w: &mut World, stand_ins: std::ops::Range<usize>) {
    use BlockKind::*;
    w.blocks.drain(stand_ins);
    let l = layout(w);
    // Footprints to keep clear (min x, min z, max x, max z), north and east
    // halves; each is mirrored.
    let (se, zb, te) = (l.stair_end(), l.bridge_end, l.terrace_end + 0.5);
    // Low cover on the channel floor fits under the deck; trees do not.
    let deck = (-DECK_HALF - 0.5, -DECK_HALF - 0.5, DECK_HALF + 0.5, DECK_HALF + 0.5);
    let clear = [
        (-BRIDGE_HALF - 1.0, DECK_HALF, BRIDGE_HALF + 1.0, zb + 3.5),
        (-HEADWALL_HALF - 0.5, l.headwall - 1.0, HEADWALL_HALF + 0.5, te + 0.5),
        (DECK_HALF, -STAIR_HALF - 0.8, se + 3.0, STAIR_HALF + 0.8),
    ];
    let hit = |r: &[(f32, f32, f32, f32)], x0: f32, z0: f32, x1: f32, z1: f32| {
        r.iter().any(|&(a, b, c, d)| [(a, b, c, d), (-c, -d, -a, -b)].iter().any(|&(a, b, c, d)| x1 > a && x0 < c && z1 > b && z0 < d))
    };
    w.blocks.retain(|b| {
        !hit(&clear, b.min.x, b.min.z, b.max.x, b.max.z) && !(b.kind == Trunk && hit(&[deck], b.min.x, b.min.z, b.max.x, b.max.z))
    });
    w.trees.retain(|t| {
        let (x, z) = (t.base.x, t.base.z);
        !hit(&clear, x - 0.3, z - 0.3, x + 0.3, z + 0.3) && !hit(&[deck], x - 0.3, z - 0.3, x + 0.3, z + 0.3)
    });
    // Nor may anything stand inside the structure itself.
    let parts = structure(w);
    w.blocks.retain(|b| !parts.iter().any(|p| b.min.cmplt(p.max).all() && p.min.cmplt(b.max).all()));
    // C's objective stands on the deck.
    w.points[2].y = l.deck;
    w.blocks.extend(parts);
}

/// Collision blocks under construction: added at absolute heights, alone
/// or with their mirror.
struct Parts(Vec<Block>);

impl Parts {
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, y0: f32, y1: f32, kind: BlockKind, tint: u8) {
        self.0.push(Block { min: vec3(cx - hx, y0, cz - hz), max: vec3(cx + hx, y1, cz + hz), kind, tint, dest: None });
    }

    #[allow(clippy::too_many_arguments)]
    fn sym(&mut self, cx: f32, cz: f32, hx: f32, hz: f32, y0: f32, y1: f32, kind: BlockKind, tint: u8) {
        self.add(cx, cz, hx, hz, y0, y1, kind, tint);
        self.add(-cx, -cz, hx, hz, y0, y1, kind, tint);
    }
}

/// C's collision: the deck, piers, bridges, outfalls, stairs, rails, cover
/// and the gate frame, exactly as `build` adds them (the client draws them
/// itself).
pub fn structure(w: &World) -> Vec<Block> {
    use BlockKind::*;
    let l = layout(w);
    let (f, y) = (l.floor, l.deck);
    let mut p = Parts(Vec::new());
    let low = |w: &World, cx: f32, cz: f32, hx: f32, hz: f32| w.ground_under(cx, cz, hx, hz) - 0.6;

    // The deck on two pier walls, their long sides with the current, and
    // a column under each corner.
    p.add(0.0, 0.0, DECK_HALF, DECK_HALF, y - SLAB, y, Concrete, 1);
    let pier = low(w, 0.0, 5.0, 3.5, 0.6);
    p.sym(0.0, 5.0, 3.5, 0.6, pier, y - SLAB, Concrete, 2);
    for (x, z) in [(COLUMN, COLUMN), (COLUMN, -COLUMN)] {
        let foot = low(w, x, z, 0.4, 0.4);
        p.sym(x, z, 0.4, 0.4, foot, y - SLAB, Concrete, 2);
    }

    // Bridges to the banks: a span over the channel to the headwall, then
    // a causeway up the bank on the headwall's back.
    let (zb, zh) = (l.bridge_end + 1.5, l.headwall);
    p.sym(0.0, (DECK_HALF + zb) * 0.5, BRIDGE_HALF, (zb - DECK_HALF) * 0.5, y - SLAB, y, Concrete, 1);
    let cz = (zh + zb) * 0.5;
    let causeway = low(w, 0.0, cz, BRIDGE_HALF, (zb - zh) * 0.5);
    p.sym(0.0, cz, BRIDGE_HALF - 0.05, (zb - zh) * 0.5, causeway, y - SLAB, Concrete, 2);

    // The outfalls: a headwall in each bank under its bridge, with two
    // culvert mouths, and the terrace filled in behind it.
    let t = l.terrace;
    let wall = low(w, 0.0, zh + HEADWALL_T * 0.5, HEADWALL_HALF, HEADWALL_T * 0.5);
    let wz = zh + HEADWALL_T * 0.5;
    let (m0, m1) = (CULVERT_X - CULVERT_HALF, CULVERT_X + CULVERT_HALF);
    for (x0, x1) in [(-HEADWALL_HALF, -m1), (-m0, m0), (m1, HEADWALL_HALF)] {
        p.sym((x0 + x1) * 0.5, wz, (x1 - x0) * 0.5, HEADWALL_T * 0.5, wall, t, Concrete, 0);
    }
    for x in [-CULVERT_X, CULVERT_X] {
        p.sym(x, wz, CULVERT_HALF, HEADWALL_T * 0.5, f + CULVERT_H, t, Concrete, 0);
    }
    let te = l.terrace_end + 0.5;
    let fill = low(w, 0.0, (zh + HEADWALL_T + te) * 0.5, HEADWALL_HALF, (te - zh - HEADWALL_T) * 0.5);
    p.sym(0.0, (zh + HEADWALL_T + te) * 0.5, HEADWALL_HALF, (te - zh - HEADWALL_T) * 0.5, fill, t, Concrete, 0);
    // The sluice gates' headstocks on the terrace edge: cover up there.
    for x in [-CULVERT_X, CULVERT_X] {
        p.sym(x, zh + HEADWALL_T + 0.45, 1.0, 0.4, t, t + 1.15, Metal, 1);
    }

    // Stairs down from the deck's east and west edges to the channel floor.
    let foot = low(w, l.stair_end() * 0.5 + DECK_HALF * 0.5, 0.0, (l.stair_end() - DECK_HALF) * 0.5, STAIR_HALF);
    for (j, &top) in l.steps.iter().enumerate() {
        let x = DECK_HALF + (j as f32 + 0.5) * TREAD;
        p.sym(x, 0.0, TREAD * 0.5, STAIR_HALF, foot, top, Step, (j % 2) as u8);
    }

    // Rails along the deck's edges, open where the bridges and stairs meet
    // it; along both sides of each bridge until the bank is close beside
    // it; and beside the upper flight of each stair.
    let edge = DECK_HALF - RAIL_INSET;
    for h in RAIL_HEIGHTS {
        let (r0, r1) = (y + h - RAIL_HALF, y + h + RAIL_HALF);
        // North and south edges, either side of the bridge.
        let span = (DECK_HALF - BRIDGE_HALF) * 0.5;
        for x in [-(BRIDGE_HALF + span), BRIDGE_HALF + span] {
            p.sym(x, edge, span, RAIL_HALF, r0, r1, Metal, 0);
        }
        // East and west edges, either side of the stair.
        let span = (edge - STAIR_HALF) * 0.5;
        for z in [-(STAIR_HALF + span), STAIR_HALF + span] {
            p.sym(edge, z, RAIL_HALF, span, r0, r1, Metal, 0);
        }
        let x = BRIDGE_HALF - RAIL_HALF;
        let (z0, z1) = (DECK_HALF, l.rails_end);
        for s in [-x, x] {
            p.sym(s, (z0 + z1) * 0.5, RAIL_HALF, (z1 - z0) * 0.5, r0, r1, Metal, 0);
        }
        // Stair rails in two-step lengths, as far down as the drop beside
        // the stair is worth guarding.
        let z = STAIR_HALF + RAIL_HALF;
        for (k, pair) in l.steps.chunks(2).enumerate() {
            let top = pair[pair.len() - 1];
            if top < f + 1.0 {
                break;
            }
            let x0 = DECK_HALF + 2.0 * k as f32 * TREAD;
            let x1 = x0 + pair.len() as f32 * TREAD;
            for s in [-z, z] {
                p.sym((x0 + x1) * 0.5, s, (x1 - x0) * 0.5, RAIL_HALF, top + h - RAIL_HALF, top + h + RAIL_HALF, Metal, 0);
            }
        }
    }

    // Cover on the deck, clear of the ways on and off it.
    p.sym(-5.2, 3.6, 0.3, 1.6, y, y + 1.1, Concrete, 2);
    p.sym(-3.2, -5.0, 1.4, 0.45, y, y + 1.0, Sandbag, 0);
    p.sym(-7.0, -6.2, 0.9, 0.7, y, y + 1.5, Metal, 1);
    p.sym(-6.6, 6.6, 1.2, 0.8, y, y + 1.2, Metal, 0);

    // The old floodgate, lifted clear across the channel in its frame, with
    // the winding house on top.
    let (px, pz) = PORTAL;
    let top = gear_bottom(y);
    for x in [-px, px] {
        p.sym(x, pz, 0.25, 0.25, y, top, Metal, 1);
    }
    p.sym(px, 0.0, 0.25, pz + 0.25, top, top + 0.45, Metal, 1);
    p.sym(0.0, 6.0, px - 0.25, 0.3, top, top + 0.45, Metal, 1);
    p.add(0.0, 0.0, 0.35, pz - 0.5, y + GATE_CLEAR, top - 0.05, Metal, 0);
    p.add(0.0, 0.0, 1.2, 1.7, top + 0.45, top + 1.6, Metal, 2);
    p.0
}

/// Underside of the gate frame's girders over a deck at `deck`.
pub fn gear_bottom(deck: f32) -> f32 {
    deck + GATE_CLEAR + 1.9
}
