//! The working valley: inhabited-looking workshops, mine portals and machinery.
//! Large silhouettes use shared collision; motion stays above the walking routes.
use super::slag_cathedral::art::{soft, stencil, wall};
use super::slag_cathedral::{beam, girder, pipe};
use super::hash01;
use crate::gfx::{rgb, shade, Col, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::{landmarks as layout, Block, BlockKind, FeatureKind, World};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const STEEL: Col = [0.24, 0.28, 0.29, 0.0];
const EDGE: Col = [0.47, 0.50, 0.47, 0.0];
const DARK: Col = [0.10, 0.13, 0.14, 0.0];
const RUST: Col = [0.54, 0.27, 0.13, 0.0];
const TIMBER: Col = [0.32, 0.23, 0.14, 0.0];
const PAINT: Col = [0.88, 0.76, 0.47, 0.0];
const SAFETY: Col = [0.94, 0.61, 0.11, 0.0];
const LAMP: Col = [1.0, 0.83, 0.54, 0.85];

fn frame(x: f32, y: f32, z: f32, side: f32) -> Mat4 {
    Mat4::from_translation(vec3(x * side, y, z * side))
        * Mat4::from_rotation_y(if side < 0.0 { PI } else { 0.0 })
}
fn point(g: &Mat4, p: Vec3) -> Vec3 { g.transform_point3(p) }
fn bar(m: &mut Mesh, g: &Mat4, a: Vec3, b: Vec3, width: f32, col: Col) {
    beam(m, point(g, a), point(g, b), width, width, col);
}
fn rod(m: &mut Mesh, g: &Mat4, a: Vec3, b: Vec3, radius: f32, col: Col) {
    pipe(m, point(g, a), point(g, b), radius, col);
}
fn box_at(m: &mut Mesh, g: &Mat4, p: Vec3, h: Vec3, col: Col) { m.obox(g, p, h, col); }

fn breach_local(w: &World, g: &Mat4, s: layout::Site) -> Option<(Vec3, Vec3)> {
    let inv = g.inverse();
    let f = w.features.iter().filter(|f| f.kind == FeatureKind::BreachPanel).find(|f| {
        let p = inv.transform_point3(f.pos);
        p.x.abs() < s.hx + 1.0 && p.z.abs() < s.hz + 1.0
    })?;
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for b in w.blocks.iter().filter(|b| b.dest == Some(f.bit)) {
        for p in [inv.transform_point3(b.min), inv.transform_point3(b.max)] {
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    Some((lo, hi))
}

fn sign(m: &mut Mesh, g: &Mat4, p: Vec3, yaw: f32, text: &str, pixel: f32, col: Col) {
    let f = *g * wall(p, yaw);
    let width = text.len() as f32 * 6.0 * pixel + 0.35;
    m.obox(&f, vec3(0.0, 0.32, -0.045), vec3(width * 0.5, 0.43, 0.04), DARK);
    stencil(m, &f, text, pixel, col);
    for x in [-width * 0.5 + 0.12, width * 0.5 - 0.12] {
        m.obox(&f, vec3(x, 0.32, 0.015), vec3(0.035, 0.035, 0.025), EDGE);
    }
}

fn lamp(m: &mut Mesh, g: &Mat4, p: Vec3) {
    box_at(m, g, p + vec3(0.0, 0.10, 0.0), vec3(0.40, 0.10, 0.25), DARK);
    box_at(m, g, p, vec3(0.31, 0.035, 0.17), LAMP);
    for x in [-0.30, 0.0, 0.30] {
        bar(m, g, p + vec3(x, -0.07, -0.20), p + vec3(x, -0.07, 0.20), 0.025, STEEL);
    }
}

fn roof(m: &mut Mesh, g: &Mat4, s: layout::Site, color: Col) {
    let (x, z, eave, ridge) = (s.hx + 0.55, s.hz + 0.55, s.height + 0.42, s.height + 2.0);
    for k in [-1.0, 1.0] {
        let p = |a, y, b| point(g, vec3(a, y, b));
        m.quad(p(-x, eave, z * k), p(x, eave, z * k), p(x, ridge, 0.0), p(-x, ridge, 0.0), shade(color, if k > 0.0 { 0.9 } else { 1.1 }));
        // The underside is closed as well: standing inside never sees through a roof.
        m.quad(p(-x, ridge - 0.1, 0.0), p(x, ridge - 0.1, 0.0), p(x, eave - 0.1, z * k), p(-x, eave - 0.1, z * k), shade(color, 0.6));
        for i in 0..=(s.hx * 2.0) as i32 {
            let u = -s.hx + i as f32;
            bar(m, g, vec3(u, eave + 0.02, z * k), vec3(u, ridge + 0.02, 0.0), 0.055, shade(color, 1.25));
        }
        bar(m, g, vec3(-x, eave, z * k), vec3(x, eave, z * k), 0.14, DARK);
    }
    bar(m, g, vec3(-x - 0.08, ridge + 0.04, 0.0), vec3(x + 0.08, ridge + 0.04, 0.0), 0.23, EDGE);
    for k in [-1.0, 1.0] {
        let x = (s.hx + 0.03) * k;
        m.tri(point(g, vec3(x, s.height + 0.30, -s.hz)), point(g, vec3(x, ridge, 0.0)), point(g, vec3(x, s.height + 0.30, s.hz)), shade(color, 0.7));
        m.tri(point(g, vec3(x, s.height + 0.30, s.hz)), point(g, vec3(x, ridge, 0.0)), point(g, vec3(x, s.height + 0.30, -s.hz)), shade(color, 0.7));
        // Gable louvres and the bolted corner frame.
        for j in 0..5 {
            box_at(m, g, vec3(x + k * 0.02, s.height + 0.55 + j as f32 * 0.14, 0.0), vec3(0.035, 0.035, 0.65 - j as f32 * 0.08), EDGE);
        }
    }
}

fn workshop(m: &mut Mesh, w: &World, side: f32) {
    let s = layout::WORKSHOPS[w.map as usize];
    let g = frame(s.x, layout::floor(w, s), s.z, side);
    let breach = breach_local(w, &g, s);
    let colors = [RUST, [0.37, 0.45, 0.45, 0.0], [0.29, 0.34, 0.25, 0.0], [0.34, 0.45, 0.48, 0.0]];
    roof(m, &g, s, colors[w.map as usize]);
    for x in [-s.hx, s.hx] {
        for z in [-s.hz, s.hz] {
            box_at(m, &g, vec3(x, s.height * 0.5, z), vec3(0.26, s.height * 0.5, 0.26), if w.map == 2 { TIMBER } else { STEEL });
            // Corner plates stay on the wall rather than filling a doorway.
            for y in [0.6, s.height - 0.5] {
                box_at(m, &g, vec3(x, y, z), vec3(0.31, 0.08, 0.31), EDGE);
            }
        }
    }
    // Wide door frames on each wall, with lintels and a protected lamp.
    for k in [-1.0f32, 1.0] {
        for x in [-1.30, 1.30] {
            box_at(m, &g, vec3(x, 1.23, k * (s.hz + 0.06)), vec3(0.075, 1.23, 0.10), EDGE);
        }
        box_at(m, &g, vec3(0.0, 2.50, k * (s.hz + 0.06)), vec3(1.38, 0.075, 0.10), EDGE);
        for z in [-1.30, 1.30] {
            box_at(m, &g, vec3(k * (s.hx + 0.06), 1.23, z), vec3(0.10, 1.23, 0.075), EDGE);
        }
        box_at(m, &g, vec3(k * (s.hx + 0.06), 2.50, 0.0), vec3(0.10, 0.075, 1.38), EDGE);
        lamp(m, &g, vec3(2.4, 2.88, k * (s.hz + 0.33)));
        let name = ["VALLEY WORKS", "STONE WORKS", "RANGER STATION", "FREIGHT CONTROL"][w.map as usize];
        sign(m, &g, vec3(0.0, s.height - 1.05, k * (s.hz + 0.30)), if k < 0.0 { PI } else { 0.0 }, name, 0.070, PAINT);
        // Rain gutters and drainpipes bolted to the wall.
        rod(m, &g, vec3(-s.hx - 0.34, 0.20, k * s.hz), vec3(-s.hx - 0.34, s.height + 0.35, k * s.hz), 0.085, STEEL);
        for x in [-s.hx * 0.60, s.hx * 0.60] {
            // This bay's window and infill are replaced together by the
            // broad breach panel, so no static frame crosses the opening.
            if breach.is_some_and(|(lo, hi)| x + 0.77 > lo.x && x - 0.77 < hi.x && (lo.z + hi.z) * k > 0.0) { continue; }
            for dx in [-0.71, 0.71] {
                box_at(m, &g, vec3(x + dx, 1.575, k * (s.hz + 0.04)), vec3(0.055, 0.58, 0.055), EDGE);
            }
            for y in [1.0, 2.15] {
                box_at(m, &g, vec3(x, y, k * (s.hz + 0.04)), vec3(0.77, 0.055, 0.055), EDGE);
            }
        }
        let end_window = (s.hz - 0.44) * 0.60;
        for z in [-end_window, end_window] {
            for dz in [-0.71, 0.71] {
                box_at(m, &g, vec3(k * (s.hx + 0.04), 1.575, z + dz), vec3(0.055, 0.58, 0.055), EDGE);
            }
            for y in [1.0, 2.15] {
                box_at(m, &g, vec3(k * (s.hx + 0.04), y, z), vec3(0.055, 0.055, 0.77), EDGE);
            }
        }
    }
    for x in [-s.hx * 0.55, s.hx * 0.55] {
        girder(m, point(&g, vec3(x, s.height - 0.20, -s.hz)), point(&g, vec3(x, s.height - 0.20, s.hz)), 0.20, 0.30, STEEL);
        rod(m, &g, vec3(x, s.height - 0.2, 0.0), vec3(x, s.height - 0.7, 0.0), 0.025, DARK);
        lamp(m, &g, vec3(x, s.height - 0.72, 0.0));
    }
    // Electrical services are on the high wall, leaving the floor entirely open.
    for k in [-1.0, 1.0] {
        rod(m, &g, vec3(-s.hx + 0.42, s.height - 0.7, k * (s.hz - 0.30)), vec3(s.hx - 0.42, s.height - 0.7, k * (s.hz - 0.30)), 0.055, DARK);
        box_at(m, &g, vec3(s.hx - 0.36, 3.2, k * 3.0), vec3(0.16, 0.45, 0.65), STEEL);
    }
}

/// Blasted cave walls and ceilings keep the exact collision silhouette.
/// The generic small-boulder taper would leave a hollow gap under a rock roof.
pub fn owns(w: &World, b: &Block) -> bool {
    if !(1..=2).contains(&w.map) || b.kind != BlockKind::Rock { return false; }
    let s = layout::PASSAGES[w.map as usize];
    let c = (b.min + b.max) * 0.5;
    [1.0, -1.0].into_iter().any(|k| (c.x - s.x * k).abs() < s.hx + 2.0 && (c.z - s.z * k).abs() < s.hz + 5.0)
}
pub fn block(m: &mut Mesh, w: &World, b: &Block) {
    let stone = if b.tint % 2 == 0 { rgb(0.42, 0.40, 0.34) } else { rgb(0.48, 0.45, 0.39) };
    m.cuboid(b.min, b.max, stone);
    if w.map == 2 {
        // The ridge's natural fissure has fractured faces and damp moss,
        // rather than the quarry's straight, cut sedimentary bands.
        let nx = ((b.max.x - b.min.x) / 2.6).ceil() as u32;
        let ny = ((b.max.y - b.min.y) / 1.6).ceil() as u32;
        for k in [-1.0, 1.0] {
            let z = if k > 0.0 { b.max.z + 0.015 } else { b.min.z - 0.015 };
            for ix in 0..nx {
                for iy in 0..ny {
                    let seed = ix * 31 + iy * 97 + b.tint as u32;
                    let dx = (b.max.x - b.min.x) / nx as f32;
                    let dy = (b.max.y - b.min.y) / ny as f32;
                    let x = b.min.x + ix as f32 * dx;
                    let y = b.min.y + iy as f32 * dy;
                    let c = vec3(x + dx * (0.3 + hash01(seed, 33) * 0.4), y + dy * (0.25 + hash01(seed, 34) * 0.5), z + k * (0.05 + hash01(seed, 35) * 0.10));
                    let mossy = hash01(seed, 36) > 0.60;
                    let color = if mossy { rgb(0.29, 0.36, 0.24) } else { shade(stone, 0.73 + hash01(seed, 37) * 0.4) };
                    let corners = [vec3(x, y, z), vec3(x + dx, y, z), vec3(x + dx, y + dy, z), vec3(x, y + dy, z)];
                    for j in 0..4 {
                        let (a, b) = if k > 0.0 { (corners[j], corners[(j + 1) % 4]) } else { (corners[(j + 1) % 4], corners[j]) };
                        m.tri(a, b, c, shade(color, 0.88 + j as f32 * 0.06));
                    }
                }
            }
        }
        // Shallow broken plates on the ceiling preserve standing clearance.
        if b.max.z - b.min.z > 7.0 && b.max.y - b.min.y < 3.0 {
            for ix in 0..nx {
                let x = b.min.x + (ix as f32 + 0.5) * (b.max.x - b.min.x) / nx as f32;
                let z = b.min.z + 1.5 + hash01(ix, 83) * ((b.max.z - b.min.z) - 3.0);
                let c = vec3(x, b.min.y - 0.15, z);
                let a = vec3(x - 1.1, b.min.y - 0.01, z - 1.0);
                let d = vec3(x + 1.1, b.min.y - 0.01, z + 1.0);
                m.tri(a, vec3(d.x, d.y, a.z), c, shade(stone, 0.73));
                m.tri(vec3(a.x, a.y, d.z), c, d, shade(stone, 0.85));
            }
        }
        return;
    }
    for k in 1..4 {
        let y = b.min.y + (b.max.y - b.min.y) * k as f32 * 0.25;
        m.cuboid(vec3(b.min.x - 0.015, y, b.min.z - 0.015), vec3(b.max.x + 0.015, y + 0.035, b.max.z + 0.015), shade(stone, 0.62));
    }
}

fn passage(m: &mut Mesh, w: &World, side: f32) {
    let s = layout::PASSAGES[w.map as usize];
    let g = frame(s.x, layout::floor(w, s), s.z, side);
    let timber = if w.map == 1 || w.map == 2 { TIMBER } else { STEEL };
    let title = ["SERVICE TUNNEL", "MINE 04", "FOX CAVE", "TRANSIT 07"][w.map as usize];
    for k in [-1.0, 1.0] {
        let x = (s.hx + 0.12) * k;
        for z in [-s.hz + 0.24, s.hz - 0.24] {
            box_at(m, &g, vec3(x, s.height * 0.5, z), vec3(0.18, s.height * 0.5, 0.18), timber);
        }
        box_at(m, &g, vec3(x, s.height - 0.12, 0.0), vec3(0.20, 0.20, s.hz), timber);
        sign(m, &g, vec3(x + k * 0.11, s.height + 0.45, 0.0), k * FRAC_PI_2, title, 0.095, PAINT);
        lamp(m, &g, vec3(x, s.height - 0.48, 0.0));
        // Number boards beside each opening make either entrance recognisable.
        sign(m, &g, vec3(x + k * 0.12, 1.25, s.hz + 0.42), k * FRAC_PI_2, if side > 0.0 { "WEST" } else { "EAST" }, 0.055, SAFETY);
    }
    for i in 0..=4 {
        let x = -s.hx + 0.35 + (s.hx * 2.0 - 0.7) * i as f32 / 4.0;
        for z in [-s.hz + 0.22, s.hz - 0.22] {
            box_at(m, &g, vec3(x, s.height * 0.5, z), vec3(0.16, s.height * 0.5, 0.16), timber);
        }
        box_at(m, &g, vec3(x, s.height - 0.12, 0.0), vec3(0.17, 0.12, s.hz), timber);
        if i % 2 == 0 { lamp(m, &g, vec3(x, s.height - 0.33, 0.0)); }
    }
    for z in [-s.hz + 0.12, s.hz - 0.12] {
        rod(m, &g, vec3(-s.hx, s.height - 0.62, z), vec3(s.hx, s.height - 0.62, z), 0.035, DARK);
    }
    if w.map == 1 {
        // Rails and sleepers are painted-height detail, never a movement obstacle.
        for z in [-0.72, 0.72] {
            box_at(m, &g, vec3(0.0, 0.045, z), vec3(s.hx + 0.2, 0.025, 0.055), EDGE);
        }
        for i in -(s.hx as i32)..=s.hx as i32 {
            box_at(m, &g, vec3(i as f32, 0.025, 0.0), vec3(0.11, 0.02, 1.0), TIMBER);
        }
    }
    if w.map == 2 {
        // Bare stone and damp earth beneath the overhang distinguish the
        // natural ridge fissure from the grassy forest outside it.
        let nx = (s.hx * 2.0 / 2.5).ceil() as u32;
        for ix in 0..nx {
            for iz in 0..4u32 {
                let (dx, dz) = (s.hx * 2.0 / nx as f32, s.hz * 0.5);
                let (x, z) = (-s.hx + ix as f32 * dx, -s.hz + iz as f32 * dz);
                let c = if hash01(ix, iz + 300) > 0.66 { rgb(0.28, 0.32, 0.20) } else { rgb(0.35, 0.32, 0.26) };
                let p = |a, b| point(&g, vec3(a, 0.022, b));
                m.tri(p(x, z), p(x, z + dz), p(x + dx, z), shade(c, 0.9 + hash01(ix, iz + 302) * 0.2));
                m.tri(p(x + dx, z + dz), p(x + dx, z), p(x, z + dz), shade(c, 0.9 + hash01(ix, iz + 304) * 0.2));
            }
        }
    }
    if w.map == 3 {
        for i in -(s.hx as i32)..=s.hx as i32 {
            for k in [-1.0, 1.0] {
                box_at(m, &g, vec3(i as f32, s.height * 0.5, k * (s.hz + 0.03)), vec3(0.065, s.height * 0.5, 0.04), EDGE);
            }
        }
    }
}

fn foundry(m: &mut Mesh, w: &World, side: f32) {
    let s = layout::WORKSHOPS[0];
    let g = frame(s.x, layout::floor(w, s), s.z, side);
    // Flanged process pipes connect the workshop roof with its great stack.
    rod(m, &g, vec3(-6.0, s.height + 1.0, -1.0), vec3(-6.0, s.height + 1.0, -9.0), 0.33, RUST);
    rod(m, &g, vec3(-6.0, s.height + 1.0, -9.0), vec3(-6.0, 12.0, -9.0), 0.33, RUST);
    for stack in [-6.0, 6.0] {
        for y in [7.0, 11.0, 15.0, 19.0, 21.0] {
            box_at(m, &g, vec3(stack, y, -9.0), vec3(1.50, 0.10, 1.50), EDGE);
        }
        for y in 0..11 {
            let y = 2.0 + y as f32 * 1.7;
            bar(m, &g, vec3(stack - 0.55, y, -10.25), vec3(stack + 0.55, y, -10.25), 0.05, DARK);
        }
        for x in [stack - 0.55, stack + 0.55] {
            bar(m, &g, vec3(x, 0.6, -10.25), vec3(x, 21.5, -10.25), 0.07, STEEL);
        }
        box_at(m, &g, vec3(stack, 21.4, -9.0), vec3(1.44, 0.6, 1.44), RUST);
    }
    sign(m, &g, vec3(-6.0, 13.8, -10.35), PI, "VALLEY", 0.09, PAINT);
    // A pulley bed mounted to the stack, high above the surrounding apron.
    for x in [-2.3, 2.3] {
        bar(m, &g, vec3(x - 6.0, 5.3, -10.7), vec3(x - 6.0, 7.5, -10.7), 0.18, STEEL);
    }
}

fn headframe(m: &mut Mesh, w: &World, side: f32) {
    let s = layout::PASSAGES[1];
    let g = frame(s.x - 8.0, layout::floor(w, s), s.z + 9.0, side);
    for k in [-1.0, 1.0] {
        for y in [3.5, 7.5, 11.5, 15.5] {
            bar(m, &g, vec3(-2.2, y - 2.0, k * 2.2), vec3(2.2, y + 2.0, k * 2.2), 0.17, RUST);
            bar(m, &g, vec3(2.2, y - 2.0, k * 2.2), vec3(-2.2, y + 2.0, k * 2.2), 0.17, RUST);
        }
        for y in [4.0, 8.0, 12.0, 16.0] {
            bar(m, &g, vec3(-2.2, y, k * 2.2), vec3(2.2, y, k * 2.2), 0.25, SAFETY);
        }
    }
    for k in [-1.0, 1.0] {
        for x in [-2.7, 2.7] {
            bar(m, &g, vec3(x, 18.2, k * 2.7), vec3(x, 19.3, k * 2.7), 0.06, EDGE);
        }
        bar(m, &g, vec3(-2.7, 19.3, k * 2.7), vec3(2.7, 19.3, k * 2.7), 0.06, EDGE);
        box_at(m, &g, vec3(0.0, 18.8, k * 0.55), vec3(0.32, 0.6, 0.16), STEEL);
    }
    sign(m, &g, vec3(0.0, 13.2, -2.35), PI, "PIT 04", 0.11, PAINT);
    for z in [-0.72, 0.72] {
        rod(m, &g, vec3(-0.8, 7.0, z), vec3(-0.8, 18.5, z), 0.035, DARK);
    }
}

fn ridge(m: &mut Mesh, w: &World, side: f32) {
    let s = layout::WORKSHOPS[2];
    let g = frame(s.x, layout::floor(w, s), s.z, side);
    let breach = breach_local(w, &g, s);
    // A ranger station's wood siding, high louvres and roof ridge weather vane.
    for k in [-1.0, 1.0] {
        for j in 0..14 {
            let y = 0.25 + j as f32 * 0.27;
            if y > 0.95 && y < 2.20 { continue; }
            for x in [-1.0, 1.0] {
                let (c, h) = (x * (s.hx * 0.5 + 0.9), (s.hx - 1.8) * 0.5);
                let runs = if let Some((lo, hi)) = breach.filter(|(lo, hi)| (lo.z + hi.z) * k > 0.0 && y + 0.025 > lo.y && y - 0.025 < hi.y) {
                    [(c - h, lo.x.clamp(c - h, c + h)), (hi.x.clamp(c - h, c + h), c + h)]
                } else { [(c - h, c + h), (0.0, 0.0)] };
                for (a, b) in runs {
                    if b > a {
                        box_at(m, &g, vec3((a + b) * 0.5, y, k * (s.hz + 0.10)), vec3((b - a) * 0.5, 0.025, 0.025), TIMBER);
                    }
                }
            }
        }
    }
    // Tower bracing is overhead and attached to the shared mast.
    let (mx, mz) = (s.x + s.hx + 4.0, s.z + s.hz + 4.0);
    let mast = frame(mx, w.height(mx, mz), mz, side);
    for y in [6.0, 11.0, 16.0, 21.0] {
        box_at(m, &mast, vec3(0.0, y, 0.0), vec3(0.61, 0.10, 0.61), EDGE);
    }
    rod(m, &mast, vec3(0.0, 21.0, 0.0), vec3(0.0, 21.0, -1.6), 0.38, STEEL);
    // Open mesh panels on top of the station, rather than glass hiding targets.
    for k in [-1.0, 1.0] {
        bar(m, &g, vec3(-4.0, s.height + 0.4, k * 2.2), vec3(4.0, s.height + 0.4, k * 2.2), 0.06, TIMBER);
    }
}

fn crane(m: &mut Mesh, w: &World) {
    let y = w.height(0.0, 0.0);
    let g = frame(0.0, y, 0.0, 1.0);
    let (hx, hz, top) = (layout::CRANE_HALF_X, layout::CRANE_HALF_Z, layout::CRANE_HEIGHT);
    // A rail-mounted shipyard gantry: repeated bracing gives it scale at a distance.
    for x in [-hx, hx] {
        for z in [-hz, hz] {
            for sy in [2.0, 5.0, 8.0, 11.0, 14.0, 17.0, 20.0, 23.0] {
                for k in [-1.0, 1.0] {
                    bar(m, &g, vec3(x - 0.88, sy - 1.0, z + k * 1.49), vec3(x + 0.88, sy + 1.0, z + k * 1.49), 0.13, SAFETY);
                    bar(m, &g, vec3(x + k * 0.99, sy - 1.0, z - 1.35), vec3(x + k * 0.99, sy + 1.0, z + 1.35), 0.13, RUST);
                }
            }
            box_at(m, &g, vec3(x, 1.0, z), vec3(1.25, 0.36, 1.8), DARK);
            for dz in [-1.1, 1.1] {
                let f = g * Mat4::from_translation(vec3(x, 0.65, z + dz)) * Mat4::from_rotation_y(FRAC_PI_2);
                m.tube(&f, Vec3::ZERO, 0.59, 0.22, 0.80, 10, EDGE);
            }
            for h in [4.0, 13.0, 22.0] {
                box_at(m, &g, vec3(x, h, z), vec3(0.97, 0.20, 0.97), SAFETY);
            }
        }
        for sy in [top - 0.4, top + 1.2] {
            bar(m, &g, vec3(x, sy, -hz - 3.0), vec3(x, sy, hz + 3.0), 0.28, SAFETY);
        }
        for i in -7..7 {
            let z = i as f32 * 4.0;
            bar(m, &g, vec3(x, top - 0.4, z), vec3(x, top + 1.2, z + 4.0), 0.16, SAFETY);
        }
        // Runway light fixtures remain well above containers and players.
        for z in [-20.0, 0.0, 20.0] { lamp(m, &g, vec3(x, top - 0.7, z)); }
    }
    for z in [-1.2, 1.2] {
        bar(m, &g, vec3(-hx - 2.0, top + 0.25, z), vec3(hx + 2.0, top + 0.25, z), 0.3, SAFETY);
        bar(m, &g, vec3(-hx - 2.0, top + 2.0, z), vec3(hx + 2.0, top + 2.0, z), 0.3, SAFETY);
        for i in -7..7 {
            let x = i as f32 * 4.0;
            bar(m, &g, vec3(x, top + 0.25, z), vec3(x + 4.0, top + 2.0, z), 0.16, SAFETY);
        }
    }
    // High gantry lettering reads from both halves of the yard.
    for k in [-1.0, 1.0] {
        sign(m, &g, vec3(0.0, top + 0.55, k * 1.4), if k < 0.0 { PI } else { 0.0 }, "FERRITE FREIGHT", 0.13, PAINT);
    }
    // Narrow gauge rails are flush with the concrete apron.
    for x in [-hx, hx] {
        box_at(m, &g, vec3(x, 0.045, 0.0), vec3(0.10, 0.025, hz + 6.0), EDGE);
    }
}

pub fn build(m: &mut Mesh, w: &World) {
    if w.map > 3 { return; }
    for side in [1.0, -1.0] {
        workshop(m, w, side);
        passage(m, w, side);
        match w.map {
            0 => foundry(m, w, side),
            1 => headframe(m, w, side),
            2 => ridge(m, w, side),
            _ => {}
        }
    }
    if w.map == 3 { crane(m, w); }
}

fn wheel(m: &mut Mesh, g: &Mat4, center: Vec3, radius: f32, angle: f32, col: Col) {
    let f = *g * Mat4::from_translation(center) * Mat4::from_rotation_z(angle);
    m.tube(&f, Vec3::ZERO, radius, radius * 0.81, 0.12, 18, col);
    m.tube(&f, Vec3::ZERO, radius * 0.18, 0.0, 0.22, 10, EDGE);
    for i in 0..8 {
        let a = i as f32 * TAU / 8.0;
        bar(m, &f, Vec3::ZERO, vec3(a.cos() * radius * 0.88, a.sin() * radius * 0.88, 0.0), 0.13, col);
    }
}

fn smoke(alpha: &mut Mesh, p: Vec3, time: f32, seed: u32, dust: bool) {
    for i in 0..12 {
        let age = (time / 9.0 + i as f32 / 12.0).rem_euclid(1.0);
        let fade = (age * (1.0 - age) * 5.0).min(1.0);
        let drift = vec3(age * 8.0, age * 11.0, age * 2.5);
        let wobble = vec3((time * 0.3 + i as f32).sin(), 0.0, (time * 0.2 + i as f32).cos()) * age;
        let color = if dust { [0.66, 0.58, 0.44, fade * 0.16] } else { [0.48, 0.47, 0.43, fade * 0.24] };
        soft(alpha, p + drift + wobble, 0.7 + age * 2.0 + hash01(i, seed) * 0.5, 0.8 + age * 1.4, color);
    }
}

fn foundry_motion(m: &mut Mesh, alpha: &mut Mesh, w: &World, time: f32, side: f32, running: bool) {
    let s = layout::WORKSHOPS[0];
    let g = frame(s.x, layout::floor(w, s), s.z, side);
    wheel(m, &g, vec3(-6.0, 7.8, -10.8), 2.1, -time * 0.45, RUST);
    if running {
        for (i, x) in [-6.0, 6.0].into_iter().enumerate() {
            smoke(alpha, point(&g, vec3(x, 22.2, -9.0)), time + i as f32 * 1.3, 70 + i as u32, false);
        }
    }
}

fn quarry_motion(m: &mut Mesh, alpha: &mut Mesh, w: &World, time: f32, side: f32, running: bool) {
    let s = layout::PASSAGES[1];
    let g = frame(s.x - 8.0, layout::floor(w, s), s.z + 9.0, side);
    wheel(m, &g, vec3(0.0, 19.5, 0.0), 1.65, time * 0.42, SAFETY);
    // The ore skip travels within the headframe, always above the walkable apron.
    let hoist = 10.0 + 3.0 * (time * 0.16).sin();
    box_at(m, &g, vec3(-0.8, hoist, 0.0), vec3(1.05, 0.85, 1.10), RUST);
    box_at(m, &g, vec3(-0.8, hoist + 0.88, 0.0), vec3(1.09, 0.08, 1.14), EDGE);
    for i in 0..5 {
        let q = vec3(-0.8 + (hash01(i, 77) - 0.5) * 1.2, hoist + 0.88, (hash01(i, 78) - 0.5) * 1.25);
        super::octa(m, point(&g, q), vec3(0.42, 0.27, 0.39), i as f32, rgb(0.51, 0.47, 0.37));
    }
    for z in [-0.72, 0.72] {
        rod(m, &g, vec3(-0.8, hoist + 0.8, z), vec3(-0.8, 19.5, z), 0.035, DARK);
    }
    if running { smoke(alpha, point(&g, vec3(-0.8, hoist + 1.0, 0.0)), time, 78, true); }
}

fn ridge_motion(m: &mut Mesh, w: &World, time: f32, side: f32) {
    let s = layout::WORKSHOPS[2];
    let g = frame(s.x, layout::floor(w, s), s.z, side);
    let (mx, mz) = (s.x + s.hx + 4.0, s.z + s.hz + 4.0);
    let rotor = frame(mx, w.height(mx, mz), mz, side) * Mat4::from_translation(vec3(0.0, 21.0, -1.6)) * Mat4::from_rotation_z(time * 0.52);
    m.tube(&rotor, Vec3::ZERO, 0.42, 0.0, 0.35, 12, EDGE);
    for i in 0..3 {
        let f = rotor * Mat4::from_rotation_z(i as f32 * TAU / 3.0);
        let p = |x, y| point(&f, vec3(x, y, 0.0));
        m.quad(p(-0.16, 0.25), p(0.32, 0.9), p(0.20, 4.5), p(-0.11, 4.35), PAINT);
        m.quad(p(-0.11, 4.35), p(0.20, 4.5), p(0.32, 0.9), p(-0.16, 0.25), shade(PAINT, 0.75));
    }
    let vane = g * Mat4::from_translation(vec3(0.0, s.height + 3.0, 0.0)) * Mat4::from_rotation_y(0.3 + (time * 0.19).sin() * 0.24);
    rod(m, &vane, vec3(0.0, -1.0, 0.0), Vec3::ZERO, 0.035, STEEL);
    bar(m, &vane, vec3(-1.1, 0.0, 0.0), vec3(1.2, 0.0, 0.0), 0.05, STEEL);
    m.tri(point(&vane, vec3(-1.1, -0.3, 0.0)), point(&vane, vec3(-0.4, 0.0, 0.0)), point(&vane, vec3(-1.1, 0.3, 0.0)), RUST);
    m.tri(point(&vane, vec3(-1.1, 0.3, 0.0)), point(&vane, vec3(-0.4, 0.0, 0.0)), point(&vane, vec3(-1.1, -0.3, 0.0)), RUST);
}

fn crane_motion(m: &mut Mesh, add: &mut Mesh, w: &World, time: f32, running: bool) {
    let top = layout::CRANE_HEIGHT;
    let x = (time * 0.065).sin() * (layout::CRANE_HALF_X - 5.0);
    let g = frame(x, w.height(0.0, 0.0), 0.0, 1.0);
    box_at(m, &g, vec3(0.0, top + 0.35, 0.0), vec3(2.2, 0.45, 1.8), RUST);
    for z in [-1.4, 1.4] {
        wheel(m, &g, vec3(0.0, top + 0.8, z), 0.65, time * 0.25, STEEL);
    }
    // An operator cab and glass above head height travel with the trolley.
    box_at(m, &g, vec3(2.2, top - 1.6, 0.0), vec3(0.85, 1.30, 1.1), SAFETY);
    for k in [-1.0, 1.0] {
        box_at(m, &g, vec3(2.2, top - 1.25, k * 1.12), vec3(0.68, 0.62, 0.025), [0.16, 0.32, 0.34, 0.2]);
    }
    let drop = 9.0 + 2.5 * (time * 0.11).sin();
    let sway = (time * 0.23).sin() * 0.14;
    let y = top - drop;
    for dx in [-2.6, 2.6] {
        for z in [-1.05, 1.05] {
            rod(m, &g, vec3(dx * 0.55, top - 0.1, z), vec3(dx + sway, y, z), 0.032, DARK);
        }
    }
    let spreader = g * Mat4::from_translation(vec3(sway, y, 0.0));
    for z in [-1.10, 1.10] {
        bar(m, &spreader, vec3(-3.1, 0.0, z), vec3(3.1, 0.0, z), 0.22, SAFETY);
    }
    for dx in [-2.8, 0.0, 2.8] {
        bar(m, &spreader, vec3(dx, 0.0, -1.1), vec3(dx, 0.0, 1.1), 0.24, STEEL);
        for z in [-1.1, 1.1] {
            box_at(m, &spreader, vec3(dx, -0.25, z), vec3(0.12, 0.28, 0.12), RUST);
        }
    }
    let blink = ((time * 2.0).sin() * 0.5 + 0.5).powi(3);
    if running {
        for z in [-1.75, 1.75] {
            soft(add, point(&g, vec3(0.0, top + 1.1, z)), 0.42, 0.34, [1.0, 0.53, 0.08, blink * 0.8]);
        }
    }
}

fn powered(w: &World, dmask: u64, side: f32) -> bool {
    !w.features.iter().any(|f| f.kind == FeatureKind::Machinery && f.pos.x * side < 0.0 && dmask & (1u64 << f.bit) != 0)
}

/// Breakable components and every attached marking share one lifetime.
/// Static world meshes never contain these surfaces or their embellishments.
pub fn features(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, dmask: u64, time: f32) {
    for f in &w.features {
        let broken = dmask & (1u64 << f.bit) != 0;
        if broken {
            if f.kind == FeatureKind::Machinery {
                // A paper-thin scorch mark identifies the failed supply.
                // No standing debris implies cover after collision is gone.
                let y = w.height(f.pos.x, f.pos.z) + 0.025;
                let p = vec3(f.pos.x, y, f.pos.z);
                m.quad(p + vec3(-0.8, 0.0, -0.6), p + vec3(-0.8, 0.0, 0.6), p + vec3(0.8, 0.0, 0.6), p + vec3(0.8, 0.0, -0.6), [0.17, 0.15, 0.12, 0.0]);
                for i in 0..3 {
                    let age = (time * 0.15 + i as f32 / 3.0).rem_euclid(1.0);
                    soft(alpha, p + vec3(age * 0.4, 0.3 + age * 1.2, 0.0), 0.22 + age * 0.28, 0.35, [0.22, 0.23, 0.22, (1.0 - age) * 0.10]);
                }
            }
            continue;
        }
        for b in w.blocks.iter().filter(|b| b.dest == Some(f.bit)) {
            let size = b.max - b.min;
            let c = (b.min + b.max) * 0.5;
            match f.kind {
                FeatureKind::BreachPanel => {
                    // A replaceable, visibly scored panel below the window.
                    m.cuboid(b.min, b.max, [0.49, 0.30, 0.17, 0.0]);
                    for side in [-1.0, 1.0] {
                        let z = if side < 0.0 { b.min.z - 0.025 } else { b.max.z + 0.025 };
                        let g = wall(vec3(c.x, b.min.y, z), if side < 0.0 { PI } else { 0.0 });
                        for x in [-size.x * 0.45, size.x * 0.45] {
                            box_at(m, &g, vec3(x, size.y * 0.5, 0.0), vec3(0.045, size.y * 0.5, 0.025), SAFETY);
                        }
                        for y in [size.y * 0.10, size.y * 0.90] {
                            box_at(m, &g, vec3(0.0, y, 0.0), vec3(size.x * 0.48, 0.045, 0.025), SAFETY);
                        }
                        for j in 0..4 {
                            let x = -size.x * 0.3 + j as f32 * size.x * 0.2;
                            box_at(m, &g, vec3(x, size.y * 0.5, 0.008), vec3(0.012, size.y * 0.30, 0.015), DARK);
                        }
                        sign(m, &g, vec3(0.0, size.y * 0.35, 0.045), 0.0, "BREACH", 0.025, PAINT);
                    }
                }
                FeatureKind::Walkway => {
                    let color = if b.kind == BlockKind::Step { [0.47, 0.43, 0.34, 0.0] } else { STEEL };
                    m.cuboid(b.min, b.max, color);
                    if size.y < 0.4 && size.x > 5.0 {
                        for z in [b.min.z + 0.04, b.max.z - 0.04] {
                            m.cuboid(vec3(b.min.x, b.max.y + 0.01, z - 0.035), vec3(b.max.x, b.max.y + 0.035, z + 0.035), SAFETY);
                        }
                        for j in 0..=(size.x / 0.65) as i32 {
                            let x = b.min.x + j as f32 * 0.65;
                            m.cuboid(vec3(x, b.max.y + 0.012, b.min.z), vec3((x + 0.025).min(b.max.x), b.max.y + 0.025, b.max.z), DARK);
                        }
                    } else if size.y > 1.0 && size.x < 0.6 && size.z < 0.6 {
                        // Repeated painted bands identify the weak support set.
                        for t in [0.30, 0.60, 0.90] {
                            let y = b.min.y + size.y * t;
                            m.cuboid(vec3(b.min.x - 0.025, y, b.min.z - 0.025), vec3(b.max.x + 0.025, y + 0.09, b.max.z + 0.025), SAFETY);
                        }
                    }
                }
                FeatureKind::Machinery => {
                    m.cuboid(b.min, b.max, [0.34, 0.40, 0.37, 0.0]);
                    for side in [-1.0, 1.0] {
                        let z = if side < 0.0 { b.min.z - 0.025 } else { b.max.z + 0.025 };
                        let g = wall(vec3(c.x, b.min.y, z), if side < 0.0 { PI } else { 0.0 });
                        box_at(m, &g, vec3(0.0, size.y * 0.5, 0.0), vec3(size.x * 0.44, size.y * 0.43, 0.035), DARK);
                        for j in 0..4 {
                            box_at(m, &g, vec3(-size.x * 0.18, 0.28 + j as f32 * 0.10, 0.04), vec3(size.x * 0.19, 0.018, 0.015), EDGE);
                        }
                        sign(m, &g, vec3(0.0, size.y * 0.56, 0.075), 0.0, "POWER", 0.026, PAINT);
                        box_at(m, &g, vec3(size.x * 0.25, 0.38, 0.08), vec3(0.065, 0.065, 0.035), [0.31, 0.92, 0.48, 0.75]);
                        soft(add, point(&g, vec3(size.x * 0.25, 0.38, 0.13)), 0.10, 0.10, [0.3, 1.0, 0.5, 0.18]);
                    }
                    m.cuboid(vec3(b.min.x - 0.02, b.max.y - 0.12, b.min.z - 0.02), b.max + vec3(0.02, 0.025, 0.02), SAFETY);
                }
            }
        }
        if f.kind == FeatureKind::Walkway {
            let side = if f.pos.x < 0.0 { 1.0 } else { -1.0 };
            let g = frame(f.pos.x * side, f.pos.y, f.pos.z * side, side);
            sign(m, &g, vec3(0.0, -0.23, -0.23), PI, "WEAK SUPPORT", 0.033, SAFETY);
        }
    }
}

pub fn atmosphere(m: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, time: f32, dmask: u64) {
    if w.map > 3 { return; }
    for side in [1.0, -1.0] {
        let running = powered(w, dmask, side);
        let machine_time = if running { time } else { 0.0 };
        match w.map {
            0 => foundry_motion(m, alpha, w, machine_time, side, running),
            1 => quarry_motion(m, alpha, w, machine_time, side, running),
            2 => ridge_motion(m, w, machine_time, side),
            _ => {}
        }
        // A tiny warm pool at each passage entrance makes these new routes readable.
        let s = layout::PASSAGES[w.map as usize];
        let g = frame(s.x, layout::floor(w, s), s.z, side);
        for k in [-1.0, 1.0] {
            soft(add, point(&g, vec3(s.hx * k, s.height - 0.55, 0.0)), 0.44, 0.22, [1.0, 0.77, 0.38, 0.19]);
        }
    }
    if w.map == 3 {
        let running = powered(w, dmask, 1.0) && powered(w, dmask, -1.0);
        crane_motion(m, add, w, if running { time } else { 0.0 }, running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(mesh: Mesh, limit: usize) {
        assert!(mesh.triangles() < limit, "landmark mesh exceeds {limit} triangles");
        for (v, indices) in mesh.finish() {
            assert!(indices.iter().all(|&i| (i as usize) < v.len()));
            for v in v {
                assert!(v.pos.iter().chain(v.nrm.iter()).chain(v.col.iter()).all(|x| x.is_finite()));
            }
        }
    }
    #[test]
    fn neglected_maps_have_finite_detailed_landmarks_with_bounded_motion() {
        for id in 0..4 {
            let w = World::with_map(0xFE221, id);
            let mut m = Mesh::default();
            build(&mut m, &w);
            assert!(m.triangles() > 2_000, "map {id} should have a substantial authored art pass");
            valid(m, 25_000);
            for t in [0.0, 3.5, 91.0, 600.0] {
                let (mut m, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
                atmosphere(&mut m, &mut alpha, &mut add, &w, t, 0);
                assert!(m.triangles() > 0, "map {id} has no moving machinery");
                valid(m, 2_000);
                valid(alpha, 1_000);
                valid(add, 500);
            }
        }
    }
    #[test]
    fn crane_spreader_stays_well_above_the_fighting() {
        let w = World::with_map(0xFE221, 3);
        let ground = w.height(0.0, 0.0);
        let mut first = Vec::new();
        for t in [0.0, 10.0, 60.0, 200.0] {
            let (mut m, mut add) = (Mesh::default(), Mesh::default());
            crane_motion(&mut m, &mut add, &w, t, true);
            let positions: Vec<_> = m.v.iter().map(|v| v.pos).collect();
            assert!(positions.iter().all(|p| p[1] > ground + 12.0));
            if first.is_empty() { first = positions; } else { assert_ne!(positions, first); }
        }
    }
    #[test]
    fn destroyed_features_leave_no_standing_surface_or_decoration() {
        for id in 0..4 {
            let w = World::with_map(0xFE221, id);
            let all = w.features.iter().fold(0u64, |mask, f| mask | 1u64 << f.bit);
            let (mut intact, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
            features(&mut intact, &mut alpha, &mut add, &w, 0, 0.0);
            assert!(intact.triangles() > 100);
            valid(intact, 8_000);
            let (mut broken, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
            features(&mut broken, &mut alpha, &mut add, &w, all, 0.0);
            // Two flat scorch marks are the sole opaque remains.
            assert_eq!(broken.triangles(), 4);
            for v in &broken.v {
                let p = Vec3::from_array(v.pos);
                assert!((p.y - w.height(p.x, p.z)).abs() < 0.15);
            }
            assert_eq!(add.triangles(), 0, "failed supply lights must extinguish");
        }
    }
    #[test]
    fn disabled_machines_hold_a_deterministic_parked_pose() {
        for id in 0..4 {
            let w = World::with_map(0xFE221, id);
            let mask = w.features.iter().filter(|f| f.kind == FeatureKind::Machinery).fold(0, |mask, f| mask | 1u64 << f.bit);
            let mut poses = Vec::new();
            for time in [3.0, 90.0] {
                let (mut m, mut alpha, mut add) = (Mesh::default(), Mesh::default(), Mesh::default());
                atmosphere(&mut m, &mut alpha, &mut add, &w, time, mask);
                assert_eq!(alpha.triangles(), 0, "stopped machinery must not produce operating smoke or dust");
                poses.push(m.v.iter().map(|v| v.pos).collect::<Vec<_>>());
            }
            assert_eq!(poses[0], poses[1], "map {id} machinery moved after power supply failed");
        }
    }
    #[test]
    fn static_workshop_trim_never_covers_the_breachable_opening() {
        fn ray_triangle(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
            let (e1, e2) = (b - a, c - a);
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 0.00001 { return false; }
            let s = o - a;
            let u = s.dot(h) / det;
            if !(0.0..=1.0).contains(&u) { return false; }
            let q = s.cross(e1);
            let v = d.dot(q) / det;
            if v < 0.0 || u + v > 1.0 { return false; }
            let t = e2.dot(q) / det;
            t > 0.0 && t < 2.0
        }
        for id in 0..4 {
            let w = World::with_map(0xFE221, id);
            let mut mesh = Mesh::default();
            build(&mut mesh, &w);
            let chunks = mesh.finish();
            for f in w.features.iter().filter(|f| f.kind == FeatureKind::BreachPanel) {
                for y in [0.25, 0.85, 1.4, 2.15, 2.4] {
                  for dx in [-1.15, -0.7, 0.0, 0.7, 1.15] {
                    let o = vec3(f.pos.x + dx, w.height(f.pos.x, f.pos.z) + y, f.pos.z - 1.0);
                    for (vertices, indices) in &chunks {
                        for tri in indices.chunks_exact(3) {
                            assert!(!ray_triangle(o, Vec3::Z, Vec3::from_array(vertices[tri[0] as usize].pos), Vec3::from_array(vertices[tri[1] as usize].pos), Vec3::from_array(vertices[tri[2] as usize].pos)),
                                "map {id} static art covers the breach panel at {:?}", f.pos);
                        }
                    }
                  }
                }
            }
        }
    }
}
