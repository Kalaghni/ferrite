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
    pub fn connect(addr: SocketAddr, name: &str, spectator: bool, team: u8) -> Result<(Net, Welcome), String> {
        let sock = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| e.to_string())?;
        sock.connect(addr).map_err(|e| e.to_string())?;
        sock.set_read_timeout(Some(Duration::from_millis(100))).ok();
        let hello = encode(&C2S::Hello { proto: PROTO, name: name.to_string(), spectator, team });
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
                    while self.snaps.len() > 40 {
                        self.snaps.pop_front();
                    }
                }
                Some(S2C::Roster(r)) => self.roster = r,
                Some(S2C::Reject(why)) => self.rejected = Some(why),
                _ => {}
            }
        }
    }

    /// Tick-time at which remote entities are displayed right now.
    pub fn render_tick(&self, now: f64) -> f64 {
        now * 60.0 + self.clock_offset - INTERP_TICKS
    }

    pub fn send_cmds(&mut self, cmds: &[Cmd], kit: u8, now: f64) {
        if let Some(c) = cmds.last() {
            self.sent.push_back((c.seq, now));
            if self.sent.len() > 240 {
                self.sent.pop_front();
            }
        }
        let _ = self.sock.send(&encode(&C2S::Cmds { cmds: cmds.to_vec(), ack_feed: self.ack_feed, kit }));
    }

    /// Ask the server to move us to `team`. Sent a few times because the
    /// request is a single unacknowledged datagram; repeats are harmless.
    pub fn send_team(&self, team: u8) {
        for _ in 0..3 {
            let _ = self.sock.send(&encode(&C2S::Team(team)));
        }
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
