//! Authored, playable destinations for the original four battlefields.
//!
//! The heightfield supplies each passage's floor; solid rock and structure
//! blocks supply its roof. This permits genuine cover overhead without
//! asking a heightfield to represent two surfaces at the same (x, z).
use super::{smooth, BlockKind, Feature, FeatureKind, World};
use crate::glam::{vec3, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub x: f32,
    pub z: f32,
    pub hx: f32,
    pub hz: f32,
    /// Clear height under the roof, relative to the levelled ground.
    pub height: f32,
}

impl Site {
    pub fn mirrored(self) -> Self {
        Self {
            x: -self.x,
            z: -self.z,
            ..self
        }
    }
}

pub const WORKSHOPS: [Site; 4] = [
    Site {
        x: -88.0,
        z: -65.0,
        hx: 9.0,
        hz: 6.0,
        height: 5.0,
    },
    Site {
        x: -102.0,
        z: 52.0,
        hx: 8.0,
        hz: 5.0,
        height: 4.8,
    },
    Site {
        x: -90.0,
        z: -64.0,
        hx: 7.0,
        hz: 5.0,
        height: 4.2,
    },
    Site {
        x: -103.0,
        z: 53.0,
        hx: 11.0,
        hz: 7.0,
        height: 6.0,
    },
];

/// Open along X, with entrances at x +/- hx. Each has a mirrored partner.
pub const PASSAGES: [Site; 4] = [
    Site {
        x: -26.0,
        z: 49.0,
        hx: 13.0,
        hz: 3.4,
        height: 3.6,
    },
    Site {
        x: -43.0,
        z: -52.0,
        hx: 15.0,
        hz: 4.0,
        height: 4.2,
    },
    Site {
        x: -12.0,
        z: -39.0,
        hx: 15.0,
        hz: 4.0,
        height: 4.2,
    },
    Site {
        x: -83.0,
        z: -58.0,
        hx: 12.0,
        hz: 3.4,
        height: 3.8,
    },
];

pub const CRANE_HALF_X: f32 = 26.0;
pub const CRANE_HALF_Z: f32 = 25.0;
pub const CRANE_HEIGHT: f32 = 27.0;

pub fn floor(w: &World, site: Site) -> f32 {
    w.height(site.x, site.z)
}

pub fn walkway_site(map: u8) -> Site {
    let s = WORKSHOPS[map as usize % WORKSHOPS.len()];
    Site {
        x: s.x,
        z: s.z + s.hz + 10.0,
        hx: 8.2,
        hz: 2.0,
        height: 1.8,
    }
}

/// A level approach is part of a destination, not just its room interior.
/// Called while sampling terrain, before world collision is generated.
pub(super) fn settle(x: f32, z: f32, map: u8, mut h: f32, target: impl Fn(f32, f32) -> f32) -> f32 {
    if map > 3 {
        return h;
    }
    for s in [
        WORKSHOPS[map as usize],
        PASSAGES[map as usize],
        walkway_site(map),
    ] {
        for s in [s, s.mirrored()] {
            let d = ((x - s.x).abs() - s.hx)
                .max((z - s.z).abs() - s.hz)
                .max(0.0);
            let w = 1.0 - smooth(5.0, 11.0, d);
            if w > 0.0 {
                h += (target(s.x, s.z) - h) * w;
            }
        }
    }
    h
}

/// Reserve doors and both ends of passages against random cover and trees.
pub(super) fn reserved(map: u8, x: f32, z: f32, hx: f32, hz: f32) -> bool {
    if map > 3 {
        return false;
    }
    [
        WORKSHOPS[map as usize],
        PASSAGES[map as usize],
        walkway_site(map),
    ]
    .iter()
    .any(|&s| {
        [s, s.mirrored()]
            .iter()
            .any(|s| (x - s.x).abs() < s.hx + hx + 5.0 && (z - s.z).abs() < s.hz + hz + 5.0)
    })
}

pub(super) fn build(w: &mut World) {
    if w.map > 3 {
        return;
    }
    workshop(w, WORKSHOPS[w.map as usize]);
    passage(w, PASSAGES[w.map as usize]);
    match w.map {
        0 => foundry(w),
        1 => headframe(w),
        2 => wind_station(w),
        3 => gantry(w),
        _ => unreachable!(),
    }
    breakable_features(w);
}

fn feature(w: &mut World, kind: FeatureKind, pos: Vec3) -> u8 {
    assert!(w.n_crates < super::MAX_CRATES);
    let bit = w.n_crates as u8;
    w.n_crates += 1;
    w.features.push(Feature { kind, bit, pos });
    bit
}

#[allow(clippy::too_many_arguments)]
fn fragile(
    w: &mut World,
    bit: u8,
    x: f32,
    z: f32,
    hx: f32,
    hz: f32,
    y0: f32,
    y1: f32,
    kind: BlockKind,
) {
    w.add_abs(x, z, hx, hz, y0, y1, kind, 2);
    w.blocks.last_mut().unwrap().dest = Some(bit);
}

fn breakable_features(w: &mut World) {
    use BlockKind::*;
    let s = WORKSHOPS[w.map as usize];
    // A broad infill panel on the side away from the machinery can be
    // breached into a fifth doorway. Three metres also leaves a reliably
    // usable route in the bots' 1.5 m navigation grid.
    for s in [s, s.mirrored()] {
        let g = floor(w, s);
        let side = if s.x < 0.0 { 1.0 } else { -1.0 };
        let (x, z) = (s.x - side * s.hx * 0.6, s.z + side * (s.hz - 0.22));
        let mut wall = Vec::new();
        w.blocks.retain(|b| {
            let c = (b.min + b.max) * 0.5;
            if b.kind != Concrete
                || (c.z - z).abs() > 0.01
                || b.max.x <= x - 1.5
                || b.min.x >= x + 1.5
                || b.min.y >= g + 2.6
            {
                return true;
            }
            let (left, right) = (b.min.x.max(x - 1.5), b.max.x.min(x + 1.5));
            if b.min.x < left {
                let mut q = b.clone();
                q.max.x = left;
                wall.push(q);
            }
            if b.max.x > right {
                let mut q = b.clone();
                q.min.x = right;
                wall.push(q);
            }
            if b.max.y > g + 2.6 {
                let mut q = b.clone();
                q.min.x = left;
                q.max.x = right;
                q.min.y = g + 2.6;
                wall.push(q);
            }
            false
        });
        w.blocks.extend(wall);
        let bit = feature(w, FeatureKind::BreachPanel, vec3(x, g + 0.9, z));
        fragile(w, bit, x, z, 1.5, 0.22, g - 0.6, g + 2.6, Concrete);
    }
    let bridge = walkway_site(w.map);
    for s in [bridge, bridge.mirrored()] {
        let g = floor(w, s);
        let side = if s.x < 0.0 { 1.0 } else { -1.0 };
        let bit = feature(
            w,
            FeatureKind::Walkway,
            vec3(s.x, g + 0.7, s.z - 1.2 * side),
        );
        let top = g + 1.8;
        fragile(w, bit, s.x, s.z, 5.0, 1.5, top - 0.24, top, Metal);
        for dx in [-4.4, 0.0, 4.4] {
            for dz in [-1.2, 1.2] {
                fragile(
                    w,
                    bit,
                    s.x + dx,
                    s.z + dz,
                    0.18,
                    0.18,
                    g - 0.4,
                    top - 0.24,
                    Metal,
                );
            }
        }
        for dz in [-1.5, 1.5] {
            fragile(w, bit, s.x, s.z + dz, 5.0, 0.08, top, top + 0.8, Metal);
        }
        for dir in [-1.0, 1.0] {
            for i in 0..6 {
                let x = s.x + dir * (5.0 + (5 - i) as f32 * 0.5 + 0.25);
                fragile(
                    w,
                    bit,
                    x,
                    s.z,
                    0.25,
                    1.2,
                    g - 0.4,
                    g + (i + 1) as f32 * 0.3,
                    Step,
                );
            }
        }
    }
    let (x, z) = match w.map {
        0 => (s.x + 12.0, s.z - s.hz - 0.7),
        1 => (PASSAGES[1].x - 12.0, PASSAGES[1].z + 9.0),
        2 => (s.x + s.hx + 6.5, s.z + s.hz + 4.0),
        _ => (-CRANE_HALF_X - 3.0, -CRANE_HALF_Z + 2.5),
    };
    for side in [1.0, -1.0] {
        let (x, z) = (x * side, z * side);
        let g = w.height(x, z);
        let bit = feature(w, FeatureKind::Machinery, vec3(x, g + 0.7, z));
        fragile(w, bit, x, z, 0.65, 0.45, g - 0.2, g + 1.4, Metal);
    }
}

fn workshop(w: &mut World, s: Site) {
    use BlockKind::*;
    let g = floor(w, s);
    let roof = if w.map == 2 { Concrete } else { Metal };
    w.building(s.x, s.z, s.hx, s.hz, s.height, 0b1111, w.map, roof);
    w.sym_abs(
        s.x,
        s.z,
        s.hx - 0.43,
        s.hz - 0.43,
        g - 0.18,
        g + 0.06,
        Step,
        1,
    );
    // Storage counters and a workbench hug the walls; both crossing axes
    // stay open, so the building can be used to traverse or hold a flank.
    for dx in [-s.hx + 1.1, s.hx - 1.1] {
        w.sym_abs(
            s.x + dx,
            s.z + s.hz - 1.1,
            0.55,
            1.3,
            g + 0.06,
            g + 1.15,
            Metal,
            1,
        );
    }
    w.sym_abs(
        s.x - s.hx + 1.2,
        s.z - s.hz + 1.15,
        0.8,
        0.7,
        g + 0.06,
        g + 1.0,
        Concrete,
        2,
    );
    // A raised clerestory gives every room a second readable roof tier.
    w.sym_abs(
        s.x,
        s.z,
        s.hx * 0.56,
        s.hz * 0.42,
        g + s.height + 0.3,
        g + s.height + 1.0,
        Metal,
        1,
    );
}

fn passage(w: &mut World, s: Site) {
    use BlockKind::*;
    let g = floor(w, s);
    let (kind, thick) = match w.map {
        0 => (Concrete, 0.7),
        1 | 2 => (Rock, 2.1),
        _ => (Metal, 0.24),
    };
    for z in [s.z - s.hz - thick * 0.5, s.z + s.hz + thick * 0.5] {
        w.sym_abs(s.x, z, s.hx, thick * 0.5, g - 0.8, g + s.height, kind, 1);
    }
    w.sym_abs(
        s.x,
        s.z,
        s.hx,
        s.hz + thick,
        g + s.height,
        g + s.height + thick,
        kind,
        2,
    );
    if matches!(w.map, 1 | 2) {
        // A second irregular stratum and shoulders make these rock masses
        // read as a mine/cave, rather than a concrete shed painted grey.
        w.sym_abs(
            s.x - 1.3,
            s.z,
            s.hx - 2.0,
            s.hz + 0.7,
            g + s.height + thick,
            g + s.height + thick + 1.7,
            Rock,
            3,
        );
        for dz in [-s.hz - 3.0, s.hz + 3.0] {
            w.sym_abs(
                s.x + 1.0,
                s.z + dz,
                s.hx - 2.0,
                1.1,
                g - 0.8,
                g + 2.8,
                Rock,
                0,
            );
        }
    }
    // Repeated portal frames, side posts clear of the five-metre-wide
    // walking line, carry the same collision as their visible supports.
    let frame_kind = if w.map == 1 || w.map == 2 {
        BlockKind::Trunk
    } else {
        Metal
    };
    for i in 0..=4 {
        let x = s.x - s.hx + 0.35 + (2.0 * s.hx - 0.7) * i as f32 / 4.0;
        for z in [s.z - s.hz + 0.22, s.z + s.hz - 0.22] {
            w.sym_abs(x, z, 0.16, 0.16, g - 0.1, g + s.height, frame_kind, 0);
        }
        w.sym_abs(
            x,
            s.z,
            0.16,
            s.hz,
            g + s.height - 0.24,
            g + s.height,
            Metal,
            0,
        );
    }
}

fn foundry(w: &mut World) {
    use BlockKind::*;
    let s = WORKSHOPS[0];
    let g = floor(w, s);
    // Boiler court and two tall stacks make the new works easy to orient by.
    for dx in [-6.0, 6.0] {
        let (x, z) = (s.x + dx, s.z - s.hz - 3.0);
        w.sym_abs(x, z, 1.2, 1.2, g - 1.0, g + 22.0, Metal, 1);
        w.sym_abs(x, z, 1.55, 1.55, g + 21.6, g + 22.2, Metal, 0);
    }
    w.sym_abs(
        s.x + 12.0,
        s.z - s.hz - 3.0,
        2.4,
        1.6,
        g - 1.0,
        g + 3.8,
        Metal,
        0,
    );
}

fn headframe(w: &mut World) {
    use BlockKind::*;
    let s = PASSAGES[1];
    let g = floor(w, s);
    let (x, z) = (s.x - 8.0, s.z + 9.0);
    for dx in [-2.4, 2.4] {
        for dz in [-2.0, 2.0] {
            w.sym_abs(
                x + dx,
                z + dz,
                0.34,
                0.34,
                w.height(x + dx, z + dz) - 0.8,
                g + 18.0,
                Metal,
                0,
            );
        }
    }
    w.sym_abs(x, z, 3.2, 2.7, g + 17.6, g + 18.2, Metal, 1);
    // The shaft is safely capped with grating at ground level; the adit
    // itself provides the playable underground route.
    w.sym_abs(x, z, 2.7, 2.3, g - 0.3, g + 0.22, Metal, 0);
}

fn wind_station(w: &mut World) {
    use BlockKind::*;
    let s = WORKSHOPS[2];
    let (x, z) = (s.x + s.hx + 4.0, s.z + s.hz + 4.0);
    let g = w.height(x, z);
    w.sym_abs(x, z, 1.8, 1.8, g - 0.6, g + 0.35, Concrete, 0);
    w.sym_abs(x, z, 0.45, 0.45, g + 0.35, g + 22.0, Metal, 1);
    w.sym_abs(x, z, 1.1, 1.35, g + 20.0, g + 22.0, Metal, 0);
}

fn gantry(w: &mut World) {
    use BlockKind::*;
    let top = w.height(0.0, 0.0) + CRANE_HEIGHT;
    for x in [-CRANE_HALF_X, CRANE_HALF_X] {
        for z in [-CRANE_HALF_Z, CRANE_HALF_Z] {
            w.add_abs(x, z, 0.95, 1.45, w.height(x, z) - 0.7, top, Metal, 1);
            w.add_abs(
                x,
                z,
                1.6,
                2.6,
                w.height(x, z) - 0.1,
                w.height(x, z) + 1.15,
                Metal,
                0,
            );
        }
        w.add_abs(x, 0.0, 1.2, CRANE_HALF_Z + 3.0, top, top + 1.8, Metal, 1);
    }
    // The bridge is static collision; the trolley and suspended spreader
    // animate beneath it, well above the accessible play space.
    w.add_abs(
        0.0,
        0.0,
        CRANE_HALF_X + 1.8,
        1.5,
        top + 1.8,
        top + 3.1,
        Metal,
        1,
    );
    w.sym_abs(
        -CRANE_HALF_X,
        -CRANE_HALF_Z + 2.5,
        1.8,
        2.2,
        top - 4.6,
        top - 1.6,
        Metal,
        2,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glam::vec3;
    use crate::map::{Nav, MAX_CRATES};

    #[test]
    fn destinations_have_roofs_clear_approaches_and_mirrored_collision() {
        for seed in [0, 17, 0xFE221, u64::MAX] {
            for map in 0..4 {
                let w = World::with_map(seed, map);
                assert!(w.n_crates <= MAX_CRATES);
                assert_eq!(w.features.len(), 6);
                let mut feature_mask = 0u64;
                for f in &w.features {
                    assert_eq!(
                        feature_mask & (1u64 << f.bit),
                        0,
                        "feature destruction bits must be unique"
                    );
                    feature_mask |= 1u64 << f.bit;
                    assert!(f.pos.is_finite() && (f.bit as usize) < w.n_crates);
                    assert!(
                        w.blocks.iter().any(|b| b.dest == Some(f.bit)),
                        "feature has no grouped collision"
                    );
                }
                let nav = Nav::build(&w);
                for s in [WORKSHOPS[map as usize], PASSAGES[map as usize]] {
                    for s in [s, s.mirrored()] {
                        let g = floor(&w, s);
                        for i in -20..=20 {
                            let x = s.x + i as f32 * (s.hx + 3.0) / 20.0;
                            assert!(
                                (w.height(x, s.z) - g).abs() < 0.03,
                                "map {map} uneven destination floor"
                            );
                            assert!(
                                !nav.is_blocked(x, s.z),
                                "map {map}, seed {seed:x}: blocked entrance at {x},{}",
                                s.z
                            );
                        }
                        let hit = w
                            .raycast(
                                vec3(s.x, g + 1.6, s.z),
                                crate::glam::Vec3::Y,
                                s.height + 1.0,
                                0,
                            )
                            .expect("destination must have a solid roof");
                        assert!(hit.t > 1.4, "destination must have standing headroom");
                    }
                }
                for b in &w.blocks {
                    assert!(b.min.is_finite() && b.max.is_finite() && b.min.cmplt(b.max).all());
                    assert!(
                        w.blocks.iter().any(|q| {
                            (q.min.x + b.max.x).abs() < 0.002
                                && (q.max.x + b.min.x).abs() < 0.002
                                && (q.min.z + b.max.z).abs() < 0.002
                                && (q.max.z + b.min.z).abs() < 0.002
                                && (q.min.y - b.min.y).abs() < 0.05
                                && (q.max.y - b.max.y).abs() < 0.05
                                && q.kind == b.kind
                                && q.tint == b.tint
                        }),
                        "map {map}: asymmetric block at {:?}",
                        b.min
                    );
                }
            }
        }
    }

    #[test]
    fn classic_objective_routes_remain_connected_around_the_new_destinations() {
        use crate::glam::vec2;
        use crate::map::{NAV_GOALS, NAV_LANES};
        for seed in [0, 17, 0xFE221, u64::MAX] {
            for map in 0..4 {
                let w = World::with_map(seed, map);
                let broken = w.features.iter().fold(0u64, |mask, f| mask | 1u64 << f.bit);
                for mask in [0, broken] {
                    let nav = Nav::build_mask(&w, mask);
                    for goal in 0..NAV_GOALS {
                        let target = if goal < 5 {
                            w.points[goal]
                        } else {
                            w.bases[goal - 5]
                        };
                        for start in w.bases.iter().chain(w.points.iter()) {
                            for lane in 0..NAV_LANES {
                                // Objective centres can contain the original
                                // map's cover/platforms. Deploy onto the nearest
                                // open cell, as the game's spawn selection does.
                                let (cx, cz) = nav.cell(start.x, start.z);
                                let mut p = (-3..=3)
                                    .flat_map(|dx| (-3..=3).map(move |dz| (dx, dz)))
                                    .filter_map(|(dx, dz)| {
                                        let x = cx as isize + dx;
                                        let z = cz as isize + dz;
                                        if x < 0
                                            || z < 0
                                            || x >= nav.w as isize
                                            || z >= nav.h as isize
                                        {
                                            return None;
                                        }
                                        let p = nav.center(x as usize, z as usize);
                                        (!nav.is_blocked(p.x, p.y)).then_some(p)
                                    })
                                    .min_by(|a, b| {
                                        a.distance_squared(vec2(start.x, start.z))
                                            .total_cmp(&b.distance_squared(vec2(start.x, start.z)))
                                    })
                                    .expect("objective must have open deployment ground");
                                for _ in 0..1600 {
                                    if p.distance(vec2(target.x, target.z)) < 6.5 {
                                        break;
                                    }
                                    p += nav.dir(goal, lane, p.x, p.y) * 0.6;
                                }
                                assert!(p.distance(vec2(target.x, target.z)) < 6.5,
                                "map {map}, seed {seed:x}: lane {lane} from {start:?} to goal {goal} stopped at {p:?}");
                            }
                        }
                    }
                }
            }
        }
    }
}
