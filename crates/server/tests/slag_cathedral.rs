//! Geometry and physical route checks for Slag Cathedral. The existing
//! every_map_is_fair_and_playable soak also runs its full bot war on this map.

use ferrite_shared::glam::{vec2, vec3, Vec3};
use ferrite_shared::map::slag_cathedral as cathedral;
use ferrite_shared::map::{map_by_name, Block, Nav, World, HALF_X, HALF_Z, MAX_CRATES, NAV_GOALS, NAV_LANES};
use ferrite_shared::proto::Cmd;
use ferrite_shared::sim::{self, PState, P_H, P_R};

const SEEDS: [u64; 5] = [0, 1, 17, 0xFE221, u64::MAX];

fn world(seed: u64) -> World {
    let id = map_by_name("slag cathedral").expect("Slag Cathedral is selectable by name");
    assert_eq!(id, cathedral::MAP_ID, "map lookup and architecture disagree");
    assert_eq!(id, 6, "existing map ids must remain stable");
    World::with_map(seed, id)
}

/// A block as quantised corners, kind, tint and destruction bit.
type BlockPrint = ([u32; 3], [u32; 3], u8, u8, Option<u8>);

fn geometry(w: &World) -> Vec<BlockPrint> {
    w.blocks
        .iter()
        .map(|b| (b.min.to_array().map(f32::to_bits), b.max.to_array().map(f32::to_bits), b.kind as u8, b.tint, b.dest))
        .collect()
}

fn standing_overlaps(b: &Block, at: Vec3) -> bool {
    at.x + P_R > b.min.x && at.x - P_R < b.max.x && at.z + P_R > b.min.z && at.z - P_R < b.max.z && at.y + P_H > b.min.y && at.y < b.max.y
}

#[test]
fn cathedral_collision_and_landmarks_are_deterministic_and_mirrored() {
    assert_eq!(map_by_name("slag"), Some(6));
    assert_eq!(map_by_name("cathedral"), Some(6));
    for seed in SEEDS {
        let a = world(seed);
        let b = world(seed);
        assert_eq!(a.heights, b.heights, "seed {seed:x}: terrain is not deterministic");
        assert_eq!(geometry(&a), geometry(&b), "seed {seed:x}: collision geometry is not deterministic");
        let trees = |w: &World| {
            w.trees
                .iter()
                .map(|t| (t.base.to_array().map(f32::to_bits), t.height.to_bits(), t.radius.to_bits(), t.tint, t.pine))
                .collect::<Vec<_>>()
        };
        assert_eq!(trees(&a), trees(&b), "seed {seed:x}: surrounding forest is not deterministic");
        for tree in &a.trees {
            assert!(tree.base.is_finite() && tree.height.is_finite() && tree.height > 0.0 && tree.radius.is_finite() && tree.radius > 0.0);
            assert!(
                a.trees.iter().any(|other| (tree.base.x + other.base.x).abs() < 0.002
                    && (tree.base.z + other.base.z).abs() < 0.002
                    && (tree.base.y - other.base.y).abs() < 0.025
                    && tree.height == other.height
                    && tree.radius == other.radius
                    && tree.tint == other.tint
                    && tree.pine == other.pine),
                "seed {seed:x}: unmatched surrounding tree at {:?}",
                tree.base
            );
        }
        assert_eq!((a.points, a.bases, a.n_crates), (b.points, b.bases, b.n_crates));
        assert!(a.n_crates <= MAX_CRATES);
        assert!(a.heights.iter().all(|h| h.is_finite()));
        assert!(a.points[2].y.abs() < 0.001, "the central cathedral apron must remain level at y=0");
        for (i, p) in a.points.iter().enumerate() {
            let q = a.points[4 - i];
            assert!((p.x + q.x).abs() < 0.001 && (p.z + q.z).abs() < 0.001 && (p.y - q.y).abs() < 0.02);
        }
        for block in &a.blocks {
            assert!(block.min.is_finite() && block.max.is_finite() && block.min.cmplt(block.max).all());
            assert!(
                a.blocks.iter().any(|other| {
                    (other.min.x + block.max.x).abs() < 0.002
                        && (other.max.x + block.min.x).abs() < 0.002
                        && (other.min.z + block.max.z).abs() < 0.002
                        && (other.max.z + block.min.z).abs() < 0.002
                        && (other.min.y - block.min.y).abs() < 0.05
                        && (other.max.y - block.max.y).abs() < 0.05
                        && other.kind == block.kind
                        && other.tint == block.tint
                        && other.dest.is_some() == block.dest.is_some()
                }),
                "seed {seed:x}: unmatched {:?} at {:?}..{:?}",
                block.kind,
                block.min,
                block.max
            );
        }
        for x in (-116..=116).step_by(4) {
            for z in (-64..=64).step_by(4) {
                assert!(
                    (a.height(x as f32, z as f32) - a.height(-x as f32, -z as f32)).abs() < 0.025,
                    "seed {seed:x}: unfair ground at {x},{z}"
                );
            }
        }
        let max = a.blocks.iter().fold(Vec3::splat(f32::NEG_INFINITY), |p, b| p.max(b.max));
        let min = a.blocks.iter().fold(Vec3::splat(f32::INFINITY), |p, b| p.min(b.min));
        eprintln!(
            "Slag seed {seed:x}: {} collision blocks, {} crates, {} trees; extent {min:?}..{max:?}",
            a.blocks.len(),
            a.n_crates,
            a.trees.len()
        );
    }
}

#[test]
fn spawn_discs_and_capture_cores_have_standing_headroom() {
    for seed in SEEDS {
        let w = world(seed);
        let nav = Nav::build(&w);
        for (i, p) in w.bases.iter().chain(w.points.iter()).enumerate() {
            assert!(!nav.is_blocked(p.x, p.z), "seed {seed:x}: pad {i} blocks navigation");
            // Bases use the actual 8m spawn sampling disc. Objectives retain
            // a clear 3m standing core inside their wider capture radius.
            let radius: f32 = if i < 2 { 8.0 } else { 3.0 };
            let cells = (radius * 2.0) as i32;
            for dx in -cells..=cells {
                for dz in -cells..=cells {
                    let offset = vec2(dx as f32 * 0.5, dz as f32 * 0.5);
                    if offset.length() > radius {
                        continue;
                    }
                    let (x, z) = (p.x + offset.x, p.z + offset.y);
                    let at = vec3(x, w.height(x, z) + 0.02, z);
                    assert!(x.abs() + P_R < HALF_X && z.abs() + P_R < HALF_Z);
                    assert!(!w.blocks.iter().any(|b| standing_overlaps(b, at)), "seed {seed:x}: pad {i} traps a standing player at {at:?}");
                }
            }
        }
    }
}

#[test]
fn all_ground_flow_routes_reach_every_objective_and_base_across_seeds() {
    let mut longest = 0.0f32;
    let mut routes = 0;
    for seed in SEEDS {
        let w = world(seed);
        let nav = Nav::build(&w);
        for goal in 0..NAV_GOALS {
            let target = if goal < 5 { w.points[goal] } else { w.bases[goal - 5] };
            for start in w.bases.iter().chain(w.points.iter()) {
                for lane in 0..NAV_LANES {
                    let mut p = vec2(start.x, start.z);
                    let mut length = 0.0;
                    for _ in 0..1600 {
                        if p.distance(vec2(target.x, target.z)) < 6.5 {
                            break;
                        }
                        let d = nav.dir(goal, lane, p.x, p.y);
                        assert!(d.is_finite());
                        p += d * 0.6;
                        length += 0.6;
                    }
                    assert!(
                        p.distance(vec2(target.x, target.z)) < 6.5,
                        "seed {seed:x}: lane {lane} from {start:?} to goal {goal} stops at {p:?}"
                    );
                    longest = longest.max(length);
                    routes += 1;
                }
            }
        }
    }
    eprintln!("Slag ground flow: {routes} routes, longest {longest:.1}m");
}

#[test]
fn a_player_physically_walks_from_both_bases_to_the_nave_on_every_lane() {
    let mut longest = 0;
    for seed in SEEDS {
        let w = world(seed);
        let nav = Nav::build(&w);
        for base in 0..2 {
            for lane in 0..NAV_LANES {
                let mut st = PState::spawn(w.bases[base] + Vec3::Y * 0.02, 0);
                let mut ticks = 0;
                while ticks < 7200 && vec2(st.pos.x, st.pos.z).length() > 6.5 {
                    ticks += 1;
                    let d = nav.dir(2, lane, st.pos.x, st.pos.z);
                    let command = Cmd { seq: ticks, mz: 1, yaw: d.x.atan2(d.y), ..Default::default() };
                    sim::step(&mut st, &command, &w, 0);
                    assert!(st.pos.is_finite());
                    assert!(st.pos.y >= w.height(st.pos.x, st.pos.z) - 0.02, "fell beneath terrain");
                }
                assert!(
                    vec2(st.pos.x, st.pos.z).length() < 6.5,
                    "seed {seed:x}: physical route base {base}, lane {lane} stuck at {:?}",
                    st.pos
                );
                longest = longest.max(ticks);
            }
        }
    }
    eprintln!("Slag physical base-to-C routes: slowest {:.1}s", longest as f32 / 60.0);
}

#[test]
fn all_four_stairs_reach_and_cross_the_elevated_galleries_without_jumping() {
    assert!((cathedral::STAIR_START_X - cathedral::STAIR_STEPS as f32 * cathedral::STAIR_TREAD - cathedral::GALLERY_HALF_X).abs() < 0.001);
    const { assert!(cathedral::STAIR_RISE <= sim::STEP_H) };
    // Read the renderer/collision anchors rather than assuming screen-space positions.
    for seed in SEEDS {
        let w = world(seed);
        for z in [-cathedral::GALLERY_Z, cathedral::GALLERY_Z] {
            for side in [-1.0f32, 1.0] {
                let x = side * (cathedral::STAIR_START_X + 1.6);
                let start = vec3(x, w.height(x, z) + 0.02, z);
                let mut st = PState::spawn(start, 0);
                let yaw = -side * std::f32::consts::FRAC_PI_2;
                let mut reached = false;
                for seq in 1..=1200 {
                    sim::step(&mut st, &Cmd { seq, mz: 1, yaw, ..Default::default() }, &w, 0);
                    assert!(st.pos.is_finite());
                    if st.pos.x * (-side) > cathedral::GALLERY_HALF_X - 1.0 {
                        reached = true;
                        break;
                    }
                }
                assert!(
                    reached && (st.pos.y - cathedral::GALLERY_Y).abs() < 0.06,
                    "seed {seed:x}: gallery z{z}, entry side{side}: stopped at {:?}",
                    st.pos
                );
            }
        }
    }
}

#[test]
fn gallery_rails_stop_an_unjumping_player_at_both_edges() {
    for seed in SEEDS {
        let w = world(seed);
        for gallery_side in [-1.0f32, 1.0] {
            for direction in [-1.0f32, 1.0] {
                let start_z = gallery_side * cathedral::GALLERY_Z;
                let mut st = PState::spawn(vec3(0.0, cathedral::GALLERY_Y + 0.02, start_z), 0);
                for seq in 1..=12 {
                    sim::step(&mut st, &Cmd { seq, ..Default::default() }, &w, 0);
                }
                for seq in 13..=192 {
                    let yaw = if direction > 0.0 { 0.0 } else { std::f32::consts::PI };
                    sim::step(&mut st, &Cmd { seq, mz: 1, yaw, ..Default::default() }, &w, 0);
                }
                let moved = (st.pos.z - start_z) * direction;
                assert!(moved > 0.5, "rail probe did not walk toward its edge");
                assert!(
                    moved < cathedral::GALLERY_HALF_Z - P_R,
                    "seed {seed:x}: walked through gallery {gallery_side} edge {direction}: {:?}",
                    st.pos
                );
                assert!((st.pos.y - cathedral::GALLERY_Y).abs() < 0.06, "seed {seed:x}: fell through gallery rail: {:?}", st.pos);
            }
        }
    }
}

#[test]
fn intentional_inner_gallery_gates_remain_passable() {
    let spans = cathedral::GALLERY_INNER_RAIL_SPANS;
    for seed in SEEDS {
        let w = world(seed);
        for pair in spans.windows(2) {
            let x = (pair[0].1 + pair[1].0) * 0.5;
            for gallery_side in [-1.0f32, 1.0] {
                let start_z = gallery_side * cathedral::GALLERY_Z;
                let mut st = PState::spawn(vec3(x, cathedral::GALLERY_Y + 0.02, start_z), 0);
                let yaw = if gallery_side > 0.0 { std::f32::consts::PI } else { 0.0 };
                for seq in 1..=180 {
                    sim::step(&mut st, &Cmd { seq, mz: 1, yaw, ..Default::default() }, &w, 0);
                }
                assert!(
                    st.pos.z * gallery_side < cathedral::GALLERY_RAIL_INNER_Z - 2.0,
                    "seed {seed:x}: invisible rail blocks gate x{x}, gallery {gallery_side}: {:?}",
                    st.pos
                );
                assert!(st.pos.is_finite() && st.pos.y >= w.height(st.pos.x, st.pos.z) - 0.02);
            }
        }
    }
}

#[test]
fn lava_channels_are_mirrored_bridged_and_clear_of_cover_and_objectives() {
    let z = cathedral::LAVA_Z;
    // The span set is its own mirror, so both channels match under a half turn.
    for (a, b) in cathedral::LAVA_SPANS {
        assert!(a < b && b.abs().max(a.abs()) < cathedral::NAVE_HALF_X);
        assert!(cathedral::LAVA_SPANS.contains(&(-b, -a)), "span {a}..{b} has no mirror");
        let mid = (a + b) * 0.5;
        for side in [-z, z] {
            assert!(cathedral::in_lava(mid, side, 0.0));
            assert!(cathedral::in_lava(-mid, -side, 0.0));
            assert!(!cathedral::in_lava(mid, side + cathedral::LAVA_HALF_W + 0.05, 0.0));
        }
    }
    // Every bridge is wide enough for bots and safe along its whole width.
    for pair in cathedral::LAVA_SPANS.windows(2) {
        let (a, b) = (pair[0].1, pair[1].0);
        assert!(b - a >= 6.0, "bridge {a}..{b} is too narrow for the flow field");
        for side in [-z, z] {
            assert!(!cathedral::in_lava(a + 0.01, side, 0.0) && !cathedral::in_lava(b - 0.01, side, 0.0));
        }
    }
    for seed in SEEDS {
        let w = world(seed);
        // Only this map has lava; the hazard never reaches a pad.
        assert!(!World::with_map(seed, 0).lava(-20.0, z, 0.0));
        for p in w.bases.iter().chain(w.points.iter()) {
            for k in 0..64 {
                let a = k as f32 * std::f32::consts::TAU / 64.0;
                for r in [0.0f32, 3.0, 6.0] {
                    assert!(!w.lava(p.x + a.cos() * r, p.z + a.sin() * r, P_R));
                }
            }
        }
        // No cover stands in the melt, so nothing offers a safe perch in it.
        for b in &w.blocks {
            for (a, c) in cathedral::LAVA_SPANS {
                for side in [-z, z] {
                    let overlaps = b.max.x > a
                        && b.min.x < c
                        && b.max.z > side - cathedral::LAVA_HALF_W
                        && b.min.z < side + cathedral::LAVA_HALF_W
                        && b.min.y < 2.0;
                    assert!(!overlaps, "seed {seed:x}: {:?} at {:?} stands in lava", b.kind, b.min);
                }
            }
        }
        // Standing in it burns; a bridge, the bank and a jump above it do not.
        let at = |x: f32, y: f32, z: f32| vec3(x, w.height(x, z) + y, z);
        assert!(w.burning(at(-20.0, 0.02, z)) && w.burning(at(20.0, 0.02, -z)));
        assert!(!w.burning(at(0.0, 0.02, z)) && !w.burning(at(-11.0, 0.02, -z)));
        assert!(!w.burning(at(-20.0, 0.02, z + 1.0)));
        assert!(!w.burning(at(-20.0, cathedral::LAVA_REACH + 0.05, z)));
        let nav = Nav::build(&w);
        assert!(nav.is_blocked(-20.0, z) && nav.is_blocked(20.0, -z));
        assert!(!nav.is_blocked(0.0, z) && !nav.is_blocked(0.0, -z));
    }
}

#[test]
fn standing_in_lava_burns_a_player_to_death_and_is_reported_as_lava() {
    use ferrite_server::{Config, Game};
    use ferrite_shared::proto::Feed;
    let mut g = Game::new(Config {
        port: 0,
        per_team: 1,
        seed: 0xFE221,
        map: cathedral::MAP_ID,
        rotation: Vec::new(),
        lobby_secs: 0,
        tickets: 250,
        bot_skill: 1.0,
        log: false,
        discover: false,
        ..Default::default()
    });
    let victim = 0;
    let hp = g.players[victim].hp;
    let spot = vec3(-20.0, g.world.height(-20.0, cathedral::LAVA_Z) + 0.02, cathedral::LAVA_Z);
    let mut ticks = 0;
    while g.players[victim].alive && ticks < 600 {
        g.players[victim].st.pos = spot;
        g.players[victim].st.vel = Vec3::ZERO;
        g.tick();
        ticks += 1;
    }
    assert!(!g.players[victim].alive, "lava never killed a {hp} hp player");
    let expected = hp / cathedral::LAVA_DPS * 60.0;
    assert!((ticks as f32) > expected * 0.8 && (ticks as f32) < expected * 1.2 + 4.0, "burned for {ticks} ticks");
    assert!(g.feed.iter().any(|f| matches!(f, Feed::Kill { killer, victim: v, weapon, .. }
        if *v == victim as u8 && *killer == victim as u8 && *weapon == sim::LAVA_WEAPON)));
    assert_eq!(sim::weapon_name(sim::LAVA_WEAPON), "Lava");
    assert_eq!(sim::weapon_name(sim::MELEE_WEAPON), "Melee");
}

#[test]
fn bell_yards_are_mirrored_landmarks_a_player_can_walk_round_and_under() {
    let (bx, bz) = cathedral::BELL_YARD;
    for seed in SEEDS {
        let w = world(seed);
        let nav = Nav::build(&w);
        for s in [1.0f32, -1.0] {
            let (x, z) = (bx * s, bz * s);
            let ground = w.height(x, z);
            // The bell stands five metres tall on level ground, clear of every pad.
            let crown = w
                .blocks
                .iter()
                .filter(|b| b.min.x < x && b.max.x > x && b.min.z < z && b.max.z > z)
                .fold(f32::NEG_INFINITY, |top, b| top.max(b.max.y));
            assert!(
                (crown - ground - cathedral::BELL_GANTRY_Y - cathedral::BELL_GANTRY_BEAM).abs() < 0.01,
                "seed {seed:x}: gantry beam should top the bell yard at {x},{z}"
            );
            assert!(nav.is_blocked(x, z), "bots must route round the bell");
            for p in w.bases.iter().chain(w.points.iter()) {
                assert!(vec2(p.x - x, p.z - z).length() > 20.0);
            }
            // Walk the gap between the flask and each gantry post, under the beam.
            for side in [-1.0f32, 1.0] {
                let lane = x + side * (cathedral::BELL_FLASK_HALF + cathedral::BELL_GANTRY_HALF - 0.3) * 0.5;
                let mut st = PState::spawn(vec3(lane, w.height(lane, z - 4.0) + 0.02, z - 4.0), 0);
                for seq in 1..=240 {
                    sim::step(&mut st, &Cmd { seq, mz: 1, yaw: 0.0, ..Default::default() }, &w, 0);
                }
                assert!(
                    st.pos.z > z + 3.0 && (st.pos.x - lane).abs() < 0.3,
                    "seed {seed:x}: gantry gap {side} at bell {s} is not walkable: {:?}",
                    st.pos
                );
            }
        }
        // The lavafall lands in molten metal, never on a bridge.
        let (lx, lz) = cathedral::LAVAFALL;
        assert!(cathedral::in_lava(lx, lz, 0.0) && cathedral::in_lava(-lx, -lz, 0.0));
        assert!(cathedral::in_lava(lx - 1.0, lz, 0.0) && cathedral::in_lava(lx + 1.0, lz, 0.0));
    }
}
