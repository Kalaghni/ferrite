//! Wire protocol. Compact fixed-int bincode over UDP.
//!
//! Client -> server: only intentions (buttons + view angles), each command
//! sent three times for loss tolerance. Server -> client: world snapshots
//! at 30 Hz, transient effects, and an acknowledged feed for events that
//! must not be lost (kills, captures, round state).

use crate::sim::PState;
use bincode::Options;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const PROTO: u32 = 10;
pub const DEFAULT_PORT: u16 = 27500;
pub const SPECTATOR_ID: u8 = 255;

/// LAN discovery. A client sends `DISCOVERY_QUERY` to this multicast group
/// (and to the broadcast address, for networks that drop multicast); every
/// server answers the sender directly with `DISCOVERY_REPLY` + `ServerInfo`.
pub const DISCOVERY_PORT: u16 = 27499;
pub const DISCOVERY_GROUP: [u8; 4] = [239, 255, 70, 82];
pub const DISCOVERY_QUERY: &[u8] = b"FERRITE?";
pub const DISCOVERY_REPLY: &[u8] = b"FERRITE!";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ServerInfo {
    pub proto: u32,
    /// Random per server run: one server reached over two networks answers
    /// twice, and this is how the list knows it is the same one.
    pub id: u32,
    /// UDP port the game itself is on (the address is the reply's source).
    pub port: u16,
    pub name: String,
    pub map: u8,
    pub humans: u8,
    pub slots: u8,
    pub phase: u8,
}

pub fn encode_info(i: &ServerInfo) -> Vec<u8> {
    let mut v = DISCOVERY_REPLY.to_vec();
    v.extend(encode(i));
    v
}

pub fn decode_info(b: &[u8]) -> Option<ServerInfo> {
    let i: ServerInfo = decode(b.strip_prefix(DISCOVERY_REPLY)?)?;
    (i.name.len() <= 40).then_some(i)
}

pub mod btn {
    pub const JUMP: u8 = 1;
    pub const SPRINT: u8 = 2;
    pub const FIRE: u8 = 4;
    pub const ADS: u8 = 8;
    pub const RELOAD: u8 = 16;
    pub const GRENADE: u8 = 32;
    pub const CROUCH: u8 = 64;
    pub const SWAP: u8 = 128;
}

pub mod flag {
    pub const FIRING: u8 = 1;
    pub const SURGE: u8 = 2;
    pub const MARKED: u8 = 4;
    pub const ADS: u8 = 8;
    pub const GROUND: u8 = 16;
    pub const SHIELD: u8 = 32;
    pub const CROUCH: u8 = 64;
    pub const SIDEARM: u8 = 128;
}

/// Second flag byte: animation state for other players.
pub mod flag2 {
    pub const RELOAD: u8 = 1;
    pub const SLIDE: u8 = 2;
    pub const DRAW: u8 = 4;
}

/// One 60 Hz tick of player intent.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Cmd {
    pub seq: u32,
    pub mx: i8,
    pub mz: i8,
    pub buttons: u8,
    pub yaw: f32,
    pub pitch: f32,
    /// Server tick-time the client was displaying remote players at when
    /// this command was made. Used (clamped) for lag-compensated hitscan.
    pub interp: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum C2S {
    /// `team`: 0 or 1 to ask for a side, anything else for auto-balance.
    Hello { proto: u32, name: String, spectator: bool, team: u8 },
    /// `kit`, `prim`, `side`: the loadout wanted at the next deploy.
    Cmds { cmds: Vec<Cmd>, ack_feed: u32, kit: u8, prim: u8, side: u8 },
    Bye,
    /// Ask to change sides mid-match.
    Team(u8),
    /// In the lobby: the map voted for (255 = no vote) and whether ready.
    Lobby { vote: u8, ready: bool },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum S2C {
    Welcome { id: u8, team: u8, seed: u64, map: u8, feed_id: u32, tick: u32 },
    Snap(Snapshot),
    Roster(Vec<RosterEntry>),
    Reject(String),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct NetPlayer {
    pub id: u8,
    pub pos: [f32; 3],
    pub yaw: i16,
    pub pitch: i8,
    pub flags: u8,
    pub flags2: u8,
    pub hp: u8,
}

impl NetPlayer {
    pub fn yaw_f(&self) -> f32 {
        self.yaw as f32 / 10000.0
    }
    pub fn pitch_f(&self) -> f32 {
        self.pitch as f32 / 80.0
    }
    pub fn enc_yaw(y: f32) -> i16 {
        (crate::wrap_angle(y) * 10000.0) as i16
    }
    pub fn enc_pitch(p: f32) -> i8 {
        (p * 80.0).clamp(-127.0, 127.0) as i8
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct PointNet {
    pub owner: u8,
    /// -100 = fully EMBER, +100 = fully COBALT.
    pub progress: i8,
    pub n: [u8; 2],
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct SelfState {
    pub st: PState,
    pub hp: u8,
    pub alive: bool,
    pub respawn_in: f32,
    pub spawn_count: u8,
    pub spawn_yaw: f32,
    pub killer: u8,
}

/// Cosmetic, sent once. Losing one costs a tracer, not game state.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Fx {
    Shot { who: u8, from: [f32; 3], to: [f32; 3], hit: u8 },
    Hit { attacker: u8, victim: u8, dmg: u8, head: bool, kill: bool },
    Boom { pos: [f32; 3] },
    CrateBreak { pos: [f32; 3] },
    Throw { who: u8 },
}

/// Reliable: resent every snapshot until the client acknowledges the id.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Feed {
    /// `assist`: who else did real damage, or SPECTATOR_ID for nobody.
    Kill { killer: u8, victim: u8, weapon: u8, head: bool, assist: u8 },
    Capture { point: u8, team: u8 },
    Neutralized { point: u8, by: u8 },
    MatchEnd { winner: u8 },
    MatchStart,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct NetGrenade {
    pub id: u16,
    pub pos: [f32; 3],
    pub team: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snapshot {
    pub tick: u32,
    pub ack_seq: u32,
    pub me: Option<SelfState>,
    pub players: Vec<NetPlayer>,
    pub points: [PointNet; 5],
    pub tickets: [u16; 2],
    pub dmask: u64,
    pub grenades: Vec<NetGrenade>,
    pub fx: Vec<Fx>,
    pub feed: Vec<(u32, Feed)>,
    /// 0 = playing, 1 = round over, 2 = lobby (choose map and loadout).
    /// In the lobby `phase_secs` counts down to the start, or is 0 while
    /// the server waits for a player; `next_map` is the map that will be
    /// played and `votes` the votes per map.
    pub phase: u8,
    pub winner: u8,
    pub phase_secs: f32,
    /// Map being played, and the one the rotation moves to next round.
    pub map: u8,
    pub next_map: u8,
    pub votes: [u8; 8],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RosterEntry {
    pub id: u8,
    pub name: String,
    pub team: u8,
    pub kit: u8,
    pub prim: u8,
    pub side: u8,
    pub kills: u16,
    pub deaths: u16,
    pub score: u32,
    pub bot: bool,
    /// Ready in the lobby.
    pub ready: bool,
}

fn opts() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .with_limit(64 * 1024)
}

pub fn encode<T: Serialize>(v: &T) -> Vec<u8> {
    opts().serialize(v).unwrap_or_default()
}

/// Bounded decode: malformed or oversized input returns None, never panics.
pub fn decode<T: DeserializeOwned>(b: &[u8]) -> Option<T> {
    opts().deserialize(b).ok()
}
