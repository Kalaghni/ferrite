//! Spillway's flood and the gate deck at C: the deck and its bridges stand
//! above the water, both are walked onto from the banks and the channel
//! floor, the water hurts those in it and nobody above it, holding C takes
//! standing on the deck, and the bots get there flood or not.

use ferrite_server::{Config, Game};
use ferrite_shared::events::{self, SIREN_PERIOD};
use ferrite_shared::glam::{vec2, vec3, Vec3};
use ferrite_shared::map::{spillway, World};
use ferrite_shared::proto::{Cmd, Feed};
use ferrite_shared::sim::{self, PState};
use ferrite_shared::{mode, world_clock, NEUTRAL, TICK_HZ};

const SEEDS: [u64; 6] = [0, 17, 0xFE221, 0xD091, u64::MAX, 99];

fn world(seed: u64) -> World {
    World::with_map(seed, spillway::MAP_ID)
}

/// Walk toward a point without jumping. False if the walker stalls.
fn walk_to(st: &mut PState, w: &World, seq: &mut u32, target: (f32, f32)) -> bool {
    for _ in 0..900 {
        let d = vec2(target.0 - st.pos.x, target.1 - st.pos.z);
        if d.length() < 0.2 {
            return true;
        }
        *seq += 1;
        sim::step(st, &Cmd { seq: *seq, mz: 1, yaw: d.x.atan2(d.y), ..Default::default() }, w, 0);
        assert!(st.pos.is_finite());
    }
    false
}

fn stand_on_ground(w: &World, x: f32, z: f32) -> PState {
    PState::spawn(vec3(x, w.height(x, z) + 0.02, z), 0)
}

#[test]
fn the_deck_and_bridges_stand_well_above_a_full_flood() {
    for seed in SEEDS {
        let w = world(seed);
        let l = spillway::layout(&w);
        assert_eq!(w.points[2].y, l.deck, "seed {seed:x}: C stands on the deck");
        let mut checked = 0;
        let mut z = -l.bridge_end;
        while z <= l.bridge_end {
            let mut x = -l.stair_end();
            while x <= l.stair_end() {
                if let Some(top) = spillway::walkway(&w, x, z) {
                    if let Some(s) = spillway::water_surface(&w, x, z, spillway::FULL) {
                        checked += 1;
                        // The stairs go down into it; the deck and bridges do not.
                        let stair = x.abs() > spillway::DECK_HALF && z.abs() <= spillway::STAIR_HALF;
                        if !stair {
                            assert!(top > s + 1.0, "seed {seed:x}: walkway at ({x}, {z}) only {:.2} above the flood", top - s);
                            assert!(!w.drowning(vec3(x, top, z)));
                        }
                    }
                }
                x += 0.5;
            }
            z += 0.5;
        }
        assert!(checked > 800, "seed {seed:x}: only {checked} samples over the water");
        // The slab's underside clears the water too, and the terraces over
        // the outfalls stay dry.
        let s = spillway::water_surface(&w, 0.0, 0.0, spillway::FULL).unwrap();
        assert!(l.deck - spillway::SLAB > s + 0.5, "seed {seed:x}");
        assert!(l.terrace > s + 0.5, "seed {seed:x}");
        // Head room everywhere under the gate frame, and nothing over the
        // deck that is not standing on it.
        for b in w.blocks.iter().filter(|b| b.min.x > -9.9 && b.max.x < 9.9 && b.min.z > -9.9 && b.max.z < 9.9 && b.max.y > l.deck) {
            let on = (b.min.y - l.deck).abs() < 0.01;
            assert!(on || b.min.y - l.deck >= 2.6, "seed {seed:x}: {:?} at {:?} is {:.2} over the deck", b.kind, b.min, b.min.y - l.deck);
        }
    }
}

#[test]
fn the_bridges_are_walked_from_each_bank_onto_the_deck_and_back() {
    for seed in SEEDS {
        let w = world(seed);
        let l = spillway::layout(&w);
        for side in [1.0f32, -1.0] {
            let bank = side * (l.bridge_end + 4.0);
            let mut st = stand_on_ground(&w, 0.0, bank);
            let mut seq = 0;
            assert!(walk_to(&mut st, &w, &mut seq, (0.0, side * 3.0)), "seed {seed:x}: stalled on the bridge at {:?}", st.pos);
            assert!((st.pos.y - l.deck).abs() < 0.05, "seed {seed:x}: not on the deck: {:?}", st.pos);
            assert!(walk_to(&mut st, &w, &mut seq, (0.0, bank)), "seed {seed:x}: stalled going back at {:?}", st.pos);
            assert!((st.pos.y - w.height(0.0, bank)).abs() < 0.1);
        }
    }
}

#[test]
fn the_stairs_climb_from_the_channel_floor_to_the_deck() {
    const { assert!(spillway::RISE <= sim::STEP_H) };
    for seed in SEEDS {
        let w = world(seed);
        let l = spillway::layout(&w);
        for side in [1.0f32, -1.0] {
            let foot = side * (l.stair_end() + 3.0);
            let mut st = stand_on_ground(&w, foot, 0.0);
            let mut seq = 0;
            assert!(walk_to(&mut st, &w, &mut seq, (side * 3.0, 0.0)), "seed {seed:x}: stalled on the stair at {:?}", st.pos);
            assert!((st.pos.y - l.deck).abs() < 0.05, "seed {seed:x}: not on the deck: {:?}", st.pos);
            assert!(walk_to(&mut st, &w, &mut seq, (foot, 0.0)), "seed {seed:x}: stalled going down at {:?}", st.pos);
        }
    }
}

#[test]
fn the_deck_is_mirrored() {
    for seed in SEEDS {
        let w = world(seed);
        let l = spillway::layout(&w);
        for b in &w.blocks {
            let c = (b.min + b.max) * 0.5;
            if c.x.abs() > l.stair_end() + 1.0 || c.z.abs() > l.bridge_end + 3.0 {
                continue;
            }
            let mirrored = w.blocks.iter().any(|q| {
                (q.min.x + b.max.x).abs() < 0.002
                    && (q.max.x + b.min.x).abs() < 0.002
                    && (q.min.z + b.max.z).abs() < 0.002
                    && (q.max.z + b.min.z).abs() < 0.002
                    && (q.min.y - b.min.y).abs() < 0.05
                    && (q.max.y - b.max.y).abs() < 0.05
                    && q.kind == b.kind
            });
            assert!(mirrored, "seed {seed:x}: unmatched {:?} at {:?}", b.kind, b.min);
        }
        for k in 0..400 {
            let (x, z) = (((k * 37) % 41) as f32 - 20.0, ((k * 11) % 81) as f32 - 40.0);
            assert!((w.walk_height(x, z) - w.walk_height(-x, -z)).abs() < 1e-3, "seed {seed:x}: ({x}, {z})");
            assert_eq!(spillway::walkway(&w, x, z).is_some(), spillway::walkway(&w, -x, -z).is_some());
        }
    }
}

// ------------------------------------------------------------- the water

fn cfg(m: u8, per_team: usize) -> Config {
    Config { port: 0, per_team, mode: m, map: spillway::MAP_ID, lobby_secs: 0, log: false, discover: false, ..Default::default() }
}

/// A window in which the sirens are tested and the gates release water,
/// the one after it too if `quiet` is false; or two quiet ones in a row.
fn window(seed: u64, flood: bool) -> f32 {
    let c = (1..400u32)
        .find(|&c| if flood { events::siren_tested(seed, c) } else { !events::siren_tested(seed, c) && !events::siren_tested(seed, c + 1) })
        .expect("a window");
    c as f32 * SIREN_PERIOD
}

/// Run the world clock on to `secs` on it, carrying the round's clock and
/// pending respawns along so nothing else notices.
fn set_clock(g: &mut Game, secs: f32) {
    let to = (secs as f64 * TICK_HZ as f64).round() as u32;
    let dt = to.saturating_sub(g.tick);
    g.tick = to;
    if g.round_end > 0 {
        g.round_end += dt;
    }
    for p in g.players.iter_mut().filter(|p| p.respawn_at != u32::MAX) {
        p.respawn_at = p.respawn_at.saturating_add(dt);
    }
    g.world.clock = world_clock(g.tick as f64);
}

fn put(g: &mut Game, i: usize, at: Vec3) {
    let p = &mut g.players[i];
    p.alive = true;
    p.st = PState::spawn(at, p.st.kit);
    p.respawn_at = u32::MAX;
}

fn bench(g: &mut Game, i: usize) {
    let p = &mut g.players[i];
    p.alive = false;
    p.hp = 0.0;
    p.respawn_at = u32::MAX;
}

/// A game with one person and everyone else stood down.
fn alone(m: u8) -> (Game, usize) {
    let mut g = Game::new(cfg(m, 1));
    let me = g.join_team("tester", 0).expect("slot");
    for _ in 0..30 {
        g.tick();
    }
    for i in 0..g.players.len() {
        if i != me {
            bench(&mut g, i);
        }
    }
    g.players[me].bot = None;
    (g, me)
}

#[test]
fn flood_water_hurts_and_kills_in_the_channel_but_not_on_the_deck_or_in_the_shallows() {
    let (mut g, me) = alone(mode::TDM);
    let seed = g.world.seed;
    let full = window(seed, true) + events::WATER_ARRIVES + events::WATER_RISE + 4.0;
    set_clock(&mut g, full);
    let l = spillway::layout(&g.world);
    let s = spillway::water_surface(&g.world, -30.0, 0.0, spillway::FULL).unwrap();
    let max = sim::kit(g.players[me].st.kit).hp;

    // In the channel: about WATER_DPS, and dead within a few seconds.
    let deep = vec3(-30.0, g.world.height(-30.0, 0.0), 0.0);
    put(&mut g, me, deep);
    g.players[me].hp = max;
    for _ in 0..TICK_HZ {
        g.tick();
    }
    let lost = max - g.players[me].hp;
    assert!((lost - spillway::WATER_DPS).abs() < 3.0, "lost {lost} in a second");
    for _ in 0..(5 * TICK_HZ) {
        g.tick();
    }
    assert!(!g.players[me].alive, "still alive after six seconds in the flood");
    let drowned = g
        .feed
        .iter()
        .any(|f| matches!(f, Feed::Kill { victim, weapon, .. } if *victim == g.players[me].id && *weapon == sim::WATER_WEAPON));
    assert!(drowned, "the kill feed should say the flood did it");

    // On the deck and on a bridge over the water: nothing.
    let bridge = 0.5 * (spillway::DECK_HALF + l.headwall);
    for at in [vec3(4.0, l.deck, -3.0), vec3(0.0, l.deck, bridge), vec3(0.0, l.deck, -bridge)] {
        assert!(g.world.water(at.x, at.z).is_some(), "there is water under {at:?}");
        put(&mut g, me, at);
        g.players[me].hp = max;
        for _ in 0..(2 * TICK_HZ) {
            g.tick();
        }
        assert!(g.players[me].alive && g.players[me].hp >= max - 0.01, "hurt on the walkway at {at:?}");
    }

    // At the water's edge on a bank, ankle deep: nothing.
    let mut z = 10.0;
    while g.world.height(-30.0, z) < s - spillway::WADE + 0.08 {
        z += 0.05;
    }
    let edge = vec3(-30.0, g.world.height(-30.0, z), z);
    assert!(g.world.water(edge.x, edge.z).is_some(), "the edge at {edge:?} should be wet");
    put(&mut g, me, edge);
    g.players[me].hp = max;
    for _ in 0..TICK_HZ {
        g.tick();
        g.players[me].st.pos = edge; // no sliding down into the deep
    }
    assert!(g.players[me].hp >= max - 0.01, "wading at the edge hurt");

    // Ahead of the surge, where it is still a thin sheet: nothing.
    let surge = window(seed, true) + events::WATER_ARRIVES + 4.0;
    set_clock(&mut g, surge);
    let f = events::flood(seed, g.world.clock);
    let deep = |x: f32| g.world.water(x, 0.0).map_or(0.0, |y| y - g.world.height(x, 0.0));
    let mut x = -f.reach - 2.0;
    while deep(x) < 0.1 {
        x += 0.1;
    }
    let (sheet, depth) = (vec3(x, g.world.height(x, 0.0), 0.0), deep(x));
    assert!(depth > 0.0 && depth < spillway::WADE, "the front at {x} is {depth} deep");
    put(&mut g, me, sheet);
    g.players[me].hp = max;
    g.tick();
    g.players[me].st.pos = sheet;
    g.tick();
    assert!(g.players[me].hp >= max - 0.01, "the thin front hurt");
}

#[test]
fn holding_c_takes_the_deck_not_the_channel_under_it() {
    let (mut g, me) = alone(mode::KOTH);
    let l = spillway::layout(&g.world);
    let under = vec3(3.0, g.world.height(3.0, 2.0), 2.0);
    let on = vec3(3.0, l.deck, 2.0);
    assert!(!g.world.on_point(2, under) && g.world.on_point(2, on));
    assert!(spillway::under(&g.world, under) && !spillway::under(&g.world, on));
    // A dry window, so nobody is washed off the floor.
    let dry = window(g.world.seed, false);
    set_clock(&mut g, dry);
    put(&mut g, me, under);
    for _ in 0..(12 * TICK_HZ) {
        g.tick();
    }
    assert_eq!(g.points[2].owner, NEUTRAL, "standing under the deck took C");
    assert_eq!(g.points[2].progress, 0.0);
    put(&mut g, me, on);
    for _ in 0..(11 * TICK_HZ) {
        g.tick();
    }
    assert_eq!(g.points[2].owner, 0, "standing on the deck did not take C");

    // Other maps keep the old rule: within the disc and 7 m up or down.
    let w = World::with_map(0xFE221, 0);
    let c = w.points[2];
    assert!(w.on_point(2, c + vec3(5.0, 6.5, 5.0)) && w.on_point(2, c + vec3(5.0, -6.5, 0.0)));
    assert!(!w.on_point(2, c + vec3(0.0, 7.5, 0.0)) && !w.on_point(2, c + vec3(9.1, 0.0, 0.0)));
}

// ------------------------------------------------------------------ bots

/// KOTH with one team's bots: they go for C. Returns the game once C is
/// theirs (or the time runs out) and how many bot-ticks were spent in
/// deep water.
fn bots_take_c(seed: u64, team: u8, start: f32, secs: u32) -> (Game, u32, bool) {
    let mut g = Game::new(Config { seed, ..cfg(mode::KOTH, 4) });
    for i in 0..g.players.len() {
        if g.players[i].team != team {
            bench(&mut g, i);
        }
    }
    set_clock(&mut g, start);
    let (mut wet, mut flooded_capture) = (0, false);
    for _ in 0..(secs * TICK_HZ) {
        g.tick();
        wet += g.players.iter().filter(|p| p.alive && g.world.drowning(p.st.pos)).count() as u32;
        if g.points[2].owner == team {
            flooded_capture = events::flood(g.world.seed, g.world.clock).level > 0.9;
            break;
        }
    }
    (g, wet, flooded_capture)
}

#[test]
fn bots_take_c_by_the_deck_when_the_channel_is_dry() {
    for (seed, team) in [(Config::default().seed, 0), (Config::default().seed, 1), (17, 0), (0xD091, 1)] {
        let (g, wet, _) = bots_take_c(seed, team, window(seed, false), 150);
        assert_eq!(g.points[2].owner, team, "seed {seed:x}: team {team}'s bots never took C");
        assert!(g.players.iter().any(|p| p.alive && p.team == team && g.world.on_point(2, p.st.pos)));
        assert_eq!(wet, 0);
    }
}

#[test]
fn bots_take_c_over_the_bridges_while_it_floods_and_keep_out_of_the_water() {
    for (seed, team) in [(Config::default().seed, 0), (Config::default().seed, 1), (17, 1), (0xD091, 0)] {
        // They set out as the water rises; the channel is full when they
        // reach C, and stays full a good while.
        let start = window(seed, true) + events::WATER_ARRIVES + 4.0;
        let (g, wet, flooded) = bots_take_c(seed, team, start, 30);
        assert_eq!(g.points[2].owner, team, "seed {seed:x}: team {team}'s bots never took C");
        assert!(flooded, "C should have fallen while the channel was full");
        assert_eq!(wet, 0, "bots walked into the flood");
        assert!(!g.feed.iter().any(|f| matches!(f, Feed::Kill { weapon, .. } if *weapon == sim::WATER_WEAPON)));
    }
}

#[test]
fn a_bot_caught_in_the_flood_makes_for_dry_ground() {
    let seed = Config::default().seed;
    for (x, z) in [(-30.0f32, 3.0f32), (25.0, -6.0), (-6.0, 4.0), (60.0, 0.0)] {
        let mut g = Game::new(cfg(mode::KOTH, 1));
        let bot = g.players.iter().position(|p| p.team == 0).unwrap();
        for i in 0..g.players.len() {
            if i != bot {
                bench(&mut g, i);
            }
        }
        set_clock(&mut g, window(seed, true) + events::WATER_ARRIVES + events::WATER_RISE + 2.0);
        let at = vec3(x, g.world.height(x, z), z);
        put(&mut g, bot, at);
        g.players[bot].hp = 1.0e5; // to see where it goes, not whether it lives
        assert!(g.world.drowning(g.players[bot].st.pos));
        let mut out = None;
        for t in 0..(12 * TICK_HZ) {
            g.tick();
            if !g.world.drowning(g.players[bot].st.pos) {
                out = Some(t);
                break;
            }
        }
        assert!(out.is_some(), "a bot put in the flood at ({x}, {z}) is still in it at {:?}", g.players[bot].st.pos);
    }
}
