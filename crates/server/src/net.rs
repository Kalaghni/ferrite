//! UDP front end: sessions, the fixed 60 Hz loop and snapshot fan-out.

use crate::{Config, Game};
use ferrite_shared::proto::*;
use ferrite_shared::TICK_HZ;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Datagrams read from the socket in one pass of the loop; the rest wait
/// for the next tick, so a flood cannot stall the simulation.
pub const RECV_PER_TICK: usize = 2048;
/// Datagrams from one address in one pass (a client sends a few a tick).
pub const RECV_PER_ADDR: u16 = 48;
/// Sessions from one IP address (a household or LAN party behind one NAT).
pub const CONNS_PER_IP: usize = 8;
/// Sessions in all.
pub const MAX_CONNS: usize = 64;
/// How long a challenge cookie stays valid: two buckets of this length.
const COOKIE_BUCKET: Duration = Duration::from_secs(10);

/// Per-process secrets and per-tick counters that protect the session
/// layer. Nothing here is stored per unverified address, so spoofed traffic
/// costs the server no memory.
pub(crate) struct Guard {
    keys: std::collections::hash_map::RandomState,
    start: Instant,
    seen: HashMap<SocketAddr, u16>,
    rejects: u16,
    /// Status replies sent this tick, to anyone.
    statuses: u16,
    issued: u64,
}

impl Guard {
    pub(crate) fn new() -> Guard {
        Guard {
            keys: std::collections::hash_map::RandomState::new(),
            start: Instant::now(),
            seen: HashMap::new(),
            rejects: 0,
            statuses: 0,
            issued: 0,
        }
    }
    fn bucket(&self) -> u64 {
        self.start.elapsed().as_secs() / COOKIE_BUCKET.as_secs()
    }
    /// A keyed hash of the address and time bucket. Never 0, which means
    /// "no cookie" on the wire.
    fn cookie_at(&self, addr: SocketAddr, bucket: u64) -> u64 {
        self.keys.hash_one((addr, bucket, 0x636f_6f6b_u32)) | 1
    }
    fn cookie(&self, addr: SocketAddr) -> u64 {
        self.cookie_at(addr, self.bucket())
    }
    /// Valid for the current and the previous bucket (10 to 20 seconds).
    fn cookie_ok(&self, addr: SocketAddr, cookie: u64) -> bool {
        let b = self.bucket();
        cookie != 0 && (cookie == self.cookie_at(addr, b) || (b > 0 && cookie == self.cookie_at(addr, b - 1)))
    }
    /// An unguessable session token.
    fn token(&mut self, addr: SocketAddr) -> u64 {
        self.issued += 1;
        self.keys.hash_one((addr, self.issued, self.start.elapsed().as_nanos())) | 1
    }
    /// Count a datagram from `addr`; false once it is over its share.
    fn admit(&mut self, addr: SocketAddr) -> bool {
        let n = self.seen.entry(addr).or_insert(0);
        *n = n.saturating_add(1);
        *n <= RECV_PER_ADDR
    }
    fn new_tick(&mut self) {
        self.seen.clear();
        self.rejects = 0;
        self.statuses = 0;
    }
}

struct Conn {
    player: Option<usize>,
    /// Every packet after the Welcome must carry this.
    token: u64,
    /// When the Welcome was last (re)sent, to rate-limit repeats.
    welcomed: Instant,
    last_heard: Instant,
    ack_feed: u32,
    fx_sent: u32,
    team_switch: Instant,
    /// Newest chat line the client has, the id of the last thing it said,
    /// and how many more messages it may send right now.
    chat_ack: u32,
    chat_id: u32,
    chat_budget: f32,
    chat_at: Instant,
    icon_at: Instant,
}

/// Status replies the server sends in one tick, to everyone together.
const STATUS_PER_TICK: u16 = 32;

/// What the server list shows for this server.
fn info(game: &Game, id: u32, port: u16) -> ServerInfo {
    ServerInfo {
        proto: PROTO,
        id,
        port,
        name: game.cfg.name.clone(),
        map: game.world.map,
        humans: game.humans() as u8,
        slots: game.players.len() as u8,
        phase: game.phase,
        mode: game.mode,
    }
}

/// The roster, cut into datagrams.
fn roster_msgs(game: &Game) -> Vec<Vec<u8>> {
    let r = game.roster();
    let total = r.len() as u8;
    if r.is_empty() {
        return vec![encode(&S2C::Roster { at: 0, total: 0, entries: Vec::new() })];
    }
    r.chunks(ROSTER_PART)
        .enumerate()
        .map(|(k, c)| encode(&S2C::Roster { at: (k * ROSTER_PART) as u8, total, entries: c.to_vec() }))
        .collect()
}

/// The address this machine uses on its local network, if it has one.
pub fn lan_ip() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("192.0.2.1", 9)).ok()?;
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

/// Socket that hears discovery queries: multicast on every interface we can
/// name, plus plain broadcast. The address is shared, so several servers on
/// one machine all hear a multicast query.
fn discovery_socket() -> io::Result<UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(unix)]
    s.set_reuse_port(true)?;
    s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT).into())?;
    s.set_nonblocking(true)?;
    let group = Ipv4Addr::from(DISCOVERY_GROUP);
    let any = s.join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED);
    let lan = lan_ip().map(|ip| s.join_multicast_v4(&group, &ip));
    if any.is_err() && !matches!(lan, Some(Ok(()))) {
        // Broadcast queries still arrive; multicast ones will not.
    }
    Ok(s.into())
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
    // The world and its navigation are built before returning, so the
    // server answers as soon as the caller has its address. Built on the
    // thread instead, a hello sent straight away waited out the build
    // (over 100 ms in a debug build, far more on a busy machine).
    let game = Game::new(cfg);
    let thread = std::thread::Builder::new().name("ferrite-server".into()).spawn(move || serve(game, sock, s2))?;
    Ok(ServerHandle { addr, stop, thread: Some(thread) })
}

pub fn run(cfg: Config, sock: UdpSocket, stop: Arc<AtomicBool>) {
    serve(Game::new(cfg), sock, stop)
}

fn serve(mut game: Game, sock: UdpSocket, stop: Arc<AtomicBool>) {
    let log = game.cfg.log;
    sock.set_nonblocking(true).expect("nonblocking socket");
    let mut conns: HashMap<SocketAddr, Conn> = HashMap::new();
    let mut guard = Guard::new();
    let instance = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() ^ d.as_secs() as u32)
        ^ std::process::id();
    let game_port = sock.local_addr().map(|a| a.port()).unwrap_or(game.cfg.port);
    let disc = if game.cfg.discover {
        match discovery_socket() {
            Ok(d) => Some(d),
            Err(e) => {
                if log {
                    println!("LAN discovery is off: {e}");
                }
                None
            }
        }
    } else {
        None
    };
    let tick_dur = Duration::from_secs_f64(1.0 / TICK_HZ as f64);
    let mut next = Instant::now() + tick_dur;
    let mut buf = [0u8; 2048];
    let (mut work_sum, mut work_max, mut work_n) = (0.0f64, 0.0f64, 0u32);
    let (mut bytes_out, mut pkts_in) = (0u64, 0u64);
    if log {
        println!(
            "FERRITE server on {} - map {} - {} - {} per team, seed {:#x}",
            sock.local_addr().map(|a| a.to_string()).unwrap_or_default(),
            ferrite_shared::map::map_def(game.world.map).name,
            ferrite_shared::mode_def(game.mode).name,
            game.cfg.per_team,
            game.cfg.seed
        );
    }
    while !stop.load(Ordering::Relaxed) {
        // Drain the socket, up to a limit. Commands are simulated on arrival.
        guard.new_tick();
        for _ in 0..RECV_PER_TICK {
            match sock.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    pkts_in += 1;
                    if !guard.admit(addr) {
                        continue;
                    }
                    // The public server list asking how we are.
                    if is_status_query(&buf[..n]) {
                        if guard.statuses < STATUS_PER_TICK {
                            guard.statuses += 1;
                            let _ = sock.send_to(&encode_info(&info(&game, instance, game_port)), addr);
                        }
                    } else {
                        handle(&mut game, &mut conns, &sock, addr, &buf[..n], log, &mut guard);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break, // e.g. Windows ICMP "connection reset" noise
            }
        }
        // Someone on the network is looking for games.
        if let Some(d) = &disc {
            let mut q = [0u8; 64];
            while let Ok((n, from)) = d.recv_from(&mut q) {
                if &q[..n] == DISCOVERY_QUERY {
                    let _ = d.send_to(&encode_info(&info(&game, instance, game_port)), from);
                }
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
        if t.is_multiple_of(2) {
            for (addr, c) in conns.iter_mut() {
                let snap = game.snapshot(c.player, c.ack_feed, c.fx_sent);
                c.fx_sent = game.fx_seq;
                let bytes = encode(&S2C::Snap(snap));
                bytes_out += bytes.len() as u64;
                let _ = sock.send_to(&bytes, addr);
                // Chat is resent until the client says it has it.
                let lines = game.chat_for(c.player, c.chat_ack);
                if !lines.is_empty() {
                    let bytes = encode(&S2C::Chat(lines));
                    bytes_out += bytes.len() as u64;
                    let _ = sock.send_to(&bytes, addr);
                }
            }
        }
        if t.is_multiple_of(30) && !conns.is_empty() {
            let parts = roster_msgs(&game);
            // Round results and who wears which icon change rarely: once
            // a second is plenty.
            let slow = t.is_multiple_of(60).then(|| [encode(&game.rounds_msg()), encode(&game.icons_msg())]);
            for (addr, c) in conns.iter() {
                // What the round just finished paid, until the next begins.
                if let Some((round, credits, won)) = c.player.and_then(|i| game.players[i].award) {
                    let _ = sock.send_to(&encode(&S2C::Award { round, credits, won }), addr);
                }
                for p in &parts {
                    let _ = sock.send_to(p, addr);
                }
                for m in slow.iter().flatten() {
                    let _ = sock.send_to(m, addr);
                }
            }
        }
        let w = w0.elapsed().as_secs_f64() * 1000.0;
        work_sum += w;
        work_max = work_max.max(w);
        work_n += 1;
        if log && t.is_multiple_of(10 * TICK_HZ) {
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

#[allow(clippy::too_many_arguments)]
fn handle(
    game: &mut Game,
    conns: &mut HashMap<SocketAddr, Conn>,
    sock: &UdpSocket,
    addr: SocketAddr,
    data: &[u8],
    log: bool,
    guard: &mut Guard,
) {
    // The protocol version leads every datagram. An old or newer client is
    // told why it cannot join - but only when the request is at least as big
    // as the answer, and only a few times a tick, so this cannot amplify.
    let Some(proto) = data.get(..4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) else { return };
    if proto != PROTO {
        if data.len() >= 48 && guard.rejects < 8 {
            guard.rejects += 1;
            let _ = sock.send_to(&encode(&S2C::Reject(format!("protocol mismatch: server speaks v{}", PROTO))), addr);
        }
        return;
    }
    let Some(Packet { token, msg, .. }) = decode::<Packet>(data) else { return };
    // After the Welcome, only packets carrying the session's token count.
    if let Some(c) = conns.get(&addr) {
        if token != c.token && !matches!(msg, C2S::Hello { .. }) {
            return;
        }
    }
    match msg {
        C2S::Hello { cookie, name, spectator, team, uuid, icon } => {
            if let Some(c) = conns.get_mut(&addr) {
                // A repeat (the Welcome was lost): prove it is the same
                // client, and answer at most a few times a second.
                if (token != c.token && !guard.cookie_ok(addr, cookie)) || c.welcomed.elapsed() < Duration::from_millis(200) {
                    return;
                }
                c.welcomed = Instant::now();
            } else {
                // First contact: a small challenge, no state kept. Only a
                // Hello that comes back with the cookie gets a slot.
                if !guard.cookie_ok(addr, cookie) {
                    let _ = sock.send_to(&encode(&S2C::Challenge { cookie: guard.cookie(addr) }), addr);
                    return;
                }
                let same_ip = conns.keys().filter(|a| a.ip() == addr.ip()).count();
                if conns.len() >= MAX_CONNS || same_ip >= CONNS_PER_IP {
                    let _ = sock.send_to(&encode(&S2C::Reject("server full".into())), addr);
                    return;
                }
                // Not shown their own arrival, nor anything said before it.
                let chat_ack = game.chat_seq + if spectator { 0 } else { 1 };
                let player = if spectator { None } else { game.join_as(&name, team, uuid, &icon) };
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
                    Conn {
                        player,
                        token: guard.token(addr),
                        welcomed: Instant::now(),
                        last_heard: Instant::now(),
                        ack_feed: game.feed.len() as u32,
                        fx_sent: game.fx_seq,
                        team_switch: Instant::now() - Duration::from_secs(60),
                        chat_ack: chat_ack.min(game.chat_seq),
                        chat_id: 0,
                        chat_budget: 4.0,
                        chat_at: Instant::now(),
                        icon_at: Instant::now() - Duration::from_secs(1),
                    },
                );
            }
            let c = &conns[&addr];
            let (id, team) = match c.player {
                Some(i) => (i as u8, game.players[i].team),
                None => (SPECTATOR_ID, 0),
            };
            let w =
                S2C::Welcome { id, team, seed: game.cfg.seed, map: game.world.map, feed_id: c.ack_feed, tick: game.tick, token: c.token };
            let _ = sock.send_to(&encode(&w), addr);
            for p in roster_msgs(game) {
                let _ = sock.send_to(&p, addr);
            }
            let _ = sock.send_to(&encode(&game.rounds_msg()), addr);
            let _ = sock.send_to(&encode(&game.icons_msg()), addr);
        }
        C2S::Cmds { mut cmds, ack_feed, kit, prim, side, skin, wskin, ack_chat } => {
            let Some(c) = conns.get_mut(&addr) else { return };
            c.last_heard = Instant::now();
            c.ack_feed = c.ack_feed.max(ack_feed.min(game.feed.len() as u32));
            c.chat_ack = c.chat_ack.max(ack_chat.min(game.chat_seq));
            if let Some(i) = c.player {
                game.players[i].select_loadout(kit, prim, side, skin, wskin);
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
                for p in roster_msgs(game) {
                    let _ = sock.send_to(&p, addr);
                }
            }
        }
        C2S::Chat { id, team, text } => {
            let Some(c) = conns.get_mut(&addr) else { return };
            c.last_heard = Instant::now();
            let Some(i) = c.player else { return };
            if id <= c.chat_id {
                return; // a repeat of something already said
            }
            c.chat_id = id;
            // A message every second and a half, four in a burst.
            let now = Instant::now();
            c.chat_budget = (c.chat_budget + now.duration_since(c.chat_at).as_secs_f32() / 1.5).min(4.0);
            c.chat_at = now;
            if c.chat_budget < 1.0 {
                return;
            }
            c.chat_budget -= 1.0;
            if game.say(i, team, &text) && log {
                if let Some(l) = game.chat.last() {
                    println!("[{}] <{}{}> {}", game.tick, l.name, if team { " (team)" } else { "" }, l.text);
                }
            }
        }
        C2S::WantIcon(id) => {
            let Some(c) = conns.get_mut(&addr) else { return };
            if c.icon_at.elapsed() < Duration::from_millis(40) {
                return;
            }
            c.icon_at = Instant::now();
            if let Some(p) = game.players.get(id as usize).filter(|p| p.icon_hash != 0) {
                let _ = sock.send_to(&encode(&S2C::Icon { id, hash: p.icon_hash, rgba: p.icon.clone() }), addr);
            }
        }
        C2S::Lobby { vote, ready, mode } => {
            let Some(c) = conns.get_mut(&addr) else { return };
            c.last_heard = Instant::now();
            if let Some(i) = c.player {
                game.players[i].vote = vote;
                game.players[i].mode_vote = mode;
                game.players[i].ready = ready;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[test]
    fn cookies_are_bound_to_the_address_and_never_zero() {
        let g = Guard::new();
        let c = g.cookie(at(5000));
        assert_ne!(c, 0);
        assert!(g.cookie_ok(at(5000), c));
        assert!(!g.cookie_ok(at(5001), c));
        assert!(!g.cookie_ok(at(5000), 0));
        // Another process has other keys.
        assert!(!Guard::new().cookie_ok(at(5000), c));
    }

    #[test]
    fn one_address_gets_a_bounded_share_of_each_tick() {
        let mut g = Guard::new();
        let admitted = (0..500).filter(|_| g.admit(at(6000))).count();
        assert_eq!(admitted, RECV_PER_ADDR as usize);
        assert!(g.admit(at(6001)), "other addresses are unaffected");
        g.new_tick();
        assert!(g.admit(at(6000)));
    }

    #[test]
    fn tokens_differ_per_session() {
        let mut g = Guard::new();
        let t: Vec<u64> = (0..100).map(|_| g.token(at(7000))).collect();
        let mut u = t.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), t.len());
        assert!(t.iter().all(|&x| x != 0));
    }
}
