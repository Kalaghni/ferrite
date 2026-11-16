//! Client networking: handshake, redundant command upload, snapshot buffer
//! and the clock used to interpolate remote players.

use ferrite_shared::glam::Vec3;
use ferrite_shared::proto::*;
use ferrite_shared::wrap_angle;
use std::collections::VecDeque;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Remote players are drawn this many server ticks in the past so there is
/// always a pair of snapshots to interpolate between (100 ms at 60 Hz).
pub const INTERP_TICKS: f64 = 6.0;
/// Snapshots kept (30 a second).
const HISTORY: usize = 260;

pub struct Welcome {
    pub id: u8,
    pub team: u8,
    pub seed: u64,
    pub map: u8,
}

pub struct Net {
    sock: UdpSocket,
    pub snaps: VecDeque<Snapshot>,
    pub roster: Vec<RosterEntry>,
    /// A roster still arriving, part by part.
    roster_build: Vec<RosterEntry>,
    /// Chat lines that have arrived and not been shown yet, and the newest
    /// one acknowledged.
    pub chat_in: Vec<ChatLine>,
    pub ack_chat: u32,
    /// Things we said that are still being repeated to the server:
    /// (id, team only, text, sends left, when to send next).
    chat_out: Vec<(u32, bool, String, u8, f64)>,
    chat_id: u32,
    /// Who wears an icon (id, hash), and icons that have just arrived.
    pub icon_ids: Vec<(u8, u32)>,
    pub icon_in: Vec<(u8, u32, Vec<u8>)>,
    /// Rounds won by each team on this server, and its recent rounds.
    pub wins: [u16; 2],
    pub rounds: Vec<RoundResult>,
    pub ack_feed: u32,
    pub fresh: Vec<Snapshot>,
    clock_offset: f64,
    have_clock: bool,
    sent: VecDeque<(u32, f64)>,
    pub ping_ms: f32,
    pub last_recv: f64,
    pub snaps_recv: u64,
    pub bytes_in: u64,
    pub rejected: Option<String>,
    /// Credits paid for finished rounds, not yet banked: (credits, won).
    pub awards: Vec<(u32, bool)>,
    award_round: u32,
}

#[derive(Clone, Copy)]
pub struct Remote {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,
    pub flags2: u8,
    pub hp: u8,
}

impl Net {
    /// Blocking handshake with retries (runs before the first frame).
    pub fn connect(addr: SocketAddr, name: &str, spectator: bool, team: u8, uuid: [u8; 16], icon: &[u8]) -> Result<(Net, Welcome), String> {
        let sock = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| e.to_string())?;
        sock.connect(addr).map_err(|e| e.to_string())?;
        sock.set_read_timeout(Some(Duration::from_millis(100))).ok();
        let hello = encode(&C2S::Hello { proto: PROTO, name: name.to_string(), spectator, team, uuid, icon: icon.to_vec() });
        let start = Instant::now();
        let mut buf = [0u8; 4096];
        let mut welcome = None;
        while start.elapsed() < Duration::from_secs(4) && welcome.is_none() {
            let _ = sock.send(&hello);
            let wait = Instant::now();
            while wait.elapsed() < Duration::from_millis(250) {
                if let Ok(n) = sock.recv(&mut buf) {
                    match decode::<S2C>(&buf[..n]) {
                        Some(S2C::Welcome { id, team, seed, map, feed_id, .. }) => {
                            welcome = Some((Welcome { id, team, seed, map }, feed_id));
                            break;
                        }
                        Some(S2C::Reject(why)) => return Err(why),
                        _ => {}
                    }
                }
            }
        }
        let (w, feed_id) = welcome.ok_or_else(|| format!("no response from {addr}"))?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok((
            Net {
                sock,
                snaps: VecDeque::new(),
                roster: Vec::new(),
                roster_build: Vec::new(),
                chat_in: Vec::new(),
                ack_chat: 0,
                chat_out: Vec::new(),
                chat_id: 0,
                icon_ids: Vec::new(),
                icon_in: Vec::new(),
                wins: [0; 2],
                rounds: Vec::new(),
                ack_feed: feed_id,
                fresh: Vec::new(),
                clock_offset: 0.0,
                have_clock: false,
                sent: VecDeque::new(),
                ping_ms: 0.0,
                last_recv: 0.0,
                snaps_recv: 0,
                bytes_in: 0,
                rejected: None,
                awards: Vec::new(),
                award_round: 0,
            },
            w,
        ))
    }

    /// Drain the socket. New snapshots land in `fresh` in tick order.
    pub fn poll(&mut self, now: f64) {
        let mut buf = [0u8; 8192];
        self.fresh.clear();
        while let Ok(n) = self.sock.recv(&mut buf) {
            self.bytes_in += n as u64;
            match decode::<S2C>(&buf[..n]) {
                Some(S2C::Snap(s)) => {
                    let newest = self.snaps.back().map_or(0, |b| b.tick);
                    if s.tick <= newest {
                        continue; // out of order or duplicate
                    }
                    self.snaps_recv += 1;
                    self.last_recv = now;
                    // Estimate server tick-time as a smoothed offset from our clock.
                    let off = s.tick as f64 - now * 60.0;
                    if !self.have_clock || (off - self.clock_offset).abs() > 12.0 {
                        self.clock_offset = off;
                        self.have_clock = true;
                    } else {
                        self.clock_offset += (off - self.clock_offset) * 0.05;
                    }
                    while let Some(&(seq, t)) = self.sent.front() {
                        if seq > s.ack_seq {
                            break;
                        }
                        if seq == s.ack_seq {
                            let rtt = ((now - t) * 1000.0) as f32;
                            self.ping_ms = if self.ping_ms == 0.0 { rtt } else { self.ping_ms * 0.9 + rtt * 0.1 };
                        }
                        self.sent.pop_front();
                    }
                    self.fresh.push(s.clone());
                    self.snaps.push_back(s);
                    // Several seconds are kept: the killcam replays them.
                    while self.snaps.len() > HISTORY {
                        self.snaps.pop_front();
                    }
                }
                Some(S2C::Roster { at, total, entries }) => {
                    if at == 0 {
                        self.roster_build.clear();
                    }
                    if at as usize == self.roster_build.len() {
                        self.roster_build.extend(entries);
                        if self.roster_build.len() >= total as usize {
                            self.roster = std::mem::take(&mut self.roster_build);
                        }
                    }
                }
                Some(S2C::Chat(lines)) => {
                    for l in lines {
                        if l.seq > self.ack_chat {
                            self.ack_chat = l.seq;
                            self.chat_in.push(l);
                        }
                    }
                }
                Some(S2C::Icons(list)) => self.icon_ids = list,
                Some(S2C::Icon { id, hash, rgba }) => {
                    if rgba.len() == ICON_BYTES && icon_hash(&rgba) == hash {
                        self.icon_in.push((id, hash, rgba));
                    }
                }
                Some(S2C::Rounds { wins, rounds }) => {
                    self.wins = wins;
                    self.rounds = rounds;
                }
                Some(S2C::Reject(why)) => self.rejected = Some(why),
                Some(S2C::Award { round, credits, won }) => {
                    // Resent while the result is up: each round pays once.
                    if round > self.award_round {
                        self.award_round = round;
                        self.awards.push((credits, won));
                    }
                }
                _ => {}
            }
        }
        // Chat rides on plain UDP: say each thing three times.
        for m in self.chat_out.iter_mut().filter(|m| now >= m.4) {
            let _ = self.sock.send(&encode(&C2S::Chat { id: m.0, team: m.1, text: m.2.clone() }));
            m.3 -= 1;
            m.4 = now + 0.12;
        }
        self.chat_out.retain(|m| m.3 > 0);
    }

    /// Say something to everyone, or to our own side.
    pub fn say(&mut self, team: bool, text: &str) {
        self.chat_id += 1;
        self.chat_out.push((self.chat_id, team, text.to_string(), 3, 0.0));
    }

    pub fn want_icon(&self, id: u8) {
        let _ = self.sock.send(&encode(&C2S::WantIcon(id)));
    }

    /// Tick-time at which remote entities are displayed right now.
    pub fn render_tick(&self, now: f64) -> f64 {
        now * 60.0 + self.clock_offset - INTERP_TICKS
    }

    pub fn send_cmds(&mut self, cmds: &[Cmd], loadout: (u8, u8, u8), skin: u8, wskin: [u8; 2], now: f64) {
        let (kit, prim, side) = loadout;
        if let Some(c) = cmds.last() {
            self.sent.push_back((c.seq, now));
            if self.sent.len() > 240 {
                self.sent.pop_front();
            }
        }
        let _ = self.sock.send(&encode(&C2S::Cmds { cmds: cmds.to_vec(), ack_feed: self.ack_feed, kit, prim, side, skin, wskin, ack_chat: self.ack_chat }));
    }

    /// Ask the server to move us to `team`. Sent a few times because the
    /// request is a single unacknowledged datagram; repeats are harmless.
    pub fn send_team(&self, team: u8) {
        for _ in 0..3 {
            let _ = self.sock.send(&encode(&C2S::Team(team)));
        }
    }

    pub fn send_lobby(&self, vote: u8, ready: bool, mode: u8) {
        let _ = self.sock.send(&encode(&C2S::Lobby { vote, ready, mode }));
    }

    pub fn bye(&self) {
        let _ = self.sock.send(&encode(&C2S::Bye));
    }

    pub fn latest(&self) -> Option<&Snapshot> {
        self.snaps.back()
    }

    /// Interpolated state of a remote player at render time `rt`.
    pub fn remote(&self, id: u8, rt: f64) -> Option<Remote> {
        let mut prev: Option<(&Snapshot, &NetPlayer)> = None;
        let mut next: Option<(&Snapshot, &NetPlayer)> = None;
        for s in self.snaps.iter() {
            let Some(p) = s.players.iter().find(|p| p.id == id) else {
                if (s.tick as f64) <= rt {
                    prev = None; // was dead at this point
                }
                continue;
            };
            if (s.tick as f64) <= rt {
                prev = Some((s, p));
            } else {
                next = Some((s, p));
                break;
            }
        }
        let mk = |p: &NetPlayer| Remote { pos: Vec3::from(p.pos), yaw: p.yaw_f(), pitch: p.pitch_f(), flags: p.flags, flags2: p.flags2, hp: p.hp };
        match (prev, next) {
            (Some((s0, a)), Some((s1, b))) => {
                let f = ((rt - s0.tick as f64) / (s1.tick - s0.tick).max(1) as f64).clamp(0.0, 1.0) as f32;
                let (pa, pb) = (Vec3::from(a.pos), Vec3::from(b.pos));
                if (pb - pa).length() > 6.0 {
                    return Some(mk(b)); // respawn teleport: do not slide
                }
                let mut r = mk(b);
                r.pos = pa + (pb - pa) * f;
                r.yaw = a.yaw_f() + wrap_angle(b.yaw_f() - a.yaw_f()) * f;
                r.pitch = a.pitch_f() + (b.pitch_f() - a.pitch_f()) * f;
                Some(r)
            }
            (Some((s0, a)), None) => {
                // Starved of snapshots: hold the last pose briefly, then hide.
                if rt - s0.tick as f64 > 30.0 {
                    None
                } else {
                    Some(mk(a))
                }
            }
            (None, Some(_)) => None,
            (None, None) => None,
        }
    }
}
