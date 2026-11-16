//! Game modes, flags, pickups, streaks and credits.

use ferrite_server::{round_credits, streak_reward, Config, Game};
use ferrite_shared::map::{HALF_X, HALF_Z};
use ferrite_shared::proto::*;
use ferrite_shared::{mode, mode_by_name, vec3, Vec3, FLAG_POINT, MODES, NEUTRAL};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

fn cfg(m: u8, per_team: usize) -> Config {
    Config { port: 0, per_team, mode: m, lobby_secs: 0, log: false, discover: false, ..Default::default() }
}

/// Put a player on their feet at `at`, whatever they were doing.
fn stand(g: &mut Game, i: usize, at: Vec3) {
    let p = &mut g.players[i];
    p.alive = true;
    p.hp = 100.0;
    p.st.pos = vec3(at.x, g.world.height(at.x, at.z), at.z);
    p.st.vel = Vec3::ZERO;
    p.respawn_at = u32::MAX;
}

/// Take a player off the field for good.
fn bench(g: &mut Game, i: usize) {
    let p = &mut g.players[i];
    p.alive = false;
    p.hp = 0.0;
    p.respawn_at = u32::MAX;
}

/// A one-a-side game with a person on EMBER and the COBALT bot stood down.
fn duel(m: u8, score: u32) -> (Game, usize, usize) {
    let mut g = Game::new(Config { score_limit: score, ..cfg(m, 1) });
    let me = g.join_team("tester", 0).expect("slot");
    let foe = g.players.iter().position(|p| p.team == 1).expect("an enemy");
    for _ in 0..40 {
        g.tick();
    }
    assert!(g.players[me].alive && g.players[me].team == 0);
    bench(&mut g, foe);
    (g, me, foe)
}

fn flag_events(g: &Game) -> Vec<(u8, u8, u8)> {
    g.feed.iter().filter_map(|f| if let Feed::Flag { flag, by, what } = f { Some((*flag, *by, *what)) } else { None }).collect()
}

#[test]
fn modes_have_names_and_keys() {
    assert_eq!(MODES.len(), 4, "votes and snapshots carry four modes");
    assert_eq!(mode_by_name("tdm"), Some(mode::TDM));
    assert_eq!(mode_by_name(" CTF "), Some(mode::CTF));
    assert_eq!(mode_by_name("koth"), Some(mode::KOTH));
    assert_eq!(mode_by_name("Stronghold"), Some(mode::KOTH));
    assert_eq!(mode_by_name("team deathmatch"), Some(mode::TDM));
    assert_eq!(mode_by_name("0"), Some(mode::CONQUEST));
    assert_eq!(mode_by_name("9"), None);
    assert_eq!(mode_by_name("payload"), None);
    // The flags stand on mirrored objectives.
    assert_eq!(FLAG_POINT[0] + FLAG_POINT[1], ferrite_shared::NUM_POINTS - 1);
}

/// Conquest is the default and is untouched: tickets, a held point each.
#[test]
fn conquest_is_still_the_default() {
    let g = Game::new(cfg(0, 4));
    assert_eq!(g.mode, mode::CONQUEST);
    assert_eq!(g.tickets, [250, 250]);
    assert_eq!((g.points[0].owner, g.points[4].owner, g.points[2].owner), (0, 1, NEUTRAL));
    let s = g.snapshot(None, 0, 0);
    assert_eq!((s.mode, s.goal, s.secs_left), (mode::CONQUEST, 250, 0));
}

#[test]
fn deathmatch_counts_kills_and_ends_at_the_target() {
    let (mut g, me, foe) = duel(mode::TDM, 3);
    assert_eq!((g.tickets, g.goal), ([0, 0], 3));
    assert!(g.points.iter().all(|p| p.owner == NEUTRAL), "no objectives in a deathmatch");
    let s = g.snapshot(Some(me), 0, 0);
    assert_eq!((s.mode, s.goal), (mode::TDM, 3));
    assert!(s.secs_left > 400, "the clock is running: {}", s.secs_left);
    let at = g.players[me].st.pos + vec3(6.0, 0.0, 0.0);
    for k in 1..=3 {
        stand(&mut g, foe, at);
        let t = g.tick;
        g.damage(foe, Some(me), 1000.0, false, 0);
        assert_eq!(g.tickets, [k, 0]);
        // Back in three seconds, not on the next conquest wave.
        assert_eq!(g.players[foe].respawn_at, t + 180);
    }
    assert_eq!(g.phase, 0);
    g.tick();
    assert_eq!((g.phase, g.winner), (1, 0));
    assert!(matches!(g.feed.last(), Some(Feed::MatchEnd { winner: 0 })));
    // Dying to nobody, or by your own hand, scores for nobody.
    let (mut g, me, _) = duel(mode::TDM, 3);
    g.damage(me, None, 1000.0, false, ferrite_shared::sim::LAVA_WEAPON);
    assert_eq!(g.tickets, [0, 0]);
}

#[test]
fn deathmatch_spawns_forward_and_runs_out_the_clock() {
    let mut g = Game::new(Config { time_limit: 20, ..cfg(mode::TDM, 6) });
    for _ in 0..30 {
        g.tick();
    }
    // Everyone starts round B or D, not back at base.
    for p in g.players.iter().filter(|p| p.alive) {
        let c = g.world.points[if p.team == 0 { 1 } else { 3 }];
        assert!((p.st.pos - c).length() < 30.0, "{} spawned {:.0} m from the forward point", p.name, (p.st.pos - c).length());
    }
    for _ in 0..20 * 60 {
        g.tick();
    }
    assert_eq!(g.phase, 1, "the clock ends the round");
    assert!(g.winner < 2);
    if g.tickets[0] != g.tickets[1] {
        assert_eq!(g.winner, (g.tickets[1] > g.tickets[0]) as u8);
    }
}

#[test]
fn flags_are_taken_dropped_returned_and_captured() {
    let (mut g, me, foe) = duel(mode::CTF, 2);
    let (home, theirs) = (g.world.points[FLAG_POINT[0]], g.world.points[FLAG_POINT[1]]);
    assert_eq!((g.flags[0].state, g.flags[1].state), (0, 0));
    assert!((g.flags[1].pos - theirs).length() < 0.01);

    // Walk onto their flag: it is ours to carry.
    stand(&mut g, me, theirs);
    g.tick();
    assert_eq!((g.flags[1].state, g.flags[1].carrier), (1, me));
    assert_eq!(flag_events(&g), [(1, me as u8, flag_ev::TAKEN)]);
    // Our own flag is not something we can pick up.
    assert_eq!(g.flags[0].state, 0);
    let s = g.snapshot(Some(me), 0, 0);
    assert_eq!((s.flags[1].state, s.flags[1].carrier), (1, me as u8));
    assert!(s.players.iter().any(|p| p.id as usize == me && p.flags2 & flag2::CARRIER != 0));

    // It goes where we go, and lies where we fall.
    let mid = vec3(0.0, 0.0, 0.0);
    stand(&mut g, me, mid);
    g.tick();
    assert!((g.flags[1].pos - g.players[me].st.pos).length() < 0.5);
    g.damage(me, None, 1000.0, false, 0);
    g.tick();
    assert_eq!(g.flags[1].state, 2);
    assert_eq!(flag_events(&g).last(), Some(&(1, me as u8, flag_ev::DROPPED)));
    let fell = g.flags[1].pos;

    // Its own side touches it: home it goes.
    stand(&mut g, foe, fell);
    g.tick();
    assert_eq!(g.flags[1].state, 0);
    assert!((g.flags[1].pos - theirs).length() < 0.01);
    assert_eq!(flag_events(&g).last(), Some(&(1, foe as u8, flag_ev::RETURNED)));
    bench(&mut g, foe);

    // Left lying, it goes home by itself.
    stand(&mut g, me, theirs);
    g.tick();
    assert_eq!(g.flags[1].state, 1);
    stand(&mut g, me, mid);
    g.tick();
    bench(&mut g, me);
    g.tick();
    assert_eq!(g.flags[1].state, 2);
    for _ in 0..26 * 60 {
        g.tick();
    }
    assert_eq!(g.flags[1].state, 0);
    assert_eq!(flag_events(&g).last(), Some(&(1, SPECTATOR_ID, flag_ev::RETURNED)));

    // Carried all the way to our stand: a capture, and the flag resets.
    let score = g.players[me].score;
    for n in 1..=2 {
        stand(&mut g, me, theirs);
        g.tick();
        assert_eq!(g.flags[1].state, 1);
        stand(&mut g, me, home);
        g.tick();
        assert_eq!(g.flags[1].state, 0);
        assert_eq!(g.tickets, [n, 0]);
        assert_eq!(flag_events(&g).last(), Some(&(1, me as u8, flag_ev::CAPTURED)));
    }
    assert_eq!(g.players[me].score, score + 800);
    // Two was the target.
    assert_eq!((g.phase, g.winner), (1, 0));
}

#[test]
fn stronghold_scores_for_holding_the_centre() {
    let (mut g, me, foe) = duel(mode::KOTH, 5);
    assert!(g.points.iter().all(|p| p.owner == NEUTRAL));
    let centre = g.world.points[2];
    stand(&mut g, me, centre);
    // One person takes ten seconds to bring a neutral point over.
    for _ in 0..599 {
        g.tick();
    }
    assert_eq!(g.tickets, [0, 0], "nothing is scored before the point is held");
    for _ in 0..62 {
        g.tick();
    }
    assert_eq!(g.points[2].owner, 0);
    assert!(g.tickets[0] >= 1 && g.tickets[1] == 0);
    // The other objectives are not in play: standing on one does nothing.
    let e = g.world.points[4];
    stand(&mut g, foe, e);
    g.players[foe].bot = None; // (so it stays where it is put)
    for _ in 0..120 {
        g.tick();
    }
    assert_eq!(g.points[4].owner, NEUTRAL);
    assert_eq!(g.points[4].progress, 0.0);
    for _ in 0..240 {
        g.tick();
    }
    assert_eq!((g.phase, g.winner), (1, 0), "five points was the target: {:?}", g.tickets);
}

#[test]
fn streaks_are_announced_and_leave_their_reward() {
    assert_eq!(streak_reward(1), None);
    assert_eq!(streak_reward(3), Some(pickup::ARMOR));
    assert_eq!(streak_reward(5), Some(pickup::OVERDRIVE));
    assert_eq!(streak_reward(7), Some(pickup::OVERCHARGE));
    assert_eq!(streak_reward(8), None);
    assert_eq!(streak_reward(10), Some(pickup::OVERCHARGE));
    assert_eq!(streak_reward(15), Some(pickup::OVERCHARGE));

    let (mut g, me, foe) = duel(mode::TDM, 500);
    let at = g.players[me].st.pos + vec3(8.0, 0.0, 0.0);
    let mut rewards = Vec::new();
    for k in 1..=7u16 {
        g.pickups.clear();
        stand(&mut g, foe, at);
        g.damage(foe, Some(me), 1000.0, false, 0);
        assert_eq!(g.players[me].streak, k);
        if let Some(kind) = streak_reward(k) {
            // It lies where the victim fell.
            let p = g.pickups.iter().find(|p| p.kind == kind).unwrap_or_else(|| panic!("no reward for {k} kills"));
            assert!((p.pos - g.players[foe].st.pos).length() < 0.01);
            rewards.push(kind);
        }
    }
    assert_eq!(rewards, [pickup::ARMOR, pickup::OVERDRIVE, pickup::OVERCHARGE]);
    let streaks: Vec<u8> = g
        .feed
        .iter()
        .filter_map(|f| match f {
            Feed::Streak { who, n } if *who as usize == me => Some(*n),
            _ => None,
        })
        .collect();
    assert_eq!(streaks, [3, 5, 7]);
    assert_eq!(g.snapshot(Some(me), 0, 0).me.unwrap().streak, 7);

    // Whoever ends a long streak is paid for it; the streak starts again.
    g.players[foe].bot = None;
    stand(&mut g, foe, at);
    let before = g.players[foe].score;
    g.damage(me, Some(foe), 1000.0, false, 0);
    assert_eq!(g.players[me].streak, 0);
    assert_eq!(g.players[foe].score, before + 100 + 100);
    assert!(matches!(g.feed.iter().rev().find(|f| matches!(f, Feed::Shutdown { .. })), Some(Feed::Shutdown { n: 7, .. })));

    // With drops off, kills leave nothing behind.
    let mut g = Game::new(Config { drops: false, ..cfg(mode::TDM, 6) });
    for _ in 0..60 * 60 {
        g.tick();
        assert!(g.pickups.is_empty());
    }
    assert!(g.stats.kills > 5);
}

#[test]
fn pickups_do_what_they_say() {
    let (mut g, me, foe) = duel(mode::TDM, 500);
    let here = g.players[me].st.pos;
    let far = here + vec3(30.0, 0.0, 0.0);
    // Three kills: armour where the third fell.
    for _ in 0..3 {
        stand(&mut g, foe, far);
        g.damage(foe, Some(me), 1000.0, false, 0);
    }
    g.pickups.retain(|p| p.kind == pickup::ARMOR);
    assert_eq!(g.pickups.len(), 1);
    // Out of reach it stays put.
    g.tick();
    assert_eq!(g.pickups.len(), 1);
    let fx0 = g.fx_seq;
    let at = g.pickups[0].pos;
    stand(&mut g, me, at);
    g.tick();
    assert!(g.pickups.is_empty());
    assert_eq!(g.players[me].armor, 50.0);
    assert!(g.fx.iter().any(|(n, f)| *n > fx0 && matches!(f, Fx::Pickup { who, kind: pickup::ARMOR, .. } if *who as usize == me)));
    let s = g.snapshot(Some(me), 0, 0);
    assert_eq!(s.me.unwrap().armor, 50);
    assert!(s.players.iter().any(|p| p.id as usize == me && p.flags2 & flag2::ARMOR != 0));

    // Armour takes six parts in ten of a hit until it is gone.
    g.players[foe].bot = None;
    stand(&mut g, foe, far);
    g.damage(me, Some(foe), 50.0, false, 0);
    let near = |a: f32, b: f32| (a - b).abs() < 0.01;
    assert!(near(g.players[me].hp, 80.0) && near(g.players[me].armor, 20.0), "{} hp, {} armour", g.players[me].hp, g.players[me].armor);
    g.damage(me, Some(foe), 50.0, false, 0);
    assert!(near(g.players[me].hp, 50.0) && g.players[me].armor == 0.0, "{} hp, {} armour", g.players[me].hp, g.players[me].armor);

    // A health pack mends 60, never past full, and is left alone by
    // someone who does not need it.
    let drop = |g: &mut Game, kind: u8, at: Vec3| {
        g.pickups.push(ferrite_server::Pickup { id: 900 + kind as u16, kind, pos: at, until: g.tick + 600 });
    };
    let at = g.players[me].st.pos;
    drop(&mut g, pickup::HEALTH, at);
    g.tick();
    assert!(g.pickups.is_empty());
    assert!(g.players[me].hp >= 100.0, "50 + 60 stops at the class's 100");
    let at = g.players[me].st.pos;
    drop(&mut g, pickup::HEALTH, at);
    g.tick();
    assert_eq!(g.pickups.len(), 1, "full health: the pack is left for someone else");
    g.pickups.clear();

    // Ammunition: both weapons and the frags.
    {
        let p = &mut g.players[me];
        p.st.ammo = 1;
        p.st.ammo_alt = 0;
        p.st.grenades = 0;
    }
    let at = g.players[me].st.pos;
    drop(&mut g, pickup::AMMO, at);
    g.tick();
    let p = &g.players[me];
    let k = ferrite_shared::sim::kit(p.st.kit);
    assert_eq!(
        (p.st.ammo, p.st.ammo_alt, p.st.grenades),
        (ferrite_shared::sim::weapon_def(p.st.prim).mag, ferrite_shared::sim::weapon_def(p.st.side).mag, k.grenades)
    );

    // Overdrive is the capture surge; overcharge is half again the damage.
    let at = g.players[me].st.pos;
    drop(&mut g, pickup::OVERDRIVE, at);
    g.tick();
    g.tick();
    assert!(g.players[me].st.surge);
    let at = g.players[me].st.pos;
    drop(&mut g, pickup::OVERCHARGE, at);
    g.tick();
    assert!(g.snapshot(Some(me), 0, 0).me.unwrap().power >= 9);
    stand(&mut g, foe, far);
    g.damage(foe, Some(me), 40.0, false, 0);
    assert!(near(g.players[foe].hp, 40.0));
    // It wears off, and dying loses the lot.
    for _ in 0..601 {
        g.tick();
    }
    stand(&mut g, foe, far);
    g.damage(foe, Some(me), 40.0, false, 0);
    assert!(near(g.players[foe].hp, 60.0));
    g.players[me].armor = 70.0;
    g.damage(me, None, 1000.0, false, 0);
    assert_eq!(g.players[me].armor, 0.0);

    // Nothing lies about for ever.
    drop(&mut g, pickup::HEALTH, far + vec3(40.0, 0.0, 0.0));
    for _ in 0..601 {
        g.tick();
    }
    assert!(g.pickups.iter().all(|p| p.id != 900));
}

#[test]
fn winning_pays_more_than_losing() {
    // A full-length round, there for all of it.
    for score in [0, 400, 1200, 5000] {
        let (win, loss) = (round_credits(true, score, 1.0, 600.0), round_credits(false, score, 1.0, 600.0));
        assert!(win >= loss + 75, "score {score}: a win pays {win}, a loss {loss}");
        assert!((120..=240).contains(&win) && (45..=165).contains(&loss));
    }
    // A better score pays more, up to a ceiling.
    assert!(round_credits(true, 1000, 1.0, 600.0) > round_credits(true, 100, 1.0, 600.0));
    assert_eq!(round_credits(true, 5000, 1.0, 600.0), round_credits(true, 50000, 1.0, 600.0));
    // Joining late, or a round set up to end at once, pays in proportion.
    assert_eq!(round_credits(true, 0, 0.5, 600.0), 60);
    assert_eq!(round_credits(true, 0, 1.0, 90.0), 60);
    assert_eq!(round_credits(true, 5000, 1.0, 0.0), 0);
    assert_eq!(round_credits(false, 0, 0.0, 600.0), 0);

    // In a game: people are paid when the round ends, bots are not.
    let mut g = Game::new(Config { score_limit: 5000, ..cfg(mode::TDM, 2) });
    let me = g.join_team("winner", 0).expect("slot");
    let them = g.join_team("loser", 1).expect("slot");
    for _ in 0..200 * 60 {
        g.tick();
        if g.phase == 1 {
            break;
        }
        // (three minutes in, EMBER reaches the target)
        if g.tick >= 190 * 60 {
            g.tickets = [5000, 0];
        }
    }
    assert_eq!((g.phase, g.winner), (1, 0));
    let (round, won_credits, won) = g.players[me].award.expect("the winner is paid");
    let (_, lost_credits, lost) = g.players[them].award.expect("so is the loser");
    assert!(won && !lost);
    assert_eq!(round, g.round_no);
    assert!(won_credits >= 120 && lost_credits >= 45 && won_credits > lost_credits, "{won_credits} vs {lost_credits}");
    assert!(g.players.iter().filter(|p| p.bot.is_some()).all(|p| p.award.is_none()));
    // The next round starts with a clean slate.
    for _ in 0..11 * 60 {
        g.tick();
    }
    assert_eq!(g.phase, 0);
    assert!(g.players[me].award.is_none());
    assert_eq!(g.round_no, round + 1);
}

#[test]
fn modes_rotate_and_the_lobby_can_vote() {
    // A rotation moves on each round.
    let mut g = Game::new(Config { mode_rotation: vec![mode::TDM, mode::KOTH, mode::CTF], score_limit: 1, ..cfg(mode::TDM, 2) });
    let mut seen = vec![g.mode];
    for _ in 0..3 {
        assert_eq!(g.snapshot(None, 0, 0).next_mode, g.next_mode());
        g.tickets = [g.goal, 0];
        for _ in 0..11 * 60 {
            g.tick();
        }
        assert_eq!(g.phase, 0);
        seen.push(g.mode);
        // Each round begins as its mode should.
        assert_eq!(g.tickets, [0, 0]);
        assert!(g.flags.iter().all(|f| f.state == 0));
    }
    assert_eq!(seen, [mode::TDM, mode::KOTH, mode::CTF, mode::TDM]);

    // Without one the mode stays, unless the lobby votes.
    let mut g = Game::new(Config { lobby_secs: 20, ..cfg(mode::CONQUEST, 2) });
    let a = g.join("a").unwrap();
    let b = g.join("b").unwrap();
    g.tick();
    assert_eq!(g.phase, 2);
    assert_eq!(g.lobby_mode_choice(), mode::CONQUEST);
    g.players[a].mode_vote = mode::CTF;
    assert_eq!(g.mode_votes(), [0, 0, 1, 0]);
    assert_eq!(g.lobby_mode_choice(), mode::CTF);
    assert_eq!(g.snapshot(Some(a), 0, 0).next_mode, mode::CTF);
    assert_eq!(g.snapshot(Some(a), 0, 0).mode_votes, [0, 0, 1, 0]);
    // A tie with what was going to be played keeps it.
    g.players[b].mode_vote = mode::CONQUEST;
    assert_eq!(g.lobby_mode_choice(), mode::CONQUEST);
    g.players[b].mode_vote = mode::CTF;
    g.players[a].mode_vote = 99; // nonsense is no vote
    assert_eq!(g.mode_votes(), [0, 0, 1, 0]);
    g.players[a].ready = true;
    g.players[b].ready = true;
    for _ in 0..4 * 60 {
        g.tick();
    }
    assert_eq!((g.phase, g.mode), (0, mode::CTF));
    let s = g.snapshot(Some(a), 0, 0);
    assert_eq!((s.mode, s.goal), (mode::CTF, 3));
    assert!(s.secs_left > 600);
}

/// Bots play every mode to a finish, stay on the map, and the snapshot
/// still fits one packet with flags and pickups aboard.
#[test]
fn every_mode_plays_out_between_bots() {
    for m in [mode::TDM, mode::CTF, mode::KOTH] {
        let mut g = Game::new(Config { time_limit: 240, ..cfg(m, 12) });
        let mut biggest = 0;
        let mut most_pickups = 0;
        for _ in 0..(260 * 60) {
            g.tick();
            if g.tick.is_multiple_of(20) {
                for p in &g.players {
                    assert!(p.st.pos.is_finite(), "{} has non-finite position", p.name);
                    assert!(p.st.pos.x.abs() <= HALF_X && p.st.pos.z.abs() <= HALF_Z, "{} left the map", p.name);
                    assert!((0.0..=100.0).contains(&p.armor));
                    if p.alive {
                        assert!(p.hp > 0.0 && p.hp <= ferrite_shared::sim::kit(p.st.kit).hp);
                    }
                }
                assert!(g.pickups.len() <= 14);
                most_pickups = most_pickups.max(g.pickups.len());
                for f in &g.flags {
                    assert!(f.pos.is_finite() && f.state <= 2);
                    if f.state == 1 {
                        assert!(g.players[f.carrier].alive || m != mode::CTF || g.tick.is_multiple_of(20));
                    }
                }
                let snap = g.snapshot(Some(0), g.feed.len() as u32, g.fx_seq);
                biggest = biggest.max(encode(&S2C::Snap(snap)).len());
            }
        }
        let name = MODES[m as usize].name;
        assert!(g.stats.rounds >= 1, "{name}: no round finished");
        assert!(g.stats.kills > 60, "{name}: only {} kills", g.stats.kills);
        assert!(most_pickups > 0, "{name}: nothing was ever dropped");
        assert!(biggest < 1400, "{name}: a {biggest}-byte snapshot would fragment");
        match m {
            mode::CTF => assert!(flag_events(&g).iter().any(|e| e.2 == flag_ev::TAKEN), "{name}: no bot ever took a flag"),
            mode::KOTH => assert!(g.stats.captures >= 1, "{name}: the centre was never taken"),
            _ => assert!(g.history.iter().all(|r| r.mode == m)),
        }
    }
}

/// Over real UDP: the mode is announced, a mode vote is counted, and a
/// finished round's credits arrive.
#[test]
fn mode_vote_and_credits_cross_the_wire() {
    let mut server = ferrite_server::spawn(Config { lobby_secs: 30, score_limit: 1, time_limit: 2, ..cfg(mode::TDM, 2) }).expect("server");
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.connect(("127.0.0.1", server.addr.port())).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let mut buf = [0u8; 8192];
    let hello = C2S::Hello { cookie: 0, name: "voter".into(), spectator: false, team: 0, uuid: [9; 16], icon: Vec::new() };
    let (_, token) = handshake(&sock, &hello, Duration::from_secs(4)).expect("welcome");
    let start = Instant::now();
    let (mut seq, mut voted, mut played, mut award) = (0u32, false, None, None);
    let mut last = Instant::now() - Duration::from_secs(1);
    while start.elapsed() < Duration::from_secs(20) && award.is_none() {
        if last.elapsed() > Duration::from_millis(16) {
            last = Instant::now();
            seq += 1;
            let cmd = Cmd { seq, ..Default::default() };
            sock.send(&encode_c2s(
                token,
                &C2S::Cmds { cmds: vec![cmd], ack_feed: 0, kit: 0, prim: 0, side: 4, skin: 0, wskin: [0, 0], ack_chat: 0 },
            ))
            .unwrap();
            sock.send(&encode_c2s(token, &C2S::Lobby { vote: 255, ready: true, mode: mode::KOTH })).unwrap();
        }
        let Ok(n) = sock.recv(&mut buf) else { continue };
        match decode::<S2C>(&buf[..n]) {
            Some(S2C::Snap(s)) => {
                if s.phase == 2 && s.mode_votes == [0, 0, 0, 1] {
                    voted = true;
                    assert_eq!(s.next_mode, mode::KOTH);
                }
                if s.phase == 0 {
                    played = Some(s.mode);
                }
            }
            Some(S2C::Award { round, credits, won }) => award = Some((round, credits, won)),
            _ => {}
        }
    }
    server.stop();
    assert!(voted, "the vote never showed in a snapshot");
    assert_eq!(played, Some(mode::KOTH), "the lobby's choice was not played");
    let (round, credits, _) = award.expect("no credits arrived when the round ended");
    assert!(round >= 2);
    // Two seconds of a round pays next to nothing: no farming short rounds.
    assert!(credits <= 3, "a two-second round paid {credits}");
}
