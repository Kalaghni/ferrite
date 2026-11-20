//! Where someone deploys.
//!
//! A team never has a single spawn spot. It has a set of zones along its own
//! stretch of the front line - its base, the points it holds, the ground
//! between them - and every arrival is scored across all of them: out of
//! sight of the enemy, away from where teammates just appeared, not on top of
//! anyone, with a little chance thrown in. Holding more ground gives a team
//! more places to come from, and the other side has no single doorway to
//! watch.

use super::{map, Game};
use ferrite_shared::sim::EYE;
use ferrite_shared::{mode, vec3, Vec3, NUM_POINTS};

/// A patch of ground a team may deploy on: a ring round `c`, and how much
/// the team would rather come in here than elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub c: Vec3,
    pub r0: f32,
    pub r1: f32,
    pub w: f32,
}

/// How far an enemy's eye is checked for a view of a spawn spot.
const SIGHT: f32 = 85.0;
/// Valid spots scored for one arrival.
const CANDIDATES: usize = 12;

fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

impl Game {
    /// The spawn zones `team` may use right now, front first.
    pub fn spawn_zones(&self, team: u8) -> Vec<Zone> {
        let base = self.world.bases[team as usize];
        let pts = self.world.points;
        // The team's own line: base, then points from its end inward.
        let line: Vec<usize> = if team == 0 { (0..NUM_POINTS).collect() } else { (0..NUM_POINTS).rev().collect() };
        let enemies_near =
            |c: Vec3, r: f32| self.players.iter().filter(|p| p.alive && p.team != team && (p.st.pos - c).length() < r).count();
        let point = |c: Vec3, w: f32| Zone { c, r0: 5.0, r1: 20.0, w };
        let between = |c: Vec3, w: f32| Zone { c, r0: 0.0, r1: 14.0, w };
        let home = Zone { c: base, r0: 4.0, r1: map::BASE_SPAWN_RADIUS, w: 0.25 };
        let mut zones = Vec::new();
        match self.mode {
            mode::CONQUEST => {
                // Every point held firmly and not under attack, and the
                // ground between it and the last such one behind it,
                // weighted to the front.
                let owned: Vec<usize> = line.iter().copied().take_while(|&k| self.points[k].owner == team).collect();
                let mut prev = Some(base);
                for (n, &k) in owned.iter().enumerate() {
                    let w = [1.0, 0.55, 0.35][(owned.len() - 1 - n).min(2)];
                    if self.points[k].progress.abs() <= 0.6 || enemies_near(pts[k], 16.0) > 0 {
                        prev = None;
                        continue;
                    }
                    zones.push(point(pts[k], w));
                    if let Some(q) = prev.map(|q| lerp(q, pts[k], 0.5)).filter(|&q| enemies_near(q, 16.0) == 0) {
                        zones.push(between(q, w * 0.7));
                    }
                    prev = Some(pts[k]);
                }
                zones.push(Zone { w: if zones.is_empty() { 1.0 } else { home.w }, ..home });
            }
            mode::TDM | mode::KOTH => {
                // Forward spawns keep the fight close: the team's forward
                // point, the ground either side of it along the line, and
                // its home point. Ground the enemy has overrun is skipped.
                let (a, b) = (pts[line[0]], pts[line[1]]);
                let c = pts[NUM_POINTS / 2];
                for z in [point(b, 1.0), between(lerp(b, c, 0.3), 0.75), between(lerp(a, b, 0.5), 0.85), point(a, 0.6)] {
                    if enemies_near(z.c, 18.0) < 2 {
                        zones.push(z);
                    }
                }
                zones.push(home);
            }
            _ => {
                // Capture the flag: the base, and behind the team's flag.
                let a = pts[line[0]];
                zones.push(Zone { w: 0.75, ..home });
                for z in [between(lerp(base, a, 0.5), 0.7), point(a, 0.6)] {
                    if enemies_near(z.c, 18.0) == 0 {
                        zones.push(z);
                    }
                }
            }
        }
        zones.sort_by(|a, b| b.w.total_cmp(&a.w));
        zones
    }

    /// Whether someone can deploy with their feet at `p`: open, level enough
    /// ground under open sky, not in lava or deep water.
    pub fn spawn_ok(&self, p: Vec3) -> bool {
        let (x, z) = (p.x, p.z);
        if self.nav.is_blocked(x, z) || self.world.danger(p, 1.0) {
            return false;
        }
        let h = |dx: f32, dz: f32| self.world.walk_height(x + dx, z + dz);
        if (h(1.0, 0.0) - h(-1.0, 0.0)).abs() > 1.1 || (h(0.0, 1.0) - h(0.0, -1.0)).abs() > 1.1 {
            return false;
        }
        // Not indoors, nor under a deck or a bridge.
        self.world.raycast(p + vec3(0.0, 0.5, 0.0), vec3(0.0, 1.0, 0.0), 10.0, self.dmask).is_none()
    }

    /// How exposed someone of `team` standing at `p` would be: for each
    /// living enemy with a view of them, one, plus up to one more the
    /// closer that enemy is.
    fn spawn_seen(&self, team: u8, p: Vec3) -> f32 {
        let head = p + vec3(0.0, EYE, 0.0);
        let mut threat = 0.0;
        for q in self.players.iter().filter(|q| q.alive && q.team != team) {
            let eye = q.st.pos + vec3(0.0, EYE, 0.0);
            let d = (eye - head).length();
            if d > SIGHT {
                continue;
            }
            if self.world.los(eye, head, self.dmask) && !self.world.foliage(eye, head) && !self.world.screened(eye, head) {
                threat += 1.0 + (SIGHT - d) / SIGHT;
            }
        }
        threat
    }

    /// Where player `i` of `team` deploys.
    pub(crate) fn spawn_spot(&mut self, i: usize, team: u8) -> Vec3 {
        let zones = self.spawn_zones(team);
        let t = self.tick;
        let mut best = (f32::MIN, None);
        let mut found = 0;
        // Deal tries round the zones, from a random one, so every zone gets
        // looked at and scoring - not the order - decides.
        let start = self.rng.below(zones.len() as u32) as usize;
        for k in 0..80 {
            let zone = zones[(start + k) % zones.len()];
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            // Uniform over the ring's area, not bunched at its middle.
            let r = (zone.r0 * zone.r0 + self.rng.f32() * (zone.r1 * zone.r1 - zone.r0 * zone.r0)).sqrt();
            let (x, z) = (zone.c.x + a.cos() * r, zone.c.z + a.sin() * r);
            // On Spillway's gate deck, on the deck, not the channel under it.
            let p = vec3(x, self.world.walk_height(x, z) + 0.05, z);
            if !self.spawn_ok(p) {
                continue;
            }
            let flat = |q: Vec3| ((q.x - p.x).powi(2) + (q.z - p.z).powi(2)).sqrt();
            let mut score = zone.w * 30.0 + self.rng.range(0.0, 10.0);
            let (mut friend, mut foe) = (14.0f32, 60.0f32);
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
            // Room to stand, enemies well away, and nobody watching.
            score += friend;
            if friend < 2.5 {
                score -= 40.0;
            }
            score += foe * 0.4;
            if foe < 20.0 {
                score -= (20.0 - foe) * 4.0;
            }
            score -= self.spawn_seen(team, p) * 45.0;
            // Somewhere the team has not just come in.
            for &(_, q, when) in self.recent_spawns.iter().filter(|s| s.0 == team) {
                let d = flat(q);
                if d < 12.0 {
                    let fresh = 1.0 - t.saturating_sub(when) as f32 / super::SPAWN_MEMORY as f32;
                    score -= (12.0 - d) * 2.5 * fresh.max(0.0);
                }
            }
            if score > best.0 {
                best = (score, Some(p));
            }
            found += 1;
            if found >= CANDIDATES {
                break;
            }
        }
        best.1.unwrap_or_else(|| {
            let c = zones[0].c;
            vec3(c.x, self.world.walk_height(c.x, c.z) + 0.05, c.z)
        })
    }
}
