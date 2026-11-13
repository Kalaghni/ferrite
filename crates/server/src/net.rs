//! UDP front end: sessions, the fixed 60 Hz loop and snapshot fan-out.

use crate::{Config, Game};
use ferrite_shared::proto::*;
use ferrite_shared::TICK_HZ;
use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Conn {
    player: Option<usize>,
    last_heard: Instant,
    ack_feed: u32,
    fx_sent: u32,
    team_switch: Instant,
}

pub struct ServerHandle {
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ServerHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Bind and run the server on a background thread (used by the client to
/// host a match). Falls back to an ephemeral port if the default is taken.
pub fn spawn(cfg: Config) -> io::Result<ServerHandle> {
    let sock = UdpSocket::bind(("0.0.0.0", cfg.port)).or_else(|_| UdpSocket::bind(("0.0.0.0", 0)))?;
    let addr = sock.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let s2 = stop.clone();
    let thread = std::thread::Builder::new().name("ferrite-server".into()).spawn(move || run(cfg, sock, s2))?;
    Ok(ServerHandle { addr, stop, thread: Some(thread) })
}

pub fn run(cfg: Config, sock: UdpSocket, stop: Arc<AtomicBool>) {
    let log = cfg.log;
    sock.set_nonblocking(true).expect("nonblocking socket");
    let mut game = Game::new(cfg);
    let mut conns: HashMap<SocketAddr, Conn> = HashMap::new();
    let tick_dur = Duration::from_secs_f64(1.0 / TICK_HZ as f64);
    let mut next = Instant::now() + tick_dur;
    let mut buf = [0u8; 2048];
    let (mut work_sum, mut work_max, mut work_n) = (0.0f64, 0.0f64, 0u32);
    let (mut bytes_out, mut pkts_in) = (0u64, 0u64);
    if log {
        println!(
            "FERRITE server on {} - map {} - {} per team, seed {:#x}",
            sock.local_addr().map(|a| a.to_string()).unwrap_or_default(),
            ferrite_shared::map::map_def(game.world.map).name,
            game.cfg.per_team,
            game.cfg.seed
        );
    }
    while !stop.load(Ordering::Relaxed) {
        // Drain the socket. Commands are simulated on arrival.
        loop {
            match sock.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    pkts_in += 1;
                    handle(&mut game, &mut conns, &sock, addr, &buf[..n], log);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break, // e.g. Windows ICMP "connection reset" noise
            }
        }
        let now = Instant::now();
        if now < next {
            std::thread::sleep((next - now).min(Duration::from_millis(1)));
            continue;
        }
        next += tick_dur;
        if now > next + Duration::from_millis(250) {
            next = now + tick_dur; // fell far behind: resync instead of spiralling
        }
        let w0 = Instant::now();
        game.tick();
        let t = game.tick;

        // Drop silent sessions; their slot reverts to a bot.
        let dead: Vec<SocketAddr> =
            conns.iter().filter(|(_, c)| c.last_heard.elapsed() > Duration::from_secs(5)).map(|(a, _)| *a).collect();
        for a in dead {
            if let Some(c) = conns.remove(&a) {
                if let Some(i) = c.player {
                    if log {
                        println!("[{}] {} timed out", t, game.players[i].name);
                    }
                    game.leave(i);
                }
            }
        }
        if t % 2 == 0 {
            for (addr, c) in conns.iter_mut() {
                let snap = game.snapshot(c.player, c.ack_feed, c.fx_sent);
                c.fx_sent = t;
                let bytes = encode(&S2C::Snap(snap));
                bytes_out += bytes.len() as u64;
                let _ = sock.send_to(&bytes, addr);
            }
        }
        if t % 30 == 0 && !conns.is_empty() {
            let bytes = encode(&S2C::Roster(game.roster()));
            for addr in conns.keys() {
                let _ = sock.send_to(&bytes, addr);
            }
        }
        let w = w0.elapsed().as_secs_f64() * 1000.0;
        work_sum += w;
        work_max = work_max.max(w);
        work_n += 1;
        if log && t % (10 * TICK_HZ) == 0 {
            println!(
                "[{:>7}] humans {} | tickets {}-{} | points {:?} | kills {} caps {} rounds {} | tick avg {:.3} ms max {:.3} ms | out {:.1} kB/s in {} pkt/s | dropped cmds {}",
                t,
                conns.values().filter(|c| c.player.is_some()).count(),
                game.tickets[0],
                game.tickets[1],
                game.owners(),
                game.stats.kills,
                game.stats.captures,
                game.stats.rounds,
                work_sum / work_n.max(1) as f64,
                work_max,
                bytes_out as f64 / 10_000.0,
                pkts_in / 10,
                game.stats.dropped_cmds,
            );
            work_sum = 0.0;
            work_max = 0.0;
            work_n = 0;
            bytes_out = 0;
            pkts_in = 0;
        }
    }
}

fn handle(game: &mut Game, conns: &mut HashMap<SocketAddr, Conn>, sock: &UdpSocket, addr: SocketAddr, data: &[u8], log: bool) {
    let Some(msg) = decode::<C2S>(data) else { return };
    match msg {
        C2S::Hello { proto, name, spectator, team } => {
            if proto != PROTO {
                let _ = sock.send_to(&encode(&S2C::Reject(format!("protocol mismatch: server speaks v{}", PROTO))), addr);
                return;
            }
            if !conns.contains_key(&addr) {
                if conns.len() >= 64 {
                    let _ = sock.send_to(&encode(&S2C::Reject("server full".into())), addr);
                    return;
                }
                let player = if spectator { None } else { game.join_team(&name, team) };
                if !spectator && player.is_none() {
                    let _ = sock.send_to(&encode(&S2C::Reject("server full".into())), addr);
                    return;
                }
                if log {
                    match player {
                        Some(i) => println!("[{}] {} joined team {} from {}", game.tick, game.players[i].name, game.players[i].team, addr),
                        None => println!("[{}] spectator from {}", game.tick, addr),
                    }
                }
                conns.insert(
                    addr,
                    Conn { player, last_heard: Instant::now(), ack_feed: game.feed.len() as u32, fx_sent: game.tick, team_switch: Instant::now() - Duration::from_secs(60) },
                );
            }
            let c = &conns[&addr];
            let (id, team) = match c.player {
                Some(i) => (i as u8, game.players[i].team),
                None => (SPECTATOR_ID, 0),
            };
            let w = S2C::Welcome { id, team, seed: game.cfg.seed, map: game.world.map, feed_id: c.ack_feed, tick: game.tick };
            let _ = sock.send_to(&encode(&w), addr);
            let _ = sock.send_to(&encode(&S2C::Roster(game.roster())), addr);
        }
        C2S::Cmds { mut cmds, ack_feed, kit } => {
            let Some(c) = conns.get_mut(&addr) else { return };
            c.last_heard = Instant::now();
            c.ack_feed = c.ack_feed.max(ack_feed.min(game.feed.len() as u32));
            if let Some(i) = c.player {
                game.players[i].next_kit = kit % 3;
                cmds.truncate(8);
                cmds.sort_by_key(|c| c.seq);
                for cmd in cmds {
                    game.apply_cmd(i, cmd);
                }
            }
        }
        C2S::Team(team) => {
            let Some(c) = conns.get_mut(&addr) else { return };
            let Some(i) = c.player else { return };
            // One change every few seconds is plenty.
            if c.team_switch.elapsed() < Duration::from_secs(4) || game.players[i].team == team {
                return;
            }
            if game.set_team(i, team) {
                c.team_switch = Instant::now();
                if log {
                    println!("[{}] {} switched to team {}", game.tick, game.players[i].name, team);
                }
                let _ = sock.send_to(&encode(&S2C::Roster(game.roster())), addr);
            }
        }
        C2S::Bye => {
            if let Some(c) = conns.remove(&addr) {
                if let Some(i) = c.player {
                    if log {
                        println!("[{}] {} left", game.tick, game.players[i].name);
                    }
                    game.leave(i);
                }
            }
        }
    }
}
