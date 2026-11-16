//! Structural destruction is authoritative, persistent and explosives-only.
use ferrite_server::{Config, Game, Grenade};
use ferrite_shared::map::{Block, BlockKind, FeatureKind, Nav, World};
use ferrite_shared::proto::{decode, encode, Fx, Snapshot};
use ferrite_shared::{vec3, Vec3};

fn game(map: u8, seed: u64) -> Game {
    let mut g = Game::new(Config { map, seed, per_team: 1, max_bots: 0, lobby_secs: 0, discover: false, log: false, ..Default::default() });
    for player in &mut g.players {
        player.bot = None;
        player.alive = false;
        player.respawn_at = u32::MAX;
    }
    g
}

fn detonate(g: &mut Game, at: Vec3) {
    g.grenades.push(Grenade { id: 500, pos: at, vel: Vec3::ZERO, fuse: 1, owner: 0, team: 0 });
    g.tick();
    assert!(g.grenades.is_empty(), "the live grenade must actually detonate");
}

#[test]
fn grenades_release_each_group_once_and_replicate_to_a_late_client() {
    for seed in [17, 0xFE221] {
        for map in 0..4 {
            let mut g = game(map, seed);
            let features = g.world.features.clone();
            assert_eq!(features.len(), 6, "each landmark has three independent mirrored mechanisms");
            for feature in features {
                let mask = 1u64 << feature.bit;
                assert_eq!(g.dmask & mask, 0);
                let group: Vec<_> =
                    g.world.blocks.iter().enumerate().filter(|(_, block)| block.dest == Some(feature.bit)).map(|(i, _)| i).collect();
                assert!(!group.is_empty(), "every marked feature must remove real collision");
                assert!(group.iter().all(|&i| g.world.solid(i, g.dmask)));
                let mirror = g.world.features.iter().find(|other| other.kind == feature.kind && other.pos.x * feature.pos.x < 0.0).unwrap();
                let mirror_bit = 1u64 << mirror.bit;
                let mirror_was_broken = g.dmask & mirror_bit;
                let before = g.fx.iter().filter(|(_, fx)| matches!(fx, Fx::MapBreak { .. })).count();
                detonate(&mut g, feature.pos + vec3(0.0, 0.15, -0.9));
                assert_ne!(g.dmask & mask, 0, "map {map}, seed {seed:x}: {:?} did not release at {:?}", feature.kind, feature.pos);
                assert!(group.iter().all(|&i| !g.world.solid(i, g.dmask)), "every collider in a group must disappear together");
                assert_eq!(g.dmask & mirror_bit, mirror_was_broken, "the opposite team's feature must remain independent");
                assert_eq!(g.fx.iter().filter(|(_, fx)| matches!(fx, Fx::MapBreak { .. })).count(), before + 1);
                let saved = g.dmask;
                detonate(&mut g, feature.pos + vec3(0.0, 0.15, -0.9));
                assert_eq!(g.dmask, saved, "a second grenade must not retrigger destruction");
                assert_eq!(g.fx.iter().filter(|(_, fx)| matches!(fx, Fx::MapBreak { .. })).count(), before + 1);
            }
            // A late joiner needs no historical MapBreak effects. The mask in
            // its first snapshot reproduces the authoritative geometry alone.
            let snap = g.snapshot(None, 0, u32::MAX);
            assert!(snap.fx.is_empty());
            let decoded: Snapshot = decode(&encode(&snap)).expect("snapshot round trip");
            assert_eq!(decoded.dmask, g.dmask);
            let late = World::with_map(seed, map);
            for (i, block) in late.blocks.iter().enumerate() {
                if block.dest.is_some_and(|bit| late.features.iter().any(|feature| feature.bit == bit)) {
                    assert!(!late.solid(i, decoded.dmask));
                }
            }
            assert!(g.nav.blocked == Nav::build_mask(&g.world, g.dmask).blocked, "bots must use the broken field");
        }
    }
}

#[test]
fn a_solid_wall_shields_a_nearby_breach_panel_and_range_is_limited() {
    let mut g = game(0, 0xFE221);
    let panel = g.world.features.iter().find(|feature| feature.kind == FeatureKind::BreachPanel).unwrap().clone();
    let p = panel.pos;
    detonate(&mut g, p + vec3(0.0, 0.1, -7.0));
    assert_eq!(g.dmask & (1u64 << panel.bit), 0, "distant explosions cannot breach a panel");
    g.world.blocks.push(Block {
        min: p + vec3(-5.0, -3.0, -1.7),
        max: p + vec3(5.0, 5.0, -1.3),
        kind: BlockKind::Concrete,
        tint: 0,
        dest: None,
    });
    detonate(&mut g, p + vec3(0.0, 0.1, -3.0));
    assert_eq!(g.dmask & (1u64 << panel.bit), 0, "permanent walls must shield a structure within blast range");
    assert!(g.fx.iter().all(|(_, fx)| !matches!(fx, Fx::MapBreak { .. })));
}

#[test]
fn one_explosion_cannot_chain_through_a_panel_it_just_released() {
    let mut g = game(0, 0xFE221);
    let panels: Vec<_> = g.world.features.iter().filter(|feature| feature.kind == FeatureKind::BreachPanel).cloned().collect();
    let front = &panels[0];
    let rear = &panels[1];
    // Put two independent, real structural panels in line. Both are within
    // the radius, but the first one shields the second at detonation time.
    let new_rear = front.pos + vec3(0.0, 0.0, 2.0);
    let shift = new_rear - rear.pos;
    for block in &mut g.world.blocks {
        if block.dest == Some(rear.bit) {
            block.min += shift;
            block.max += shift;
        }
    }
    g.world.features.iter_mut().find(|feature| feature.bit == rear.bit).unwrap().pos = new_rear;
    let blast = front.pos + vec3(0.0, 0.1, -1.8);
    detonate(&mut g, blast);
    assert_ne!(g.dmask & (1u64 << front.bit), 0);
    assert_eq!(g.dmask & (1u64 << rear.bit), 0, "the same blast cannot propagate through the newly opened wall");
    detonate(&mut g, blast);
    assert_ne!(g.dmask & (1u64 << rear.bit), 0, "a subsequent grenade can reach the now-exposed panel");
}

#[test]
fn a_repeated_map_restores_collision_and_the_intact_navigation_field() {
    let mut g = game(3, 0xFE221);
    let intact_nav = g.nav.blocked.clone();
    let feature = g.world.features.iter().find(|feature| feature.kind == FeatureKind::BreachPanel).unwrap().clone();
    detonate(&mut g, feature.pos + vec3(0.0, 0.15, -0.9));
    assert_ne!(g.dmask & (1u64 << feature.bit), 0);
    assert!(g.nav.blocked != intact_nav, "the breached opening must change a walking route");
    let old_round = g.round_no;
    g.phase = 1;
    g.phase_until = g.tick + 1;
    g.tick();
    assert_eq!(g.round_no, old_round + 1);
    assert_eq!(g.world.map, 3, "repeat this map, exercising same-map reset");
    assert_eq!(g.dmask, 0);
    assert!(g.nav.blocked == intact_nav, "same-map round reset must restore intact routes");
    for (i, block) in g.world.blocks.iter().enumerate() {
        if block.dest == Some(feature.bit) {
            assert!(g.world.solid(i, g.dmask));
        }
    }
}
