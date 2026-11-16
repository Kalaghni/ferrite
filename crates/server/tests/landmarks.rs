//! Exercise the authored interiors with the actual player controller, so an
//! apparently open door or mine entrance cannot hide a collision snag.

use ferrite_shared::glam::{vec2, vec3, Vec3};
use ferrite_shared::map::landmarks::{floor, walkway_site, Site, PASSAGES, WORKSHOPS};
use ferrite_shared::map::{FeatureKind, World};
use ferrite_shared::proto::Cmd;
use ferrite_shared::sim::{self, PState, P_H, P_R};

const SEEDS: [u64; 4] = [0, 17, 0xFE221, u64::MAX];

fn standing_is_clear(w: &World, p: Vec3, dmask: u64) -> bool {
    !w.blocks.iter().enumerate().any(|(i, b)| {
        w.solid(i, dmask)
            && p.x + P_R > b.min.x
            && p.x - P_R < b.max.x
            && p.z + P_R > b.min.z
            && p.z - P_R < b.max.z
            && p.y + P_H > b.min.y
            && p.y + 0.01 < b.max.y
    })
}

fn walk_to(w: &World, st: &mut PState, target: (f32, f32), dmask: u64) {
    for seq in 1..=900 {
        let d = vec2(target.0 - st.pos.x, target.1 - st.pos.z);
        if d.length() < 0.15 {
            return;
        }
        sim::step(
            st,
            &Cmd {
                seq,
                mz: 1,
                yaw: d.x.atan2(d.y),
                ..Default::default()
            },
            w,
            dmask,
        );
        assert!(st.pos.is_finite());
        assert!(
            standing_is_clear(w, st.pos, dmask),
            "map {}, seed {:x}: collided inside route at {:?}",
            w.map,
            w.seed,
            st.pos
        );
        assert!(st.pos.y + 0.01 >= w.height(st.pos.x, st.pos.z));
        assert!(!st.crouch, "a standing route must not require crouching");
    }
    panic!(
        "map {}, seed {:x}: stopped at {:?} on the way to {target:?}",
        w.map, w.seed, st.pos
    );
}

fn spawn(w: &World, site: Site, dx: f32, dz: f32) -> PState {
    let p = vec3(site.x + dx, floor(w, site) + 0.07, site.z + dz);
    assert!(
        standing_is_clear(w, p, 0),
        "map {}, seed {:x}: blocked entrance {p:?}",
        w.map,
        w.seed
    );
    PState::spawn(p, 0)
}

#[test]
fn every_mine_cave_culvert_and_container_passage_can_be_walked_both_ways() {
    for seed in SEEDS {
        for map in 0..4 {
            let w = World::with_map(seed, map);
            for site in [PASSAGES[map as usize], PASSAGES[map as usize].mirrored()] {
                let mut st = spawn(&w, site, -site.hx - 2.0, 0.0);
                for x in [
                    site.x,
                    site.x + site.hx + 2.0,
                    site.x,
                    site.x - site.hx - 2.0,
                ] {
                    walk_to(&w, &mut st, (x, site.z), 0);
                }
                assert!(
                    (st.pos.y - floor(&w, site)).abs() < 0.1,
                    "passage floor must stay level"
                );
            }
        }
    }
}

#[test]
fn complete_workshops_have_four_usable_doors_and_crossable_interiors() {
    for seed in SEEDS {
        for map in 0..4 {
            let w = World::with_map(seed, map);
            for site in [WORKSHOPS[map as usize], WORKSHOPS[map as usize].mirrored()] {
                for axis in 0..2 {
                    let (dx, dz) = if axis == 0 {
                        (site.hx + 2.0, 0.0)
                    } else {
                        (0.0, site.hz + 2.0)
                    };
                    let mut st = spawn(&w, site, -dx, -dz);
                    for (x, z) in [
                        (site.x, site.z),
                        (site.x + dx, site.z + dz),
                        (site.x, site.z),
                        (site.x - dx, site.z - dz),
                    ] {
                        walk_to(&w, &mut st, (x, z), 0);
                    }
                }
            }
        }
    }
}

#[test]
fn breached_panels_open_a_standing_route_through_the_wall() {
    for seed in SEEDS {
        for map in 0..4 {
            let w = World::with_map(seed, map);
            for f in w
                .features
                .iter()
                .filter(|f| f.kind == FeatureKind::BreachPanel)
            {
                let workshop = if f.pos.x < 0.0 { WORKSHOPS[map as usize] } else { WORKSHOPS[map as usize].mirrored() };
                let outward = (f.pos.z - workshop.z).signum();
                let start = vec3(
                    f.pos.x,
                    w.height(f.pos.x, f.pos.z) + 0.07,
                    f.pos.z + 2.0 * outward,
                );
                let target = (f.pos.x, f.pos.z - 2.0 * outward);
                let bit = 1u64 << f.bit;
                assert!(w
                        .raycast(start + Vec3::Y * 0.5, -Vec3::Z * outward, 4.0, 0)
                    .is_some());
                let mut st = PState::spawn(start, 0);
                walk_to(&w, &mut st, target, bit);
                walk_to(&w, &mut st, (start.x, start.z), bit);
            }
        }
    }
}

#[test]
fn service_walkways_can_be_climbed_and_collapse_under_a_player() {
    for seed in SEEDS {
        for map in 0..4 {
            let w = World::with_map(seed, map);
            let s = walkway_site(map);
            for site in [s, s.mirrored()] {
                let mut st = spawn(&w, site, -site.hx - 1.0, 0.0);
                walk_to(&w, &mut st, (site.x, site.z), 0);
                let g = floor(&w, site);
                assert!(
                    (st.pos.y - g - 1.8).abs() < 0.04,
                    "walkway stairs must reach the deck"
                );
                let f = w
                    .features
                    .iter()
                    .find(|f| f.kind == FeatureKind::Walkway && (f.pos.x - site.x).abs() < 0.01)
                    .unwrap();
                let bit = 1u64 << f.bit;
                for seq in 1..=120 {
                    sim::step(
                        &mut st,
                        &Cmd {
                            seq,
                            ..Default::default()
                        },
                        &w,
                        bit,
                    );
                    assert!(st.pos.is_finite() && standing_is_clear(&w, st.pos, bit));
                }
                assert!(
                    st.on_ground && (st.pos.y - g).abs() < 0.04,
                    "collapsed deck must release the player onto the ground"
                );
            }
        }
    }
}
