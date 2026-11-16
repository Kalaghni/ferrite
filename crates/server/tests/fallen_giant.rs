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

/// Through the hold from one broken end to the other: past the toppled
/// container, round the aft frame on its +z gap, across C, round the
/// forward frame on its -z gap and out.
fn hold_route(z: f32, from_aft: bool) -> Vec<(f32, f32)> {
    let gap = (wreck::BULKHEAD_GAP + wreck::HALF_WIDTH) * 0.5;
    let aft = [(-24.0, 0.0), (-wreck::BULKHEAD_X, gap), (0.0, 0.0), (wreck::BULKHEAD_X, -gap), (24.0, 0.0), (wreck::FLAT_X, z)];
    if from_aft {
        aft.to_vec()
    } else {
        let mut back: Vec<_> = aft.iter().rev().skip(1).map(|&(x, z)| (x, z)).collect();
        back.push((-wreck::FLAT_X, z));
        back
    }
}

#[test]
fn both_broken_ends_offer_a_standing_route_through_the_capture_point() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let g = wreck::floor(&w);
        for z in [-1.5, 0.0, 1.5] {
            for from_aft in [true, false] {
                let start = if from_aft { -wreck::FLAT_X } else { wreck::FLAT_X };
                let mut player = spawn(&w, start, z);
                let mut seq = 0;
                for target in hold_route(z, from_aft) {
                    walk(&w, &mut player, &mut seq, target);
                    assert!((player.pos.y - g).abs() < 0.05, "seed {seed:x}: cargo deck must be level at {target:?}");
                }
            }
        }
        assert!(clear(&w, w.points[2] + Vec3::Y * 0.02), "C must have room for a standing player");
    }
}

#[test]
fn no_straight_line_runs_the_length_of_the_hold() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let g = wreck::floor(&w);
        let x = wreck::HALF_LENGTH + 1.0;
        let zs: Vec<f32> = (-28..=28).map(|k| k as f32 * 0.2).collect();
        // Any eye height from a crouch to the roof, any pair of points in
        // the two broken ends.
        for y in [0.9, 1.3, 1.62, 2.4, 3.5, 5.0, 6.5, 7.8] {
            for &za in &zs {
                for &zb in zs.iter().step_by(4) {
                    let (a, b) = (vec3(-x, g + y, za), vec3(x, g + y, zb));
                    assert!(!w.los(a, b, 0), "seed {seed:x}: clear line through the hold from {a:?} to {b:?}");
                }
                // Neither end sees the capture point at all.
                for end in [-x, x] {
                    let a = vec3(end, g + y, za);
                    for c in [vec3(0.0, g + 1.0, 0.0), vec3(0.0, g + 1.62, 0.0)] {
                        assert!(!w.los(a, c, 0), "seed {seed:x}: C visible from the open end at {a:?}");
                    }
                }
            }
        }
        // The base-to-base line along the hold's axis.
        let (a, b) = (w.bases[0] + Vec3::Y * 1.62, w.bases[1] + Vec3::Y * 1.62);
        assert!(!w.los(a, b, 0));
        assert!(!w.los(vec3(-x - 30.0, g + 1.62, 0.0), vec3(x + 30.0, g + 1.62, 0.0), 0));
    }
}

#[test]
fn capture_point_has_cover_against_the_side_tears() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let g = wreck::floor(&w);
        // Crouched on the inner side of a pallet, a defender is hidden from
        // someone standing in the centre tear on the pallet's side.
        for side in [-1.0f32, 1.0] {
            let hide = vec3(side * 5.2, g + 0.9, side * 2.3);
            let tear = vec3(side * 1.5, g + 1.62, side * 7.5);
            assert!(!w.los(tear, hide, 0), "seed {seed:x}: pallet does not cover C from {tear:?}");
            let hit = w.raycast(tear, (hide - tear).normalize(), 20.0, 0).unwrap();
            assert_eq!(w.blocks[hit.block.unwrap()].tint, wreck::CARGO_TINT);
        }
    }
}

#[test]
fn every_side_tear_can_be_crossed_without_jumping_or_crouching() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let nav = Nav::build(&w);
        let approach = wreck::FLAT_Z + 2.0;
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
fn bots_route_through_the_hold_and_round_both_frames() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let nav = Nav::build(&w);
        let c = w.points[2];
        for side in [-1.0f32, 1.0] {
            // Start in the broken end and follow the flow field to C.
            let mut p = vec2(side * (wreck::HALF_LENGTH + 1.0), 0.0);
            let mut rounded = false;
            for _ in 0..400 {
                if vec2(c.x - p.x, c.z - p.y).length() < 2.0 {
                    break;
                }
                let d = nav.dir(2, 0, p.x, p.y);
                assert!(d.length() > 0.5, "seed {seed:x}: no route at {p:?}");
                p += d * 0.5;
                assert!(p.y.abs() < wreck::HALF_WIDTH, "seed {seed:x}: bot route left the hold at {p:?}");
                if (p.x.abs() - wreck::BULKHEAD_X).abs() < 0.5 {
                    // The aft frame is open at +z, the forward one at -z.
                    assert!(-side * p.y > wreck::BULKHEAD_GAP, "seed {seed:x}: route through a frame at {p:?}");
                    rounded = true;
                }
            }
            assert!(vec2(c.x - p.x, c.z - p.y).length() < 2.0, "seed {seed:x}: bot stalled at {p:?}");
            assert!(rounded);
        }
    }
}

#[test]
fn intact_cargo_shell_provides_solid_cover_and_generous_headroom() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        let g = wreck::floor(&w);
        for x in [-34.0, -24.0, -6.0, 0.0, 6.0, 24.0, 34.0] {
            let hit = w
                .raycast(vec3(x, g + P_H, 0.0), Vec3::Y, wreck::ROOF_HEIGHT + 1.0, 0)
                .expect("cargo hold must provide real overhead cover");
            let b = &w.blocks[hit.block.expect("the roof must be collision, not terrain")];
            assert!(wreck::owns(b));
            assert!((hit.t - (wreck::ROOF_HEIGHT - P_H)).abs() < 0.02);
        }
        // Each roof tear is open to the sky on the side its frame blocks.
        for side in [-1.0f32, 1.0] {
            let x = side * (wreck::ROOF_TEAR[0] + wreck::ROOF_TEAR[1]) * 0.5;
            assert!(w.raycast(vec3(x, g + 2.0, side * 2.0), Vec3::Y, 30.0, 0).is_none());
            assert!(w.raycast(vec3(x, g + 2.0, -side * 3.0), Vec3::Y, 30.0, 0).is_some());
        }
        for (x, side) in [(-26.0, -1.0), (-26.0, 1.0), (26.0, -1.0), (26.0, 1.0)] {
            let origin = vec3(x, g + 1.0, 0.0);
            let hit = w.raycast(origin, Vec3::Z * side, 10.0, 0).expect("intact hull wall must stop shots");
            assert!(wreck::owns(&w.blocks[hit.block.unwrap()]));
            assert!((hit.t - wreck::HALF_WIDTH).abs() < 0.02);
            let mut player = spawn(&w, x, 0.0);
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
        // The buckled frames stop shots at head height.
        for side in [-1.0f32, 1.0] {
            let hit = w.raycast(vec3(-side * 20.0, g + 1.62, side * -3.0), Vec3::X * side, 20.0, 0).unwrap();
            assert_eq!(w.blocks[hit.block.unwrap()].tint, wreck::FRAME_TINT);
            assert!((hit.t - (20.0 - wreck::BULKHEAD_X - wreck::BULKHEAD_HALF)).abs() < 0.02);
        }
    }
}

#[test]
fn wreckage_outside_is_solid_where_it_is_drawn() {
    for seed in SEEDS {
        let w = World::with_map(seed, wreck::MAP_ID);
        for p in wreck::PIECES {
            for (x, z) in [(p.x, p.z), (-p.x, -p.z)] {
                let ground = w.height(x, z);
                // Levelled pad: the piece sits flush on flat ground.
                for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    assert!((w.height(x + sx * p.hx, z + sz * p.hz) - ground).abs() < 0.15, "seed {seed:x}: piece at {x},{z} on a slope");
                }
                let from = vec3(x, ground + 1.0, z - z.signum() * (p.hz + 4.0));
                let hit = w.raycast(from, Vec3::Z * z.signum(), 6.0, 0).expect("wreckage must stop shots");
                assert_eq!(w.blocks[hit.block.unwrap()].tint, wreck::DEBRIS_TINT);
            }
        }
    }
}
