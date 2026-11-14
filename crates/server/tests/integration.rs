use ferrite_server::{Config, Game};
use ferrite_shared::map::{HALF_X, HALF_Z};
use ferrite_shared::proto::*;
use ferrite_shared::rng::Rng;
use ferrite_shared::sim;
use ferrite_shared::{eligible, vec3, NEUTRAL};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

fn cfg(per_team: usize) -> Config {
    Config { port: 0, per_team, seed: 0xFE221, map: 0, rotation: Vec::new(), lobby_secs: 0, tickets: 250, bot_skill: 1.0, log: false, discover: false, ..Default::default() }
}

fn random_cmd(rng: &mut Rng, seq: u32, yaw: &mut f32) -> Cmd {
    *yaw += rng.range(-0.08, 0.08);
    let mut buttons = 0;
    if rng.chance(0.05) {
        buttons |= btn::JUMP;
    }
    if rng.chance(0.5) {
        buttons |= btn::SPRINT;
    }
    if rng.chance(0.2) {
        buttons |= btn::FIRE;
    }
    Cmd { seq, mx: rng.below(3) as i8 - 1, mz: if rng.chance(0.8) { 1 } else { -1 }, buttons, more: (seq % 37 == 0) as u8, yaw: *yaw, pitch: rng.range(-0.3, 0.3), interp: 0.0 }
}

/// The client predicts by running the same `sim::step`. With no loss the
/// predicted state must equal the authoritative one bit for bit.
#[test]
fn prediction_matches_server_exactly() {
    let mut g = Game::new(cfg(1));
    let me = g.join("tester").expect("slot");
    for _ in 0..40 {
        g.tick();
    }
    assert!(g.players[me].alive, "human should have spawned");
    let mut predicted = g.players[me].st;
    let mut rng = Rng::new(7);
    let mut yaw = g.players[me].yaw;
    let mut compared = 0;
    for seq in 1..=900u32 {
        let c = random_cmd(&mut rng, seq, &mut yaw);
        if !g.players[me].alive {
            break;
        }
        predicted.surge = g.players[me].st.surge;
        sim::step(&mut predicted, &c, &g.world, g.dmask);
        g.apply_cmd(me, c);
        assert_eq!(predicted, g.players[me].st, "divergence at seq {seq}");
        compared += 1;
        g.tick();
    }
    assert!(compared > 300, "only compared {compared} ticks");
}

/// Reconciliation after loss: replaying unacknowledged commands on top of a
/// snapshot must land on the same state as never having lost anything.
#[test]
fn replay_after_snapshot_converges() {
    let mut g = Game::new(cfg(1));
    let me = g.join("tester").unwrap();
    for _ in 0..40 {
        g.tick();
    }
    let mut rng = Rng::new(11);
    let mut yaw = 0.0;
    let mut local = g.players[me].st;
    let mut pending: Vec<Cmd> = Vec::new();
    for seq in 1..=300u32 {
        let c = random_cmd(&mut rng, seq, &mut yaw);
        sim::step(&mut local, &c, &g.world, g.dmask);
        pending.push(c);
        // The server only hears about commands in bursts of ten.
        if seq % 10 == 0 {
            for c in pending.iter().rev().take(10).rev() {
                g.apply_cmd(me, *c);
            }
        }
        g.tick();
        if seq % 7 == 0 && g.players[me].alive {
            let snap = g.snapshot(Some(me), 0, g.tick);
            let mut st = snap.me.unwrap().st;
            pending.retain(|c| c.seq > snap.ack_seq);
            for c in &pending {
                sim::step(&mut st, c, &g.world, g.dmask);
            }
            assert!((st.pos - local.pos).length() < 1e-4, "replay diverged at seq {seq}");
            local = st;
        }
    }
}

/// A client cannot buy extra simulated time by flooding commands.
#[test]
fn command_flood_is_rate_limited() {
    let mut g = Game::new(cfg(1));
    let me = g.join("speedy").unwrap();
    for _ in 0..40 {
        g.tick();
    }
    let start = g.players[me].st.pos;
    let yaw = g.players[me].yaw;
    for seq in 1..=600u32 {
        g.apply_cmd(me, Cmd { seq, mz: 1, buttons: btn::SPRINT, yaw, ..Default::default() });
    }
    let moved = (g.players[me].st.pos - start).length();
    // 600 commands = 10 s of sprinting (~80 m) if accepted. Budget is 12.
    assert!(moved < 3.0, "moved {moved} m from a single-tick flood");
    assert!(g.stats.dropped_cmds > 500);
    assert_eq!(g.players[me].last_seq, 600, "sequence still acknowledged so the client reconciles");
}

#[test]
fn hostile_input_is_clamped_or_ignored() {
    let mut g = Game::new(cfg(1));
    let me = g.join("x").unwrap();
    for _ in 0..40 {
        g.tick();
    }
    let before = g.players[me].st;
    g.apply_cmd(me, Cmd { seq: 1, yaw: f32::NAN, pitch: f32::INFINITY, mz: 1, ..Default::default() });
    assert_eq!(before, g.players[me].st, "non-finite view must not be simulated");
    g.apply_cmd(me, Cmd { seq: 2, mx: 127, mz: -128, pitch: 99.0, ..Default::default() });
    let p = &g.players[me];
    assert!(p.st.pos.is_finite() && p.pitch.abs() <= 1.56);
    // replayed / old sequence numbers do nothing
    let st = p.st;
    g.apply_cmd(me, Cmd { seq: 2, mz: 1, ..Default::default() });
    assert_eq!(st, g.players[me].st);
    // garbage on the wire decodes to nothing rather than panicking
    assert!(decode::<C2S>(&[]).is_none());
    assert!(decode::<C2S>(&[0xFF; 64]).is_none());
    let huge_len = [1u8, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F];
    assert!(decode::<C2S>(&huge_len).is_none(), "oversized length prefix must be rejected");
}

/// Five simulated minutes of 12v12 bots: the match must actually function
/// and never break its own invariants.
#[test]
fn bot_battle_soak_holds_invariants() {
    let mut g = Game::new(cfg(12));
    for _ in 0..(300 * 60) {
        g.tick();
        if g.tick % 20 == 0 {
            let o = g.owners();
            // Territory is contiguous: EMBER prefix, COBALT suffix.
            let first_not0 = o.iter().position(|&x| x != 0).unwrap_or(5);
            let last_not1 = o.iter().rposition(|&x| x != 1).map_or(0, |i| i + 1);
            assert!(o[first_not0..].iter().all(|&x| x != 0), "broken EMBER line {o:?}");
            assert!(o[..last_not1].iter().all(|&x| x != 1), "broken COBALT line {o:?}");
            for p in &g.players {
                assert!(p.st.pos.is_finite(), "{} has non-finite position", p.name);
                assert!(p.st.pos.x.abs() <= HALF_X && p.st.pos.z.abs() <= HALF_Z, "{} left the map", p.name);
                if p.alive {
                    assert!(p.st.pos.y >= g.world.height(p.st.pos.x, p.st.pos.z) - 0.05, "{} fell through terrain", p.name);
                    assert!(p.hp > 0.0 && p.hp <= sim::kit(p.st.kit).hp);
                }
            }
        }
    }
    assert!(g.stats.kills > 100, "kills {}", g.stats.kills);
    assert!(g.stats.captures >= 3, "captures {}", g.stats.captures);
    let snap = g.snapshot(Some(0), g.feed.len() as u32, g.tick);
    let bytes = encode(&S2C::Snap(snap)).len();
    assert!(bytes < 1400, "snapshot {bytes} bytes would fragment");
}

#[test]
fn front_line_rule() {
    let o = [0, 0, NEUTRAL, 1, 1];
    assert!(eligible(&o, 0, 2) && eligible(&o, 1, 2));
    assert!(!eligible(&o, 0, 3), "EMBER cannot skip past the neutral centre");
    assert!(!eligible(&o, 1, 1));
    let start = [0, NEUTRAL, NEUTRAL, NEUTRAL, 1];
    assert!(eligible(&start, 0, 1) && !eligible(&start, 0, 2));
}

/// Real sockets: join over UDP, drive the player, watch acknowledgements
/// advance in snapshots, then leave.
#[test]
fn udp_round_trip() {
    let mut server = ferrite_server::spawn(cfg(4)).expect("server");
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.connect(("127.0.0.1", server.addr.port())).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    let mut buf = [0u8; 8192];

    // wrong protocol version is refused with a reason
    sock.send(&encode(&C2S::Hello { proto: PROTO + 1, name: "old".into(), spectator: false, team: 2, uuid: [0; 16], icon: Vec::new() })).unwrap();
    let n = sock.recv(&mut buf).unwrap();
    assert!(matches!(decode::<S2C>(&buf[..n]), Some(S2C::Reject(_))));

    // An id and a 16 x 16 icon go up with the hello.
    let icon: Vec<u8> = (0..ICON_BYTES).map(|k| (k * 7 % 251) as u8).collect();
    sock.send(&encode(&C2S::Hello { proto: PROTO, name: "net-test".into(), spectator: false, team: 2, uuid: [7; 16], icon: icon.clone() })).unwrap();
    let (mut id, mut seed, mut first_team) = (SPECTATOR_ID, 0, 0u8);
    for _ in 0..10 {
        let n = sock.recv(&mut buf).unwrap();
        if let Some(S2C::Welcome { id: i, seed: s, team, .. }) = decode::<S2C>(&buf[..n]) {
            id = i;
            seed = s;
            first_team = team;
            break;
        }
    }
    assert_ne!(id, SPECTATOR_ID, "no welcome");
    assert_eq!(seed, 0xFE221);

    let start = Instant::now();
    let (mut seq, mut max_ack, mut snaps, mut saw_self, mut in_roster) = (0u32, 0u32, 0, false, false);
    let mut last_send = Instant::now() - Duration::from_secs(1);
    let mut history: Vec<Cmd> = Vec::new();
    let (mut asked_switch, mut roster_team) = (false, first_team);
    let (mut parts, mut got_total) = (Vec::new(), 0usize);
    let (mut ack_chat, mut heard, mut icon_listed, mut icon_back) = (0u32, Vec::new(), false, false);
    let requested_finishes = [1, 6];
    let (mut saw_finish_loadout, mut saw_finishes) = (false, false);
    sock.set_read_timeout(Some(Duration::from_millis(4))).unwrap();
    while start.elapsed() < Duration::from_millis(1500) {
        if last_send.elapsed() >= Duration::from_micros(16_666) {
            last_send = Instant::now();
            seq += 1;
            history.push(Cmd { seq, mz: 1, yaw: 1.0, ..Default::default() });
            let recent: Vec<Cmd> = history.iter().rev().take(3).rev().copied().collect();
            sock.send(&encode(&C2S::Cmds { cmds: recent, ack_feed: 0, kit: 1, prim: 1, side: 4, skin: 2, wskin: requested_finishes, ack_chat })).unwrap();
            // Said three times, heard once.
            if seq == 20 || seq == 21 || seq == 22 {
                sock.send(&encode(&C2S::Chat { id: 1, team: false, text: "hello  there\u{7}".into() })).unwrap();
            }
            if seq == 30 {
                sock.send(&encode(&C2S::WantIcon(id))).unwrap();
            }
        }
        if !asked_switch && start.elapsed() > Duration::from_millis(600) {
            asked_switch = true;
            sock.send(&encode(&C2S::Team(1 - first_team))).unwrap();
        }
        if let Ok(n) = sock.recv(&mut buf) {
            match decode::<S2C>(&buf[..n]) {
                Some(S2C::Snap(s)) => {
                    snaps += 1;
                    max_ack = max_ack.max(s.ack_seq);
                    if let Some(me) = s.me {
                        saw_self |= me.alive;
                        saw_finish_loadout |= me.alive && me.st.prim == 1 && me.st.side == 4;
                    }
                }
                Some(S2C::Roster { at, total, entries }) => {
                    assert!(entries.len() <= ROSTER_PART);
                    assert!(n < 1200, "a roster part of {n} bytes");
                    if at == 0 {
                        parts.clear();
                    }
                    if at as usize == parts.len() {
                        parts.extend(entries);
                    }
                    if parts.len() == total as usize {
                        got_total = parts.len();
                        let r = &parts;
                        in_roster |= r.iter().any(|e| e.id == id && e.name == "net-test" && !e.bot);
                        if let Some(e) = r.iter().find(|e| e.id == id) {
                            roster_team = e.team;
                            saw_finishes |= e.prim == 1 && e.side == 4 && e.wskin == requested_finishes;
                        }
                    }
                }
                Some(S2C::Chat(lines)) => {
                    for l in lines {
                        if l.seq > ack_chat {
                            ack_chat = l.seq;
                            heard.push(l);
                        }
                    }
                }
                Some(S2C::Icons(list)) => icon_listed |= list.contains(&(id, icon_hash(&icon))),
                Some(S2C::Icon { id: who, hash, rgba }) => icon_back |= who == id && hash == icon_hash(&icon) && rgba == icon,
                _ => {}
            }
        }
    }
    assert!(snaps >= 30, "only {snaps} snapshots in 1.5 s (expected ~45)");
    assert!(max_ack + 12 >= seq, "server acked {max_ack} of {seq}");
    assert!(saw_self && in_roster);
    assert!(saw_finish_loadout && saw_finishes, "issued weapons reach snapshots and non-factory finishes reach the roster");
    assert_eq!(got_total, 8, "the roster arrives whole, in parts");
    let said: Vec<_> = heard.iter().filter(|l| l.from == id).collect();
    assert_eq!(said.len(), 1, "{heard:?}");
    assert_eq!((said[0].text.as_str(), said[0].name.as_str(), said[0].all), ("hello  there", "net-test", true));
    assert!(icon_listed && icon_back, "icon listed {icon_listed}, sent back {icon_back}");
    sock.send(&encode(&C2S::Bye)).unwrap();
    assert_eq!(roster_team, 1 - first_team, "team change request was applied");
    server.stop();
}

/// Crouching halves speed, lowers the hit volume and stays predicted.
#[test]
fn crouch_is_slower_and_lower() {
    let mut g = Game::new(cfg(1));
    let me = g.join("ducker").unwrap();
    for _ in 0..40 {
        g.tick();
    }
    let yaw = g.players[me].yaw;
    let run = |g: &mut Game, from: u32, buttons: u8| {
        let start = g.players[me].st.pos;
        for seq in from..from + 60 {
            g.apply_cmd(me, Cmd { seq, mz: 1, buttons, yaw, ..Default::default() });
            g.tick();
        }
        let d = g.players[me].st.pos - start;
        (d.x * d.x + d.z * d.z).sqrt()
    };
    let walked = run(&mut g, 1, 0);
    let crouched = run(&mut g, 100, btn::CROUCH);
    assert!(g.players[me].st.crouch);
    assert!(crouched < walked * 0.65, "crouch {crouched} m vs walk {walked} m");
    let snap = g.snapshot(None, 0, g.tick);
    assert!(snap.players.iter().any(|p| p.id as usize == me && p.flags & flag::CROUCH != 0));

    // A chest-height shot hits a standing player and passes over a crouched one.
    let pos = ferrite_shared::vec3(0.0, 0.0, 10.0);
    let o = ferrite_shared::vec3(0.0, 1.3, 0.0);
    let d = ferrite_shared::vec3(0.0, 0.0, 1.0);
    assert!(sim::ray_player(o, d, pos, false).is_some());
    assert!(sim::ray_player(o, d, pos, true).is_none());
    assert!(sim::spread(0, false, 0.0, false, true, 0) < sim::spread(0, false, 0.0, false, false, 0));
}

/// Sidearm: swapping keeps separate magazines, blocks fire during the draw,
/// and the pistol is semi-automatic (holding the trigger fires once).
#[test]
fn sidearm_swap_and_semi_auto() {
    let mut g = Game::new(cfg(1));
    let me = g.join("gunner").unwrap();
    for _ in 0..40 {
        g.tick();
    }
    let yaw = g.players[me].yaw;
    let mut seq = 0;
    let mut send = |g: &mut Game, buttons: u8| {
        seq += 1;
        g.apply_cmd(me, Cmd { seq, buttons, yaw, pitch: 1.2, ..Default::default() });
        g.tick();
    };
    for _ in 0..12 {
        send(&mut g, btn::FIRE);
    }
    let primary_left = g.players[me].st.ammo;
    assert_eq!(primary_left, 28, "carbine fires every 6 ticks");
    send(&mut g, btn::SWAP);
    assert_eq!(g.players[me].st.slot, 1);
    assert_eq!(sim::wid(&g.players[me].st), sim::SIDEARM_ID);
    assert_eq!(g.players[me].st.ammo, sim::weapon_def(sim::SIDEARM_ID).mag);
    assert_eq!(g.players[me].st.ammo_alt, primary_left);
    send(&mut g, btn::FIRE);
    assert_eq!(g.players[me].st.ammo, sim::weapon_def(sim::SIDEARM_ID).mag, "cannot fire while drawing");
    for _ in 0..30 {
        send(&mut g, 0);
    }
    for _ in 0..40 {
        send(&mut g, btn::FIRE); // held
    }
    assert_eq!(g.players[me].st.ammo, sim::weapon_def(sim::SIDEARM_ID).mag - 1, "held trigger = one shot");
    for _ in 0..5 {
        send(&mut g, 0);
        for _ in 0..9 {
            send(&mut g, btn::FIRE);
        }
    }
    assert_eq!(g.players[me].st.ammo, sim::weapon_def(sim::SIDEARM_ID).mag - 6, "one shot per pull");
    let snap = g.snapshot(None, 0, g.tick);
    assert!(snap.players.iter().any(|p| p.id as usize == me && p.flags & flag::SIDEARM != 0));
    send(&mut g, btn::SWAP);
    assert_eq!((g.players[me].st.slot, g.players[me].st.ammo), (0, primary_left));
}

#[test]
fn team_choice_and_switch() {
    let mut g = Game::new(cfg(4));
    // A requested side is honoured even when auto-balance would say otherwise.
    let a = g.join_team("a", 1).unwrap();
    let b = g.join_team("b", 1).unwrap();
    assert_eq!((g.players[a].team, g.players[b].team), (1, 1));
    let count = |g: &Game, t: u8| g.players.iter().filter(|p| p.team == t).count();
    assert_eq!((count(&g, 0), count(&g, 1)), (4, 4));
    for _ in 0..600 {
        g.tick();
    }
    // Switching trades places with a bot: sizes hold, no death is charged.
    let (deaths, tickets) = (g.players[a].deaths, g.tickets);
    assert!(g.set_team(a, 0));
    assert_eq!(g.players[a].team, 0);
    assert!(!g.players[a].alive);
    assert_eq!(g.players[a].deaths, deaths);
    assert_eq!(g.tickets, tickets);
    assert_eq!((count(&g, 0), count(&g, 1)), (4, 4));
    for _ in 0..300 {
        g.tick();
    }
    assert!(g.players[a].alive, "redeploys on the new side");
    // A side with no bots left cannot be joined.
    let mut ids = vec![];
    while let Some(i) = g.join_team("x", 0) {
        if g.players[i].team != 0 {
            g.leave(i);
            break;
        }
        ids.push(i);
    }
    assert!(g.players.iter().filter(|p| p.team == 0).all(|p| p.bot.is_none()));
    assert!(!g.set_team(b, 0));
    assert_eq!(g.players[b].team, 1);
}

/// Sprint then crouch: a slide carries speed that a crouch-walk never has,
/// and it shows up in snapshots so other clients can animate it.
#[test]
fn slide_keeps_sprint_speed() {
    use ferrite_shared::map::World;
    use ferrite_shared::vec3;
    let w = World::new(0xFE221);
    let (x, z) = (-106.0, -12.0);
    let mut st = sim::PState::spawn(vec3(x, w.height(x, z), z), 0);
    let mut seq = 0;
    let mut send = |st: &mut sim::PState, buttons: u8| {
        seq += 1;
        sim::step(st, &Cmd { seq, mz: 1, buttons, yaw: 0.0, ..Default::default() }, &w, 0)
    };
    for _ in 0..50 {
        send(&mut st, btn::SPRINT);
    }
    let start = st.pos;
    assert!(send(&mut st, btn::SPRINT | btn::CROUCH).slid, "crouching at a sprint starts a slide: {st:?}");
    for _ in 0..24 {
        send(&mut st, btn::CROUCH);
    }
    assert!(st.slide > 0 && st.crouch);
    let slid = (st.pos - start).length();
    assert!(slid > 2.6, "slid only {slid} m in 25 ticks");
    for _ in 0..60 {
        send(&mut st, btn::CROUCH);
    }
    assert_eq!(st.slide, 0, "slides end");
    assert!(st.crouch);
    // A second slide needs a fresh sprint.
    assert!(!send(&mut st, btn::CROUCH).slid);

    // Other clients are told, so they can animate it.
    let mut g = Game::new(cfg(1));
    g.players[0].st.slide = 10;
    g.players[0].st.reload = 10;
    let snap = g.snapshot(None, 0, g.tick);
    assert!(snap.players.iter().any(|p| p.id == 0 && p.flags2 & flag2::SLIDE != 0 && p.flags2 & flag2::RELOAD != 0));
}

/// Holding jump against a ledge within reach climbs it; a tall wall does not.
#[test]
fn mantle_climbs_reachable_ledges_only() {
    use ferrite_shared::map::{Block, BlockKind, World};
    use ferrite_shared::vec3;
    let climb = |wall_h: f32| {
        let mut w = World::new(0xFE221);
        let (x, z) = (-106.0, 4.0);
        let g = w.height(x, z);
        w.blocks.push(Block { min: vec3(x - 3.0, g - 2.0, z + 2.0), max: vec3(x + 3.0, g + wall_h, z + 40.0), kind: BlockKind::Concrete, tint: 0, dest: None });
        let mut st = sim::PState::spawn(vec3(x, g, z), 0);
        let mut mantled = false;
        for seq in 1..110 {
            let c = Cmd { seq, mz: 1, buttons: btn::JUMP, yaw: 0.0, ..Default::default() };
            mantled |= sim::step(&mut st, &c, &w, 0).mantle;
        }
        (st.pos.y - g, st.pos.z - z, mantled)
    };
    let (up, fwd, mantled) = climb(1.9);
    assert!(mantled && up > 1.8 && fwd > 2.0, "1.9 m ledge: up {up}, forward {fwd}, mantled {mantled}");
    let (up, fwd, _) = climb(3.2);
    assert!(up < 1.5 && fwd < 2.0, "3.2 m wall must not be climbable: up {up}, forward {fwd}");
}

/// Sustained fire opens the cone and it closes again off the trigger.
#[test]
fn sustained_fire_blooms_and_recovers() {
    let mut g = Game::new(cfg(1));
    let me = g.join("sprayer").unwrap();
    for _ in 0..40 {
        g.tick();
    }
    let yaw = g.players[me].yaw;
    let mut seq = 0;
    let mut send = |g: &mut Game, buttons: u8| {
        seq += 1;
        g.apply_cmd(me, Cmd { seq, buttons, yaw, pitch: 1.2, ..Default::default() });
        g.tick();
    };
    for _ in 0..70 {
        send(&mut g, btn::FIRE);
    }
    let hot = g.players[me].st.heat;
    assert!(hot >= 80, "heat after a long burst: {hot}");
    assert!(sim::spread(0, false, 0.0, false, false, hot) > sim::spread(0, false, 0.0, false, false, 0) * 1.8);
    for _ in 0..40 {
        send(&mut g, 0);
    }
    assert_eq!(g.players[me].st.heat, 0);
}

/// Whoever did real damage before someone else lands the kill gets an assist.
#[test]
fn assists_are_credited() {
    let mut g = Game::new(cfg(2));
    let (victim, a, b) = (1, 0, 2);
    assert!(g.players[victim].team != g.players[a].team && g.players[a].team == g.players[b].team);
    g.damage(victim, Some(a), 60.0, false, 0);
    g.damage(victim, Some(b), 200.0, false, 0);
    assert!(!g.players[victim].alive);
    match g.feed.last() {
        Some(Feed::Kill { killer, assist, .. }) => assert_eq!((*killer, *assist), (b as u8, a as u8)),
        other => panic!("expected a kill, got {other:?}"),
    }
    assert_eq!(g.players[a].score, 50);
    assert_eq!(g.players[b].score, 100);
}

/// Every map must be fair (mirrored objectives and ground), navigable (a
/// bot can walk from either base to every point), and actually play: a
/// three-minute bot war on it takes ground without breaking the world.
#[test]
fn every_map_is_fair_and_playable() {
    use ferrite_shared::map::{map_by_name, Nav, World, MAPS};
    assert_eq!(map_by_name("quarry"), Some(1));
    assert_eq!(map_by_name("RIDGE"), Some(2));
    assert_eq!(map_by_name("3"), Some(3));
    assert_eq!(map_by_name("foundry valley"), Some(0));
    assert_eq!(map_by_name("nowhere"), None);
    assert_eq!(map_by_name("9"), None);

    // The original map is untouched by the map table.
    let (old, new) = (World::new(0xFE221), World::with_map(0xFE221, 0));
    assert_eq!(old.heights, new.heights);
    assert_eq!(old.blocks.len(), new.blocks.len());

    for id in 0..MAPS.len() as u8 {
        let name = MAPS[id as usize].name;
        let w = World::with_map(0xFE221, id);
        for k in 0..5 {
            let (p, q) = (w.points[k], w.points[4 - k]);
            assert!((p.x + q.x).abs() < 1e-3 && (p.z + q.z).abs() < 1e-3 && (p.y - q.y).abs() < 0.05, "{name}: point {k} is not mirrored");
        }
        let mut x = -117.0;
        while x < 117.0 {
            let mut z = -67.0;
            while z < 67.0 {
                assert!((w.height(x, z) - w.height(-x, -z)).abs() < 0.02, "{name}: ground differs at {x},{z}");
                z += 3.7;
            }
            x += 4.3;
        }
        let west = w.blocks.iter().filter(|b| b.min.x + b.max.x < -0.01).count();
        let east = w.blocks.iter().filter(|b| b.min.x + b.max.x > 0.01).count();
        assert_eq!(west, east, "{name}: cover is not mirrored");

        let nav = Nav::build(&w);
        for goal in 0..5 {
            for base in 0..2 {
                for lane in 0..3 {
                    let (mut px, mut pz) = (w.bases[base].x, w.bases[base].z);
                    let target = w.points[goal];
                    let mut reached = false;
                    for _ in 0..1200 {
                        if ((px - target.x).powi(2) + (pz - target.z).powi(2)).sqrt() < 6.5 {
                            reached = true;
                            break;
                        }
                        let d = nav.dir(goal, lane, px, pz);
                        px += d.x * 0.6;
                        pz += d.y * 0.6;
                    }
                    assert!(reached, "{name}: no route from base {base} to point {goal} (lane {lane}); stuck at {px:.1},{pz:.1}");
                }
            }
        }

        let mut g = Game::new(Config { map: id, ..cfg(10) });
        assert_eq!(g.world.map, id);
        for _ in 0..(180 * 60) {
            g.tick();
            if g.tick % 30 == 0 {
                for p in &g.players {
                    assert!(p.st.pos.is_finite() && p.st.pos.x.abs() <= HALF_X && p.st.pos.z.abs() <= HALF_Z, "{name}: {} left the map", p.name);
                    if p.alive {
                        assert!(p.st.pos.y >= g.world.height(p.st.pos.x, p.st.pos.z) - 0.05, "{name}: {} fell through terrain", p.name);
                    }
                }
            }
        }
        assert!(g.stats.kills > 40, "{name}: only {} kills in three minutes", g.stats.kills);
        assert!(g.stats.captures >= 2, "{name}: only {} captures in three minutes", g.stats.captures);
    }
}

#[test]
fn loadouts_are_validated_and_carried() {
    use ferrite_shared::sim::{fix_loadout, kit, weapon_def, PState, KITS, SIDEARMS, WEAPONS};
    // Every class offers three distinct primaries and no handguns.
    for k in 0..KITS.len() as u8 {
        let p = kit(k).prims;
        assert!(p.len() >= 3);
        for (n, id) in p.iter().enumerate() {
            assert!(!p[..n].contains(id), "duplicate primary");
        }
        for &id in p {
            assert!((id as usize) < WEAPONS.len() && weapon_def(id).arch != 4 && id != 3);
        }
    }
    for id in SIDEARMS {
        assert_eq!(weapon_def(id).arch, 4);
    }
    // A primary from another class, or nonsense, falls back to the default.
    assert_eq!(fix_loadout(0, 2, 4), (0, 0, 4));
    assert_eq!(fix_loadout(1, 200, 200), (1, 1, 4));
    assert_eq!(fix_loadout(2, 9, 10), (2, 9, 10));
    assert_eq!(fix_loadout(0, 10, 0), (0, 0, 4)); // a handgun is not a primary
    let st = PState::spawn_with(ferrite_shared::Vec3::ZERO, 2, 9, 11);
    assert_eq!((st.prim, st.side, st.ammo, st.ammo_alt), (9, 11, weapon_def(9).mag, weapon_def(11).mag));

    // Through the server: the request is applied at the next deploy.
    let mut g = Game::new(Config { per_team: 2, ..Default::default() });
    let me = g.join("tester").unwrap();
    g.players[me].next_kit = 1;
    g.players[me].next_prim = 7;
    g.players[me].next_side = 10;
    for _ in 0..40 {
        g.tick();
    }
    assert!(g.players[me].alive);
    assert_eq!((g.players[me].st.kit, g.players[me].st.prim, g.players[me].st.side), (1, 7, 10));
    assert_eq!(sim::wid(&g.players[me].st), 7);
    let r = g.roster();
    assert_eq!((r[me].prim, r[me].side), (7, 10));
    // Bots only ever carry what their class allows.
    for p in &g.players {
        assert!(kit(p.st.kit).prims.contains(&p.st.prim) && SIDEARMS.contains(&p.st.side));
    }
}

#[test]
fn maps_rotate_between_rounds() {
    let mut g = Game::new(Config { per_team: 3, tickets: 6, map: 1, rotation: vec![1, 3, 0], ..Default::default() });
    assert_eq!((g.world.map, g.next_map()), (1, 3));
    let mut seen = vec![g.world.map];
    for _ in 0..60 * 60 * 20 {
        g.tick();
        if *seen.last().unwrap() != g.world.map {
            seen.push(g.world.map);
            // A fresh round on the new map: everyone is standing on it.
            assert_eq!(g.phase, 0);
            for p in g.players.iter().filter(|p| p.alive) {
                let gy = g.world.height(p.st.pos.x, p.st.pos.z);
                assert!(p.st.pos.y > gy - 0.5, "{} is under the new map", p.name);
            }
            assert_eq!(g.snapshot(None, 0, 0).map, g.world.map);
        }
        if seen.len() == 4 {
            break;
        }
    }
    assert_eq!(seen, vec![1, 3, 0, 1], "rotation order");
    // With no rotation the map never changes.
    let mut g = Game::new(Config { per_team: 3, tickets: 6, map: 2, ..Default::default() });
    for _ in 0..60 * 60 * 6 {
        g.tick();
    }
    assert!(g.stats.rounds >= 1);
    assert_eq!((g.world.map, g.next_map()), (2, 2));
}

#[test]
fn effects_raised_between_ticks_are_still_delivered() {
    // A human's commands are simulated when they arrive, between ticks, so
    // the effects they cause (hit markers above all) appear after that
    // tick's snapshot has gone out. They must be in the next one.
    let mut g = Game::new(Config { per_team: 2, ..Default::default() });
    let me = g.join("tester").unwrap();
    for _ in 0..40 {
        g.tick();
    }
    assert!(g.players[me].alive && g.players[me].st.grenades > 0);
    let _already_sent = g.snapshot(Some(me), 0, g.fx_seq);
    let sent = g.fx_seq;
    g.apply_cmd(me, Cmd { seq: 1, buttons: btn::GRENADE, ..Default::default() });
    assert!(g.fx_seq > sent, "throwing raises an effect");
    let next = g.snapshot(Some(me), 0, sent);
    assert!(next.fx.iter().any(|f| matches!(f, Fx::Throw { who } if *who as usize == me)), "effect lost: {:?}", next.fx);
    assert!(g.snapshot(Some(me), 0, g.fx_seq).fx.is_empty(), "and it is sent only once");
}

#[test]
fn lobby_waits_counts_down_and_honours_the_vote() {
    let mut g = Game::new(Config { per_team: 2, lobby_secs: 20, tickets: 4, ..Default::default() });
    for _ in 0..300 {
        g.tick();
    }
    // Nobody here: the lobby waits, nobody deploys.
    let s = g.snapshot(None, 0, 0);
    assert_eq!((s.phase, s.phase_secs), (2, 0.0));
    assert!(g.players.iter().all(|p| !p.alive));
    let me = g.join("tester").unwrap();
    g.tick();
    let s = g.snapshot(Some(me), 0, 0);
    assert!(s.phase == 2 && s.phase_secs > 19.0, "countdown starts with the first player: {}", s.phase_secs);
    assert_eq!(s.next_map, 0);
    g.players[me].vote = 2;
    let s = g.snapshot(Some(me), 0, 0);
    assert_eq!((s.next_map, s.votes[2]), (2, 1));
    for _ in 0..120 {
        g.tick();
    }
    assert_eq!(g.phase, 2, "still counting down");
    // Everyone ready: three seconds to go.
    g.players[me].ready = true;
    g.tick();
    assert!(g.snapshot(Some(me), 0, 0).phase_secs <= 3.0);
    for _ in 0..200 {
        g.tick();
    }
    assert_eq!((g.phase, g.world.map), (0, 2));
    assert!(g.players[me].alive);
    assert!(g.roster()[me].ready);
    // After the round the lobby comes back with a clean slate.
    let mut back = false;
    for _ in 0..60 * 60 * 10 {
        g.tick();
        if g.phase == 2 {
            back = true;
            break;
        }
    }
    assert!(back, "lobby returns after the round");
    assert!(!g.players[me].ready && g.players[me].vote == 255 && !g.players[me].alive);
}

#[test]
fn burst_rifle_fires_three_rounds_per_pull() {
    use ferrite_shared::map::World;
    use ferrite_shared::vec3;
    let w = World::new(0xFE221);
    let (x, z) = (-106.0, -12.0);
    let mut st = sim::PState::spawn_with(vec3(x, w.height(x, z), z), 0, 12, 4);
    assert_eq!(sim::wid(&st), 12);
    let mut seq = 0;
    let mut fired = 0;
    let mut send = |st: &mut sim::PState, buttons: u8| {
        seq += 1;
        sim::step(st, &Cmd { seq, buttons, ..Default::default() }, &w, 0).fired as u32
    };
    fired += send(&mut st, btn::FIRE); // one tap
    for _ in 0..30 {
        fired += send(&mut st, 0);
    }
    assert_eq!(fired, 3, "a tap fires the whole burst");
    assert_eq!(st.ammo, sim::weapon_def(12).mag - 3);
    // Holding the trigger fires burst after burst, with a pause between.
    let mut held = 0;
    for _ in 0..48 {
        held += send(&mut st, btn::FIRE);
    }
    assert_eq!(held, 6, "two bursts in 0.8 s");
}

#[test]
fn servers_answer_discovery_queries() {
    let info = ServerInfo { proto: PROTO, id: 7, port: 4242, name: "box".into(), map: 3, humans: 2, slots: 24, phase: 2 };
    assert_eq!(decode_info(&encode_info(&info)), Some(info));
    assert_eq!(decode_info(b"FERRITE!"), None);
    assert_eq!(decode_info(b"nonsense"), None);

    let mut server = ferrite_server::spawn(Config { port: 0, per_team: 2, map: 1, name: "test box".into(), ..Default::default() }).expect("server");
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    sock.set_multicast_loop_v4(true).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    let mut found = None;
    let mut buf = [0u8; 512];
    'outer: for _ in 0..40 {
        // (On Windows a query sent before the server is listening makes the
        // next read fail at once, so pace the retries.)
        std::thread::sleep(Duration::from_millis(50));
        sock.send_to(DISCOVERY_QUERY, ("127.0.0.1", DISCOVERY_PORT)).unwrap();
        // Windows may deliver unicast to only one socket sharing this
        // port. Multicast also reaches our test server with a game open.
        let _ = sock.send_to(DISCOVERY_QUERY, (std::net::Ipv4Addr::from(DISCOVERY_GROUP), DISCOVERY_PORT));
        // Other servers on this machine may answer too: look for ours.
        while let Ok((n, _)) = sock.recv_from(&mut buf) {
            if let Some(i) = decode_info(&buf[..n]) {
                if i.port == server.addr.port() {
                    found = Some(i);
                    break 'outer;
                }
            }
        }
    }
    server.stop();
    let i = found.expect("no discovery reply");
    assert_eq!((i.proto, i.name.as_str(), i.map, i.humans, i.slots), (PROTO, "test box", 1, 0, 4));
}

#[test]
fn difficulty_presets_have_names() {
    use ferrite_server::{difficulty_by_name, DIFFICULTIES};
    assert_eq!(difficulty_by_name("regular"), Some(1.0));
    assert_eq!(difficulty_by_name(" Elite "), Some(DIFFICULTIES[3].1));
    assert_eq!(difficulty_by_name("nightmare"), None);
    // Harder presets are strictly more skilled.
    assert!(DIFFICULTIES.windows(2).all(|w| w[0].1 < w[1].1));
}

#[test]
fn rounds_and_player_records_are_kept() {
    let file = std::env::temp_dir().join(format!("ferrite-stats-test-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let conf = || Config { per_team: 3, tickets: 6, discover: false, stats_file: Some(file.clone()), ..Default::default() };
    let mut g = Game::new(conf());
    let me = g.join("tester").unwrap();
    // Play two rounds to the end, noting what the roster said last.
    let mut seen = Vec::new();
    for _ in 0..60 * 60 * 20 {
        let before = g.stats.rounds;
        g.tick();
        if g.stats.rounds != before {
            seen.push(g.roster()[me].clone());
            if g.stats.rounds == 2 {
                break;
            }
        }
    }
    assert_eq!(g.history.len(), 2, "two rounds finished");
    assert_eq!(g.wins[0] + g.wins[1], 2);
    for r in &g.history {
        assert!(r.winner < 2 && !r.mvp.is_empty() && r.secs > 0, "{r:?}");
        assert!(r.tickets[0] == 0 || r.tickets[1] == 0);
    }
    let ferrite_shared::proto::S2C::Rounds { wins, rounds } = g.rounds_msg() else { panic!() };
    assert_eq!((wins, rounds.len()), (g.wins, 2));
    // The record spans rounds: round counters reset, totals do not.
    let (first, second) = (&seen[0], &seen[1]);
    assert_eq!(first.total.rounds, 1);
    assert_eq!(second.total.rounds, 2);
    assert_eq!(second.total.deaths, first.deaths as u32 + second.deaths as u32);
    assert_eq!(second.total.score, first.score + second.score);
    let my_wins = g.history.iter().filter(|r| r.winner == seen[0].team).count() as u32;
    assert_eq!(second.total.wins, my_wins);
    // Leaving and coming back under the same name keeps it.
    let total = g.totals(me);
    g.leave(me);
    let again = g.join("tester").unwrap();
    assert_eq!(g.roster()[again].total, total);
    assert_eq!(g.roster()[again].kills, 0);
    // And so does restarting the server: people are saved, bots are not.
    let text = std::fs::read_to_string(&file).expect("stats file written");
    assert!(text.lines().any(|l| l.starts_with("tester ")), "{text}");
    assert_eq!(text.lines().filter(|l| !l.starts_with('#')).count(), 1, "{text}");
    let mut fresh = Game::new(conf());
    let back = fresh.join("tester").unwrap();
    assert_eq!(fresh.roster()[back].total, total);
    assert!(fresh.history.is_empty());
    // That record was kept by name, from before players had ids. Whoever
    // next brings an id under that name takes it over...
    fresh.leave(back);
    let id_a = [0xA1; 16];
    let a = fresh.join_as("tester", 2, id_a, &[]).unwrap();
    assert_eq!(fresh.roster()[a].total, total);
    fresh.players[a].kills = 3;
    fresh.players[a].score = 300;
    let grown = fresh.totals(a);
    assert_eq!(grown.kills, total.kills + 3);
    // ...and keeps it under a new callsign,
    fresh.leave(a);
    let a = fresh.join_as("renamed", 2, id_a, &[]).unwrap();
    assert_eq!(fresh.roster()[a].total, grown);
    // while someone else using the old callsign starts from nothing.
    let b = fresh.join_as("tester", 2, [0xB2; 16], &[]).unwrap();
    assert_eq!(fresh.roster()[b].total, Totals::default());
    fresh.players[b].deaths = 1;
    fresh.leave(a);
    fresh.leave(b);
    // The file carries the ids, and they survive a restart.
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.lines().any(|l| l.starts_with("renamed ") && l.ends_with(&uuid_text(&id_a))), "{text}");
    assert_eq!(uuid_parse(&uuid_text(&id_a)), Some(id_a));
    assert_eq!(uuid_parse("not-an-id"), None);
    let mut third = Game::new(conf());
    let a = third.join_as("again", 2, id_a, &[]).unwrap();
    assert_eq!(third.roster()[a].total, grown);
    let b = third.join_as("tester", 2, [0xB2; 16], &[]).unwrap();
    assert_eq!(third.roster()[b].total.deaths, 1);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn chat_reaches_the_right_people() {
    let mut g = Game::new(cfg(3));
    let a = g.join_team("ann", 0).unwrap();
    let b = g.join_team("bob", 1).unwrap();
    let c = g.join_team("cy", 0).unwrap();
    let start = g.chat_seq; // three "joined" notices
    assert_eq!(start, 3);
    assert!(g.chat_for(Some(b), 0).iter().all(|l| l.from == SPECTATOR_ID && l.all));
    assert!(g.say(a, false, "  good luck all  "));
    assert!(g.say(a, true, "push B"));
    assert!(!g.say(a, false, " \n\t "), "nothing to say");
    let bot = g.players.iter().position(|p| p.bot.is_some()).unwrap();
    assert!(!g.say(bot, false, "beep"), "bots do not chat");
    let long = "x".repeat(500);
    assert!(g.say(b, false, &long));
    let texts = |g: &Game, who: Option<usize>| g.chat_for(who, start).into_iter().map(|l| (l.name, l.text, l.all)).collect::<Vec<_>>();
    // A teammate hears both, the other side and a spectator only what was said to all.
    let mine = texts(&g, Some(c));
    assert_eq!(mine[0], ("ann".to_string(), "good luck all".to_string(), true));
    assert_eq!(mine[1], ("ann".to_string(), "push B".to_string(), false));
    assert_eq!(mine[2].1.len(), CHAT_MAX);
    for who in [Some(b), None] {
        let t = texts(&g, who);
        assert_eq!(t.len(), 2, "{t:?}");
        assert!(t.iter().all(|l| l.2));
    }
    // Acknowledged lines are not sent again.
    assert!(g.chat_for(Some(c), g.chat_seq).is_empty());
    // A full backlog comes a few lines at a time, never more than a packet.
    for k in 0..30 {
        g.say(b, false, &format!("{k} {long}"));
    }
    let mut ack = start;
    let mut got = 0;
    loop {
        let lines = g.chat_for(Some(a), ack);
        if lines.is_empty() {
            break;
        }
        assert!(encode(&S2C::Chat(lines.clone())).len() < 1300);
        got += lines.len();
        ack = lines.last().unwrap().seq;
    }
    assert_eq!(got, 33);
    g.leave(c);
    assert_eq!(g.chat.last().unwrap().text, "cy left");
}

#[test]
fn melee_hits_close_and_kills_from_behind() {
    use ferrite_shared::sim::{MELEE_DAMAGE, MELEE_TICKS, MELEE_WEAPON};
    let mut g = Game::new(cfg(2));
    let me = g.join_team("stabber", 0).unwrap();
    for _ in 0..200 {
        g.tick();
    }
    let foe = g.players.iter().position(|p| p.team == 1).unwrap();
    let mut seq = g.players[me].last_seq;
    // Put the two face to face on open ground, `gap` metres apart, the
    // target facing `facing` (0 = toward us).
    let stage = |g: &mut Game, gap: f32, facing: f32| {
        let base = g.world.points[2] + vec3(0.0, 0.0, 14.0);
        for (k, i) in [me, foe].into_iter().enumerate() {
            let p = &mut g.players[i];
            let x = base.x + k as f32 * gap;
            p.st = sim::PState::spawn(vec3(x, 0.0, base.z), 0);
            p.alive = true;
            p.hp = 100.0;
            p.protect_until = 0;
        }
        for i in [me, foe] {
            let (x, z) = (g.players[i].st.pos.x, g.players[i].st.pos.z);
            g.players[i].st.pos.y = g.world.height(x, z);
        }
        g.players[foe].yaw = -std::f32::consts::FRAC_PI_2 + facing;
        g.tick = g.tick.max(1);
    };
    let mut act = |g: &mut Game, buttons: u8, more: u8| {
        seq += 1;
        g.players[me].credits = 5.0;
        g.apply_cmd(me, Cmd { seq, buttons, more, yaw: std::f32::consts::FRAC_PI_2, interp: g.tick as f64, ..Default::default() });
    };
    let mut swing = |g: &mut Game| act(g, 0, btn2::MELEE);
    // In reach, from the front: hurt, not dead.
    stage(&mut g, 1.6, 0.0);
    let feed0 = g.feed.len();
    swing(&mut g);
    assert_eq!(g.players[foe].hp, 100.0 - MELEE_DAMAGE);
    assert!(g.players[foe].alive);
    assert_eq!(g.players[me].st.melee, MELEE_TICKS);
    assert!(matches!(g.fx.iter().rev().find(|f| matches!(f.1, Fx::Swing { .. })), Some((_, Fx::Swing { hit: true, .. }))));
    // Mid-swing nothing else happens: no second blow, no shot.
    let ammo = g.players[me].st.ammo;
    swing(&mut g);
    swing(&mut g);
    assert_eq!(g.players[foe].hp, 100.0 - MELEE_DAMAGE);
    assert_eq!(g.players[me].st.ammo, ammo);
    // Out of reach: a miss.
    stage(&mut g, 3.2, 0.0);
    swing(&mut g);
    assert_eq!(g.players[foe].hp, 100.0);
    assert!(matches!(g.fx.iter().rev().find(|f| matches!(f.1, Fx::Swing { .. })), Some((_, Fx::Swing { hit: false, .. }))));
    // Behind us: a miss too.
    stage(&mut g, -1.6, 0.0);
    swing(&mut g);
    assert_eq!(g.players[foe].hp, 100.0);
    // From behind them: dead, and the feed says how.
    stage(&mut g, 1.6, std::f32::consts::PI);
    swing(&mut g);
    assert!(!g.players[foe].alive);
    assert!(g.feed[feed0..].iter().any(|f| matches!(f, Feed::Kill { killer, victim, weapon, .. } if *killer == me as u8 && *victim == foe as u8 && *weapon == MELEE_WEAPON)));
    assert_eq!(sim::weapon_def(MELEE_WEAPON).name, "Melee");
    // A swing interrupts a reload.
    stage(&mut g, 9.0, 0.0);
    g.players[me].st.reload = 60;
    swing(&mut g);
    assert_eq!(g.players[me].st.reload, 0);
}

#[test]
fn weapon_finishes_are_validated_and_replicated() {
    use ferrite_shared::sim::{fix_weapon_skin, weapon_skin, weapon_skins_for, WEAPONS, WEAPON_SKINS};
    // Finish ids are stored in settings and sent over the network. Factory
    // remains id zero; every existing gun offers all six new finishes.
    let names: Vec<_> = WEAPON_SKINS.iter().map(|s| s.name).collect();
    assert_eq!(names, ["FACTORY", "OXIDE", "ARCTIC", "WOODLAND", "MIDNIGHT", "HAZARD", "SANDSTORM"]);
    for wid in 0..WEAPONS.len() as u8 {
        let fits = weapon_skins_for(wid);
        assert_eq!(fits, (0..WEAPON_SKINS.len() as u8).collect::<Vec<_>>());
        for s in 0..=255u8 {
            let fixed = fix_weapon_skin(wid, s);
            assert!(fits.contains(&fixed));
            assert_eq!(fixed == s, fits.contains(&s));
        }
    }
    assert!(WEAPON_SKINS[0].weapons.is_empty() && WEAPON_SKINS[0].body.is_none());
    assert_eq!(weapon_skin(200).name, WEAPON_SKINS[200 % WEAPON_SKINS.len()].name);

    let mut g = Game::new(Config { max_bots: 0, ..cfg(2) });
    let me = g.join("tester").unwrap();
    g.players[me].next_wskin = [1, 6];
    for _ in 0..40 { g.tick(); }
    assert!(g.players[me].alive);
    assert_eq!(g.players[me].wskin, [1, 6]);
    // Choosing a finish while alive affects the next deploy, preserving
    // the currently issued loadout until then.
    g.players[me].next_wskin = [2, 5];
    g.tick();
    assert_eq!(g.players[me].wskin, [1, 6]);

    // Exercise every finish on both primary and sidearm, across classes
    // and weapon families. The periodic roster carries their finish ids;
    // the movement snapshot carries the corresponding issued weapons.
    for (skin, kit, prim, side) in [(1, 0, 0, 4), (2, 1, 1, 10), (3, 2, 2, 11), (4, 0, 6, 16), (5, 1, 14, 4), (6, 2, 15, 10)] {
        let wants = [skin, 7 - skin];
        let p = &mut g.players[me];
        (p.next_kit, p.next_prim, p.next_side) = (kit, prim, side);
        p.next_wskin = wants;
        p.alive = false;
        p.hp = 0.0;
        p.respawn_at = g.tick;
        g.tick();
        assert!(g.players[me].alive);
        assert_eq!(g.players[me].wskin, wants);
        let snap = g.snapshot(Some(me), 0, g.tick);
        let state = snap.me.unwrap();
        assert_eq!((state.st.kit, state.st.prim, state.st.side), (kit, prim, side));
        let entries = g.roster();
        let packet = encode(&S2C::Roster { at: 0, total: entries.len() as u8, entries });
        let Some(S2C::Roster { entries, .. }) = decode::<S2C>(&packet) else { panic!("roster did not round-trip") };
        let e = entries.iter().find(|e| e.id as usize == me).unwrap();
        assert_eq!((e.prim, e.side, e.wskin), (prim, side, wants));
    }
    // Malformed finish ids are rejected when weapons are issued, without
    // changing either the class or the underlying weapon selection.
    let p = &mut g.players[me];
    p.next_wskin = [200, 201];
    p.alive = false;
    p.hp = 0.0;
    p.respawn_at = g.tick;
    g.tick();
    assert_eq!(g.players[me].wskin, [0, 0]);

    let bots = Game::new(cfg(12));
    let mut finishes = std::collections::HashSet::new();
    for e in bots.roster() {
        for (wid, finish) in [(e.prim, e.wskin[0]), (e.side, e.wskin[1])] {
            assert_eq!(fix_weapon_skin(wid, finish), finish);
            finishes.insert(finish);
        }
    }
    assert!(finishes.len() > 1 && finishes.iter().any(|&s| s != 0), "bots wear a valid mix of finishes");
}

#[test]
fn bot_limit_leaves_seats_empty_and_evens_the_sides() {
    let mut g = Game::new(Config { max_bots: 3, ..cfg(6) });
    let side = |g: &Game, t: u8| g.players.iter().filter(|p| p.team == t && !p.benched).count();
    assert_eq!(g.active_bots(), 3);
    assert_eq!(g.roster().len(), 3, "empty seats are not listed");
    for _ in 0..300 {
        g.tick();
    }
    assert_eq!(g.players.iter().filter(|p| p.alive).count(), 3, "only the bots in the match deploy");
    assert!(g.players.iter().all(|p| !p.benched || !p.alive));
    // A human takes an empty seat: the three bots stay, and they shift to
    // keep the sides level (2 v 2).
    let me = g.join_team("tester", 0).unwrap();
    assert_eq!(g.active_bots(), 3);
    assert_eq!(g.roster().len(), 4);
    assert_eq!((side(&g, 0), side(&g, 1)), (2, 2));
    assert!(!g.players[me].benched);
    // Changing side still works, and the bots even it up again.
    assert!(g.set_team(me, 1));
    assert_eq!((side(&g, 0), side(&g, 1)), (2, 2));
    g.leave(me);
    assert_eq!((g.active_bots(), g.roster().len()), (3, 3));
    // No bots at all: an empty field until people join.
    let mut g = Game::new(Config { max_bots: 0, ..cfg(4) });
    assert_eq!(g.active_bots(), 0);
    let a = g.join("a").unwrap();
    let b = g.join("b").unwrap();
    assert_ne!(g.players[a].team, g.players[b].team);
    assert_eq!(g.roster().len(), 2);
    // The default fills every slot, as before.
    let g = Game::new(cfg(6));
    assert_eq!((g.active_bots(), g.roster().len()), (12, 12));
    // Bots wear a mix of looks.
    let looks: std::collections::HashSet<u8> = g.roster().iter().map(|r| r.skin).collect();
    assert!(looks.len() > 1 && looks.iter().all(|&s| (s as usize) < sim::SKINS.len()));
}
