//! Timed map events on the world clock (`crate::world_clock`): Slag
//! Cathedral's eruptions and the lava they send down the runner, and the
//! Great Dam's siren tests and the flood that follows them into Spillway's
//! channel.
//!
//! Everything here is a pure function of the world seed and the world clock,
//! so the server (which burns and drowns players) and every client (which
//! draws the lava and water) agree on what is happening at every moment,
//! whenever they joined.

/// One eruption window. About 40% of windows erupt.
pub const ERUPTION_PERIOD: f32 = 96.0;
/// One siren window at the Great Dam. About half of them test the sirens,
/// and each test is followed by a release of water.
pub const SIREN_PERIOD: f32 = 90.0;
/// When in its window a siren test sounds (start, end).
pub const SIREN: (f32, f32) = (30.0, 44.0);

/// Lava from an eruption reaches the edge of the field this far into the
/// window (the fountain starts at 26 s) ...
pub const LAVA_ARRIVES: f32 = 30.0;
/// ... and runs down the runner at this speed (metres per second) ...
pub const LAVA_SPEED: f32 = 5.5;
/// ... until it crusts over and can be walked on again at this time. The
/// glow fades for a while after.
pub const LAVA_CRUSTS: f32 = 68.0;
pub const LAVA_COOLED: f32 = 82.0;

/// The dam's outlet gates start to open this far into a siren window ...
pub const GATES_OPEN: f32 = 35.0;
/// ... water comes out into the channel at C at this time ...
pub const WATER_ARRIVES: f32 = 39.0;
/// ... and spreads along it at this speed (metres per second) as it rises,
/// reaching full depth after `WATER_RISE` seconds.
pub const WATER_SPEED: f32 = 8.0;
pub const WATER_RISE: f32 = 9.0;
/// The gates close at this time and the channel drains by `WATER_DRAINED`.
pub const GATES_CLOSE: f32 = 72.0;
pub const WATER_DRAINED: f32 = 84.0;

pub fn hash01(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h & 0xFFFF) as f32 / 65535.0
}

pub fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn seed32(seed: u64) -> u32 {
    (seed ^ (seed >> 32)) as u32
}

fn clean(time: f32) -> f32 {
    if time.is_finite() {
        time.max(0.0)
    } else {
        0.0
    }
}

/// Slag Cathedral's volcano. Smoke thickens first, then a lava fountain
/// launches bombs, then the plume cools.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eruption {
    pub warning: f32,
    pub fountain: f32,
    pub ash: f32,
    /// Seconds since the fountain started (negative before it).
    pub age: f32,
    pub cycle: u32,
    pub erupts: bool,
}

pub fn eruption(seed: u64, time: f32) -> Eruption {
    let time = clean(time);
    let cycle = (time / ERUPTION_PERIOD).floor() as u32;
    let phase = time.rem_euclid(ERUPTION_PERIOD);
    let rolled = hash01(seed32(seed) ^ cycle.wrapping_mul(0x9E37), 0xCADA);
    let mut out = Eruption { warning: 0.0, fountain: 0.0, ash: 0.0, age: phase - 26.0, cycle, erupts: false };
    if rolled >= 0.40 {
        return out;
    }
    out.erupts = true;
    out.warning = smooth(16.0, 26.0, phase) * (1.0 - smooth(44.0, 62.0, phase));
    out.fountain = smooth(26.0, 29.0, phase) * (1.0 - smooth(39.0, 46.0, phase));
    out.ash = smooth(21.0, 36.0, phase) * (1.0 - smooth(57.0, 82.0, phase));
    out
}

/// The lava an eruption sends down Slag Cathedral's runner.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct LavaFlow {
    /// How far down the runner (metres from its source) molten lava has
    /// reached. Zero when there is none to burn anyone.
    pub front: f32,
    /// Brightness of whatever lava lies in the runner, molten or cooling:
    /// 1 while it flows, fading to 0 once it has crusted.
    pub glow: f32,
    /// How far down the runner any lava, molten or crusted, lies.
    pub extent: f32,
}

pub fn lava_flow(seed: u64, time: f32) -> LavaFlow {
    let e = eruption(seed, time);
    let phase = clean(time).rem_euclid(ERUPTION_PERIOD);
    if !e.erupts || phase < LAVA_ARRIVES {
        return LavaFlow::default();
    }
    let extent = (phase.min(LAVA_CRUSTS) - LAVA_ARRIVES) * LAVA_SPEED;
    let front = if phase < LAVA_CRUSTS { extent } else { 0.0 };
    let glow = 1.0 - smooth(LAVA_CRUSTS - 2.0, LAVA_COOLED, phase);
    LavaFlow { front, glow, extent: if phase < LAVA_COOLED { extent } else { 0.0 } }
}

/// Seconds into a siren test at the Great Dam, if one is sounding.
pub fn siren(seed: u64, time: f32) -> Option<f32> {
    let time = clean(time);
    let phase = time.rem_euclid(SIREN_PERIOD);
    (siren_tested(seed, siren_cycle(time)) && (SIREN.0..SIREN.1).contains(&phase)).then_some(phase - SIREN.0)
}

pub fn siren_cycle(time: f32) -> u32 {
    (clean(time) / SIREN_PERIOD).floor() as u32
}

/// Whether the sirens are tested (and water released) in window `cycle`.
pub fn siren_tested(seed: u64, cycle: u32) -> bool {
    hash01(seed32(seed) ^ cycle.wrapping_mul(0x9E37_79B9), 0x5124) < 0.5
}

/// Water released from the Great Dam after a siren test.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Flood {
    /// How far open the outlet gates are, 0 to 1.
    pub gates: f32,
    /// Depth of the water as a fraction of its full depth, 0 to 1.
    pub level: f32,
    /// How far along the channel either way from C the water has spread.
    pub reach: f32,
    /// Seconds since water first came out (negative before).
    pub age: f32,
}

pub fn flood(seed: u64, time: f32) -> Flood {
    let time = clean(time);
    let phase = time.rem_euclid(SIREN_PERIOD);
    if !siren_tested(seed, siren_cycle(time)) {
        return Flood { age: phase - WATER_ARRIVES, ..Flood::default() };
    }
    let gates = smooth(GATES_OPEN, GATES_OPEN + 4.0, phase) * (1.0 - smooth(GATES_CLOSE, GATES_CLOSE + 4.0, phase));
    let rise = smooth(WATER_ARRIVES, WATER_ARRIVES + WATER_RISE, phase);
    let drain = 1.0 - smooth(GATES_CLOSE, WATER_DRAINED, phase);
    let level = rise * drain;
    let reach = if level > 0.0 { (phase - WATER_ARRIVES).max(0.0) * WATER_SPEED } else { 0.0 };
    Flood { gates, level, reach, age: phase - WATER_ARRIVES }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEDS: [u64; 4] = [0, 17, 0xFE221, u64::MAX];

    #[test]
    fn eruptions_send_lava_that_flows_crusts_and_cools() {
        for seed in SEEDS {
            let active: Vec<u32> = (0..64).filter(|&c| eruption(seed, c as f32 * ERUPTION_PERIOD + 33.0).fountain > 0.5).collect();
            assert!(!active.is_empty() && active.len() < 64, "need both erupting and dormant windows");
            let base = active[0] as f32 * ERUPTION_PERIOD;
            assert_eq!(lava_flow(seed, base + LAVA_ARRIVES - 0.5).front, 0.0);
            let a = lava_flow(seed, base + 40.0);
            let b = lava_flow(seed, base + 50.0);
            assert!(a.front > 0.0 && b.front > a.front && a.glow == 1.0);
            let crust = lava_flow(seed, base + LAVA_CRUSTS + 1.0);
            assert_eq!(crust.front, 0.0, "crusted lava does not burn");
            assert!(crust.glow > 0.0 && crust.extent > 0.0, "but still glows a while");
            assert_eq!(lava_flow(seed, base + LAVA_COOLED + 1.0), LavaFlow::default());
            let dormant = (0..64).find(|c| !active.contains(c)).unwrap() as f32 * ERUPTION_PERIOD;
            assert_eq!(lava_flow(seed, dormant + 50.0), LavaFlow::default());
        }
    }

    #[test]
    fn floods_follow_siren_tests_and_drain_before_the_next() {
        for seed in SEEDS {
            let tested: Vec<u32> = (0..64).filter(|&c| siren_tested(seed, c)).collect();
            assert!(!tested.is_empty() && tested.len() < 64);
            let base = tested[0] as f32 * SIREN_PERIOD;
            assert!(siren(seed, base + SIREN.0 + 1.0).is_some());
            assert_eq!(flood(seed, base + SIREN.0 + 1.0).level, 0.0, "the siren warns before the water");
            let f = flood(seed, base + WATER_ARRIVES + WATER_RISE + 2.0);
            assert!(f.level > 0.99 && f.gates > 0.99 && f.reach > 80.0);
            assert_eq!(flood(seed, base + WATER_DRAINED + 0.5).level, 0.0);
            assert_eq!(flood(seed, base + SIREN_PERIOD - 0.01).level, 0.0);
            let quiet = (0..64).find(|c| !tested.contains(c)).unwrap() as f32 * SIREN_PERIOD;
            assert!(siren(seed, quiet + SIREN.0 + 1.0).is_none());
            assert_eq!(flood(seed, quiet + 50.0).level, 0.0);
        }
    }

    #[test]
    fn events_tolerate_any_clock() {
        for t in [f32::NAN, f32::INFINITY, -5.0, 0.0, 1e7] {
            let _ = (eruption(1, t), lava_flow(1, t), siren(1, t), flood(1, t));
        }
    }
}
