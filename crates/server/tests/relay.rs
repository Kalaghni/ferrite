//! Relay's signal towers: every stair is climbed on foot, the decks hold a
//! player in, and the towers stay fair and clear of the objectives.

use ferrite_shared::glam::{vec2, vec3, Vec3};
use ferrite_shared::map::relay::{self, CORNERS};
use ferrite_shared::map::{map_by_name, Nav, World};
use ferrite_shared::proto::Cmd;
use ferrite_shared::sim::{self, PState, P_H, P_R};

const SEEDS: [u64; 5] = [0, 1, 17, 0xFE221, u64::MAX];

fn world(seed: u64) -> World {
    let id = map_by_name("relay").expect("Relay is selectable by name");
    assert_eq!(id, relay::MAP_ID);
    World::with_map(seed, id)
}

/// Every tower on the map: (x, z, flights, quarter turns, mirror sign).
fn towers() -> Vec<(f32, f32, usize, u8, f32)> {
    let mut out = Vec::new();
    for (x, z, flights, rot) in relay::TOWERS {
        for side in [1.0f32, -1.0] {
            out.push((x * side, z * side, flights, rot, side));
        }
    }
    out
}

/// Walk toward a point without ever pressing jump. Returns false if the
/// player stalls before getting there.
fn walk_to(st: &mut PState, w: &World, seq: &mut u32, target: (f32, f32)) -> bool {
    for _ in 0..360 {
        let d = vec2(target.0 - st.pos.x, target.1 - st.pos.z);
        if d.length() < 0.12 {
            return true;
        }
        *seq += 1;
        sim::step(st, &Cmd { seq: *seq, mz: 1, yaw: d.x.atan2(d.y), ..Default::default() }, w, 0);
        assert!(st.pos.is_finite());
    }
    false
}

#[test]
fn every_tower_stair_is_walked_to_its_deck_without_jumping() {
    const { assert!(relay::RISE <= sim::STEP_H) };
    for seed in SEEDS {
        let w = world(seed);
        for (x, z, flights, rot, side) in towers() {
            let base = relay::pad_top(&w, x, z);
            let deck = base + relay::deck_height(flights);
            let at = |dx: f32, dz: f32| relay::place(x, z, rot, side, dx, dz);
            let start = CORNERS[relay::start_corner(flights)];
            let (sx, sz) = at(start.0, start.1);
            let mut st = PState::spawn(vec3(sx, base + 0.02, sz), 0);
            let mut seq = 0;
            for j in 0..flights {
                let (_, b, foot) = relay::flight(flights, j);
                let (tx, tz) = at(CORNERS[b].0, CORNERS[b].1);
                assert!(walk_to(&mut st, &w, &mut seq, (tx, tz)), "seed {seed:x}: tower {x},{z} flight {j} stalled at {:?}", st.pos);
                let want = base + foot + relay::FLIGHT_RISE;
                assert!((st.pos.y - want).abs() < 0.05, "seed {seed:x}: tower {x},{z} flight {j} ended at {} not {want}", st.pos.y);
            }
            // Step off the arrival corner onto the deck and cross to a parapet.
            for target in [at(0.0, -1.85), at(1.85, 0.0)] {
                assert!(walk_to(&mut st, &w, &mut seq, target), "seed {seed:x}: deck of {x},{z} is not walkable");
                assert!((st.pos.y - deck).abs() < 0.05);
            }
            // Headroom to stand anywhere on the deck's two covered sides.
            let h = vec3(st.pos.x, deck + 0.02, st.pos.z);
            assert!(!w.blocks.iter().any(|b| {
                h.x + P_R > b.min.x
                    && h.x - P_R < b.max.x
                    && h.z + P_R > b.min.z
                    && h.z - P_R < b.max.z
                    && h.y + P_H > b.min.y
                    && h.y < b.max.y
            }));
        }
    }
}

#[test]
fn deck_rails_and_parapets_keep_a_walking_player_on_the_tower() {
    for seed in SEEDS {
        let w = world(seed);
        for (x, z, flights, rot, side) in towers() {
            let deck = relay::pad_top(&w, x, z) + relay::deck_height(flights);
            let at = |dx: f32, dz: f32| relay::place(x, z, rot, side, dx, dz);
            // From the middle of each covered side, push straight at every
            // outer and stairwell edge in turn.
            for (from, toward) in [
                ((0.0, -1.85), (0.0, -9.0)),
                ((1.7, -1.85), (1.7, -9.0)),
                ((1.85, 0.0), (9.0, 0.0)),
                ((1.85, -1.7), (9.0, -1.7)),
                ((1.85, 1.85), (1.85, 9.0)),
                ((1.85, 1.85), (-9.0, 1.85)),
                ((1.85, 0.6), (-9.0, 0.6)),
                ((0.6, -1.85), (0.6, 9.0)),
                ((-1.85, -1.85), (-9.0, -1.85)),
            ] {
                let (fx, fz) = at(from.0, from.1);
                let mut st = PState::spawn(vec3(fx, deck + 0.02, fz), 0);
                let mut seq = 0;
                for _ in 0..12 {
                    seq += 1;
                    sim::step(&mut st, &Cmd { seq, ..Default::default() }, &w, 0);
                }
                let (tx, tz) = at(toward.0, toward.1);
                let _ = walk_to(&mut st, &w, &mut seq, (tx, tz));
                assert!(
                    (st.pos.y - deck).abs() < 0.05,
                    "seed {seed:x}: tower {x},{z}: walked off the deck from {from:?} toward {toward:?}: {:?}",
                    st.pos
                );
            }
        }
    }
}

#[test]
fn towers_are_mirrored_vantage_points_clear_of_objectives_and_spawns() {
    for seed in SEEDS {
        let w = world(seed);
        let nav = Nav::build(&w);
        let all = towers();
        for &(x, z, flights, rot, side) in &all {
            // Its mirror exists, with the same climb and the same foundation.
            assert!(all.iter().any(|&(mx, mz, mf, mr, ms)| mx == -x && mz == -z && mf == flights && mr == rot && ms == -side));
            assert!((relay::pad_top(&w, x, z) - relay::pad_top(&w, -x, -z)).abs() < 0.03, "seed {seed:x}: uneven tower sites");
            assert!(nav.is_blocked(x, z), "bots must route round a tower");
            for (i, p) in w.points.iter().enumerate() {
                let d = vec2(p.x - x, p.z - z).length();
                assert!(d > 9.0, "seed {seed:x}: tower {x},{z} crowds point {i} ({d:.1} m)");
            }
            for p in &w.bases {
                assert!(vec2(p.x - x, p.z - z).length() > 40.0, "a tower overlooks a spawn too closely");
            }
            // Sites are levelled: the apron, then the pad, are each one
            // walkable step up from the ground on every side.
            let base = relay::pad_top(&w, x, z);
            const { assert!(relay::APRON_DROP <= sim::STEP_H) };
            for (dx, dz) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                for t in [-1.0f32, 0.0, 1.0] {
                    let reach = relay::APRON_HALF + 0.5;
                    let (px, pz) = (x + dx * reach + dz * t * 2.5, z + dz * reach + dx * t * 2.5);
                    let lip = base - relay::APRON_DROP - w.height(px, pz);
                    assert!(lip <= sim::STEP_H, "seed {seed:x}: tower {x},{z} apron lip is {lip:.2} m on side {dx},{dz}");
                }
            }
            // The deck sees the objective it stands beside.
            let eye = vec3(x, base + relay::deck_height(flights) + 1.6, z);
            let (ex, ez) = relay::place(x, z, rot, side, 2.0, -2.0);
            let eye = Vec3::new(ex, eye.y, ez);
            let nearest = w.points.iter().min_by(|a, b| (**a - eye).length().total_cmp(&(**b - eye).length())).unwrap();
            // Trees and cover differ by seed, so ask for a view of most of
            // the capture area rather than of one exact spot.
            let mut seen = 0;
            for k in 0..16 {
                let a = k as f32 * std::f32::consts::TAU / 16.0;
                let p = *nearest + vec3(a.cos() * 4.0, 0.0, a.sin() * 4.0);
                let p = vec3(p.x, w.height(p.x, p.z) + 1.0, p.z);
                seen += w.los(eye, p, 0) as usize;
            }
            assert!(seen >= 8, "seed {seed:x}: tower {x},{z} sees only {seen}/16 of its objective");
        }
        eprintln!("Relay seed {seed:x}: {} collision blocks", w.blocks.len());
    }
}
