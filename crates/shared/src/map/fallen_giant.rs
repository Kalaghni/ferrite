//! The Fallen Giant's cargo hold spans Ridgeline's central saddle.
//! The accessible shell and cover are balanced under a half turn. The
//! inaccessible upper silhouette can carry a distinct nose and tail in art.
use super::{smooth, Block, BlockKind, World};

pub const MAP_ID: u8 = 2;
pub const HALF_LENGTH: f32 = 42.0;
pub const HALF_WIDTH: f32 = 6.0;
pub const WALL_THICKNESS: f32 = 0.35;
pub const ROOF_HEIGHT: f32 = 8.0;
pub const DOOR_HEIGHT: f32 = 4.0;
pub const DOOR_HALF_WIDTH: f32 = 3.0;
pub const SIDE_DOORS: [f32; 3] = [-20.0, 0.0, 20.0];
pub const SHELL_TINT: u8 = 40;
pub const CARGO_TINT: u8 = 41;

pub fn floor(w: &World) -> f32 {
    w.height(0.0, 0.0)
}

pub fn owns(block: &Block) -> bool {
    block.kind == BlockKind::Metal && matches!(block.tint, SHELL_TINT | CARGO_TINT)
}

/// Flat ground inside and at every entrance; the surrounding ridge blends
/// into this crash scar without introducing a step at a torn hull opening.
pub(super) fn settle(x: f32, z: f32, h: f32, target: f32) -> f32 {
    let d = (x.abs() - HALF_LENGTH - 5.0).max(z.abs() - HALF_WIDTH - 5.0).max(0.0);
    let weight = 1.0 - smooth(0.0, 8.0, d);
    h + (target - h) * weight
}

pub(super) fn reserved(map: u8, x: f32, z: f32, hx: f32, hz: f32) -> bool {
    map == MAP_ID && x.abs() < HALF_LENGTH + 5.0 + hx && z.abs() < HALF_WIDTH + 5.0 + hz
}

pub(super) fn build(w: &mut World) {
    let g = floor(w);
    // Sidewall segments between the paired tears. End-on entry has no
    // blocking ramp lip, and the centre stays clear for objective C.
    for z in [-HALF_WIDTH - WALL_THICKNESS * 0.5, HALF_WIDTH + WALL_THICKNESS * 0.5] {
        let mut start = -HALF_LENGTH;
        for end in SIDE_DOORS.map(|x| x - DOOR_HALF_WIDTH).into_iter().chain([HALF_LENGTH]) {
            let center = (start + end) * 0.5;
            w.add_abs(center, z, (end - start) * 0.5, WALL_THICKNESS * 0.5, g - 0.6, g + ROOF_HEIGHT, BlockKind::Metal, SHELL_TINT);
            start = end + DOOR_HALF_WIDTH * 2.0;
        }
        for x in SIDE_DOORS {
            w.add_abs(x, z, DOOR_HALF_WIDTH, WALL_THICKNESS * 0.5, g + DOOR_HEIGHT, g + ROOF_HEIGHT, BlockKind::Metal, SHELL_TINT);
        }
    }
    w.add_abs(
        0.0,
        0.0,
        HALF_LENGTH,
        HALF_WIDTH + WALL_THICKNESS,
        g + ROOF_HEIGHT,
        g + ROOF_HEIGHT + WALL_THICKNESS,
        BlockKind::Metal,
        SHELL_TINT,
    );
    // Cargo cases are low cover along the hold edges, leaving a broad
    // clear walking line through the entire aircraft.
    w.sym_abs(-30.0, -3.8, 2.8, 0.8, g, g + 1.2, BlockKind::Metal, CARGO_TINT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glam::{vec3, Vec3};

    #[test]
    fn cargo_shell_is_balanced_and_has_grounded_clear_openings() {
        for seed in [0, 17, 0xFE221, u64::MAX] {
            let world = World::with_map(seed, MAP_ID);
            let g = floor(&world);
            let shell: Vec<_> = world.blocks.iter().filter(|b| owns(b)).collect();
            assert_eq!(shell.len(), 17);
            for b in &shell {
                assert!(b.min.is_finite() && b.max.is_finite() && b.min.cmplt(b.max).all());
                assert!(
                    shell.iter().any(|q| {
                        (q.min.x + b.max.x).abs() < 0.001
                            && (q.max.x + b.min.x).abs() < 0.001
                            && (q.min.z + b.max.z).abs() < 0.001
                            && (q.max.z + b.min.z).abs() < 0.001
                            && (q.min.y - b.min.y).abs() < 0.001
                            && (q.max.y - b.max.y).abs() < 0.001
                            && q.tint == b.tint
                    }),
                    "seed {seed:x}: unbalanced aircraft block at {:?}",
                    b.min
                );
            }
            for x in [-46.0, 0.0, 46.0] {
                assert!((world.height(x, 0.0) - g).abs() < 0.01);
            }
            for x in SIDE_DOORS {
                for z in [-10.0, -6.0, 0.0, 6.0, 10.0] {
                    assert!((world.height(x, z) - g).abs() < 0.01);
                }
                assert!(world.los(vec3(x, g + 1.7, -10.0), vec3(x, g + 1.7, 10.0), 0));
            }
            let roof = world.raycast(vec3(0.0, g + 1.7, 0.0), Vec3::Y, 10.0, 0).unwrap();
            assert!((roof.t - (ROOF_HEIGHT - 1.7)).abs() < 0.01);
        }
    }
}
