use ferrite::*;
use std::{
    collections::HashMap,
    env,
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant},
};

const MAX_HUMANS: usize = 24;
const RESPAWN_TICKS: u64 = 80;
const ROUND_END_TICKS: u64 = 100;
const MAX_PACKET_RATE: u32 = 60; // per established client, per one-second window
const FIRE_INTERVAL_TICKS: u64 = 3; // 6.67 rounds/s at 20Hz

struct Human {
    soldier: Soldier,
    input: Input,
    last_seen: Instant,
    rx_window: Instant,
    rx_count: u32,
    ack_seq: u64,
    last_fire_tick: u64,
    respawn_at: Option<u64>,
}
struct Bot {
    soldier: Soldier,
    last_fire_tick: u64,
    respawn_at: Option<u64>,
}

fn spawn(id: u32, team: u8, name: String, bot: bool) -> Soldier {
    let lane = (id % 7) as f32 * 4.0 - 12.0;
    Soldier {
        id,
        name,
        team,
        x: if team == 0 { -42.0 } else { 42.0 },
        z: lane,
        yaw: spawn_yaw(team),
        health: MAX_HEALTH,
        kills: 0,
        deaths: 0,
        bot,
    }
}
fn revive(previous: &Soldier) -> Soldier {
    let mut new = spawn(
        previous.id,
        previous.team,
        previous.name.clone(),
        previous.bot,
    );
    new.kills = previous.kills;
    new.deaths = previous.deaths;
    new
}

fn main() -> std::io::Result<()> {
    let bind = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".into());
    let sock = UdpSocket::bind(&bind)?;
    sock.set_nonblocking(true)?;
    println!(
        "FERRITE {} | UDP {bind} | protocol v{} | {} Hz | LAN only",
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION,
        TICK_HZ
    );
    let mut humans: HashMap<SocketAddr, Human> = HashMap::new();
    let mut bots: Vec<Bot> = (0..8)
        .map(|i| Bot {
            soldier: spawn(1000 + i, (i % 2) as u8, format!("BOT-{}", i + 1), true),
            last_fire_tick: 0,
            respawn_at: None,
        })
        .collect();
    let mut zones: Vec<Zone> = ZONES
        .iter()
        .map(|&(x, z)| Zone {
            x,
            z,
            owner: -1,
            progress: 0.0,
        })
        .collect();
    let mut score = [0_u32, 0];
    let mut winner: Option<u8> = None;
    let mut end_round_at: Option<u64> = None;
    let mut next_id = 1_u32;
    let mut event_id = 0_u64;
    let mut event_buffer: Vec<CombatEvent> = Vec::new();
    let mut tick = 0_u64;
    let mut last_report = Instant::now();
    let step = Duration::from_millis(1000 / TICK_HZ as u64);
    let mut next_tick = Instant::now();
    let mut buf = [0_u8; 2048];
    loop {
        // Consume messages BEFORE the next simulation tick. UDP can reorder or lose packets.
        // Bound receive processing so a flood cannot starve simulation ticks.
        for _ in 0..256 {
            match sock.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    let Ok(mut input) = serde_json::from_slice::<Input>(&buf[..n]) else {
                        continue;
                    };
                    if !valid_input(&input) {
                        continue;
                    }
                    input.forward = input.forward.clamp(-1.0, 1.0);
                    input.strafe = input.strafe.clamp(-1.0, 1.0);
                    input.pitch = input.pitch.clamp(-1.25, 1.25);
                    input.yaw = input.yaw.rem_euclid(std::f32::consts::TAU);
                    let now = Instant::now();
                    if !humans.contains_key(&addr) {
                        if humans.len() >= MAX_HUMANS {
                            continue;
                        }
                        let blue = humans.values().filter(|h| h.soldier.team == 0).count();
                        let orange = humans.len() - blue;
                        let team = if blue <= orange { 0 } else { 1 };
                        let name: String = input
                            .name
                            .chars()
                            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .take(16)
                            .collect();
                        let soldier = spawn(
                            next_id,
                            team,
                            if name.is_empty() {
                                format!("Player{next_id}")
                            } else {
                                name
                            },
                            false,
                        );
                        println!("JOIN {addr} #{} team {team}", next_id);
                        humans.insert(
                            addr,
                            Human {
                                soldier,
                                input: input.clone(),
                                last_seen: now,
                                rx_window: now,
                                rx_count: 0,
                                ack_seq: 0,
                                last_fire_tick: 0,
                                respawn_at: None,
                            },
                        );
                        next_id = next_id.saturating_add(1);
                    }
                    if let Some(h) = humans.get_mut(&addr) {
                        if now.duration_since(h.rx_window) >= Duration::from_secs(1) {
                            h.rx_window = now;
                            h.rx_count = 0;
                        }
                        h.rx_count += 1;
                        if h.rx_count > MAX_PACKET_RATE {
                            continue;
                        }
                        if input.seq > h.input.seq {
                            h.input = input;
                        }
                        h.last_seen = now;
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => {
                    eprintln!("UDP receive: {e}");
                    break;
                }
            }
        }
        humans.retain(|addr, h| {
            let live = h.last_seen.elapsed() < Duration::from_secs(10);
            if !live {
                println!("TIMEOUT {addr}");
            }
            live
        });

        if let Some(until) = end_round_at {
            if tick >= until {
                winner = None;
                end_round_at = None;
                score = [0, 0];
                for z in &mut zones {
                    z.progress = 0.0;
                    z.owner = -1;
                }
                for h in humans.values_mut() {
                    h.soldier = revive(&h.soldier);
                    h.respawn_at = None;
                }
                for b in &mut bots {
                    b.soldier = revive(&b.soldier);
                    b.respawn_at = None;
                }
                println!("ROUND RESTART at tick {tick}");
            }
        }
        for h in humans.values_mut() {
            if let Some(at) = h.respawn_at {
                if tick >= at {
                    h.soldier = revive(&h.soldier);
                    h.respawn_at = None;
                }
            }
            apply_input(&mut h.soldier, &h.input);
            h.ack_seq = h.input.seq;
        }
        let alive: Vec<Soldier> = humans
            .values()
            .map(|h| h.soldier.clone())
            .chain(bots.iter().map(|b| b.soldier.clone()))
            .collect();
        for b in &mut bots {
            if let Some(at) = b.respawn_at {
                if tick >= at {
                    b.soldier = revive(&b.soldier);
                    b.respawn_at = None;
                }
            }
            if b.soldier.health <= 0 {
                continue;
            }
            let enemy = alive
                .iter()
                .filter(|s| s.team != b.soldier.team && s.health > 0)
                .min_by(|a, c| {
                    let da = (a.x - b.soldier.x).hypot(a.z - b.soldier.z);
                    let dc = (c.x - b.soldier.x).hypot(c.z - b.soldier.z);
                    da.total_cmp(&dc)
                });
            let zone = &zones[(b.soldier.id as usize + (tick / 350) as usize) % 3];
            let (tx, tz) = match enemy {
                Some(e) if (e.x - b.soldier.x).hypot(e.z - b.soldier.z) < 19.0 => (e.x, e.z),
                _ => (zone.x, zone.z),
            };
            let dx = tx - b.soldier.x;
            let dz = tz - b.soldier.z;
            b.soldier.yaw = dx.atan2(dz);
            if dx.hypot(dz) > 5.0 {
                move_actor(&mut b.soldier, 1.0, 0.0, 5.0);
            } else if tick % 60 < 30 {
                move_actor(&mut b.soldier, 0.0, 0.65, 5.0);
            }
        }

        let all: Vec<Soldier> = humans
            .values()
            .map(|h| h.soldier.clone())
            .chain(bots.iter().map(|b| b.soldier.clone()))
            .collect();
        if winner.is_none() {
            let mut hits: Vec<(u32, ShotHit, Option<u64>)> = Vec::new();
            for h in humans.values_mut() {
                if h.input.fire
                    && h.soldier.health > 0
                    && tick.saturating_sub(h.last_fire_tick) >= FIRE_INTERVAL_TICKS
                {
                    h.last_fire_tick = tick;
                    if let Some(hit) = trace_shot(&h.soldier, h.input.pitch, &all) {
                        hits.push((h.soldier.id, hit, Some(h.input.seq)));
                    }
                }
            }
            for b in &mut bots {
                if b.soldier.health <= 0 || tick.saturating_sub(b.last_fire_tick) < 14 {
                    continue;
                }
                let target = all
                    .iter()
                    .filter(|s| s.team != b.soldier.team && s.health > 0)
                    .min_by(|a, c| {
                        (a.x - b.soldier.x)
                            .hypot(a.z - b.soldier.z)
                            .total_cmp(&(c.x - b.soldier.x).hypot(c.z - b.soldier.z))
                    });
                if let Some(target) = target {
                    let distance = (target.x - b.soldier.x)
                        .hypot(target.z - b.soldier.z)
                        .max(0.1);
                    b.soldier.yaw = (target.x - b.soldier.x).atan2(target.z - b.soldier.z);
                    let pitch = (-0.75 / distance).atan(); // aim for torso, not heads
                    if let Some(hit) = trace_shot(&b.soldier, pitch, &all) {
                        b.last_fire_tick = tick;
                        hits.push((b.soldier.id, hit, None));
                    }
                }
            }
            for (shooter, hit, input_seq) in hits {
                let mut actually_hit = false;
                let mut killed = false;
                for h in humans.values_mut() {
                    if h.soldier.id == hit.target && h.soldier.health > 0 {
                        actually_hit = true;
                        h.soldier.health = (h.soldier.health - hit.damage).max(0);
                        if h.soldier.health == 0 {
                            killed = true;
                            h.soldier.deaths += 1;
                            h.respawn_at = Some(tick + RESPAWN_TICKS);
                        }
                    }
                }
                for b in &mut bots {
                    if b.soldier.id == hit.target && b.soldier.health > 0 {
                        actually_hit = true;
                        b.soldier.health = (b.soldier.health - hit.damage).max(0);
                        if b.soldier.health == 0 {
                            killed = true;
                            b.soldier.deaths += 1;
                            b.respawn_at = Some(tick + RESPAWN_TICKS);
                        }
                    }
                }
                if !actually_hit {
                    continue;
                }
                if killed {
                    for h in humans.values_mut() {
                        if h.soldier.id == shooter {
                            h.soldier.kills += 1;
                        }
                    }
                    for b in &mut bots {
                        if b.soldier.id == shooter {
                            b.soldier.kills += 1;
                        }
                    }
                }
                event_id += 1;
                event_buffer.push(CombatEvent {
                    input_seq,
                    id: event_id,
                    tick,
                    shooter,
                    victim: hit.target,
                    damage: hit.damage,
                    headshot: hit.headshot,
                    killed,
                });
            }
            let all_after: Vec<Soldier> = humans
                .values()
                .map(|h| h.soldier.clone())
                .chain(bots.iter().map(|b| b.soldier.clone()))
                .collect();
            for z in &mut zones {
                update_zone(z, &all_after);
            }
            if tick > 0 && tick.is_multiple_of(TICK_HZ as u64) {
                for z in &zones {
                    if z.owner >= 0 {
                        score[z.owner as usize] += 1;
                    }
                }
            }
            if score[0] >= 500 {
                winner = Some(0);
            } else if score[1] >= 500 {
                winner = Some(1);
            }
            if winner.is_some() {
                end_round_at = Some(tick + ROUND_END_TICKS);
            }
        }

        if tick.is_multiple_of(2) {
            let snapshot_soldiers: Vec<Soldier> = humans
                .values()
                .map(|h| h.soldier.clone())
                .chain(bots.iter().map(|b| b.soldier.clone()))
                .collect();
            for (addr, h) in &humans {
                let snap = Snapshot {
                    version: PROTOCOL_VERSION,
                    you: h.soldier.id,
                    tick,
                    ack_seq: h.ack_seq,
                    soldiers: snapshot_soldiers.clone(),
                    zones: zones.clone(),
                    score,
                    winner,
                    events: event_buffer.clone(),
                };
                if let Ok(bytes) = serde_json::to_vec(&snap) {
                    if bytes.len() <= 60_000 {
                        if let Err(e) = sock.send_to(&bytes, addr) {
                            eprintln!("UDP send {addr}: {e}");
                        }
                    } else {
                        eprintln!("Snapshot exceeds UDP budget: {} bytes", bytes.len());
                    }
                }
            }
            event_buffer.clear();
        }
        tick += 1;
        next_tick += step;
        let now = Instant::now();
        if next_tick > now {
            thread::sleep(next_tick - now);
        } else {
            next_tick = now;
        }
        if last_report.elapsed() > Duration::from_secs(30) {
            println!(
                "tick={tick} humans={} score={score:?} winner={winner:?}",
                humans.len()
            );
            last_report = Instant::now();
        }
    }
}
