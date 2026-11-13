//! Shared, deterministic-ish fixed-tick simulation and versioned wire messages.
//! This is a LAN prototype, NOT a secure Internet-facing multiplayer protocol.
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 2;
pub const TICK_HZ: u32 = 20;
pub const TICK_DT: f32 = 1.0 / TICK_HZ as f32;
pub const MAP_HALF: f32 = 48.0;
pub const PLAYER_SPEED: f32 = 8.0;
pub const SPRINT_SPEED: f32 = 12.0;
pub const MAX_HEALTH: i32 = 100;
pub const BODY_DAMAGE: i32 = 25;
pub const HEAD_DAMAGE: i32 = 75;
pub const ZONES: [(f32, f32); 3] = [(-25.0, 0.0), (0.0, 0.0), (25.0, 0.0)];

/// Face from the team spawn toward the center of the map.
pub fn spawn_yaw(team: u8) -> f32 {
    if team == 0 {
        std::f32::consts::FRAC_PI_2
    } else {
        -std::f32::consts::FRAC_PI_2
    }
}

/// One client command per 50ms predicted simulation step (best-effort UDP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    pub version: u16,
    pub seq: u64,
    pub name: String,
    pub forward: f32,
    pub strafe: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fire: bool,
    pub sprint: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soldier {
    pub id: u32,
    pub name: String,
    pub team: u8,
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    pub health: i32,
    pub kills: u32,
    pub deaths: u32,
    pub bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub x: f32,
    pub z: f32,
    pub owner: i8,
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatEvent {
    pub id: u64,
    pub tick: u64,
    pub shooter: u32,
    pub victim: u32,
    pub damage: i32,
    pub headshot: bool,
    pub killed: bool,
    /// Human command used for this shot; absent for bots and older v2 servers.
    #[serde(default)]
    pub input_seq: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u16,
    pub you: u32,
    pub tick: u64,
    /// The newest input sequence actually consumed by the server for this player.
    pub ack_seq: u64,
    pub soldiers: Vec<Soldier>,
    pub zones: Vec<Zone>,
    pub score: [u32; 2],
    pub winner: Option<u8>,
    pub events: Vec<CombatEvent>,
}

// Center X/Z, width, depth, height. Render and collision use the same geometry.
pub const COVER: [(f32, f32, f32, f32, f32); 13] = [
    (-35.0, -13.0, 8.0, 5.0, 5.0),
    (-35.0, 13.0, 8.0, 5.0, 5.0),
    (-17.0, -12.0, 7.0, 8.0, 4.0),
    (-17.0, 14.0, 7.0, 6.0, 5.0),
    (-3.0, -20.0, 10.0, 5.0, 4.0),
    (4.0, 19.0, 11.0, 5.0, 5.0),
    (0.0, -7.0, 7.0, 5.0, 3.0),
    (0.0, 8.0, 7.0, 5.0, 3.0),
    (18.0, -15.0, 7.0, 7.0, 5.0),
    (18.0, 13.0, 7.0, 7.0, 4.0),
    (35.0, -14.0, 8.0, 5.0, 5.0),
    (35.0, 14.0, 8.0, 5.0, 5.0),
    (0.0, 0.0, 3.0, 3.0, 2.5),
];

pub fn valid_input(input: &Input) -> bool {
    input.version == PROTOCOL_VERSION
        && input.seq > 0
        && input.name.len() <= 32
        && input.forward.is_finite()
        && input.strafe.is_finite()
        && input.yaw.is_finite()
        && input.pitch.is_finite()
}

pub fn blocked(x: f32, z: f32) -> bool {
    if !x.is_finite() || !z.is_finite() || x.abs() > MAP_HALF - 1.0 || z.abs() > MAP_HALF - 1.0 {
        return true;
    }
    COVER.iter().any(|&(cx, cz, w, d, _)| {
        (x - cx).abs() < w * 0.5 + 0.52 && (z - cz).abs() < d * 0.5 + 0.52
    })
}

/// Shared client/server horizontal movement. Axis-separated collisions permit wall sliding.
pub fn move_actor(s: &mut Soldier, forward: f32, strafe: f32, speed: f32) {
    if s.health <= 0 {
        return;
    }
    let (sin, cos) = s.yaw.sin_cos();
    // The camera looks along +Z at yaw=0; its screen-right axis is -X.
    let mut dx = sin * forward - cos * strafe;
    let mut dz = cos * forward + sin * strafe;
    let len = dx.hypot(dz);
    if len > 1.0 {
        dx /= len;
        dz /= len;
    }
    let offset = speed * TICK_DT;
    if !blocked(s.x + dx * offset, s.z) {
        s.x += dx * offset;
    }
    if !blocked(s.x, s.z + dz * offset) {
        s.z += dz * offset;
    }
}

pub fn apply_input(s: &mut Soldier, input: &Input) {
    if s.health <= 0 {
        return;
    }
    s.yaw = input.yaw;
    move_actor(
        s,
        input.forward.clamp(-1.0, 1.0),
        input.strafe.clamp(-1.0, 1.0),
        if input.sprint {
            SPRINT_SPEED
        } else {
            PLAYER_SPEED
        },
    );
}

/// Rebuild local predicted state from the last authoritative tick and unacknowledged commands.
pub fn replay_unacked(authoritative: &Soldier, inputs: &[Input], ack_seq: u64) -> Soldier {
    let mut predicted = authoritative.clone();
    for input in inputs.iter().filter(|i| i.seq > ack_seq) {
        apply_input(&mut predicted, input);
    }
    predicted
}

/// Slab ray/AABB intersection. Returns the FIRST nonnegative ray distance, if any.
/// Direction is assumed normalized; zero components are handled explicitly.
pub fn ray_box(origin: [f32; 3], direction: [f32; 3], min: [f32; 3], max: [f32; 3]) -> Option<f32> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        let d = direction[axis];
        if d.abs() < 1e-6 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let mut a = (min[axis] - origin[axis]) / d;
        let mut b = (max[axis] - origin[axis]) / d;
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        near = near.max(a);
        far = far.min(b);
        if near > far {
            return None;
        }
    }
    if far < 0.0 {
        None
    } else {
        Some(near.max(0.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShotHit {
    pub target: u32,
    pub damage: i32,
    pub headshot: bool,
}

/// Server-side single-bullet hit test, including real vertical cover occlusion.
/// One ray can damage only the nearest eligible enemy. No penetration yet.
pub fn trace_shot(shooter: &Soldier, pitch: f32, soldiers: &[Soldier]) -> Option<ShotHit> {
    if shooter.health <= 0 || !pitch.is_finite() {
        return None;
    }
    let origin = [shooter.x, 1.75, shooter.z];
    let direction = [
        shooter.yaw.sin() * pitch.cos(),
        pitch.sin(),
        shooter.yaw.cos() * pitch.cos(),
    ];
    let nearest_cover = COVER
        .iter()
        .filter_map(|&(x, z, w, d, h)| {
            ray_box(
                origin,
                direction,
                [x - w / 2.0, 0.0, z - d / 2.0],
                [x + w / 2.0, h, z + d / 2.0],
            )
        })
        .fold(f32::INFINITY, f32::min);

    let mut best: Option<(f32, ShotHit)> = None;
    for enemy in soldiers {
        if enemy.id == shooter.id || enemy.team == shooter.team || enemy.health <= 0 {
            continue;
        }
        let body = ray_box(
            origin,
            direction,
            [enemy.x - 0.44, 0.22, enemy.z - 0.44],
            [enemy.x + 0.44, 2.17, enemy.z + 0.44],
        );
        let Some(distance) = body else {
            continue;
        };
        if distance > 62.0 || nearest_cover <= distance {
            continue;
        }
        if best.is_some_and(|(old, _)| old <= distance) {
            continue;
        }
        let head_distance = ray_box(
            origin,
            direction,
            [enemy.x - 0.30, 1.60, enemy.z - 0.30],
            [enemy.x + 0.30, 2.17, enemy.z + 0.30],
        );
        let headshot = head_distance.is_some_and(|t| t < 62.0 && t < nearest_cover);
        best = Some((
            distance,
            ShotHit {
                target: enemy.id,
                damage: if headshot { HEAD_DAMAGE } else { BODY_DAMAGE },
                headshot,
            },
        ));
    }
    best.map(|(_, hit)| hit)
}

/// True contention pauses capture, rather than rewarding the larger crowd.
pub fn update_zone(zone: &mut Zone, soldiers: &[Soldier]) {
    let mut count = [0_u32; 2];
    for s in soldiers {
        if s.health > 0 && s.team < 2 && (s.x - zone.x).hypot(s.z - zone.z) < 10.0 {
            count[s.team as usize] += 1;
        }
    }
    if count[0] > 0 && count[1] == 0 {
        zone.progress = (zone.progress - 0.7 * count[0].min(3) as f32).max(-100.0);
    } else if count[1] > 0 && count[0] == 0 {
        zone.progress = (zone.progress + 0.7 * count[1].min(3) as f32).min(100.0);
    }
    zone.owner = if zone.progress <= -95.0 {
        0
    } else if zone.progress >= 95.0 {
        1
    } else {
        -1
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    fn soldier(id: u32, team: u8, x: f32, z: f32) -> Soldier {
        Soldier {
            id,
            team,
            x,
            z,
            yaw: 0.0,
            name: format!("{id}"),
            health: 100,
            kills: 0,
            deaths: 0,
            bot: false,
        }
    }
    fn input(seq: u64) -> Input {
        Input {
            version: PROTOCOL_VERSION,
            seq,
            name: "test".into(),
            forward: 1.0,
            strafe: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            fire: false,
            sprint: false,
        }
    }
    #[test]
    fn spawn_views_point_into_the_map() {
        assert!(spawn_yaw(0).sin() > 0.99);
        assert!(spawn_yaw(1).sin() < -0.99);
    }
    #[test]
    fn older_v2_combat_events_remain_readable_without_shot_correlation() {
        let event: CombatEvent = serde_json::from_str(
            r#"{"id":1,"tick":20,"shooter":1,"victim":2,"damage":25,"headshot":false,"killed":false}"#,
        ).unwrap();
        assert_eq!(event.input_seq, None);
    }
    #[test]
    fn walls_stop_movement() {
        let mut s = soldier(1, 0, 46.7, 20.0);
        s.yaw = std::f32::consts::FRAC_PI_2;
        for _ in 0..20 {
            move_actor(&mut s, 1.0, 0.0, PLAYER_SPEED);
        }
        assert!(s.x <= 47.0);
    }
    #[test]
    fn diagonal_speed_is_normalized() {
        let mut a = soldier(1, 0, -40.0, 0.0);
        let mut b = a.clone();
        move_actor(&mut a, 1.0, 0.0, PLAYER_SPEED);
        move_actor(&mut b, 1.0, 1.0, PLAYER_SPEED);
        assert!(((a.x + 40.0).hypot(a.z) - (b.x + 40.0).hypot(b.z)).abs() < 0.0001);
    }
    #[test]
    fn strafe_right_follows_camera_right_at_two_yaws() {
        let mut s = soldier(1, 0, -40.0, 0.0);
        move_actor(&mut s, 0.0, 1.0, PLAYER_SPEED);
        assert!((s.x + 40.4).abs() < 0.0001);
        assert!(s.z.abs() < 0.0001);
        s.yaw = std::f32::consts::FRAC_PI_2;
        move_actor(&mut s, 0.0, 1.0, PLAYER_SPEED);
        assert!((s.z - 0.4).abs() < 0.0001);
    }
    #[test]
    fn prediction_replays_only_unacked_commands() {
        let s = soldier(1, 0, -40.0, 0.0);
        let predicted = replay_unacked(&s, &[input(3), input(4), input(5)], 4);
        assert!((predicted.z - 0.4).abs() < 0.0001);
    }
    #[test]
    fn ray_box_parallel_axis_rejects_outside_ray() {
        assert_eq!(
            ray_box(
                [5.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [-1.0, -1.0, 4.0],
                [1.0, 1.0, 5.0]
            ),
            None
        );
    }
    #[test]
    fn ray_box_intersection_has_expected_distance() {
        assert_eq!(
            ray_box(
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [-1.0, -1.0, 4.0],
                [1.0, 1.0, 5.0]
            ),
            Some(4.0)
        );
    }
    #[test]
    fn nearby_headshot_works() {
        let a = soldier(1, 0, -40.0, 30.0);
        let b = soldier(2, 1, -40.0, 35.0);
        let hit = trace_shot(&a, 0.0, &[a.clone(), b]).unwrap();
        assert_eq!(hit.target, 2);
        assert!(hit.headshot);
        assert_eq!(hit.damage, HEAD_DAMAGE);
    }
    #[test]
    fn torso_hit_works() {
        let a = soldier(1, 0, -40.0, 30.0);
        let b = soldier(2, 1, -40.0, 35.0);
        let hit = trace_shot(&a, -0.15, &[a.clone(), b]).unwrap();
        assert_eq!(hit.damage, BODY_DAMAGE);
    }
    #[test]
    fn obstacle_blocks_shot() {
        let a = soldier(1, 0, 18.0, -23.0);
        let b = soldier(2, 1, 18.0, -8.0);
        assert!(trace_shot(&a, 0.0, &[a.clone(), b]).is_none());
    }
    #[test]
    fn closest_target_gets_hit() {
        let a = soldier(1, 0, -40.0, 30.0);
        let b = soldier(2, 1, -40.0, 35.0);
        let c = soldier(3, 1, -40.0, 40.0);
        assert_eq!(trace_shot(&a, 0.0, &[a.clone(), c, b]).unwrap().target, 2);
    }
    #[test]
    fn allies_are_ignored() {
        let a = soldier(1, 0, -40.0, 30.0);
        let b = soldier(2, 0, -40.0, 35.0);
        assert!(trace_shot(&a, 0.0, &[a.clone(), b]).is_none());
    }
    #[test]
    fn contested_zone_freezes_progress() {
        let mut z = Zone {
            x: 25.0,
            z: 0.0,
            owner: -1,
            progress: 20.0,
        };
        update_zone(
            &mut z,
            &[soldier(1, 0, 26.0, 1.0), soldier(2, 1, 25.0, 1.0)],
        );
        assert_eq!(z.progress, 20.0);
    }
    #[test]
    fn empty_zone_does_not_capture() {
        let mut z = Zone {
            x: 25.0,
            z: 0.0,
            owner: -1,
            progress: 0.0,
        };
        update_zone(&mut z, &[]);
        assert_eq!(z.progress, 0.0);
    }
    #[test]
    fn invalid_packet_version_is_rejected() {
        let mut i = input(1);
        i.version += 1;
        assert!(!valid_input(&i));
    }
    #[test]
    fn input_json_roundtrip() {
        let p = input(7);
        let data = serde_json::to_vec(&p).unwrap();
        let decoded: Input = serde_json::from_slice(&data).unwrap();
        assert_eq!(decoded.seq, 7);
        assert!(valid_input(&decoded));
    }
}
