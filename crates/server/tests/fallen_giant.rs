//! The Fallen Giant is a playable route, not just a large backdrop. Exercise
//! its openings with the same controller used by the authoritative server.

use ferrite_shared::glam::{vec2, vec3, Vec3};
use ferrite_shared::map::{fallen_giant as wreck, Nav, World};
use ferrite_shared::proto::Cmd;
use ferrite_shared::sim::{self, PState, P_H, P_R};

const SEEDS: [u64; 4] = [0, 17, 0xFE221, u64::MAX];

fn clear(w: &World, p: Vec3) -> bool {
    !w.blocks.iter().any(|b| {
        p.x + P_R > b.min.x
            && p.x - P_R < b.max.x
            && p.z + P_R > b.min.z
            && p.z - P_R < b.max.z
            && p.y + P_H > b.min.y
            && p.y + 0.01 < b.max.y
    })
}

fn spawn(w: &World, x: f32, z: f32) -> PState {
    let p = vec3(x, w.height(x, z) + 0.02, z);
    assert!(clear(w, p), "seed {:x}: blocked wreck approach at {p:?}", w.seed);
    PState::spawn(p, 0)
}

fn walk(w: &World, player: &mut PState, seq: &mut u32, target: (f32, f32)) {
    for _ in 0..1500 {
        let d = vec2(target.0 - player.pos.x, target.1 - player.pos.z);
        if d.length() < 0.15 {
            return;
        }
        *seq += 1;
        sim::step(player, &Cmd { seq: *seq, mz: 1, yaw: d.x.atan2(d.y), ..Default::default() }, w, 0);
        assert!(player.pos.is_finite());
        assert!(clear(w, player.pos), "seed {:x}: standing collision at {:?}", w.seed, player.pos);
        assert!(player.pos.y + 0.01 >= w.height(player.pos.x, player.pos.z));
        assert!(!player.crouch, "cargo routes must be usable while standing");
    }
    panic!("seed {:x}: stalled at {:?} walking to {target:?}", w.seed, player.pos);
}

#[test]
fn both_broken_ends_offer_a_standing_route_through_the_capture_point() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let approach = wreck::HALF_LENGTH + 4.0;
        for z in [-1.5, 0.0, 1.5] {
            let mut player = spawn(&w, -approach, z);
            let mut seq = 0;
            for x in [0.0, approach, 0.0, -approach] {
                walk(&w, &mut player, &mut seq, (x, z));
                assert!((player.pos.y - wreck::floor(&w)).abs() < 0.05, "seed {seed:x}: cargo deck must be level");
            }
        }
        assert!(clear(&w, w.points[2] + Vec3::Y * 0.02), "C must have room for a standing player");
    }
}

#[test]
fn every_side_tear_can_be_crossed_without_jumping_or_crouching() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let nav = Nav::build(&w);
        let approach = wreck::HALF_WIDTH + 4.0;
        for x in wreck::SIDE_DOORS {
            for dx in [-1.2, 0.0, 1.2] {
                let x = x + dx;
                let mut player = spawn(&w, x, -approach);
                let mut seq = 0;
                for z in [0.0, approach, 0.0, -approach] {
                    walk(&w, &mut player, &mut seq, (x, z));
                    assert!(!nav.is_blocked(x, z), "seed {seed:x}: bots cannot use side tear at {x},{z}");
                }
            }
        }
    }
}

#[test]
fn intact_cargo_shell_provides_solid_cover_and_generous_headroom() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let g = wreck::floor(&w);
        for x in [-34.0, -10.0, 0.0, 10.0, 34.0] {
            let hit = w
                .raycast(vec3(x, g + P_H, 0.0), Vec3::Y, wreck::ROOF_HEIGHT + 1.0, 0)
                .expect("cargo hold must provide real overhead cover");
            let b = &w.blocks[hit.block.expect("the roof must be collision, not terrain")];
            assert!(wreck::owns(b));
            assert!((hit.t - (wreck::ROOF_HEIGHT - P_H)).abs() < 0.02);
        }
        for side in [-1.0f32, 1.0] {
            let origin = vec3(10.0, g + 1.0, 0.0);
            let hit = w.raycast(origin, Vec3::Z * side, 10.0, 0).expect("intact hull wall must stop shots");
            assert!(wreck::owns(&w.blocks[hit.block.unwrap()]));
            assert!((hit.t - wreck::HALF_WIDTH).abs() < 0.02);
            let mut player = spawn(&w, 10.0, 0.0);
            for seq in 1..=240 {
                sim::step(
                    &mut player,
                    &Cmd { seq, mz: 1, yaw: if side > 0.0 { 0.0 } else { std::f32::consts::PI }, ..Default::default() },
                    &w,
                    0,
                );
                assert!(clear(&w, player.pos));
            }
            assert!(player.pos.z.abs() < wreck::HALF_WIDTH - P_R + 0.02, "the visible cargo shell must stop a walking player");
            assert!(player.pos.z.abs() > wreck::HALF_WIDTH - P_R - 0.05, "there must be no hidden obstacle before the cargo wall");
        }
        for (x, side) in [(-30.0, -1.0), (30.0, 1.0)] {
            let hit = w.raycast(vec3(x, g + 0.6, 0.0), Vec3::Z * side, 5.0, 0).expect("cargo fixtures must be usable solid cover");
            assert_eq!(w.blocks[hit.block.unwrap()].tint, wreck::CARGO_TINT);
        }
    }
}
