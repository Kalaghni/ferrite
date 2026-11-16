//! Wire protocol. Compact fixed-int bincode over UDP.
//!
//! Client -> server: only intentions (buttons + view angles), each command
//! sent three times for loss tolerance. Server -> client: world snapshots
//! at 30 Hz, transient effects, and an acknowledged feed for events that
//! must not be lost (kills, captures, round state).

use crate::sim::PState;
use bincode::Options;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// 19: session handshake. Every client datagram is a `Packet` (protocol
/// version, session token, message); a first Hello is answered with a
/// `Challenge` cookie and only a Hello that returns it gets a slot.
/// 18: Ridgeline's Fallen Giant replaces the central post and terrain pads.
pub const PROTO: u32 = 19;
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
    /// Game mode being played: index into `ferrite_shared::MODES`.
    pub mode: u8,
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

/// Second button byte (`Cmd::more`).
pub mod btn2 {
    pub const MELEE: u8 = 1;
    /// Movement axes are signed analog values in -127..127.
    pub const ANALOG_MOVE: u8 = 2;
}

/// Second flag byte: animation state for other players.
pub mod flag2 {
    pub const RELOAD: u8 = 1;
    pub const SLIDE: u8 = 2;
    pub const DRAW: u8 = 4;
    pub const MELEE: u8 = 8;
    /// Carrying the enemy flag.
    pub const CARRIER: u8 = 16;
    /// Wearing armour picked up on the field.
    pub const ARMOR: u8 = 32;
    /// Overcharged: dealing extra damage.
    pub const POWER: u8 = 64;
}

/// Things lying on the field to be walked over.
pub mod pickup {
    pub const HEALTH: u8 = 0;
    pub const ARMOR: u8 = 1;
    pub const AMMO: u8 = 2;
    pub const OVERDRIVE: u8 = 3;
    pub const OVERCHARGE: u8 = 4;
    pub const NAMES: [&str; 5] = ["HEALTH PACK", "ARMOUR", "RESUPPLY", "OVERDRIVE", "OVERCHARGE"];
    pub const BLURBS: [&str; 5] =
        ["+60 health", "+50 armour", "ammunition and frags refilled", "move and reload faster", "half again the damage"];
}

/// What happened to a flag (`Feed::Flag::what`).
pub mod flag_ev {
    pub const TAKEN: u8 = 0;
    pub const DROPPED: u8 = 1;
    pub const RETURNED: u8 = 2;
    pub const CAPTURED: u8 = 3;
}

/// Player icons are this many pixels square, sent as RGBA8.
pub const ICON_PX: usize = 16;
pub const ICON_BYTES: usize = ICON_PX * ICON_PX * 4;
/// Longest chat message, in characters.
pub const CHAT_MAX: usize = 120;
/// Roster entries per datagram: a full server's roster is sent in parts
/// so no part outgrows one packet.
pub const ROSTER_PART: usize = 12;

/// Identifies an icon's pixels (FNV-1a). Never 0: that means "no icon".
pub fn icon_hash(rgba: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in rgba {
        h = (h ^ *b as u32).wrapping_mul(0x0100_0193);
    }
    h.max(1)
}

/// A player id as text: 32 hex digits in the usual 8-4-4-4-12 groups.
pub fn uuid_text(id: &[u8; 16]) -> String {
    let h: String = id.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Reads what `uuid_text` writes (hyphens optional). All zeros is not an id.
pub fn uuid_parse(s: &str) -> Option<[u8; 16]> {
    let h: Vec<u8> = s.trim().bytes().filter(|b| *b != b'-').collect();
    if h.len() != 32 {
        return None;
    }
    let mut id = [0u8; 16];
    for (k, pair) in h.chunks(2).enumerate() {
        id[k] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    (id != [0u8; 16]).then_some(id)
}

/// One 60 Hz tick of player intent.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Cmd {
    pub seq: u32,
    pub mx: i8,
    pub mz: i8,
    pub buttons: u8,
    /// More buttons: `btn2`.
    pub more: u8,
    pub yaw: f32,
    pub pitch: f32,
    /// Server tick-time the client was displaying remote players at when
    /// this command was made. Used (clamped) for lag-compensated hitscan.
    pub interp: f64,
}

impl Cmd {
    /// Preserve keyboard intent while retaining analog stick magnitude.
    pub fn movement(&self) -> (f32, f32) {
        if self.more & btn2::ANALOG_MOVE != 0 {
            (self.mx.max(-127) as f32 / 127.0, self.mz.max(-127) as f32 / 127.0)
        } else {
            (self.mx.clamp(-1, 1) as f32, self.mz.clamp(-1, 1) as f32)
        }
    }

    /// Sanitize either command form before authoritative simulation.
    pub fn clamp_movement(&mut self) {
        let limit = if self.more & btn2::ANALOG_MOVE != 0 { 127 } else { 1 };
        self.mx = self.mx.clamp(-limit, limit);
        self.mz = self.mz.clamp(-limit, limit);
    }

    pub fn set_analog_movement(&mut self, right: f32, forward: f32) {
        let quantize = |v: f32| if v.is_finite() { (v.clamp(-1.0, 1.0) * 127.0).round() as i8 } else { 0 };
        self.mx = quantize(right);
        self.mz = quantize(forward);
        self.more |= btn2::ANALOG_MOVE;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum C2S {
    /// `team`: 0 or 1 to ask for a side, anything else for auto-balance.
    /// `uuid`: who this is, whatever callsign they use today (all zeros =
    /// nobody in particular). `icon`: empty, or `ICON_BYTES` of RGBA.
    /// `cookie`: 0 at first; then the one the server's `Challenge` gave.
    Hello {
        cookie: u64,
        name: String,
        spectator: bool,
        team: u8,
        uuid: [u8; 16],
        icon: Vec<u8>,
    },
    /// `kit`, `prim`, `side`: the loadout wanted at the next deploy, and
    /// `wskin` the finish on the primary and the sidearm. `ack_chat`: the
    /// newest chat line received.
    Cmds {
        cmds: Vec<Cmd>,
        ack_feed: u32,
        kit: u8,
        prim: u8,
        side: u8,
        skin: u8,
        wskin: [u8; 2],
        ack_chat: u32,
    },
    Bye,
    /// Ask to change sides mid-match.
    Team(u8),
    /// In the lobby: the map voted for (255 = no vote), whether ready, and
    /// the game mode voted for (255 = no vote).
    Lobby {
        vote: u8,
        ready: bool,
        mode: u8,
    },
    /// Say something, to everyone or to our own side. `id` goes up by one
    /// per message, so a repeat of the same datagram is said only once.
    Chat {
        id: u32,
        team: bool,
        text: String,
    },
    /// Ask for a player's icon (answered with `S2C::Icon`).
    WantIcon(u8),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum S2C {
    Welcome {
        id: u8,
        team: u8,
        seed: u64,
        map: u8,
        feed_id: u32,
        tick: u32,
        /// The session token every later packet from this client carries.
        token: u64,
    },
    Snap(Snapshot),
    /// The roster, `ROSTER_PART` entries at a time: `at` is the index of
    /// the first entry here, `total` how many there are in all.
    Roster {
        at: u8,
        total: u8,
        entries: Vec<RosterEntry>,
    },
    Reject(String),
    /// Chat lines the client has not acknowledged yet, oldest first.
    Chat(Vec<ChatLine>),
    /// Who wears an icon: (player id, icon hash). Anyone absent has none.
    Icons(Vec<(u8, u32)>),
    Icon {
        id: u8,
        hash: u32,
        rgba: Vec<u8>,
    },
    /// Rounds won by each team since the server started, and the most
    /// recent finished rounds, oldest first.
    Rounds {
        wins: [u16; 2],
        rounds: Vec<RoundResult>,
    },
    /// Credits this player earned in round `round` (resent while the
    /// result is on screen; the round number says which it is).
    Award {
        round: u32,
        credits: u32,
        won: bool,
    },
    /// Reply to a Hello without a valid cookie: send the Hello again with
    /// this one. Proves the client receives at the address it claims before
    /// the server spends a slot on it. Added last so older clients still
    /// decode `Reject` at its old position.
    Challenge {
        cookie: u64,
    },
}

/// Every datagram a client sends. `proto` comes first so a server can tell
/// an incompatible client from noise and answer it with a `Reject`; `token`
/// is 0 until the Welcome and that session's token after it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Packet {
    pub proto: u32,
    pub token: u64,
    pub msg: C2S,
}

/// Encode a client message for the wire.
pub fn encode_c2s(token: u64, msg: &C2S) -> Vec<u8> {
    encode(&Packet { proto: PROTO, token, msg: msg.clone() })
}

/// Join a server on a connected socket: send `hello` (its cookie is filled
/// in here), answer the challenge, and return the Welcome and the session
/// token. Retries for `limit`. Shared by the game client and the tests so
/// there is one implementation of the handshake.
pub fn handshake(sock: &std::net::UdpSocket, hello: &C2S, limit: std::time::Duration) -> Result<(S2C, u64), String> {
    use std::time::{Duration, Instant};
    let C2S::Hello { name, spectator, team, uuid, icon, .. } = hello else {
        return Err("handshake needs a Hello".into());
    };
    let hello_with = |cookie: u64| {
        encode_c2s(0, &C2S::Hello { cookie, name: name.clone(), spectator: *spectator, team: *team, uuid: *uuid, icon: icon.clone() })
    };
    let previous = sock.read_timeout().ok().flatten();
    sock.set_read_timeout(Some(Duration::from_millis(50))).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut cookie = 0u64;
    let mut buf = [0u8; 8192];
    let mut result = Err("no response".to_string());
    'outer: while start.elapsed() < limit {
        let _ = sock.send(&hello_with(cookie));
        let wait = Instant::now();
        while wait.elapsed() < Duration::from_millis(250) {
            let Ok(n) = sock.recv(&mut buf) else { continue };
            match decode::<S2C>(&buf[..n]) {
                Some(S2C::Challenge { cookie: c }) => {
                    if c != cookie {
                        cookie = c;
                        continue 'outer; // answer straight away
                    }
                }
                Some(w @ S2C::Welcome { token, .. }) => {
                    result = Ok((w, token));
                    break 'outer;
                }
                Some(S2C::Reject(why)) => {
                    result = Err(why);
                    break 'outer;
                }
                _ => {}
            }
        }
    }
    let _ = sock.set_read_timeout(previous);
    result
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
    /// Armour worn, 0..=100; kills since the last death; and seconds of
    /// overcharge left.
    pub armor: u8,
    pub streak: u8,
    pub power: u8,
}

/// Cosmetic, sent once. Losing one costs a tracer, not game state.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Fx {
    Shot {
        who: u8,
        from: [f32; 3],
        to: [f32; 3],
        hit: u8,
    },
    Hit {
        attacker: u8,
        victim: u8,
        dmg: u8,
        head: bool,
        kill: bool,
    },
    Boom {
        pos: [f32; 3],
    },
    CrateBreak {
        pos: [f32; 3],
    },
    Throw {
        who: u8,
    },
    /// A melee swing, and whether it landed.
    Swing {
        who: u8,
        hit: bool,
    },
    /// Someone picked something up (`pickup::*`).
    Pickup {
        who: u8,
        kind: u8,
        pos: [f32; 3],
    },
    /// A designated wall panel, walkway or machine changed state. The
    /// persistent state is in Snapshot::dmask; this only adds debris/sound.
    MapBreak {
        kind: u8,
        pos: [f32; 3],
    },
}

/// One line of chat. `seq` counts up across the whole server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatLine {
    pub seq: u32,
    /// Who said it, or `SPECTATOR_ID` for a notice from the server.
    pub from: u8,
    /// The speaker's side when they said it.
    pub team: u8,
    /// Said to everyone, rather than to the speaker's side only.
    pub all: bool,
    pub name: String,
    pub text: String,
}

/// Reliable: resent every snapshot until the client acknowledges the id.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Feed {
    /// `assist`: who else did real damage, or SPECTATOR_ID for nobody.
    Kill {
        killer: u8,
        victim: u8,
        weapon: u8,
        head: bool,
        assist: u8,
    },
    Capture {
        point: u8,
        team: u8,
    },
    Neutralized {
        point: u8,
        by: u8,
    },
    MatchEnd {
        winner: u8,
    },
    MatchStart,
    /// Flag `flag` (the team it belongs to) was taken, dropped, returned
    /// or captured (`flag_ev::*`) by player `by` (SPECTATOR_ID: nobody,
    /// it went home on its own).
    Flag {
        flag: u8,
        by: u8,
        what: u8,
    },
    /// `who` has `n` kills without dying.
    Streak {
        who: u8,
        n: u8,
    },
    /// `by` ended `victim`'s streak of `n`.
    Shutdown {
        by: u8,
        victim: u8,
        n: u8,
    },
}

/// A flag in capture-the-flag. `state`: 0 at home, 1 carried, 2 dropped.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct FlagNet {
    pub state: u8,
    pub carrier: u8,
    pub pos: [f32; 3],
}

/// Something to pick up. Position in decimetres.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NetPickup {
    pub id: u16,
    pub kind: u8,
    pub pos: [i16; 3],
}

impl NetPickup {
    pub fn pos_f(&self) -> [f32; 3] {
        [self.pos[0] as f32 * 0.1, self.pos[1] as f32 * 0.1, self.pos[2] as f32 * 0.1]
    }
    pub fn enc(p: [f32; 3]) -> [i16; 3] {
        p.map(|v| (v * 10.0).round().clamp(-32000.0, 32000.0) as i16)
    }
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
    /// Game mode (index into `MODES`), the one the next round will use,
    /// and the lobby's votes per mode.
    pub mode: u8,
    pub next_mode: u8,
    pub mode_votes: [u8; 4],
    /// Score to reach (tickets a team starts with, in conquest), and the
    /// seconds left on the clock (0 = no clock). Outside conquest
    /// `tickets` holds each team's score.
    pub goal: u16,
    pub secs_left: u16,
    /// Capture the flag: each team's flag.
    pub flags: [FlagNet; 2],
    pub pickups: Vec<NetPickup>,
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
    /// Headshot kills this round.
    pub heads: u16,
    /// This player's record on this server, this round included.
    pub total: Totals,
    /// Look of the soldier: index into `sim::SKINS`.
    pub skin: u8,
    /// Finish on the primary and the sidearm: index into `sim::WEAPON_SKINS`.
    pub wskin: [u8; 2],
}

/// What a player has done on one server, under one name.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub kills: u32,
    pub deaths: u32,
    pub score: u32,
    pub heads: u32,
    /// Rounds seen to the end, and how many of those their team won.
    pub rounds: u32,
    pub wins: u32,
}

/// How one finished round went.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RoundResult {
    pub map: u8,
    /// Game mode it was played in.
    pub mode: u8,
    pub winner: u8,
    pub tickets: [u16; 2],
    /// Length of the round in seconds.
    pub secs: u16,
    /// Top scorer of the round.
    pub mvp: String,
    pub mvp_team: u8,
    pub mvp_score: u32,
}

fn opts() -> impl Options {
    bincode::DefaultOptions::new().with_fixint_encoding().allow_trailing_bytes().with_limit(64 * 1024)
}

pub fn encode<T: Serialize>(v: &T) -> Vec<u8> {
    opts().serialize(v).unwrap_or_default()
}

/// Bounded decode: malformed or oversized input returns None, never panics.
pub fn decode<T: DeserializeOwned>(b: &[u8]) -> Option<T> {
    opts().deserialize(b).ok()
}
