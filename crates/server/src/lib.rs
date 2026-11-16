//! Authoritative FERRITE server: game rules, bots and the UDP front end.
//! `Game` is pure simulation (no sockets) so it can be tested headlessly;
//! `net` wraps it in a 60 Hz loop. The client embeds this crate to host.

pub mod bots;
pub mod net;

use bots::Bot;
use ferrite_shared::map::{self, Nav, World};
use ferrite_shared::proto::*;
use ferrite_shared::rng::Rng;
use ferrite_shared::sim::{self, PState, GRENADE_WEAPON};
use ferrite_shared::{eligible, mode, vec3, Vec3, DT, FLAG_POINT, NEUTRAL, NUM_POINTS, POINT_RADIUS, TICK_HZ};

pub use net::{run, spawn, ServerHandle, CONNS_PER_IP, MAX_CONNS, RECV_PER_ADDR, RECV_PER_TICK};

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub per_team: usize,
    /// Most bots in the match at once. Slots beyond it stay empty until
    /// a human takes them.
    pub max_bots: usize,
    pub seed: u64,
    /// Index into `ferrite_shared::map::MAPS`.
    pub map: u8,
    /// Maps played in turn, one per round, starting after `map`. Fewer
    /// than two entries means the map never changes.
    pub rotation: Vec<u8>,
    /// Length of the lobby before each round, in seconds. 0 = no lobby:
    /// rounds follow each other directly.
    pub lobby_secs: u32,
    pub tickets: i32,
    pub bot_skill: f32,
    pub log: bool,
    /// Name shown to players looking for games on their network.
    pub name: String,
    /// Answer LAN discovery queries.
    pub discover: bool,
    /// Where the players' records are kept between runs. `None` keeps
    /// them in memory only.
    pub stats_file: Option<std::path::PathBuf>,
    /// Game mode of the first round: index into `ferrite_shared::MODES`.
    pub mode: u8,
    /// Modes played in turn, one per round. Fewer than two entries means
    /// the mode only changes when the lobby votes for another.
    pub mode_rotation: Vec<u8>,
    /// Score that wins a round outside conquest (0 = the mode's own), and
    /// the length of its clock in seconds (0 = the mode's own).
    pub score_limit: u32,
    pub time_limit: u32,
    /// Kills leave health, ammunition and streak rewards on the field.
    pub drops: bool,
}

/// Bot difficulty presets: a name and the skill it stands for. Skill
/// divides reaction time and aim error.
pub const DIFFICULTIES: [(&str, f32); 4] = [("RECRUIT", 0.55), ("REGULAR", 1.0), ("VETERAN", 1.5), ("ELITE", 2.2)];

pub fn difficulty_by_name(s: &str) -> Option<f32> {
    DIFFICULTIES.iter().find(|d| d.0.eq_ignore_ascii_case(s.trim())).map(|d| d.1)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: DEFAULT_PORT,
            per_team: 12,
            max_bots: 64,
            seed: 0xFE221,
            map: 0,
            rotation: Vec::new(),
            lobby_secs: 0,
            tickets: 250,
            bot_skill: 1.0,
            log: false,
            name: host_name(),
            discover: true,
            stats_file: None,
            mode: 0,
            mode_rotation: Vec::new(),
            score_limit: 0,
            time_limit: 0,
            drops: true,
        }
    }
}

/// This machine's name, for the server list.
pub fn host_name() -> String {
    let n = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    if n.trim().is_empty() {
        "FERRITE server".to_string()
    } else {
        n.trim().chars().take(24).collect()
    }
}

const HIST: usize = 64;
const MAX_REWIND: f64 = 30.0;
const CAP_RATE: f32 = 0.1;
const BOT_NAMES: [&str; 32] = [
    "Anvil", "Brick", "Cinder", "Dross", "Echo", "Flint", "Gasket", "Hex", "Ingot", "Jolt", "Kiln", "Latch", "Mica", "Nickel", "Onyx",
    "Piston", "Quartz", "Rivet", "Slag", "Tongs", "Umber", "Vise", "Weld", "Xylo", "Yoke", "Zinc", "Bolt", "Crank", "Dynamo", "Ember",
    "Forge", "Girder",
];

pub struct Player {
    pub id: u8,
    pub name: String,
    pub team: u8,
    pub next_kit: u8,
    pub next_prim: u8,
    pub next_side: u8,
    /// Lobby: map voted for (255 = none) and ready flag.
    pub vote: u8,
    pub ready: bool,
    /// Lobby: game mode voted for (255 = none).
    pub mode_vote: u8,
    /// Armour picked up (0..=100): soaks most of each hit until it is gone.
    pub armor: f32,
    /// Kills since the last death.
    pub streak: u16,
    /// Overcharged until this tick: damage dealt is half again as much.
    pub power_until: u32,
    /// Ticks of the round in progress spent in it as a person.
    pub played: u32,
    /// Credits earned in the round just finished: (round, credits, won).
    pub award: Option<(u32, u32, bool)>,
    pub st: PState,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: f32,
    pub alive: bool,
    pub respawn_at: u32,
    pub last_dmg_tick: u32,
    pub last_seq: u32,
    pub last_cmd_tick: u32,
    pub credits: f32,
    pub ads: bool,
    pub kills: u16,
    pub deaths: u16,
    pub score: u32,
    /// Headshot kills this round.
    pub heads: u16,
    pub bot: Option<Bot>,
    /// A bot slot left empty by the bot limit: never deployed, not listed.
    pub benched: bool,
    /// Look of the soldier: index into `sim::SKINS`.
    pub skin: u8,
    /// Who this is across callsigns and sessions; all zeros for a bot or
    /// a client that sent none.
    pub uuid: [u8; 16],
    /// Their icon (`ICON_BYTES` of RGBA, or empty) and its hash (0 = none).
    pub icon: Vec<u8>,
    pub icon_hash: u32,
    /// Finish asked for on the primary and the sidearm, and the pair the
    /// weapons in hand are wearing.
    pub next_wskin: [u8; 2],
    pub wskin: [u8; 2],
    pub marked_until: u32,
    pub surge_until: u32,
    pub firing_until: u32,
    pub protect_until: u32,
    pub spawn_count: u8,
    pub spawn_yaw: f32,
    pub killer: u8,
    /// Recent damage taken: (attacker, tick, total). Decides assists.
    hits: Vec<(u8, u32, f32)>,
    hist: [Vec3; HIST],
}

impl Player {
    /// Queue gameplay choices for deployment, while cosmetics on the issued
    /// weapons change as soon as the client selects them. A finish sent for
    /// a different pending weapon must never repaint the weapon still held.
    pub fn select_loadout(&mut self, kit: u8, prim: u8, side: u8, skin: u8, wskin: [u8; 2]) {
        let (kit, prim, side) = sim::fix_loadout(kit, prim, side);
        (self.next_kit, self.next_prim, self.next_side) = (kit, prim, side);
        self.skin = skin % sim::SKINS.len() as u8;
        self.next_wskin = [sim::fix_weapon_skin(prim, wskin[0]), sim::fix_weapon_skin(side, wskin[1])];
        for (slot, issued, wanted) in [(0, self.st.prim, prim), (1, self.st.side, side)] {
            if issued == wanted {
                self.wskin[slot] = self.next_wskin[slot];
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub owner: u8,
    /// -1 = fully team 0, +1 = fully team 1.
    pub progress: f32,
    pub n: [u8; 2],
}

/// The kill feed and match events, as clients see them: every entry has an
/// absolute sequence number (its index since the server started, from 1 on
/// the wire), but only the newest `FEED_KEEP` are held, so a server that runs
/// for weeks does not grow without bound. Clients acknowledge by sequence
/// number; one that has fallen further behind than that simply skips ahead.
#[derive(Default)]
pub struct FeedLog {
    /// Sequence number of the oldest entry held.
    base: usize,
    items: std::collections::VecDeque<Feed>,
}

/// How many feed entries a server keeps (ten go out per snapshot).
pub const FEED_KEEP: usize = 256;

impl FeedLog {
    pub fn push(&mut self, f: Feed) {
        self.items.push_back(f);
        if self.items.len() > FEED_KEEP {
            self.items.pop_front();
            self.base += 1;
        }
    }
    /// Entries ever pushed, including those no longer held.
    pub fn len(&self) -> usize {
        self.base + self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Entries still held, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Feed> + '_ {
        self.items.iter()
    }
    pub fn last(&self) -> Option<&Feed> {
        self.items.back()
    }
    /// Held entries from absolute index `from` on, with their indices.
    pub fn since(&self, from: usize) -> impl Iterator<Item = (usize, &Feed)> + '_ {
        let skip = from.saturating_sub(self.base);
        self.items.iter().enumerate().skip(skip).map(move |(i, f)| (self.base + i, f))
    }
    /// How many entries are held (at most `FEED_KEEP`).
    pub fn held(&self) -> usize {
        self.items.len()
    }
}

/// A flag in capture the flag. `state`: 0 at home, 1 carried, 2 dropped.
#[derive(Clone, Copy, Debug)]
pub struct Flag {
    pub state: u8,
    pub pos: Vec3,
    pub carrier: usize,
    /// Tick it was dropped at.
    pub since: u32,
}

/// Something lying on the field: `kind` is one of `pickup::*`.
#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub id: u16,
    pub kind: u8,
    pub pos: Vec3,
    pub until: u32,
}

/// Streaks that are announced and rewarded.
pub fn streak_reward(n: u16) -> Option<u8> {
    match n {
        3 => Some(pickup::ARMOR),
        5 => Some(pickup::OVERDRIVE),
        7 => Some(pickup::OVERCHARGE),
        n if n >= 10 && n % 5 == 0 => Some(pickup::OVERCHARGE),
        _ => None,
    }
}

/// Credits for a finished round: a win pays far more than a loss, a good
/// score adds to either, and both scale with how much of the round the
/// player was there for and with rounds shorter than three minutes.
pub fn round_credits(won: bool, score: u32, played: f32, secs: f32) -> u32 {
    let base = if won { 120.0 } else { 45.0 };
    let bonus = (score as f32 / 20.0).min(120.0);
    ((base + bonus) * played.clamp(0.0, 1.0) * (secs / 180.0).clamp(0.0, 1.0)).round() as u32
}

fn wants(p: &Player, kind: u8) -> bool {
    match kind {
        pickup::HEALTH => p.hp < sim::kit(p.st.kit).hp,
        pickup::ARMOR => p.armor < 100.0,
        _ => true,
    }
}

pub struct Grenade {
    pub id: u16,
    pub pos: Vec3,
    pub vel: Vec3,
    pub fuse: i32,
    pub owner: usize,
    pub team: u8,
}

/// Whose record: a bot's goes by its name, a player's by their id. `Name`
/// is a player without an id (a record from before ids, or a client that
/// sent none).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Who {
    Bot(String),
    Id([u8; 16]),
    Name(String),
}

/// A record and the callsign last seen with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub name: String,
    pub totals: Totals,
}

fn who(p: &Player) -> Who {
    if p.bot.is_some() {
        Who::Bot(p.name.clone())
    } else if p.uuid != [0u8; 16] {
        Who::Id(p.uuid)
    } else {
        Who::Name(p.name.clone())
    }
}

#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub kills: u32,
    pub shots: u32,
    pub captures: u32,
    pub rounds: u32,
    pub dropped_cmds: u32,
}

pub struct Game {
    pub cfg: Config,
    pub world: World,
    pub nav: Nav,
    pub tick: u32,
    pub players: Vec<Player>,
    pub points: [Point; NUM_POINTS],
    pub tickets: [i32; 2],
    pub dmask: u64,
    crate_hp: Vec<f32>,
    pub grenades: Vec<Grenade>,
    next_gren: u16,
    /// Effects, each with a sequence number so every client gets each one
    /// exactly once, whenever in the tick it happened.
    pub fx: Vec<(u32, Fx)>,
    pub fx_seq: u32,
    /// Map the lobby falls back to when nobody votes.
    lobby_map: u8,
    pub feed: FeedLog,
    pub phase: u8,
    pub winner: u8,
    pub phase_until: u32,
    pub stats: Stats,
    /// Finished rounds, oldest first, and rounds won by each team.
    pub history: Vec<RoundResult>,
    pub wins: [u16; 2],
    round_start: u32,
    /// Every player's record on this server, banked whenever their round
    /// counters are reset. A player's follows their id, not their callsign.
    pub records: std::collections::HashMap<Who, Record>,
    /// Recent chat, oldest first, and the number of the newest line.
    pub chat: Vec<ChatLine>,
    pub chat_seq: u32,
    /// Each team's current push target (bots mostly follow it).
    pub orders: [usize; 2],
    /// Game mode of this round, and the one the lobby falls back to.
    pub mode: u8,
    lobby_mode: u8,
    /// Rounds started so far; score that wins this one; tick its clock
    /// runs out at (0 = no clock).
    pub round_no: u32,
    pub goal: i32,
    pub round_end: u32,
    pub flags: [Flag; 2],
    pub pickups: Vec<Pickup>,
    next_pickup: u16,
    rng: Rng,
}

fn initial_points(m: u8) -> [Point; NUM_POINTS] {
    let mut p = [Point { owner: NEUTRAL, progress: 0.0, n: [0; 2] }; NUM_POINTS];
    if m == mode::CONQUEST {
        p[0] = Point { owner: 0, progress: -1.0, n: [0; 2] };
        p[NUM_POINTS - 1] = Point { owner: 1, progress: 1.0, n: [0; 2] };
    }
    p
}

impl Game {
    pub fn new(cfg: Config) -> Game {
        let world = World::with_map(cfg.seed, cfg.map);
        let nav = Nav::build(&world);
        let mut g = Game {
            world,
            nav,
            tick: 0,
            players: Vec::new(),
            points: initial_points(cfg.mode % ferrite_shared::MODES.len() as u8),
            tickets: [cfg.tickets; 2],
            dmask: 0,
            crate_hp: Vec::new(),
            grenades: Vec::new(),
            next_gren: 0,
            fx: Vec::new(),
            fx_seq: 0,
            lobby_map: cfg.map,
            feed: FeedLog::default(),
            phase: 0,
            winner: NEUTRAL,
            phase_until: 0,
            stats: Stats::default(),
            history: Vec::new(),
            wins: [0; 2],
            round_start: 0,
            records: load_records(cfg.stats_file.as_deref()),
            chat: Vec::new(),
            chat_seq: 0,
            orders: [1, 3],
            mode: cfg.mode % ferrite_shared::MODES.len() as u8,
            lobby_mode: cfg.mode % ferrite_shared::MODES.len() as u8,
            round_no: 1,
            goal: cfg.tickets,
            round_end: 0,
            flags: [Flag { state: 0, pos: Vec3::ZERO, carrier: 0, since: 0 }; 2],
            pickups: Vec::new(),
            next_pickup: 0,
            rng: Rng::new(cfg.seed ^ 0x5EED),
            cfg,
        };
        g.reset_field();
        g.crate_hp = vec![110.0; g.world.n_crates];
        let n = (g.cfg.per_team * 2).min(64);
        for i in 0..n {
            let team = (i % 2) as u8;
            let mut p = Player { id: i as u8, team, ..dummy() };
            g.make_bot(&mut p);
            g.players.push(p);
        }
        g.balance_bots();
        if g.cfg.lobby_secs > 0 {
            // Start in the lobby and wait there for the first player.
            g.phase = 2;
        } else {
            for i in 0..n {
                g.spawn_player(i);
            }
        }
        g
    }

    /// The score a round of the current mode is played to.
    pub fn mode_goal(&self) -> i32 {
        if self.mode == mode::CONQUEST {
            return self.cfg.tickets;
        }
        if self.cfg.score_limit > 0 {
            return self.cfg.score_limit.min(60000) as i32;
        }
        match self.mode {
            mode::TDM => (self.cfg.per_team as i32 * 8).max(20),
            mode::CTF => 3,
            _ => 180,
        }
    }

    /// Length of the current mode's clock in seconds; 0 = none.
    pub fn mode_secs(&self) -> u32 {
        if self.mode == mode::CONQUEST {
            return 0;
        }
        if self.cfg.time_limit > 0 {
            return self.cfg.time_limit.min(3600);
        }
        match self.mode {
            mode::CTF => 720,
            _ => 480,
        }
    }

    /// Objectives, scores, flags and the clock as a round of the current
    /// mode begins.
    fn reset_field(&mut self) {
        self.points = initial_points(self.mode);
        self.goal = self.mode_goal();
        self.tickets = if self.mode == mode::CONQUEST { [self.cfg.tickets; 2] } else { [0; 2] };
        for f in 0..2 {
            self.flags[f] = Flag { state: 0, pos: self.world.points[FLAG_POINT[f]], carrier: 0, since: 0 };
        }
        self.pickups.clear();
        let secs = self.mode_secs();
        self.round_end = if secs > 0 { self.tick + secs * TICK_HZ } else { 0 };
        self.orders = if self.mode == mode::CONQUEST { [1, 3] } else { [2, 2] };
    }

    fn make_bot(&mut self, p: &mut Player) {
        let i = p.id as usize;
        p.name = BOT_NAMES[i % BOT_NAMES.len()].to_string();
        p.next_kit = [0, 0, 1, 2][(i / 2) % 4];
        // Bots carry a mix of what their class allows, weighted to the default.
        let prims = sim::kit(p.next_kit).prims;
        p.next_prim = if self.rng.below(5) < 2 { prims[0] } else { prims[self.rng.below(prims.len() as u32) as usize] };
        p.next_side =
            if self.rng.below(2) == 0 { sim::SIDEARM_ID } else { sim::SIDEARMS[self.rng.below(sim::SIDEARMS.len() as u32) as usize] };
        p.vote = 255;
        p.ready = false;
        p.mode_vote = 255;
        p.armor = 0.0;
        p.streak = 0;
        p.power_until = 0;
        p.played = 0;
        p.award = None;
        // Bots turn out in a mix of looks, and of finishes where a weapon
        // has more than one.
        p.skin = self.rng.below(sim::SKINS.len() as u32) as u8;
        for (k, wid) in [p.next_prim, p.next_side].into_iter().enumerate() {
            let fits = sim::weapon_skins_for(wid);
            p.next_wskin[k] = if fits.len() > 1 { fits[self.rng.below(fits.len() as u32) as usize] } else { 0 };
        }
        p.uuid = [0; 16];
        p.icon = Vec::new();
        p.icon_hash = 0;
        let skill = self.cfg.bot_skill * self.rng.range(0.7, 1.15);
        p.bot = Some(Bot::new(self.cfg.seed ^ (i as u64 * 977 + 13), skill, i));
        p.kills = 0;
        p.deaths = 0;
        p.score = 0;
        p.heads = 0;
        p.last_seq = 0;
    }

    /// Apply the bot limit: decide which bot slots are in the match. Bots
    /// go to whichever side is short, so a few bots even up uneven teams.
    /// Called whenever a human comes, goes or changes side.
    pub fn balance_bots(&mut self) {
        let n = self.players.len();
        let humans = self.players.iter().filter(|p| p.bot.is_none()).count();
        let total = self.cfg.max_bots.min(n - humans);
        let count = |g: &Game, t: u8, human: bool| g.players.iter().filter(|p| p.team == t && p.bot.is_none() == human).count();
        let free = [count(self, 0, false), count(self, 1, false)];
        let mut size = [count(self, 0, true), count(self, 1, true)];
        let mut want = [0usize; 2];
        for _ in 0..total {
            let t = if want[0] >= free[0] {
                1
            } else if want[1] >= free[1] {
                0
            } else if size[1] < size[0] {
                1
            } else {
                0
            };
            want[t] += 1;
            size[t] += 1;
        }
        let t = self.tick;
        for team in 0..2u8 {
            let mut left = want[team as usize];
            // Bots already in the match keep their place first.
            for pass in [false, true] {
                for p in self.players.iter_mut().filter(|p| p.team == team && p.bot.is_some() && p.benched == pass) {
                    if left > 0 {
                        left -= 1;
                        if p.benched {
                            p.benched = false;
                            p.respawn_at = t + 2 * TICK_HZ;
                        }
                    } else if !p.benched {
                        p.benched = true;
                        p.alive = false;
                        p.hp = 0.0;
                        p.marked_until = 0;
                    }
                }
            }
        }
    }

    /// Bots in the match right now.
    pub fn active_bots(&self) -> usize {
        self.players.iter().filter(|p| p.bot.is_some() && !p.benched).count()
    }

    /// A human takes over a bot slot on the team with fewer humans.
    pub fn join(&mut self, name: &str) -> Option<usize> {
        self.join_team(name, NEUTRAL)
    }

    /// As `join`, but honours a requested side (0 or 1) while that side
    /// still has a bot to replace. Friends may stack one team against bots.
    pub fn join_team(&mut self, name: &str, want: u8) -> Option<usize> {
        self.join_as(name, want, [0; 16], &[])
    }

    /// As `join_team`, for a player with an id and perhaps an icon. Their
    /// record on this server follows the id, whatever they call themselves.
    pub fn join_as(&mut self, name: &str, want: u8, uuid: [u8; 16], icon: &[u8]) -> Option<usize> {
        let humans = |t: u8, g: &Game| g.players.iter().filter(|p| p.team == t && p.bot.is_none()).count();
        let team = if want < 2 {
            want
        } else if humans(1, self) < humans(0, self) {
            1
        } else {
            0
        };
        // An empty seat first, so no bot is pushed out mid-fight.
        let slot = self
            .players
            .iter()
            .position(|p| p.team == team && p.bot.is_some() && p.benched)
            .or_else(|| self.players.iter().position(|p| p.team == team && p.bot.is_some()))
            .or_else(|| self.players.iter().position(|p| p.bot.is_some()))?;
        let clean: String = name.chars().filter(|c| c.is_ascii_graphic()).take(14).collect();
        let t = self.tick;
        self.bank(slot); // what the bot did stays with the bot
        let p = &mut self.players[slot];
        p.bot = None;
        p.name = if clean.is_empty() { format!("Player{}", slot) } else { clean };
        p.kills = 0;
        p.deaths = 0;
        p.score = 0;
        p.last_seq = 0;
        p.last_cmd_tick = t;
        p.next_kit = 0;
        p.next_prim = 0;
        p.next_side = sim::SIDEARM_ID;
        p.vote = 255;
        p.ready = false;
        p.mode_vote = 255;
        p.armor = 0.0;
        p.streak = 0;
        p.power_until = 0;
        p.played = 0;
        p.award = None;
        p.alive = false;
        p.benched = false;
        p.skin = 0;
        p.next_wskin = [0; 2];
        p.respawn_at = t + 20;
        p.uuid = uuid;
        p.icon = if icon.len() == ICON_BYTES { icon.to_vec() } else { Vec::new() };
        p.icon_hash = if p.icon.is_empty() { 0 } else { icon_hash(&p.icon) };
        let (name, team) = (p.name.clone(), p.team);
        if uuid != [0u8; 16] {
            // A record kept under this callsign before ids existed is theirs.
            if !self.records.contains_key(&Who::Id(uuid)) {
                if let Some(r) = self.records.remove(&Who::Name(name.clone())) {
                    self.records.insert(Who::Id(uuid), r);
                }
            }
            if let Some(r) = self.records.get_mut(&Who::Id(uuid)) {
                r.name = name.clone();
            }
        }
        self.notice(format!("{name} joined {}", ferrite_shared::TEAM_NAMES[team as usize % 2]));
        self.balance_bots();
        Some(slot)
    }

    /// Move a human to the other side by trading places with one of its
    /// bots, so the teams stay the same size. Both redeploy; no death or
    /// ticket is charged. Returns false if the side has no bot to trade.
    pub fn set_team(&mut self, i: usize, team: u8) -> bool {
        if i >= self.players.len() || team > 1 || self.players[i].bot.is_some() {
            return false;
        }
        if self.players[i].team == team {
            return true;
        }
        let Some(j) = self.players.iter().position(|p| p.team == team && p.bot.is_some()) else { return false };
        let t = self.tick;
        self.players[j].team = 1 - team;
        self.players[i].team = team;
        for k in [i, j] {
            let p = &mut self.players[k];
            p.alive = false;
            p.hp = 0.0;
            p.killer = SPECTATOR_ID;
            p.marked_until = 0;
            p.respawn_at = t + 3 * TICK_HZ;
        }
        self.balance_bots();
        true
    }

    pub fn leave(&mut self, i: usize) {
        if i >= self.players.len() || self.players[i].bot.is_some() {
            return;
        }
        self.bank(i);
        self.save_records();
        self.notice(format!("{} left", self.players[i].name));
        let mut p = std::mem::replace(&mut self.players[i], dummy());
        self.make_bot(&mut p);
        p.benched = false;
        p.alive = false;
        self.players[i] = p;
        self.balance_bots();
    }

    /// Add a player's round counters to their record and zero them. Called
    /// wherever the counters are reset, so record + counters is always the
    /// player's total.
    fn bank(&mut self, i: usize) {
        let p = &mut self.players[i];
        if !p.name.is_empty() && (p.kills > 0 || p.deaths > 0 || p.score > 0) {
            let rec = self.records.entry(who(p)).or_default();
            rec.name = p.name.clone();
            let r = &mut rec.totals;
            r.kills += p.kills as u32;
            r.deaths += p.deaths as u32;
            r.score += p.score;
            r.heads += p.heads as u32;
        }
        p.kills = 0;
        p.deaths = 0;
        p.score = 0;
        p.heads = 0;
    }

    /// A player's record on this server, the round in progress included.
    pub fn totals(&self, i: usize) -> Totals {
        let p = &self.players[i];
        let mut r = self.records.get(&who(p)).map(|r| r.totals).unwrap_or_default();
        r.kills += p.kills as u32;
        r.deaths += p.deaths as u32;
        r.score += p.score;
        r.heads += p.heads as u32;
        r
    }

    /// The round is decided: note how it went and credit everyone with it.
    fn close_round(&mut self) {
        let w = self.winner;
        self.stats.rounds += 1;
        if w < 2 {
            self.wins[w as usize] += 1;
        }
        for p in self.players.iter().filter(|p| !p.name.is_empty()) {
            let rec = self.records.entry(who(p)).or_default();
            rec.name = p.name.clone();
            rec.totals.rounds += 1;
            if p.team == w {
                rec.totals.wins += 1;
            }
        }
        // Pay the people who played: a win is worth far more than a loss.
        let len = (self.tick - self.round_start).max(1) as f32;
        let round = self.round_no;
        for p in self.players.iter_mut().filter(|p| p.bot.is_none() && !p.name.is_empty()) {
            let won = p.team == w;
            p.award = Some((round, round_credits(won, p.score, p.played as f32 / len, len / TICK_HZ as f32), won));
        }
        let mvp = self.players.iter().max_by_key(|p| p.score);
        self.history.push(RoundResult {
            map: self.world.map,
            mode: self.mode,
            winner: w,
            tickets: [self.tickets[0].max(0) as u16, self.tickets[1].max(0) as u16],
            secs: ((self.tick - self.round_start) / TICK_HZ).min(u16::MAX as u32) as u16,
            mvp: mvp.map_or(String::new(), |p| p.name.clone()),
            mvp_team: mvp.map_or(NEUTRAL, |p| p.team),
            mvp_score: mvp.map_or(0, |p| p.score),
        });
        if self.history.len() > 32 {
            self.history.remove(0);
        }
        self.save_records();
    }

    /// The last few rounds, for the scoreboard.
    pub fn rounds_msg(&self) -> S2C {
        let from = self.history.len().saturating_sub(8);
        S2C::Rounds { wins: self.wins, rounds: self.history[from..].to_vec() }
    }

    /// Write the human players' records to the stats file, best first.
    /// Bots are left out: theirs last only as long as the server runs.
    pub fn save_records(&self) {
        let Some(path) = &self.cfg.stats_file else { return };
        let mut all: std::collections::HashMap<Who, Record> =
            self.records.iter().filter(|(k, _)| !matches!(k, Who::Bot(_))).map(|(k, v)| (k.clone(), v.clone())).collect();
        for (i, p) in self.players.iter().enumerate().filter(|(_, p)| p.bot.is_none() && !p.name.is_empty()) {
            all.insert(who(p), Record { name: p.name.clone(), totals: self.totals(i) });
        }
        let mut rows: Vec<_> = all.into_iter().filter(|(_, r)| !r.name.is_empty()).collect();
        let id = |k: &Who| match k {
            Who::Id(u) => uuid_text(u),
            _ => String::new(),
        };
        rows.sort_by(|a, b| b.1.totals.score.cmp(&a.1.totals.score).then(a.1.name.cmp(&b.1.name)).then(id(&a.0).cmp(&id(&b.0))));
        let mut out = String::from("# FERRITE player records: name kills deaths score headshots rounds wins id\n");
        for (k, rec) in rows {
            let r = rec.totals;
            out += format!("{} {} {} {} {} {} {} {}", rec.name, r.kills, r.deaths, r.score, r.heads, r.rounds, r.wins, id(&k)).trim_end();
            out.push('\n');
        }
        if let Err(e) = std::fs::write(path, out) {
            if self.cfg.log {
                println!("could not write {}: {e}", path.display());
            }
        }
    }

    fn push_chat(&mut self, mut line: ChatLine) {
        self.chat_seq += 1;
        line.seq = self.chat_seq;
        self.chat.push(line);
        if self.chat.len() > 80 {
            self.chat.drain(..20);
        }
    }

    /// Player `i` says something, to everyone or to their own side.
    /// Returns false if there was nothing to say.
    pub fn say(&mut self, i: usize, team_only: bool, text: &str) -> bool {
        let clean: String = text.chars().filter(|c| !c.is_control()).take(CHAT_MAX).collect();
        let clean = clean.trim();
        if clean.is_empty() || i >= self.players.len() || self.players[i].bot.is_some() {
            return false;
        }
        let p = &self.players[i];
        let line = ChatLine { seq: 0, from: i as u8, team: p.team, all: !team_only, name: p.name.clone(), text: clean.to_string() };
        self.push_chat(line);
        true
    }

    /// A line from the server itself, shown to everyone.
    pub fn notice(&mut self, text: String) {
        self.push_chat(ChatLine { seq: 0, from: SPECTATOR_ID, team: NEUTRAL, all: true, name: String::new(), text });
    }

    /// Chat newer than line `after` that `me` may read: everything said to
    /// all, and what their own side said among themselves. A few lines at
    /// a time, so the reply fits one packet.
    pub fn chat_for(&self, me: Option<usize>, after: u32) -> Vec<ChatLine> {
        let team = me.map(|i| self.players[i].team);
        let mut bytes = 0;
        self.chat
            .iter()
            .filter(|l| l.seq > after && (l.all || Some(l.team) == team))
            .take(4)
            .take_while(|l| {
                bytes += l.text.len() + l.name.len() + 24;
                bytes <= 1100
            })
            .cloned()
            .collect()
    }

    /// Who wears an icon.
    pub fn icons_msg(&self) -> S2C {
        S2C::Icons(self.players.iter().filter(|p| p.icon_hash != 0 && !p.benched).map(|p| (p.id, p.icon_hash)).collect())
    }

    pub fn owners(&self) -> [u8; NUM_POINTS] {
        let mut o = [NEUTRAL; NUM_POINTS];
        for (i, p) in self.points.iter().enumerate() {
            o[i] = p.owner;
        }
        o
    }

    pub fn owned(&self, team: u8) -> i32 {
        self.points.iter().filter(|p| p.owner == team).count() as i32
    }

    fn push_fx(&mut self, fx: Fx) {
        self.fx_seq += 1;
        self.fx.push((self.fx_seq, fx));
    }

    fn pos_at(&self, j: usize, tick_f: f64) -> Vec3 {
        let p = &self.players[j];
        let now = self.tick as f64;
        if tick_f >= now {
            return p.st.pos;
        }
        let t = tick_f.max(now - MAX_REWIND).max(0.0);
        let i0 = t.floor() as usize;
        let f = (t - i0 as f64) as f32;
        let a = p.hist[i0 % HIST];
        let b = if i0 + 1 >= self.tick as usize { p.st.pos } else { p.hist[(i0 + 1) % HIST] };
        a + (b - a) * f
    }

    /// Run one client command. Every input is validated here: sequence
    /// must advance, the per-tick budget bounds how much simulated time a
    /// client can claim, and angles/axes are clamped. Position is never
    /// accepted from the network.
    pub fn apply_cmd(&mut self, i: usize, mut cmd: Cmd) {
        let t = self.tick;
        {
            let p = &mut self.players[i];
            if cmd.seq <= p.last_seq {
                return;
            }
            p.last_seq = cmd.seq;
            p.last_cmd_tick = t;
            if p.credits < 1.0 {
                self.stats.dropped_cmds += 1;
                return;
            }
            p.credits -= 1.0;
            if !cmd.yaw.is_finite() || !cmd.pitch.is_finite() || !cmd.interp.is_finite() {
                return;
            }
            cmd.pitch = cmd.pitch.clamp(-1.55, 1.55);
            cmd.clamp_movement();
            p.yaw = cmd.yaw;
            p.pitch = cmd.pitch;
            p.ads = cmd.buttons & btn::ADS != 0;
            if !p.alive {
                return;
            }
        }
        let out = sim::step(&mut self.players[i].st, &cmd, &self.world, self.dmask);
        if out.fired {
            self.fire(i, &cmd, out.heat);
        }
        if out.threw {
            let p = &self.players[i];
            let dir = sim::view_dir(cmd.yaw, cmd.pitch);
            let g = Grenade {
                id: self.next_gren,
                pos: p.st.pos + vec3(0.0, sim::eye_h(p.st.crouch), 0.0) + dir * 0.5,
                vel: dir * 19.0 + vec3(0.0, 4.0, 0.0) + p.st.vel * 0.5,
                fuse: 132,
                owner: i,
                team: p.team,
            };
            self.next_gren = self.next_gren.wrapping_add(1);
            self.grenades.push(g);
            self.push_fx(Fx::Throw { who: i as u8 });
        }
        if out.melee {
            self.melee(i, &cmd);
        }
    }

    /// A melee swing: it lands on the nearest enemy within reach and
    /// roughly in front, judged (like a shot) against where they were on
    /// the attacker's screen. From behind it kills outright.
    fn melee(&mut self, i: usize, cmd: &Cmd) {
        let t = self.tick;
        let (team, origin, human) = {
            let p = &self.players[i];
            (p.team, p.st.pos + vec3(0.0, sim::eye_h(p.st.crouch), 0.0), p.bot.is_none())
        };
        let dir = sim::view_dir(cmd.yaw, cmd.pitch);
        let rewind = if human { cmd.interp.clamp(t as f64 - MAX_REWIND, t as f64) } else { t as f64 };
        let mut best: Option<(usize, f32, Vec3)> = None;
        for (j, p) in self.players.iter().enumerate() {
            if !p.alive || p.team == team || p.protect_until > t {
                continue;
            }
            let pos = self.pos_at(j, rewind);
            let chest = pos + vec3(0.0, if p.st.crouch { 0.6 } else { 1.0 }, 0.0);
            let to = chest - origin;
            let d = to.length();
            if d > sim::MELEE_REACH || (d > 0.7 && to.dot(dir) / d < 0.5) {
                continue;
            }
            if best.is_none_or(|(_, bd, _)| d < bd) && self.world.los(origin, chest, self.dmask) {
                best = Some((j, d, pos));
            }
        }
        self.players[i].protect_until = 0;
        self.push_fx(Fx::Swing { who: i as u8, hit: best.is_some() });
        if let Some((j, _, pos)) = best {
            // Behind them: the blow comes from the direction they face away from.
            let mut h = pos - self.players[i].st.pos;
            h.y = 0.0;
            let behind = h.normalize_or_zero().dot(sim::view_dir(self.players[j].yaw, 0.0)) > 0.35;
            self.damage(j, Some(i), if behind { sim::MELEE_BACKSTAB } else { sim::MELEE_DAMAGE }, false, sim::MELEE_WEAPON);
        }
    }

    fn fire(&mut self, i: usize, cmd: &Cmd, heat: u8) {
        let t = self.tick;
        let (team, kit, origin, hspeed, airborne, human, crouch) = {
            let p = &self.players[i];
            let v = p.st.vel;
            (
                p.team,
                sim::wid(&p.st),
                p.st.pos + vec3(0.0, sim::eye_h(p.st.crouch), 0.0),
                (v.x * v.x + v.z * v.z).sqrt(),
                !p.st.on_ground,
                p.bot.is_none(),
                p.st.crouch,
            )
        };
        let wd = sim::weapon_def(kit);
        let ads = cmd.buttons & btn::ADS != 0;
        // Lag compensation: judge the shot against where targets were on
        // the shooter's screen, bounded to half a second of history.
        let rewind = if human { cmd.interp.clamp(t as f64 - MAX_REWIND, t as f64) } else { t as f64 };
        let targets: Vec<(usize, Vec3, bool)> = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.alive && p.team != team && p.protect_until <= t)
            .map(|(j, p)| (j, self.pos_at(j, rewind), p.st.crouch))
            .collect();
        self.players[i].firing_until = t + 90;
        self.players[i].protect_until = 0;
        self.stats.shots += 1;
        for d in sim::shot_dirs(kit, cmd.seq, cmd.yaw, cmd.pitch, ads, hspeed, airborne, crouch, heat) {
            let wh = self.world.raycast(origin, d, 300.0, self.dmask);
            let mut best_t = wh.map_or(300.0, |h| h.t);
            let mut hit_player = None;
            for &(j, pos, tc) in &targets {
                if let Some((tt, head)) = sim::ray_player(origin, d, pos, tc) {
                    if tt < best_t {
                        best_t = tt;
                        hit_player = Some((j, head));
                    }
                }
            }
            let end = origin + d * best_t;
            let mut kind = 0;
            if let Some((j, head)) = hit_player {
                kind = 2;
                let mut dmg = wd.damage * sim::falloff(kit, best_t);
                if head {
                    dmg *= wd.head_mult;
                }
                if wd.arch == 2 {
                    self.players[j].marked_until = t + 6 * TICK_HZ;
                }
                self.damage(j, Some(i), dmg, head, kit);
            } else if let Some(h) = wh {
                kind = 1;
                if let Some(bi) = h.block {
                    self.hit_block(bi, wd.damage);
                }
            }
            self.push_fx(Fx::Shot { who: i as u8, from: origin.to_array(), to: end.to_array(), hit: kind });
        }
    }

    fn hit_block(&mut self, bi: usize, dmg: f32) {
        if let Some(d) = self.world.blocks[bi].dest {
            // Structural groups share the persistent destruction mask with
            // crates, but only explosives can release their marked supports.
            if self.world.features.iter().any(|feature| feature.bit == d) {
                return;
            }
            let d = d as usize;
            if self.dmask & (1 << d) != 0 {
                return;
            }
            self.crate_hp[d] -= dmg;
            if self.crate_hp[d] <= 0.0 {
                self.dmask |= 1 << d;
                let b = &self.world.blocks[bi];
                let c = (b.min + b.max) * 0.5;
                self.push_fx(Fx::CrateBreak { pos: [c.x, b.max.y - 0.5, c.z] });
            }
        }
    }

    pub fn damage(&mut self, j: usize, attacker: Option<usize>, dmg: f32, head: bool, weapon: u8) {
        let t = self.tick;
        if !self.players[j].alive {
            return;
        }
        let mut dmg = dmg;
        if attacker.is_some_and(|a| a != j && t < self.players[a].power_until) {
            dmg *= 1.5;
        }
        let p = &mut self.players[j];
        if p.armor > 0.0 && weapon != sim::LAVA_WEAPON {
            let soak = (dmg * 0.6).min(p.armor);
            p.armor -= soak;
            dmg -= soak;
        }
        p.hp -= dmg;
        p.last_dmg_tick = t;
        if let Some(a) = attacker.filter(|&a| a != j) {
            match p.hits.iter_mut().find(|h| h.0 == a as u8) {
                Some(h) => {
                    h.1 = t;
                    h.2 += dmg;
                }
                None => p.hits.push((a as u8, t, dmg)),
            }
        }
        let kill = p.hp <= 0.0;
        if let (Some(b), Some(a)) = (p.bot.as_mut(), attacker) {
            b.alert(a, t);
        }
        // Squadmates close by notice where the fire is coming from.
        if let Some(a) = attacker {
            let (vp, vteam) = (self.players[j].st.pos, self.players[j].team);
            if self.players[a].team != vteam {
                for q in self.players.iter_mut().filter(|q| q.alive && q.team == vteam) {
                    if (q.st.pos - vp).length() < 18.0 {
                        if let Some(b) = q.bot.as_mut() {
                            b.alert_soft(a, t);
                        }
                    }
                }
            }
        }
        if let Some(a) = attacker {
            self.push_fx(Fx::Hit { attacker: a as u8, victim: j as u8, dmg: dmg.min(255.0) as u8, head, kill });
        }
        if kill {
            self.kill(j, attacker, weapon, head);
        }
    }

    fn kill(&mut self, j: usize, attacker: Option<usize>, weapon: u8, head: bool) {
        let t = self.tick;
        let team = self.players[j].team;
        // Rubber band: the more ground a team holds, the slower it reinforces.
        // Reinforcements arrive in waves so pushes land together.
        let period = ((5.0 + 1.5 * self.owned(team) as f32) * TICK_HZ as f32) as u32;
        let phase = if team == 1 { period / 2 } else { 0 };
        let earliest = t + 2 * TICK_HZ + period - 1;
        let wave = (earliest - phase.min(earliest)) / period * period + phase;
        let m = self.mode;
        let p = &mut self.players[j];
        p.alive = false;
        p.hp = 0.0;
        p.deaths += 1;
        // Outside conquest everyone is back after a fixed, short wait.
        p.respawn_at = match m {
            mode::TDM => t + 3 * TICK_HZ,
            mode::KOTH => t + 5 * TICK_HZ,
            // Flag runs need a gap to run through: each side comes back
            // together, every twelve seconds.
            mode::CTF => {
                let period = 12 * TICK_HZ;
                let phase = if team == 1 { period / 2 } else { 0 };
                let earliest = t + 3 * TICK_HZ + period - 1;
                (earliest - phase) / period * period + phase
            }
            _ => wave.max(t + 2 * TICK_HZ),
        };
        p.killer = attacker.map_or(SPECTATOR_ID, |a| a as u8);
        let hits = std::mem::take(&mut p.hits);
        let ended = std::mem::take(&mut p.streak);
        p.armor = 0.0;
        p.power_until = 0;
        let at = p.st.pos;
        let mut killer_id = j as u8;
        if let Some(a) = attacker {
            if a != j {
                self.players[a].kills += 1;
                self.players[a].heads += head as u16;
                self.players[a].score += if head { 125 } else { 100 };
                killer_id = a as u8;
                if m == mode::TDM && self.phase == 0 {
                    self.tickets[self.players[a].team as usize % 2] += 1;
                }
                if ended >= 5 {
                    self.players[a].score += 100;
                    self.feed.push(Feed::Shutdown { by: a as u8, victim: j as u8, n: ended.min(255) as u8 });
                }
                // A streak is announced at its milestones, and the one
                // who fell leaves its reward where they lay. Any other
                // kill may leave a health pack or ammunition.
                self.players[a].streak += 1;
                let n = self.players[a].streak;
                match streak_reward(n) {
                    Some(kind) => {
                        self.feed.push(Feed::Streak { who: a as u8, n: n.min(255) as u8 });
                        self.drop_pickup(kind, at, 40);
                        if n >= 10 {
                            self.drop_pickup(pickup::ARMOR, at + vec3(1.2, 0.0, 0.0), 40);
                        }
                    }
                    None => {
                        if self.cfg.drops && self.rng.chance(0.22) {
                            let kind = if self.rng.chance(0.6) { pickup::HEALTH } else { pickup::AMMO };
                            self.drop_pickup(kind, at, 30);
                        }
                    }
                }
            }
        }
        // Assist: whoever else did the most real damage in the last ten seconds.
        let mut assist = SPECTATOR_ID;
        let mut best = 30.0;
        for (a, when, dmg) in hits {
            let ai = a as usize;
            if ai != killer_id as usize
                && ai < self.players.len()
                && self.players[ai].team != team
                && t - when < 10 * TICK_HZ
                && dmg >= best
            {
                best = dmg;
                assist = a;
            }
        }
        if assist != SPECTATOR_ID {
            self.players[assist as usize].score += 50;
        }
        self.stats.kills += 1;
        if self.phase == 0 && m == mode::CONQUEST {
            self.tickets[team as usize] -= 1;
        }
        self.feed.push(Feed::Kill { killer: killer_id, victim: j as u8, weapon, head, assist });
    }

    fn spawn_player(&mut self, i: usize) {
        if self.players[i].benched {
            return;
        }
        let t = self.tick;
        let team = self.players[i].team;
        let mut anchor = self.world.bases[team as usize];
        let mut at_base = true;
        let order: Vec<usize> = if self.mode != mode::CONQUEST {
            Vec::new()
        } else if team == 0 {
            (0..NUM_POINTS).rev().collect()
        } else {
            (0..NUM_POINTS).collect()
        };
        if matches!(self.mode, mode::TDM | mode::KOTH) {
            // Forward spawns keep the fight close: B for EMBER, D for
            // COBALT, unless the enemy is standing on it.
            let c = self.world.points[if team == 0 { 1 } else { NUM_POINTS - 2 }];
            let overrun = self.players.iter().filter(|p| p.alive && p.team != team && (p.st.pos - c).length() < 18.0).count() >= 2;
            if !overrun {
                anchor = c;
                at_base = false;
            }
        }
        for k in order {
            if self.points[k].owner != team {
                continue;
            }
            let c = self.world.points[k];
            let threatened = self.players.iter().any(|p| p.alive && p.team != team && (p.st.pos - c).length() < 16.0);
            if !threatened && self.points[k].progress.abs() > 0.6 {
                anchor = c;
                at_base = false;
                break;
            }
        }
        // Spread arrivals out: try a handful of open spots across the spawn
        // ground and take the one farthest from anyone already standing
        // there, weighing nearby enemies more heavily than friends.
        let mut pos = anchor;
        let mut best = f32::MIN;
        let (r0, r1) = if at_base { (4.0, map::BASE_SPAWN_RADIUS) } else { (6.0, 16.0) };
        let mut found = 0;
        for _ in 0..40 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            // Uniform over the ring's area, not bunched at its middle.
            let r = (r0 * r0 + self.rng.f32() * (r1 * r1 - r0 * r0)).sqrt();
            let (x, z) = (anchor.x + a.cos() * r, anchor.z + a.sin() * r);
            if self.nav.is_blocked(x, z) || self.world.lava(x, z, 1.0) {
                continue;
            }
            let p = vec3(x, self.world.height(x, z) + 0.05, z);
            let mut score = 0.0f32;
            let (mut friend, mut foe) = (14.0f32, 40.0f32);
            for (j, q) in self.players.iter().enumerate() {
                if j == i || !q.alive {
                    continue;
                }
                let d = (q.st.pos - p).length();
                if q.team == team {
                    friend = friend.min(d)
                } else {
                    foe = foe.min(d)
                }
            }
            score += friend + foe * 0.6;
            if score > best {
                best = score;
                pos = p;
            }
            found += 1;
            if found >= 8 {
                break;
            }
        }
        let front = match self.mode {
            mode::CONQUEST => self.world.points[self.orders[team as usize].min(NUM_POINTS - 1)],
            mode::CTF => self.world.points[FLAG_POINT[1 - team as usize % 2]],
            _ => self.world.points[NUM_POINTS / 2],
        };
        let p = &mut self.players[i];
        p.st = PState::spawn_with(pos, p.next_kit, p.next_prim, p.next_side);
        p.wskin = [sim::fix_weapon_skin(p.st.prim, p.next_wskin[0]), sim::fix_weapon_skin(p.st.side, p.next_wskin[1])];
        p.hp = sim::kit(p.next_kit).hp;
        p.alive = true;
        p.protect_until = t + 2 * TICK_HZ;
        p.marked_until = 0;
        p.armor = 0.0;
        p.power_until = 0;
        p.spawn_count = p.spawn_count.wrapping_add(1);
        p.hits.clear();
        // Face the fight: toward the point the team is pushing.
        let to = front - pos;
        p.spawn_yaw = if to.x * to.x + to.z * to.z > 4.0 {
            to.x.atan2(to.z)
        } else if team == 0 {
            std::f32::consts::FRAC_PI_2
        } else {
            -std::f32::consts::FRAC_PI_2
        };
        p.yaw = p.spawn_yaw;
        p.pitch = 0.0;
        p.hist = [pos; HIST];
        if let Some(b) = p.bot.as_mut() {
            b.on_spawn(p.spawn_yaw);
        }
    }

    fn explode(&mut self, pos: Vec3, owner: usize, team: u8) {
        self.push_fx(Fx::Boom { pos: pos.to_array() });
        let before = self.dmask;
        // Decide exposure against the intact field. A released panel cannot
        // let this same blast reach another structure through its new hole.
        let broken: Vec<_> = self
            .world
            .features
            .iter()
            .filter(|feature| {
                let bit = 1u64 << feature.bit;
                if before & bit != 0 {
                    return false;
                }
                let exposed = |target: Vec3| pos.distance_squared(target) <= 4.5 * 4.5 && self.world.los(pos, target, before | bit);
                // Ignore only this group's own collision when testing exposure.
                // Nearby permanent walls and other intact groups still shield it.
                exposed(feature.pos)
                    || self.world.blocks.iter().any(|block| block.dest == Some(feature.bit) && exposed(pos.clamp(block.min, block.max)))
            })
            .map(|feature| (feature.bit, feature.kind as u8, feature.pos))
            .collect();
        for bi in 0..self.world.blocks.len() {
            let b = &self.world.blocks[bi];
            if b.dest.is_some() && ((b.min + b.max) * 0.5 - pos).length() < 5.5 {
                self.hit_block(bi, 1000.0);
            }
        }
        let structural_change = !broken.is_empty();
        for (bit, kind, at) in broken {
            self.dmask |= 1u64 << bit;
            self.push_fx(Fx::MapBreak { kind, pos: at.to_array() });
        }
        if structural_change {
            self.nav = Nav::build_mask(&self.world, self.dmask);
        }
        let src = pos + vec3(0.0, 0.3, 0.0);
        for j in 0..self.players.len() {
            let p = &self.players[j];
            if !p.alive || (p.team == team && j != owner) || p.protect_until > self.tick {
                continue;
            }
            let chest = p.st.pos + vec3(0.0, if p.st.crouch { 0.55 } else { 0.9 }, 0.0);
            let d = (chest - src).length();
            if d > 8.0 || !self.world.los(src, chest, self.dmask) {
                continue;
            }
            let mut dmg = 140.0 * (1.0 - d / 8.0);
            if j == owner {
                dmg *= 0.5;
            }
            self.damage(j, Some(owner), dmg, false, GRENADE_WEAPON);
        }
    }

    fn update_grenades(&mut self) {
        let mut boom = Vec::new();
        for g in self.grenades.iter_mut() {
            g.vel.y -= 20.0 * DT;
            let step = g.vel * DT;
            let len = step.length();
            if len > 1e-5 {
                let d = step / len;
                match self.world.raycast(g.pos, d, len + 0.06, self.dmask) {
                    Some(h) => {
                        g.pos += d * (h.t - 0.06).max(0.0);
                        g.vel = (g.vel - 2.0 * g.vel.dot(h.normal) * h.normal) * 0.42;
                        if g.vel.length() < 1.2 {
                            g.vel = Vec3::ZERO;
                        }
                    }
                    None => g.pos += step,
                }
            }
            g.fuse -= 1;
            if g.fuse <= 0 {
                boom.push((g.pos, g.owner, g.team));
            }
        }
        self.grenades.retain(|g| g.fuse > 0);
        for (pos, owner, team) in boom {
            self.explode(pos, owner, team);
        }
    }

    /// Leave something on the field for `secs` seconds. Nothing is left
    /// in lava, and the oldest goes when the field is littered.
    fn drop_pickup(&mut self, kind: u8, pos: Vec3, secs: u32) {
        if !self.cfg.drops || self.world.lava(pos.x, pos.z, 0.5) {
            return;
        }
        self.pickups.push(Pickup { id: self.next_pickup, kind, pos, until: self.tick + secs * TICK_HZ });
        self.next_pickup = self.next_pickup.wrapping_add(1);
        if self.pickups.len() > 14 {
            self.pickups.remove(0);
        }
    }

    /// Whoever walks over a pickup they have a use for takes it.
    fn update_pickups(&mut self) {
        let t = self.tick;
        self.pickups.retain(|p| p.until > t);
        let mut k = 0;
        while k < self.pickups.len() {
            let Pickup { kind, pos, .. } = self.pickups[k];
            let taker = self.players.iter().position(|p| {
                let d = p.st.pos - pos;
                p.alive && d.x * d.x + d.z * d.z < 1.5 * 1.5 && d.y.abs() < 2.2 && wants(p, kind)
            });
            let Some(i) = taker else {
                k += 1;
                continue;
            };
            let p = &mut self.players[i];
            match kind {
                pickup::HEALTH => p.hp = (p.hp + 60.0).min(sim::kit(p.st.kit).hp),
                pickup::ARMOR => p.armor = (p.armor + 50.0).min(100.0),
                pickup::AMMO => {
                    let (hand, other) = if p.st.slot == 1 { (p.st.side, p.st.prim) } else { (p.st.prim, p.st.side) };
                    p.st.ammo = sim::weapon_def(hand).mag;
                    p.st.ammo_alt = sim::weapon_def(other).mag;
                    p.st.reload = 0;
                    p.st.grenades = sim::kit(p.st.kit).grenades;
                }
                pickup::OVERDRIVE => p.surge_until = t + 12 * TICK_HZ,
                _ => p.power_until = t + 10 * TICK_HZ,
            }
            p.score += 10;
            self.pickups.remove(k);
            self.push_fx(Fx::Pickup { who: i as u8, kind, pos: pos.to_array() });
        }
    }

    /// Capture the flag: carry theirs to where yours stands.
    fn update_flags(&mut self) {
        let t = self.tick;
        for f in 0..2usize {
            let enemy = 1 - f as u8;
            let home = self.world.points[FLAG_POINT[f]];
            let near = |g: &Game, team: u8, at: Vec3| {
                g.players.iter().position(|p| {
                    let d = p.st.pos - at;
                    p.alive && p.team == team && d.x * d.x + d.z * d.z < 1.8 * 1.8 && d.y.abs() < 2.5
                })
            };
            if self.flags[f].state == 1 {
                let c = self.flags[f].carrier;
                let held = c < self.players.len() && self.players[c].alive && self.players[c].team == enemy;
                if !held {
                    // The carrier fell (or left): the flag lies where they were.
                    let pos = self.flags[f].pos;
                    if self.world.lava(pos.x, pos.z, 0.5) {
                        self.flags[f] = Flag { state: 0, pos: home, carrier: 0, since: t };
                        self.feed.push(Feed::Flag { flag: f as u8, by: SPECTATOR_ID, what: flag_ev::RETURNED });
                    } else {
                        self.flags[f].state = 2;
                        self.flags[f].since = t;
                        self.feed.push(Feed::Flag { flag: f as u8, by: c.min(254) as u8, what: flag_ev::DROPPED });
                    }
                    continue;
                }
                let pos = self.players[c].st.pos;
                self.flags[f].pos = pos;
                let d = pos - self.world.points[FLAG_POINT[1 - f]];
                if d.x * d.x + d.z * d.z < 2.6 * 2.6 && d.y.abs() < 4.0 {
                    self.flags[f] = Flag { state: 0, pos: home, carrier: 0, since: t };
                    self.players[c].score += 400;
                    self.stats.captures += 1;
                    if self.phase == 0 {
                        self.tickets[enemy as usize] += 1;
                    }
                    self.feed.push(Feed::Flag { flag: f as u8, by: c as u8, what: flag_ev::CAPTURED });
                    for p in self.players.iter_mut().filter(|p| p.team == enemy && p.alive) {
                        p.surge_until = t + 8 * TICK_HZ;
                    }
                }
                continue;
            }
            if self.flags[f].state == 2 {
                // Its own side touches it and it is home; left alone it
                // goes back by itself.
                let by = near(self, f as u8, self.flags[f].pos);
                if by.is_some() || t - self.flags[f].since > 25 * TICK_HZ {
                    self.flags[f] = Flag { state: 0, pos: home, carrier: 0, since: t };
                    if let Some(i) = by {
                        self.players[i].score += 100;
                    }
                    self.feed.push(Feed::Flag { flag: f as u8, by: by.map_or(SPECTATOR_ID, |i| i as u8), what: flag_ev::RETURNED });
                    continue;
                }
            }
            if let Some(i) = near(self, enemy, self.flags[f].pos) {
                self.flags[f].state = 1;
                self.flags[f].carrier = i;
                self.players[i].protect_until = 0;
                self.feed.push(Feed::Flag { flag: f as u8, by: i as u8, what: flag_ev::TAKEN });
            }
        }
    }

    fn update_points(&mut self) {
        let t = self.tick;
        // Stronghold has the one point, open to both sides at all times.
        let free = self.mode == mode::KOTH;
        let live = match self.mode {
            mode::CONQUEST => 0..NUM_POINTS,
            mode::KOTH => NUM_POINTS / 2..NUM_POINTS / 2 + 1,
            _ => 0..0,
        };
        for k in live {
            let c = self.world.points[k];
            let mut n = [0u8; 2];
            for p in self.players.iter().filter(|p| p.alive) {
                let d = p.st.pos - c;
                if d.x * d.x + d.z * d.z < POINT_RADIUS * POINT_RADIUS && d.y.abs() < 7.0 {
                    n[p.team as usize] += 1;
                }
            }
            let owners = self.owners();
            let pt = &mut self.points[k];
            pt.n = n;
            let diff = n[1] as i32 - n[0] as i32;
            if diff != 0 {
                let team = if diff > 0 { 1u8 } else { 0 };
                let dir = if team == 1 { 1.0 } else { -1.0 };
                if free || pt.owner == team || eligible(&owners, team, k) {
                    let rate = CAP_RATE * (diff.abs().min(3) as f32);
                    pt.progress = (pt.progress + dir * rate * DT).clamp(-1.0, 1.0);
                }
            } else if n[0] == 0 {
                let target = match pt.owner {
                    0 => -1.0,
                    1 => 1.0,
                    _ => 0.0,
                };
                let step = 0.04 * DT;
                pt.progress += (target - pt.progress).clamp(-step, step);
            }
            let sign = match pt.owner {
                0 => -1.0,
                1 => 1.0,
                _ => 0.0,
            };
            if pt.owner != NEUTRAL && pt.progress * sign <= 0.0 {
                let by = 1 - pt.owner;
                pt.owner = NEUTRAL;
                self.feed.push(Feed::Neutralized { point: k as u8, by });
                for p in self.players.iter_mut().filter(|p| p.team == by && p.alive) {
                    let d = p.st.pos - c;
                    if d.x * d.x + d.z * d.z < POINT_RADIUS * POINT_RADIUS {
                        p.score += 75;
                    }
                }
            }
            let pt = &mut self.points[k];
            if pt.owner == NEUTRAL && pt.progress.abs() >= 1.0 {
                let team = if pt.progress > 0.0 { 1u8 } else { 0 };
                pt.owner = team;
                self.stats.captures += 1;
                self.feed.push(Feed::Capture { point: k as u8, team });
                // Surge: the capturing team gets a short burst of momentum.
                for p in self.players.iter_mut().filter(|p| p.team == team && p.alive) {
                    p.surge_until = t + 8 * TICK_HZ;
                    let d = p.st.pos - c;
                    if d.x * d.x + d.z * d.z < POINT_RADIUS * POINT_RADIUS {
                        p.score += 200;
                    }
                }
            }
        }
    }

    /// Pick one point per team to mass on: a capturable point, preferring
    /// the one the enemy is defending most thinly.
    fn update_orders(&mut self) {
        if self.mode != mode::CONQUEST {
            self.orders = [NUM_POINTS / 2; 2];
            return;
        }
        let owners = self.owners();
        for team in 0..2u8 {
            let mut best: Option<(usize, f32)> = None;
            for k in 0..NUM_POINTS {
                if self.points[k].owner == team || !eligible(&owners, team, k) {
                    continue;
                }
                let c = self.world.points[k];
                let defenders = self.players.iter().filter(|p| p.alive && p.team != team && (p.st.pos - c).length() < 22.0).count();
                let score = defenders as f32 + self.rng.range(0.0, 2.5);
                if best.is_none_or(|(_, s)| score < s) {
                    best = Some((k, score));
                }
            }
            if let Some((k, _)) = best {
                self.orders[team as usize] = k;
            }
        }
    }

    /// The map the next round will be played on.
    pub fn next_map(&self) -> u8 {
        let r = &self.cfg.rotation;
        if r.len() < 2 {
            return self.world.map;
        }
        match r.iter().position(|&m| m == self.world.map) {
            Some(k) => r[(k + 1) % r.len()],
            None => r[0],
        }
    }

    pub fn humans(&self) -> usize {
        self.players.iter().filter(|p| p.bot.is_none()).count()
    }

    /// Votes per map from the players in the lobby.
    pub fn votes(&self) -> [u8; 8] {
        let mut v = [0u8; 8];
        for p in self.players.iter().filter(|p| p.bot.is_none()) {
            if (p.vote as usize) < ferrite_shared::map::MAPS.len() {
                v[p.vote as usize] += 1;
            }
        }
        v
    }

    /// The map the lobby will start: most votes, the rotation's choice
    /// when nobody voted or the vote is tied with it.
    pub fn lobby_choice(&self) -> u8 {
        let v = self.votes();
        let best = *v.iter().max().unwrap_or(&0);
        if best == 0 || v[self.lobby_map as usize % 8] == best {
            return self.lobby_map;
        }
        v.iter().position(|&n| n == best).unwrap_or(0) as u8
    }

    /// The mode the next round will be played in.
    pub fn next_mode(&self) -> u8 {
        let r = &self.cfg.mode_rotation;
        if r.len() < 2 {
            return self.mode;
        }
        match r.iter().position(|&m| m == self.mode) {
            Some(k) => r[(k + 1) % r.len()],
            None => r[0],
        }
    }

    /// Votes per game mode from the players in the lobby.
    pub fn mode_votes(&self) -> [u8; 4] {
        let mut v = [0u8; 4];
        for p in self.players.iter().filter(|p| p.bot.is_none()) {
            if (p.mode_vote as usize) < v.len() {
                v[p.mode_vote as usize] += 1;
            }
        }
        v
    }

    /// The mode the lobby will start: most votes, the rotation's choice
    /// when nobody voted or the vote is tied with it.
    pub fn lobby_mode_choice(&self) -> u8 {
        let v = self.mode_votes();
        let best = *v.iter().max().unwrap_or(&0);
        if best == 0 || v[self.lobby_mode as usize % 4] == best {
            return self.lobby_mode;
        }
        v.iter().position(|&n| n == best).unwrap_or(0) as u8
    }

    fn enter_lobby(&mut self) {
        self.lobby_map = if self.stats.rounds == 0 { self.world.map } else { self.next_map() };
        self.lobby_mode = if self.stats.rounds == 0 { self.mode } else { self.next_mode() };
        self.phase = 2;
        self.phase_until = 0;
        self.winner = NEUTRAL;
        self.grenades.clear();
        self.reset_field();
        self.round_end = 0;
        for p in self.players.iter_mut() {
            p.alive = false;
            p.vote = 255;
            p.mode_vote = 255;
            p.ready = false;
        }
    }

    /// Lobby: wait for a player, count down (straight to three seconds
    /// once every player is ready), then start on the chosen map.
    fn tick_lobby(&mut self) {
        let t = self.tick;
        if self.humans() == 0 {
            self.phase_until = 0;
            return;
        }
        if self.phase_until == 0 {
            self.phase_until = t + self.cfg.lobby_secs * TICK_HZ;
        }
        let all_ready = self.players.iter().filter(|p| p.bot.is_none()).all(|p| p.ready);
        if all_ready && self.phase_until > t + 3 * TICK_HZ {
            self.phase_until = t + 3 * TICK_HZ;
        }
        if t >= self.phase_until {
            let (map, mode) = (self.lobby_choice(), self.lobby_mode_choice());
            self.start_round(map, mode);
        }
    }

    fn reset_round(&mut self) {
        let (next, mode) = (self.next_map(), self.next_mode());
        self.start_round(next, mode);
    }

    fn start_round(&mut self, next: u8, mode: u8) {
        self.mode = mode % ferrite_shared::MODES.len() as u8;
        self.round_no += 1;
        if next != self.world.map {
            self.world = World::with_map(self.cfg.seed, next);
            self.fx.clear();
            for p in self.players.iter_mut() {
                p.hist = [Vec3::ZERO; HIST];
                p.marked_until = 0;
            }
        }
        self.reset_field();
        self.dmask = 0;
        // The same map can repeat: restore routes through intact structures,
        // as well as their colliders, before choosing the next spawns.
        self.nav = Nav::build(&self.world);
        self.crate_hp = vec![110.0; self.world.n_crates];
        self.grenades.clear();
        self.phase = 0;
        self.winner = NEUTRAL;
        self.round_start = self.tick;
        for i in 0..self.players.len() {
            self.bank(i);
            let p = &mut self.players[i];
            p.surge_until = 0;
            p.streak = 0;
            p.played = 0;
            p.award = None;
        }
        for i in 0..self.players.len() {
            self.spawn_player(i);
        }
        self.feed.push(Feed::MatchStart);
    }

    /// Advance the world by one 60 Hz tick.
    pub fn tick(&mut self) {
        self.tick += 1;
        let t = self.tick;
        for p in self.players.iter_mut() {
            p.credits = (p.credits + 1.0).min(12.0);
            p.st.surge = t < p.surge_until;
        }
        if self.phase == 2 {
            self.tick_lobby();
            return;
        }
        for i in 0..self.players.len() {
            if let Some(mut bot) = self.players[i].bot.take() {
                let cmd = bots::think(self, i, &mut bot);
                self.players[i].bot = Some(bot);
                self.apply_cmd(i, cmd);
            } else if self.players[i].alive && t.saturating_sub(self.players[i].last_cmd_tick) > 12 {
                // Silent client: keep physics honest (gravity, no input).
                let p = &mut self.players[i];
                let c = Cmd { yaw: p.yaw, pitch: p.pitch, ..Default::default() };
                sim::step(&mut p.st, &c, &self.world, self.dmask);
            }
        }
        for i in 0..self.players.len() {
            let p = &mut self.players[i];
            p.hist[t as usize % HIST] = p.st.pos;
            if p.alive {
                let max = sim::kit(p.st.kit).hp;
                if p.hp < max && t - p.last_dmg_tick > 5 * TICK_HZ {
                    p.hp = (p.hp + 18.0 * DT).min(max);
                }
            } else if t >= p.respawn_at {
                self.spawn_player(i);
            }
        }
        // Lava burns whoever stands in it, friend or foe, shielded or not.
        for i in 0..self.players.len() {
            if self.players[i].alive && self.world.burning(self.players[i].st.pos) {
                let dps = ferrite_shared::map::slag_cathedral::LAVA_DPS;
                self.damage(i, None, dps * DT, false, sim::LAVA_WEAPON);
            }
        }
        self.update_grenades();
        self.update_pickups();
        self.update_points();
        if self.mode == mode::CTF {
            self.update_flags();
        }
        if t % (12 * TICK_HZ) == 1 {
            self.update_orders();
        }
        if self.phase == 0 {
            for p in self.players.iter_mut().filter(|p| p.bot.is_none()) {
                p.played += 1;
            }
        }
        if self.phase == 0 && self.mode != mode::CONQUEST {
            // Stronghold: a point a second to whoever holds the centre.
            let mid = self.points[NUM_POINTS / 2].owner;
            if self.mode == mode::KOTH && mid < 2 && t.is_multiple_of(TICK_HZ) {
                self.tickets[mid as usize] += 1;
            }
            let reached = self.tickets[0] >= self.goal || self.tickets[1] >= self.goal;
            if reached || (self.round_end > 0 && t >= self.round_end) {
                // Level on the clock: the side that scored more points
                // between them takes it.
                let total = |g: &Game, team: u8| g.players.iter().filter(|p| p.team == team).map(|p| p.score).sum::<u32>();
                self.winner = if self.tickets[0] != self.tickets[1] {
                    (self.tickets[1] > self.tickets[0]) as u8
                } else {
                    (total(self, 1) > total(self, 0)) as u8
                };
                self.phase = 1;
                self.phase_until = t + 10 * TICK_HZ;
                self.close_round();
                self.feed.push(Feed::MatchEnd { winner: self.winner });
            }
        } else if self.phase == 0 {
            if t.is_multiple_of(4 * TICK_HZ) {
                let diff = self.owned(0) - self.owned(1);
                let mult = if diff.abs() == NUM_POINTS as i32 { 3 } else { 1 };
                if diff > 0 {
                    self.tickets[1] -= diff * mult;
                } else {
                    self.tickets[0] += diff * mult;
                }
            }
            if self.tickets[0] <= 0 || self.tickets[1] <= 0 {
                self.winner = if self.tickets[0] > self.tickets[1] {
                    0
                } else if self.tickets[1] > self.tickets[0] {
                    1
                } else if self.owned(0) >= self.owned(1) {
                    0
                } else {
                    1
                };
                self.tickets[0] = self.tickets[0].max(0);
                self.tickets[1] = self.tickets[1].max(0);
                self.phase = 1;
                self.phase_until = t + 10 * TICK_HZ;
                self.close_round();
                self.feed.push(Feed::MatchEnd { winner: self.winner });
            }
        } else if t >= self.phase_until {
            if self.cfg.lobby_secs > 0 && self.humans() > 0 {
                self.enter_lobby();
            } else {
                self.reset_round();
            }
        }
        if self.fx.len() > 600 {
            let cut = self.fx.len() - 400;
            self.fx.drain(..cut);
        }
    }

    pub fn snapshot(&self, me: Option<usize>, ack_feed: u32, fx_after: u32) -> Snapshot {
        let t = self.tick;
        let players = self
            .players
            .iter()
            .filter(|p| p.alive)
            .map(|p| {
                let mut flags = 0;
                if t < p.firing_until {
                    flags |= flag::FIRING;
                }
                if p.st.surge {
                    flags |= flag::SURGE;
                }
                if t < p.marked_until {
                    flags |= flag::MARKED;
                }
                if p.ads {
                    flags |= flag::ADS;
                }
                if p.st.on_ground {
                    flags |= flag::GROUND;
                }
                if t < p.protect_until {
                    flags |= flag::SHIELD;
                }
                if p.st.crouch {
                    flags |= flag::CROUCH;
                }
                if p.st.slot == 1 {
                    flags |= flag::SIDEARM;
                }
                let mut flags2 = 0;
                if p.st.reload > 0 {
                    flags2 |= flag2::RELOAD;
                }
                if p.st.slide > 0 {
                    flags2 |= flag2::SLIDE;
                }
                if p.st.swap > 0 {
                    flags2 |= flag2::DRAW;
                }
                if p.st.melee > 0 {
                    flags2 |= flag2::MELEE;
                }
                if self.flags.iter().any(|f| f.state == 1 && f.carrier == p.id as usize) {
                    flags2 |= flag2::CARRIER;
                }
                if p.armor > 0.0 {
                    flags2 |= flag2::ARMOR;
                }
                if t < p.power_until {
                    flags2 |= flag2::POWER;
                }
                NetPlayer {
                    id: p.id,
                    pos: p.st.pos.to_array(),
                    yaw: NetPlayer::enc_yaw(p.yaw),
                    pitch: NetPlayer::enc_pitch(p.pitch),
                    flags,
                    flags2,
                    hp: p.hp.clamp(0.0, 255.0).ceil() as u8,
                }
            })
            .collect();
        let mut points = [PointNet::default(); 5];
        for (i, p) in self.points.iter().enumerate() {
            points[i] = PointNet { owner: p.owner, progress: (p.progress * 100.0) as i8, n: p.n };
        }
        // Hit confirmations are private to the two players involved, and
        // they go in first: in a big firefight the tracers alone would fill
        // the packet and the hit markers would be the ones left out.
        let m = me.map_or(SPECTATOR_ID, |m| m as u8);
        let mut fx: Vec<Fx> = Vec::new();
        for pass in 0..2 {
            for (ft, f) in &self.fx {
                if *ft <= fx_after {
                    continue;
                }
                let mine = matches!(f, Fx::Hit { attacker, victim, .. } if *attacker == m || *victim == m);
                let other_hit = matches!(f, Fx::Hit { .. }) && !mine;
                if other_hit || mine != (pass == 0) {
                    continue;
                }
                if fx.len() < 56 {
                    fx.push(*f);
                }
            }
        }
        let feed = self.feed.since(ack_feed as usize).take(10).map(|(i, f)| (i as u32 + 1, *f)).collect();
        let (ack_seq, me_state) = match me {
            Some(i) => {
                let p = &self.players[i];
                (
                    p.last_seq,
                    Some(SelfState {
                        st: p.st,
                        hp: p.hp.clamp(0.0, 255.0).ceil() as u8,
                        alive: p.alive,
                        respawn_in: p.respawn_at.saturating_sub(t) as f32 / TICK_HZ as f32,
                        spawn_count: p.spawn_count,
                        spawn_yaw: p.spawn_yaw,
                        killer: p.killer,
                        armor: p.armor.clamp(0.0, 100.0).ceil() as u8,
                        streak: p.streak.min(255) as u8,
                        power: (p.power_until.saturating_sub(t) as f32 / TICK_HZ as f32).ceil().min(255.0) as u8,
                    }),
                )
            }
            None => (0, None),
        };
        Snapshot {
            tick: t,
            ack_seq,
            me: me_state,
            players,
            points,
            tickets: [self.tickets[0].clamp(0, 65535) as u16, self.tickets[1].clamp(0, 65535) as u16],
            dmask: self.dmask,
            grenades: self.grenades.iter().map(|g| NetGrenade { id: g.id, pos: g.pos.to_array(), team: g.team }).collect(),
            fx,
            feed,
            phase: self.phase,
            winner: self.winner,
            phase_secs: self.phase_until.saturating_sub(t) as f32 / TICK_HZ as f32,
            map: self.world.map,
            next_map: if self.phase == 2 { self.lobby_choice() } else { self.next_map() },
            votes: self.votes(),
            mode: self.mode,
            next_mode: if self.phase == 2 { self.lobby_mode_choice() } else { self.next_mode() },
            mode_votes: self.mode_votes(),
            goal: self.goal.clamp(0, 65535) as u16,
            secs_left: if self.phase == 0 && self.round_end > 0 {
                (self.round_end.saturating_sub(t) / TICK_HZ).min(65535) as u16
            } else {
                0
            },
            flags: [0, 1].map(|f| {
                let fl = &self.flags[f];
                FlagNet { state: fl.state, carrier: fl.carrier.min(255) as u8, pos: fl.pos.to_array() }
            }),
            pickups: self.pickups.iter().map(|p| NetPickup { id: p.id, kind: p.kind, pos: NetPickup::enc(p.pos.to_array()) }).collect(),
        }
    }

    pub fn roster(&self) -> Vec<RosterEntry> {
        self.players
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.benched)
            .map(|(i, p)| RosterEntry {
                id: p.id,
                name: p.name.clone(),
                team: p.team,
                kit: if p.alive { p.st.kit } else { p.next_kit },
                prim: p.st.prim,
                side: p.st.side,
                kills: p.kills,
                deaths: p.deaths,
                score: p.score,
                bot: p.bot.is_some(),
                ready: p.ready,
                heads: p.heads,
                total: self.totals(i),
                skin: p.skin,
                wskin: p.wskin,
            })
            .collect()
    }
}

/// Read a stats file written by `Game::save_records`: a callsign, six
/// figures, and the player's id. A line without an id (from before ids)
/// is kept under its name until that player comes back. Anything that
/// does not parse is skipped.
fn load_records(path: Option<&std::path::Path>) -> std::collections::HashMap<Who, Record> {
    let mut m = std::collections::HashMap::new();
    let Some(text) = path.and_then(|p| std::fs::read_to_string(p).ok()) else { return m };
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 7 || f[0].len() > 14 {
            continue;
        }
        let n: Vec<u32> = f[1..7].iter().filter_map(|x| x.parse().ok()).collect();
        if n.len() != 6 {
            continue;
        }
        let key = match f.get(7).and_then(|s| uuid_parse(s)) {
            Some(id) => Who::Id(id),
            None => Who::Name(f[0].to_string()),
        };
        let totals = Totals { kills: n[0], deaths: n[1], score: n[2], heads: n[3], rounds: n[4], wins: n[5] };
        m.insert(key, Record { name: f[0].to_string(), totals });
    }
    m
}

fn dummy() -> Player {
    Player {
        id: 0,
        name: String::new(),
        team: 0,
        next_kit: 0,
        next_prim: 0,
        next_side: sim::SIDEARM_ID,
        vote: 255,
        ready: false,
        mode_vote: 255,
        armor: 0.0,
        streak: 0,
        power_until: 0,
        played: 0,
        award: None,
        st: PState::spawn(Vec3::ZERO, 0),
        yaw: 0.0,
        pitch: 0.0,
        hp: 0.0,
        alive: false,
        respawn_at: 0,
        last_dmg_tick: 0,
        last_seq: 0,
        last_cmd_tick: 0,
        credits: 0.0,
        ads: false,
        kills: 0,
        deaths: 0,
        score: 0,
        heads: 0,
        bot: None,
        benched: false,
        skin: 0,
        uuid: [0; 16],
        icon: Vec::new(),
        icon_hash: 0,
        next_wskin: [0; 2],
        wskin: [0; 2],
        marked_until: 0,
        surge_until: 0,
        firing_until: 0,
        protect_until: 0,
        spawn_count: 0,
        spawn_yaw: 0.0,
        killer: SPECTATOR_ID,
        hits: Vec::new(),
        hist: [Vec3::ZERO; HIST],
    }
}

#[cfg(test)]
mod map_feature_tests {
    use super::*;

    #[test]
    fn firearms_never_release_structural_groups_but_still_break_crates() {
        let mut g = Game::new(Config { per_team: 1, max_bots: 0, log: false, discover: false, ..Default::default() });
        let structural: Vec<_> = g
            .world
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| block.dest.is_some_and(|bit| g.world.features.iter().any(|feature| feature.bit == bit)))
            .map(|(i, _)| i)
            .collect();
        assert!(!structural.is_empty());
        for bi in structural {
            g.hit_block(bi, 10000.0);
        }
        assert_eq!(g.dmask, 0, "even overwhelming bullet damage must not release explosive charges");
        assert!(g.fx.is_empty());
        let crate_block = g
            .world
            .blocks
            .iter()
            .enumerate()
            .find(|(_, block)| block.dest.is_some_and(|bit| !g.world.features.iter().any(|feature| feature.bit == bit)))
            .map(|(i, _)| i)
            .expect("the classic crates still exist");
        let bit = g.world.blocks[crate_block].dest.unwrap();
        g.hit_block(crate_block, 110.0);
        assert_ne!(g.dmask & (1u64 << bit), 0);
        assert!(g.fx.iter().any(|(_, fx)| matches!(fx, Fx::CrateBreak { .. })));
    }
}
