//! Relay's BACKBONE mast: inside the battlefield, solid where it is steel,
//! and climbed by ladder to a caged rest platform and a top deck. Driven
//! through `sim::step` exactly as the server and the client run it.

use ferrite_shared::glam::{vec2, vec3, Vec3};
use ferrite_shared::map::relay::{self, BACKBONE, LADDERS, LANDING, TOP_DECK};
use ferrite_shared::map::{Nav, World, HALF_X, HALF_Z};
use ferrite_shared::proto::{btn, Cmd};
use ferrite_shared::sim::{self, PState, P_R};
use std::f32::consts::PI;

const SEEDS: [u64; 4] = [0, 17, 0xFE221, u64::MAX];
/// Facing +z (toward the first ladder's rungs) and -z (the second's).
const NORTH: f32 = 0.0;
const SOUTH: f32 = PI;

fn world(seed: u64) -> World {
    World::with_map(seed, relay::MAP_ID)
}

/// A point in the mast's frame, `y` above its ground.
fn at(w: &World, dx: f32, y: f32, dz: f32) -> Vec3 {
    vec3(BACKBONE.0 + dx, relay::mast_base(w) + y, BACKBONE.1 + dz)
}

struct Player<'a> {
    w: &'a World,
    st: PState,
    seq: u32,
}

impl<'a> Player<'a> {
    fn new(w: &'a World, dx: f32, y: f32, dz: f32) -> Self {
        let mut p = Player { w, st: PState::spawn(at(w, dx, y + 0.02, dz), 0), seq: 0 };
        p.idle(10);
        p
    }

    fn tick(&mut self, mx: i8, mz: i8, yaw: f32, buttons: u8) -> sim::StepOut {
        self.seq += 1;
        let out = sim::step(&mut self.st, &Cmd { seq: self.seq, mx, mz, yaw, buttons, ..Default::default() }, self.w, 0);
        assert!(self.st.pos.is_finite() && self.st.vel.is_finite());
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.tick(0, 0, NORTH, 0);
        }
    }

    /// Height above the mast's ground.
    fn y(&self) -> f32 {
        self.st.pos.y - relay::mast_base(self.w)
    }

    /// Where the player is in the mast frame.
    fn xz(&self) -> (f32, f32) {
        (self.st.pos.x - BACKBONE.0, self.st.pos.z - BACKBONE.1)
    }

    /// Walk toward a point in the mast frame. False if the player stalls.
    fn walk_to(&mut self, dx: f32, dz: f32) -> bool {
        for _ in 0..400 {
            let (x, z) = self.xz();
            let d = vec2(dx - x, dz - z);
            if d.length() < 0.1 {
                return true;
            }
            self.tick(0, 1, d.x.atan2(d.y), 0);
        }
        false
    }

    /// Hold one input until the player stands still on something, or give
    /// up. Returns whether they were on a ladder at any point.
    fn hold(&mut self, mz: i8, yaw: f32, buttons: u8, limit: usize) -> bool {
        let mut climbed = false;
        for _ in 0..limit {
            self.tick(0, mz, yaw, buttons);
            climbed |= self.st.climbing;
            if !self.st.climbing && self.st.on_ground && self.st.vel.y.abs() < 1e-3 && climbed {
                break;
            }
        }
        climbed
    }
}

#[test]
fn the_mast_stands_in_the_battlefield_clear_of_everything_that_matters() {
    let (mx, mz) = BACKBONE;
    let (x0, z0, x1, z1) = relay::MAST_SITE;
    assert!(mx + x0 > -HALF_X + 5.0 && mx + x1 < HALF_X - 5.0 && mz + z0 > -HALF_Z + 5.0 && mz + z1 < HALF_Z - 3.0);
    for seed in SEEDS {
        let w = world(seed);
        let c = vec2(mx, mz);
        let b = vec2(w.bases[0].x, w.bases[0].z).distance(c);
        assert!((b - vec2(w.bases[1].x, w.bases[1].z).distance(c)).abs() < 1e-3, "the mast must be as far from each base");
        assert!(b > 100.0);
        for p in &w.points {
            assert!(vec2(p.x, p.z).distance(c) > 60.0, "seed {seed:x}: the mast crowds an objective");
        }
        for (x, z, _, _) in relay::TOWERS {
            for s in [1.0f32, -1.0] {
                assert!(vec2(x * s, z * s).distance(c) > 40.0);
            }
        }
        for (x, z) in relay::GANTRIES.into_iter().chain([relay::DISH]) {
            for s in [1.0f32, -1.0] {
                assert!(vec2(x * s, z * s).distance(c) > 40.0);
            }
        }
        // Nothing but the mast was built on its site, and the ground under
        // it is level so every footing sits the same.
        for blk in &w.blocks {
            let m = (blk.min + blk.max) * 0.5;
            let inside = m.x > mx + x0 - 2.0 && m.x < mx + x1 + 2.0 && m.z > mz + z0 - 2.0 && m.z < mz + z1 + 2.0;
            assert!(!inside || relay::mast_owns(blk), "seed {seed:x}: {:?} built on the mast site at {m:?}", blk.kind);
        }
        for t in &w.trees {
            assert!(t.base.x < mx + x0 - 2.0 || t.base.x > mx + x1 + 2.0 || t.base.z < mz + z0 - 2.0 || t.base.z > mz + z1 + 2.0);
        }
        let g = relay::mast_base(&w);
        for (dx, dz) in [(-4.3, -4.3), (4.3, -4.3), (-4.3, 4.3), (4.3, 4.3), (0.0, 10.0)] {
            assert!((w.height(mx + dx, mz + dz) - g).abs() < 0.02, "seed {seed:x}: mast site not level");
        }
        // Bots route round the legs and the hut.
        let nav = Nav::build(&w);
        for (dx, dz) in [(-4.3, -4.3), (4.3, -4.3), (-4.3, 4.3), (4.3, 4.3), (0.0, 10.0), (0.0, -0.5)] {
            assert!(nav.is_blocked(mx + dx, mz + dz), "seed {seed:x}: bots would walk into the mast at {dx},{dz}");
        }
    }
}

#[test]
fn a_player_climbs_to_each_platform_steps_off_and_climbs_back_down() {
    for seed in SEEDS {
        let w = world(seed);
        let [low, high] = LADDERS;
        // Walk in under the lattice and up to the first ladder: pushing
        // into it while facing it takes hold.
        let mut p = Player::new(&w, 0.0, 0.0, -8.0);
        assert!(p.walk_to(0.0, -2.5), "seed {seed:x}: cannot reach the ladder: {:?}", p.xz());
        assert!(p.hold(1, NORTH, 0, 900), "seed {seed:x}: the ladder did not take hold");
        assert!((p.y() - LANDING).abs() < 0.01, "seed {seed:x}: first climb ended at {} not {LANDING}", p.y());
        assert!(!p.st.climbing && p.st.on_ground);
        assert!(p.xz().1 > low.z, "stepped forward off the rungs onto the platform");

        // Round the second ladder's back to its foot, then up to the top.
        for (x, z) in [(1.4, -0.2), (1.4, 1.1), (0.0, 1.1)] {
            assert!(p.walk_to(x, z), "seed {seed:x}: blocked on the rest platform at {:?}", p.xz());
            assert!((p.y() - LANDING).abs() < 0.01);
        }
        assert!(p.hold(1, SOUTH, 0, 900), "seed {seed:x}: the second ladder did not take hold");
        assert!((p.y() - TOP_DECK).abs() < 0.01, "seed {seed:x}: second climb ended at {} not {TOP_DECK}", p.y());
        assert!(p.xz().1 < high.z);
        // The top deck is walkable.
        for (x, z) in [(0.0, -1.2), (1.5, -1.2), (-1.5, 0.0)] {
            assert!(p.walk_to(x, z), "seed {seed:x}: blocked on the top deck at {:?}", p.xz());
            assert!((p.y() - TOP_DECK).abs() < 0.01);
        }

        // Down: back into the top hatch while facing the rungs, and the
        // ladder catches you and lowers you to the rest platform.
        assert!(p.walk_to(0.0, 0.2));
        assert!(p.hold(-1, SOUTH, 0, 900), "seed {seed:x}: backing into the top hatch did not catch the ladder");
        assert!((p.y() - LANDING).abs() < 0.01, "seed {seed:x}: climbed down to {} not the platform", p.y());
        // And from the platform, round the second ladder's back, down the
        // first ladder to the ground.
        for (x, z) in [(1.4, 1.1), (1.4, -0.2), (0.0, -0.2)] {
            assert!(p.walk_to(x, z), "seed {seed:x}: blocked on the rest platform at {:?}", p.xz());
        }
        let mut lowest = f32::MAX;
        let mut climbed = false;
        for _ in 0..900 {
            p.tick(0, -1, NORTH, 0);
            climbed |= p.st.climbing;
            lowest = lowest.min(p.y());
            if climbed && p.st.on_ground && !p.st.climbing {
                break;
            }
        }
        assert!(climbed, "seed {seed:x}: backing into the platform hatch did not catch the ladder");
        assert!(p.y().abs() < 0.05 && lowest > -0.05, "seed {seed:x}: should be back on the ground, at {}", p.y());
    }
}

#[test]
fn the_ladder_holds_still_slides_and_lets_go() {
    let w = world(0xFE221);
    let mut p = Player::new(&w, 0.0, 0.0, -1.0);
    for _ in 0..200 {
        p.tick(0, 1, NORTH, 0);
    }
    assert!(p.st.climbing);
    let h = p.y();
    assert!((h - 200.0 / 60.0 * sim::CLIMB_SPEED).abs() < 0.4, "climbed {h} m in 200 ticks");
    // Let go of the keys and you hang there.
    for _ in 0..120 {
        p.tick(0, 0, NORTH, 0);
    }
    assert!(p.st.climbing && (p.y() - h).abs() < 1e-3, "drifted on the ladder");
    // You cannot shoot, throw or swing with your hands on the rungs, but
    // you can reload.
    assert!(!p.tick(0, 0, NORTH, btn::FIRE).fired);
    assert!(!p.tick(0, 0, NORTH, btn::GRENADE).threw);
    // Crouch slides down fast.
    let before = p.y();
    for _ in 0..30 {
        p.tick(0, 0, NORTH, btn::CROUCH);
    }
    assert!(before - p.y() > 30.0 / 60.0 * sim::CLIMB_SLIDE * 0.9);
    assert!(p.st.climbing);
    // Jump lets go: you fall clear of the rungs and land.
    p.tick(0, 1, NORTH, btn::JUMP);
    assert!(!p.st.climbing);
    for _ in 0..240 {
        p.tick(0, 0, NORTH, 0);
    }
    assert!(!p.st.climbing && p.st.on_ground && p.y().abs() < 0.05, "let go and should be on the ground: {:?}", p.st.pos);
    assert!(p.xz().1 < LADDERS[0].z - P_R, "fell clear behind the rungs");
}

#[test]
fn standing_at_the_ladder_without_pushing_into_it_does_not_lift_you() {
    for seed in SEEDS {
        let w = world(seed);
        let foot = LADDERS[0].z - P_R - 0.05;
        // Idle, strafing, backing off, crouching forward, or walking at it
        // while looking away: none of these is a climb.
        for (mx, mz, yaw, buttons) in [
            (0, 0, NORTH, 0),
            (1, 0, NORTH, 0),
            (-1, 0, NORTH, 0),
            (0, -1, NORTH, 0),
            (0, 1, NORTH, btn::CROUCH),
            (0, -1, SOUTH, 0),
            (1, 0, PI * 0.5, 0),
            (0, 1, NORTH, btn::JUMP),
        ] {
            let mut p = Player::new(&w, 0.0, 0.0, foot);
            let mut top = 0.0f32;
            for _ in 0..90 {
                p.tick(mx, mz, yaw, buttons);
                assert!(!p.st.climbing, "seed {seed:x}: input {mx},{mz},{yaw},{buttons} took hold of the ladder");
                // Off the level site the ground rises; measure from it.
                top = top.max(p.st.pos.y - w.height(p.st.pos.x, p.st.pos.z));
            }
            // A jump can rise 1.2 m; nothing else lifts you.
            let limit = if buttons & btn::JUMP != 0 { 1.3 } else { 0.05 };
            assert!(top < limit, "seed {seed:x}: lifted to {top} by {mx},{mz},{yaw},{buttons}");
        }
        // From behind (inside the lattice) the ladder is solid steel.
        let mut p = Player::new(&w, 0.0, 0.0, 2.0);
        for _ in 0..120 {
            p.tick(0, 1, SOUTH, 0);
        }
        assert!(!p.st.climbing && p.xz().1 > LADDERS[0].z + relay::LADDER_BACK, "walked through the ladder: {:?}", p.xz());
    }
}

#[test]
fn the_legs_stop_players_and_bullets() {
    for seed in SEEDS {
        let w = world(seed);
        let base = relay::mast_base(&w);
        for sx in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                let leg = relay::MAST_FOOT;
                // On the ground: walk along a face straight through a leg.
                let mut p = Player::new(&w, sx * (leg + 3.0), 0.0, sz * leg);
                for _ in 0..240 {
                    p.tick(0, 1, if sx > 0.0 { -PI * 0.5 } else { PI * 0.5 }, 0);
                }
                assert!(p.xz().0 * sx > leg, "seed {seed:x}: walked through the leg at {sx},{sz}: {:?}", p.xz());
                // On the rest platform: walk along its edge into a corner leg.
                let inner = relay::mast_half(LANDING + 2.0) - relay::MAST_LEG * 0.5;
                let mut p = Player::new(&w, sx * 2.95, LANDING, 0.0);
                let _ = p.walk_to(sx * 2.95, sz * 6.0);
                assert!((p.y() - LANDING).abs() < 0.01, "seed {seed:x}: fell off the rest platform toward {sx},{sz}");
                assert!(p.xz().1.abs() < inner, "seed {seed:x}: walked through the leg at {sx},{sz} on the platform: {:?}", p.xz());
                // A bullet across the bottom of the mast at chest height
                // stops on the leg.
                let o = vec3(BACKBONE.0 + sx * (leg + 4.0), base + 1.4, BACKBONE.1 + sz * leg);
                let hit = w.raycast(o, vec3(-sx, 0.0, 0.0), 8.0, 0).expect("the leg stops a bullet");
                assert!((hit.t - 4.0).abs() < 0.6, "seed {seed:x}: hit at {} m", hit.t);
            }
        }
    }
}

#[test]
fn rails_and_parapets_keep_a_walking_player_on_both_decks() {
    let w = world(0xFE221);
    for (deck, half) in [(LANDING, relay::LANDING_HALF), (TOP_DECK, relay::TOP_HALF)] {
        // Start on the far side of the core from both hatches' climbers.
        for (from, toward) in [
            ((1.2, 0.0), (9.0, 0.0)),
            ((-1.2, 0.0), (-9.0, 0.0)),
            ((1.2, -0.2), (1.2, -9.0)),
            ((1.2, 0.2), (1.2, 9.0)),
            ((-1.2, 0.2), (-1.2, 9.0)),
        ] {
            let mut p = Player::new(&w, from.0, deck, from.1);
            assert!((p.y() - deck).abs() < 0.01, "spawned off the deck at {from:?}");
            let _ = p.walk_to(toward.0, toward.1);
            assert!((p.y() - deck).abs() < 0.01, "walked off the {deck} m deck toward {toward:?}: {:?}", p.st.pos);
            let (x, z) = p.xz();
            assert!(x.abs() < half && z.abs() < half);
        }
    }
}

#[test]
fn climbing_is_deterministic_and_the_mast_is_mirrored_across_its_axis() {
    let w = world(17);
    // A climb with a sideways start, a pause, a slide and the step-off.
    let script = |side: f32| {
        let mut p = Player::new(&w, 0.3 * side, 0.0, -2.0);
        let mut trace = Vec::new();
        for k in 0..700 {
            let (mx, mz, b) = match k {
                0..=60 => (0, 1, 0),
                61..=120 => (0, 0, 0),
                121..=140 => (0, 0, btn::CROUCH),
                _ => (0, 1, 0),
            };
            p.tick(mx, mz, NORTH + 0.2 * side, b);
            trace.push(p.st);
        }
        trace
    };
    let (a, b) = (script(1.0), script(1.0));
    assert_eq!(a, b, "the same commands must give the same result");
    let m = script(-1.0);
    for (k, (s, t)) in a.iter().zip(&m).enumerate() {
        let mirrored = vec3(2.0 * BACKBONE.0 - t.pos.x, t.pos.y, t.pos.z);
        assert!((s.pos - mirrored).length() < 2e-3, "tick {k}: {:?} vs mirrored {:?}", s.pos, mirrored);
        assert_eq!(s.climbing, t.climbing, "tick {k}");
    }
    let last = a.last().unwrap();
    assert!((last.pos.y - relay::mast_base(&w) - LANDING).abs() < 0.01 && !last.climbing);
}
