use ferrite_server::{Config, Game};
use ferrite_shared::map::{HALF_X, HALF_Z};
use ferrite_shared::proto::*;
use ferrite_shared::rng::Rng;
use ferrite_shared::sim;
use ferrite_shared::{eligible, NEUTRAL};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

fn cfg(per_team: usize) -> Config {
    Config { port: 0, per_team, seed: 0xFE221, tickets: 250, bot_skill: 1.0, log: false }
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
    Cmd { seq, mx: rng.below(3) as i8 - 1, mz: if rng.chance(0.8) { 1 } else { -1 }, buttons, yaw: *yaw, pitch: rng.range(-0.3, 0.3), interp: 0.0 }
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
    sock.send(&encode(&C2S::Hello { proto: PROTO + 1, name: "old".into(), spectator: false })).unwrap();
    let n = sock.recv(&mut buf).unwrap();
    assert!(matches!(decode::<S2C>(&buf[..n]), Some(S2C::Reject(_))));

    sock.send(&encode(&C2S::Hello { proto: PROTO, name: "net-test".into(), spectator: false })).unwrap();
    let (mut id, mut seed) = (SPECTATOR_ID, 0);
    for _ in 0..10 {
        let n = sock.recv(&mut buf).unwrap();
        if let Some(S2C::Welcome { id: i, seed: s, .. }) = decode::<S2C>(&buf[..n]) {
            id = i;
            seed = s;
            break;
        }
    }
    assert_ne!(id, SPECTATOR_ID, "no welcome");
    assert_eq!(seed, 0xFE221);

    let start = Instant::now();
    let (mut seq, mut max_ack, mut snaps, mut saw_self, mut in_roster) = (0u32, 0u32, 0, false, false);
    let mut last_send = Instant::now() - Duration::from_secs(1);
    let mut history: Vec<Cmd> = Vec::new();
    sock.set_read_timeout(Some(Duration::from_millis(4))).unwrap();
    while start.elapsed() < Duration::from_millis(1500) {
        if last_send.elapsed() >= Duration::from_micros(16_666) {
            last_send = Instant::now();
            seq += 1;
            history.push(Cmd { seq, mz: 1, yaw: 1.0, ..Default::default() });
            let recent: Vec<Cmd> = history.iter().rev().take(3).rev().copied().collect();
            sock.send(&encode(&C2S::Cmds { cmds: recent, ack_feed: 0, kit: 1 })).unwrap();
        }
        if let Ok(n) = sock.recv(&mut buf) {
            match decode::<S2C>(&buf[..n]) {
                Some(S2C::Snap(s)) => {
                    snaps += 1;
                    max_ack = max_ack.max(s.ack_seq);
                    if let Some(me) = s.me {
                        saw_self |= me.alive;
                    }
                }
                Some(S2C::Roster(r)) => in_roster |= r.iter().any(|e| e.id == id && e.name == "net-test" && !e.bot),
                _ => {}
            }
        }
    }
    assert!(snaps >= 30, "only {snaps} snapshots in 1.5 s (expected ~45)");
    assert!(max_ack + 12 >= seq, "server acked {max_ack} of {seq}");
    assert!(saw_self && in_roster);
    sock.send(&encode(&C2S::Bye)).unwrap();
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
    assert!(sim::spread(0, false, 0.0, false, true) < sim::spread(0, false, 0.0, false, false));
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
    assert_eq!(g.players[me].st.ammo, sim::SIDEARM.mag);
    assert_eq!(g.players[me].st.ammo_alt, primary_left);
    send(&mut g, btn::FIRE);
    assert_eq!(g.players[me].st.ammo, sim::SIDEARM.mag, "cannot fire while drawing");
    for _ in 0..30 {
        send(&mut g, 0);
    }
    for _ in 0..40 {
        send(&mut g, btn::FIRE); // held
    }
    assert_eq!(g.players[me].st.ammo, sim::SIDEARM.mag - 1, "held trigger = one shot");
    for _ in 0..5 {
        send(&mut g, 0);
        for _ in 0..9 {
            send(&mut g, btn::FIRE);
        }
    }
    assert_eq!(g.players[me].st.ammo, sim::SIDEARM.mag - 6, "one shot per pull");
    let snap = g.snapshot(None, 0, g.tick);
    assert!(snap.players.iter().any(|p| p.id as usize == me && p.flags & flag::SIDEARM != 0));
    send(&mut g, btn::SWAP);
    assert_eq!((g.players[me].st.slot, g.players[me].st.ammo), (0, primary_left));
}
