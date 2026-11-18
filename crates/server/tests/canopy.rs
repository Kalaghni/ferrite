//! Canopy: the hollow fig's boughs, stairs and ladders, and riding its
//! seedlines. Driven through `sim::step` exactly as the server and the
//! client run it.

use ferrite_shared::glam::{vec2, Vec3};
use ferrite_shared::map::canopy::{self, BOUGH_Y};
use ferrite_shared::map::{Nav, World};
use ferrite_shared::proto::{btn, btn2, Cmd};
use ferrite_shared::sim::{self, PState};

const SEEDS: [u64; 3] = [0, 17, 0xFE221];

fn world(seed: u64) -> World {
    World::with_map(seed, canopy::MAP_ID)
}

struct Player<'a> {
    w: &'a World,
    st: PState,
    seq: u32,
}

impl<'a> Player<'a> {
    fn at(w: &'a World, p: Vec3) -> Self {
        let mut me = Player { w, st: PState::spawn(p + Vec3::Y * 0.02, 0), seq: 0 };
        me.idle(20);
        me
    }

    fn tick(&mut self, mz: i8, yaw: f32, buttons: u8, more: u8) -> sim::StepOut {
        self.seq += 1;
        let c = Cmd { seq: self.seq, mz, yaw, buttons, more, ..Default::default() };
        let out = sim::step(&mut self.st, &c, self.w, 0);
        assert!(self.st.pos.is_finite() && self.st.vel.is_finite());
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.tick(0, 0.0, 0, 0);
        }
    }

    /// Walk straight toward (x, z). False if the player stalls.
    fn walk_to(&mut self, x: f32, z: f32) -> bool {
        for _ in 0..3000 {
            let d = vec2(x - self.st.pos.x, z - self.st.pos.z);
            if d.length() < 0.2 {
                return true;
            }
            self.tick(1, d.x.atan2(d.y), 0, 0);
        }
        false
    }

    /// Press Use facing `toward`, then hold `mz` until off the line (or
    /// `limit` ticks).
    fn ride_toward(&mut self, toward: Vec3, mz: i8, limit: usize) -> usize {
        let yaw = (toward.x - self.st.pos.x).atan2(toward.z - self.st.pos.z);
        self.tick(0, yaw, 0, btn2::USE);
        assert!(self.st.ride > 0, "did not take the line at {:?}", self.st.pos);
        let mut n = 0;
        while self.st.ride > 0 && n < limit {
            self.tick(mz, 0.0, 0, 0);
            n += 1;
        }
        n
    }
}

#[test]
fn the_court_line_carries_a_rider_up_to_the_boughs() {
    for seed in SEEDS {
        let w = world(seed);
        assert_eq!(w.lines.len(), 6, "six seedlines");
        let line = &w.lines[0];
        let mut p = Player::at(&w, line.pts[0]);
        assert_eq!(p.st.slot, 0);
        let ticks = p.ride_toward(line.pts[line.pts.len() - 1], 1, 60 * 30);
        assert_eq!(p.st.ride, 0, "still riding after {ticks} ticks");
        assert_eq!(p.st.slot, 1, "taking a line draws the sidearm");
        p.idle(30);
        let end = line.pts[line.pts.len() - 1];
        assert!(p.st.on_ground, "landed on the bough");
        assert!((p.st.pos.y - BOUGH_Y).abs() < 0.1, "on the bough at {:?}", p.st.pos);
        assert!(vec2(p.st.pos.x - end.x, p.st.pos.z - end.z).length() < 0.6);
        // About 40 m of line and two dock climbs: a few seconds, not more.
        let secs = ticks as f32 / 60.0;
        assert!((2.5..9.0).contains(&secs), "the ride took {secs} s");
    }
}

#[test]
fn every_line_runs_both_ways_and_every_dock_is_standing_room() {
    let w = world(0xFE221);
    for (k, line) in w.lines.iter().enumerate() {
        for first in [true, false] {
            let dock = if first { line.pts[0] } else { line.pts[line.pts.len() - 1] };
            let far = if first { line.pts[line.pts.len() - 1] } else { line.pts[0] };
            let mut p = Player::at(&w, dock);
            assert!(p.st.on_ground, "line {k}: the dock at {dock:?} is not standing room");
            p.ride_toward(far, 1, 60 * 30);
            assert_eq!(p.st.ride, 0, "line {k} did not finish");
            p.idle(30);
            assert!(p.st.pos.distance(far) < 0.8, "line {k}: ended at {:?}, not {far:?}", p.st.pos);
        }
    }
}

#[test]
fn letting_go_hangs_back_returns_and_jump_drops_off() {
    let w = world(0);
    let line = &w.lines[1];
    let mut p = Player::at(&w, line.pts[0]);
    let far = line.pts[line.pts.len() - 1];
    p.tick(0, (far.x - p.st.pos.x).atan2(far.z - p.st.pos.z), 0, btn2::USE);
    assert_eq!(p.st.ride, 2, "took the line it faced");
    for _ in 0..150 {
        p.tick(1, 0.0, 0, 0);
    }
    let out_at = p.st.ride_s;
    assert!(out_at > 8.0, "under way: {out_at}");
    // Let go: the sling slows to a hang and stays there.
    for _ in 0..90 {
        p.tick(0, 0.0, 0, 0);
    }
    let hang = p.st.ride_s;
    p.idle(60);
    assert!(p.st.ride > 0 && (p.st.ride_s - hang).abs() < 1e-3, "hanging still");
    assert!(p.st.pos.y > BOUGH_Y + 2.0, "hanging above the boughs");
    // Back reels toward the dock you got on at.
    for _ in 0..30 {
        p.tick(-1, 0.0, 0, 0);
    }
    assert!(p.st.ride_s < hang);
    // Jump lets go, with a little lift; then gravity.
    let out = p.tick(0, 0.0, btn::JUMP, 0);
    assert!(out.jumped && p.st.ride == 0 && p.st.vel.y > 0.0);
    for _ in 0..600 {
        p.tick(0, 0.0, 0, 0);
    }
    assert!(p.st.on_ground, "came down somewhere");
}

#[test]
fn riders_hold_their_sidearm_and_cannot_reload_or_throw() {
    let w = world(0);
    let mut p = Player::at(&w, w.lines[0].pts[0]);
    p.tick(0, 0.0, 0, btn2::USE);
    for _ in 0..40 {
        p.tick(1, 0.0, 0, 0);
    }
    let gren = p.st.grenades;
    p.tick(1, 0.0, btn::GRENADE | btn::SWAP | btn::RELOAD, 0);
    assert_eq!(p.st.slot, 1);
    assert_eq!(p.st.grenades, gren);
    assert_eq!(p.st.reload, 0);
}

#[test]
fn use_away_from_a_dock_does_nothing() {
    let w = world(0);
    let c = w.points[2];
    let mut p = Player::at(&w, c);
    p.tick(0, 0.0, 0, btn2::USE);
    assert_eq!(p.st.ride, 0);
}

#[test]
fn the_stair_behind_b_climbs_to_the_boughs_and_the_ladder_too() {
    let w = world(0xFE221);
    // Stair: start on the terrace west of flight one, walk it, the landing,
    // flight two, and on along the connector to the circuit.
    let (fx, fz) = canopy::FLIGHT_ONE;
    let mut p = Player::at(&w, Vec3::new(fx - 2.0, w.height(fx - 2.0, fz), fz));
    for (x, z) in [(canopy::LANDING.0, fz), (canopy::FLIGHT_TWO.0, canopy::FLIGHT_TWO.1 - 15.0), (-78.0, 24.0), (-42.0, 24.0)] {
        assert!(p.walk_to(x, z), "stalled on the way to ({x}, {z}) at {:?}", p.st.pos);
    }
    p.idle(10);
    assert!((p.st.pos.y - BOUGH_Y).abs() < 0.1, "up on the circuit: {:?}", p.st.pos);
    // Ladder: face the rungs, push forward, step off on top.
    let (lx, lz) = canopy::LADDER;
    let mut q = Player::at(&w, Vec3::new(lx - 0.6, w.height(lx - 0.6, lz), lz));
    let east = std::f32::consts::FRAC_PI_2;
    for _ in 0..60 * 12 {
        q.tick(1, east, 0, 0);
        if q.st.pos.y > BOUGH_Y - 0.05 && q.st.on_ground {
            break;
        }
    }
    assert!((q.st.pos.y - BOUGH_Y).abs() < 0.1 && q.st.on_ground, "climbed to {:?}", q.st.pos);
}

#[test]
fn every_objective_and_base_is_reachable_on_the_ground() {
    for seed in SEEDS {
        let w = world(seed);
        let nav = Nav::build(&w);
        for (k, p) in w.points.iter().enumerate() {
            for b in w.bases {
                let d = nav.dir_in(false, k, 0, b.x, b.z);
                assert!(d.length() > 0.5, "seed {seed:x}: no way from base {b:?} to point {k}");
            }
            let _ = p;
        }
    }
}

#[test]
fn the_boughs_are_mirrored_through_the_centre() {
    let w = world(0);
    for b in &w.blocks {
        let (min, max) = (Vec3::new(-b.max.x, b.min.y, -b.max.z), Vec3::new(-b.min.x, b.max.y, -b.min.z));
        if b.min.y > BOUGH_Y - 2.0 {
            assert!(
                w.blocks.iter().any(|o| o.min.distance(min) < 0.05 && o.max.distance(max) < 0.05),
                "no mirror for the bough block {:?}..{:?}",
                b.min,
                b.max
            );
        }
    }
}
