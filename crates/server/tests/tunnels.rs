//! Quarry's mine and Ridgeline's cave run underground: walkable floors
//! under the visible terrain, with rock roofs and walls that stop bodies and
//! bullets. Exercised with the same controller the server runs.

use ferrite_shared::glam::{vec2, vec3};
use ferrite_shared::map::{tunnels, Nav, World};
use ferrite_shared::proto::Cmd;
use ferrite_shared::sim::{self, PState};

const SEEDS: [u64; 4] = [0, 17, 0xFE221, u64::MAX];
const MAPS: [u8; 2] = [1, 2];

fn walk(w: &World, p: &mut PState, seq: &mut u32, target: (f32, f32), limit: u32) -> bool {
    for _ in 0..limit {
        let d = vec2(target.0 - p.pos.x, target.1 - p.pos.z);
        if d.length() < 0.3 {
            return true;
        }
        *seq += 1;
        sim::step(p, &Cmd { seq: *seq, mz: 1, yaw: d.x.atan2(d.y), ..Default::default() }, w, 0);
        assert!(p.pos.is_finite());
    }
    false
}

#[test]
fn print_profiles() {
    for map in MAPS {
        let w = World::with_map(0xFE221, map);
        for (k, t) in w.tunnels.iter().enumerate().take(1) {
            eprintln!("map {map} tunnel {k}: length {:.1}, portals {:?}", t.length(), t.portals);
            let mut s = 0.0;
            while s <= t.length() {
                let (p, _) = t.at(s);
                let surf = w.height(p.x, p.z);
                eprintln!(
                    "  s {s:5.1} at ({:6.1},{:6.1}) floor {:6.2} surface {:6.2} cover {:5.2}",
                    p.x,
                    p.z,
                    p.y,
                    surf,
                    surf - p.y - tunnels::HEIGHT
                );
                s += 3.0;
            }
        }
    }
}

/// Both tunnels on each map mirror each other under a half turn.
#[test]
fn tunnels_are_mirrored_pairs() {
    for map in MAPS {
        for seed in SEEDS {
            let w = World::with_map(seed, map);
            assert_eq!(w.tunnels.len(), 2, "map {map}");
            let (a, b) = (&w.tunnels[0], &w.tunnels[1]);
            for (p, q) in a.pts.iter().zip(&b.pts) {
                assert!((p.x + q.x).abs() < 1e-3 && (p.z + q.z).abs() < 1e-3 && (p.y - q.y).abs() < 1e-3);
            }
        }
    }
}

/// Every roofed stretch has rock over it and a floor gentle enough to walk.
#[test]
fn roofs_are_covered_and_floors_are_walkable() {
    for map in MAPS {
        for seed in SEEDS {
            let w = World::with_map(seed, map);
            for t in &w.tunnels {
                let mut s = t.portals[0] + 1.5;
                while s < t.portals[1] - 1.5 {
                    let (p, dir) = t.at(s);
                    let side = vec2(-dir.y, dir.x);
                    for k in [-1.0, 0.0, 1.0] {
                        let q = vec2(p.x, p.z) + side * k * (tunnels::HALF_WIDTH + 0.6);
                        let cover = w.height(q.x, q.y) - p.y - tunnels::HEIGHT;
                        assert!(cover > 1.0, "map {map} seed {seed:x}: thin roof ({cover:.2}) at s {s:.1}");
                    }
                    s += 1.0;
                }
                for i in 1..t.pts.len() {
                    let (a, b) = (t.pts[i - 1], t.pts[i]);
                    let slope = (b.y - a.y).abs() / vec2(b.x - a.x, b.z - a.z).length();
                    assert!(slope < 0.36, "map {map} seed {seed:x}: floor slope {slope:.2}");
                }
            }
        }
    }
}

/// A player walks in at one portal and out of the other, underground in
/// between, without leaving the tunnel.
#[test]
fn every_tunnel_can_be_walked_end_to_end_underground() {
    for map in MAPS {
        for seed in SEEDS {
            let w = World::with_map(seed, map);
            for t in &w.tunnels {
                for reverse in [false, true] {
                    let ends = if reverse { [t.length() - 1.0, 1.0] } else { [1.0, t.length() - 1.0] };
                    let (start, _) = t.at(ends[0]);
                    let mut p = PState::spawn(vec3(start.x, w.height(start.x, start.z) + 0.02, start.z), 0);
                    let mut seq = 0;
                    let mut deepest = 0.0f32;
                    let n = 60;
                    for k in 1..=n {
                        let s = ends[0] + (ends[1] - ends[0]) * k as f32 / n as f32;
                        let (c, _) = t.at(s);
                        assert!(
                            walk(&w, &mut p, &mut seq, (c.x, c.z), 400),
                            "map {map} seed {seed:x}: stuck at {:?} heading for s {s:.1}",
                            p.pos
                        );
                        if t.roofed(s) {
                            assert!(
                                (p.pos.y - c.y).abs() < 0.3,
                                "map {map} seed {seed:x}: off the floor at s {s:.1}: {:?} vs {}",
                                p.pos,
                                c.y
                            );
                            deepest = deepest.max(w.height(p.pos.x, p.pos.z) - p.pos.y);
                        }
                    }
                    assert!(deepest > tunnels::HEIGHT + 1.0, "map {map}: never really underground ({deepest:.1})");
                    let (end, _) = t.at(ends[1]);
                    assert!((p.pos.y - w.height(end.x, end.z)).abs() < 0.3, "map {map}: did not come out on the ground");
                }
            }
        }
    }
}

/// Rock stops bodies at the walls, heads at the roof and bullets both ways.
#[test]
fn rock_stops_walkers_jumpers_and_bullets() {
    for map in MAPS {
        let w = World::with_map(0xFE221, map);
        let t = &w.tunnels[0];
        let mid = (t.portals[0] + t.portals[1]) * 0.5;
        let (c, dir) = t.at(mid);
        let side = vec2(-dir.y, dir.x);
        // Walk sideways into the wall.
        let mut p = PState::spawn(vec3(c.x, c.y + 0.02, c.z), 0);
        let mut seq = 0;
        let far = vec2(c.x, c.z) + side * 12.0;
        walk(&w, &mut p, &mut seq, (far.x, far.y), 240);
        let off = vec2(p.pos.x - c.x, p.pos.z - c.z).length();
        assert!(off < tunnels::HALF_WIDTH + 0.05, "map {map}: walked {off:.2} m into the wall");
        assert!((p.pos.y - c.y).abs() < 0.3, "map {map}: climbed out through rock: {:?}", p.pos);
        // Jump: the roof holds.
        let mut q = PState::spawn(vec3(c.x, c.y + 0.02, c.z), 0);
        let mut top = q.pos.y;
        for k in 0..90 {
            sim::step(
                &mut q,
                &Cmd { seq: k + 1, buttons: if k == 0 { ferrite_shared::proto::btn::JUMP } else { 0 }, ..Default::default() },
                &w,
                0,
            );
            top = top.max(q.pos.y);
        }
        assert!(top + sim::P_H <= c.y + tunnels::HEIGHT + 0.01, "map {map}: head through the roof");
        // Sight and bullets: nothing from the hill above reaches the tunnel,
        // but the tunnel itself is open along its length.
        let inside = vec3(c.x, c.y + 1.5, c.z);
        let above = vec3(c.x, w.height(c.x, c.z) + 1.6, c.z);
        assert!(!w.los(above, inside, 0), "map {map}: seen through the roof");
        let (ahead, _) = t.at(mid + 4.0);
        assert!(w.los(inside, vec3(ahead.x, ahead.y + 1.5, ahead.z), 0), "map {map}: tunnel blocks its own sightline");
        // The hill above is ordinary ground.
        assert!((w.ground(c.x, c.z, above.y) - w.height(c.x, c.z)).abs() < 1e-4);
    }
}

/// Bots can reach both entrances on the navigation grid, and underground a
/// bot's wish to move is turned along the tunnel, whichever way it faces.
#[test]
fn bots_can_enter_and_follow_tunnels() {
    for map in MAPS {
        let w = World::with_map(0xFE221, map);
        let nav = Nav::build(&w);
        for t in &w.tunnels {
            for (from, to) in [(1.0, t.portals[0] + 1.0), (t.portals[1] - 1.0, t.length() - 1.0)] {
                let mut s = from;
                while s <= to {
                    let (c, _) = t.at(s);
                    assert!(!nav.is_blocked(c.x, c.z), "map {map}: entrance cell blocked at s {s:.1}");
                    s += 1.0;
                }
            }
            let mid = (t.portals[0] + t.portals[1]) * 0.5;
            let (c, dir) = t.at(mid);
            let inside = vec3(c.x, c.y, c.z);
            for want in [vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(-0.6, 0.8), -dir, vec2(-dir.y, dir.x)] {
                let go = w.tunnel_steer(inside, want);
                assert!(go.normalize().dot(dir).abs() > 0.95, "map {map}: steered {go:?} in a tunnel running {dir:?}");
            }
            let above = vec3(c.x, w.height(c.x, c.z), c.z);
            assert_eq!(w.tunnel_steer(above, vec2(1.0, 0.0)), vec2(1.0, 0.0), "the hill above is open ground");
        }
    }
}

/// The hill over a tunnel is ordinary ground: walking across it, over the
/// roof and near the portals, nothing invisible stops or lifts a player,
/// and they never drop into the tunnel through its roof.
#[test]
fn the_ground_over_a_tunnel_walks_like_any_other() {
    for map in MAPS {
        for seed in SEEDS {
            let w = World::with_map(seed, map);
            for t in &w.tunnels {
                let [a, b] = t.portals;
                for s in [a + 2.5, a + 5.0, (a + b) * 0.5, b - 5.0, b - 2.5] {
                    let (c, dir) = t.at(s);
                    let side = vec2(-dir.y, dir.x);
                    for k in [-1.0f32, 1.0] {
                        let from = vec2(c.x, c.z) + side * 9.0 * k;
                        let to = vec2(c.x, c.z) - side * 9.0 * k;
                        let mut p = PState::spawn(vec3(from.x, w.height(from.x, from.y) + 0.02, from.y), 0);
                        let mut stalled = 0;
                        let mut last = p.pos;
                        let mut ok = false;
                        for seq in 1..=900 {
                            let d = to - vec2(p.pos.x, p.pos.z);
                            if d.length() < 0.3 {
                                ok = true;
                                break;
                            }
                            sim::step(&mut p, &Cmd { seq, mz: 1, yaw: d.x.atan2(d.y), ..Default::default() }, &w, 0);
                            let surf = w.height(p.pos.x, p.pos.z);
                            let over = (vec2(p.pos.x, p.pos.z) - vec2(c.x, c.z)).dot(side).abs() < tunnels::HALF_WIDTH;
                            assert!(
                                !over || p.pos.y > c.y + tunnels::HEIGHT,
                                "map {map} seed {seed:x}: fell into the tunnel at {:?} (s {s:.1})",
                                p.pos
                            );
                            assert!(p.pos.y < surf + 1.5, "map {map} seed {seed:x}: lifted off the hill at {:?} (s {s:.1})", p.pos);
                            stalled = if p.pos.distance(last) < 1e-3 { stalled + 1 } else { 0 };
                            assert!(stalled < 30, "map {map} seed {seed:x}: invisible wall on the hill at {:?} (s {s:.1})", p.pos);
                            last = p.pos;
                        }
                        assert!(ok, "map {map} seed {seed:x}: could not cross the hill at s {s:.1}");
                    }
                }
            }
        }
    }
}
