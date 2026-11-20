//! Foliage, weather and light: what makes each map a place.
//!
//! Three things live here. `Mood` is a map's time of day: where the sun
//! stands, what colour it and the haze are, and the sky that goes with it.
//! `grow` plants undergrowth into the static world mesh: bushes, tall grass,
//! flowers, ferns, reeds, ivy, moss, deadwood and the forest beyond the
//! fence. `ambience` is what moves in the air each frame: falling leaves,
//! motes in the light, ground mist, blown dust and birds.
//!
//! None of it collides and none of it is known to the server. Anything tall
//! enough to hide behind is planted at a spot and at its mirror image, so
//! both teams get the same cover.

use super::{ground_col, hash01, mix3, pyramid, smooth, taper, LEAF};
use crate::gfx::{rgb, shade, Col, Mesh, Tone};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::*;
use ferrite_shared::rng::Rng;
use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------- mood ----

pub struct Mood {
    pub fog: [f32; 4],
    pub sun: Vec3,
    pub tone: Tone,
    zenith: [f32; 3],
    /// Colour the horizon takes on the sun's side.
    glow: [f32; 3],
    /// How clearly the sun's disc shows (0 = lost behind cloud).
    disc: f32,
    clouds: u32,
    cloud: [f32; 3],
    belly: [f32; 3],
    /// Tint of the far ranges, before the haze takes them.
    range: [f32; 3],
    /// Ground mist: strength, and how far above the lowest objective it lies.
    mist: (f32, f32),
    motes: [f32; 4],
    dust: f32,
    leaves: bool,
    birds: u32,
}

static MOODS: [Mood; 5] = [
    // FOUNDRY VALLEY: a gold afternoon late in the year.
    Mood {
        fog: [0.87, 0.76, 0.61, 0.0047],
        sun: Vec3::new(0.56, 0.46, 0.40),
        tone: [[1.16, 0.96, 0.70], [0.44, 0.55, 0.74], [0.31, 0.27, 0.33]],
        zenith: [0.27, 0.50, 0.72],
        glow: [1.0, 0.80, 0.52],
        disc: 1.0,
        clouds: 15,
        cloud: [1.0, 0.95, 0.86],
        belly: [0.93, 0.80, 0.68],
        range: [0.50, 0.50, 0.58],
        mist: (0.35, 1.5),
        motes: [1.0, 0.86, 0.55, 0.8],
        dust: 0.0,
        leaves: true,
        birds: 3,
    },
    // QUARRY: white noon, dust standing in the air.
    Mood {
        fog: [0.90, 0.84, 0.72, 0.0054],
        sun: Vec3::new(0.34, 0.82, 0.30),
        tone: [[1.14, 1.05, 0.90], [0.50, 0.58, 0.70], [0.34, 0.30, 0.30]],
        zenith: [0.36, 0.60, 0.82],
        glow: [1.0, 0.93, 0.78],
        disc: 1.0,
        clouds: 6,
        cloud: [1.0, 0.98, 0.94],
        belly: [0.95, 0.90, 0.82],
        range: [0.62, 0.56, 0.52],
        mist: (0.0, 0.0),
        motes: [1.0, 0.92, 0.72, 0.7],
        dust: 1.0,
        leaves: false,
        birds: 2,
    },
    // RIDGELINE: early, cold, mist still lying under the pines.
    Mood {
        fog: [0.69, 0.78, 0.81, 0.0066],
        sun: Vec3::new(0.62, 0.38, -0.34),
        tone: [[1.08, 0.97, 0.80], [0.42, 0.56, 0.70], [0.25, 0.29, 0.32]],
        zenith: [0.30, 0.50, 0.70],
        glow: [0.98, 0.90, 0.72],
        disc: 0.9,
        clouds: 10,
        cloud: [0.96, 0.97, 0.98],
        belly: [0.80, 0.85, 0.90],
        range: [0.36, 0.48, 0.56],
        mist: (1.0, 4.0),
        motes: [0.85, 0.95, 1.0, 0.6],
        dust: 0.0,
        leaves: true,
        birds: 3,
    },
    // DEPOT: the end of the shift, sun going down behind the stacks.
    Mood {
        fog: [0.82, 0.60, 0.48, 0.0050],
        sun: Vec3::new(-0.70, 0.36, 0.44),
        tone: [[1.22, 0.80, 0.50], [0.42, 0.45, 0.68], [0.31, 0.25, 0.31]],
        zenith: [0.22, 0.30, 0.56],
        glow: [1.0, 0.62, 0.30],
        disc: 1.0,
        clouds: 18,
        cloud: [1.0, 0.72, 0.52],
        belly: [0.56, 0.40, 0.50],
        range: [0.40, 0.32, 0.46],
        mist: (0.0, 0.0),
        motes: [1.0, 0.72, 0.42, 0.7],
        dust: 0.35,
        leaves: false,
        birds: 4,
    },
    // SPILLWAY: grey and wet, the sun a pale place in the overcast.
    Mood {
        fog: [0.66, 0.71, 0.72, 0.0062],
        sun: Vec3::new(0.36, 0.72, -0.44),
        tone: [[0.82, 0.82, 0.78], [0.58, 0.64, 0.70], [0.33, 0.34, 0.36]],
        zenith: [0.50, 0.56, 0.61],
        glow: [0.84, 0.86, 0.84],
        disc: 0.0,
        clouds: 40,
        cloud: [0.74, 0.77, 0.79],
        belly: [0.56, 0.60, 0.64],
        range: [0.42, 0.50, 0.52],
        mist: (0.85, 2.5),
        motes: [0.9, 0.95, 1.0, 0.45],
        dust: 0.0,
        leaves: true,
        birds: 2,
    },
];

/// CANOPY: a bright afternoon before a tropical squall; warm sun through a
/// break in the cloud, a blue-grey storm bank behind the crown.
static CANOPY: Mood = Mood {
    fog: [0.56, 0.70, 0.65, 0.0052],
    sun: Vec3::new(0.0, 0.766, -0.643),
    tone: [[1.02, 0.93, 0.70], [0.37, 0.52, 0.55], [0.20, 0.31, 0.20]],
    zenith: [0.18, 0.39, 0.47],
    glow: [0.84, 0.86, 0.71],
    disc: 0.85,
    clouds: 32,
    cloud: [0.88, 0.90, 0.88],
    belly: [0.38, 0.44, 0.50],
    range: [0.26, 0.40, 0.34],
    mist: (0.55, 2.0),
    motes: [1.0, 0.96, 0.70, 0.6],
    dust: 0.0,
    leaves: true,
    birds: 4,
};

/// The classic maps' moods and Canopy's. Relay and Slag Cathedral paint
/// their own.
pub fn mood(map: u8) -> Option<&'static Mood> {
    let map = ferrite_shared::map::doc::look_of(map);
    if map == super::canopy::ID {
        return Some(&CANOPY);
    }
    MOODS.get(map as usize)
}

fn dir(el: f32, az: f32) -> Vec3 {
    vec3(el.cos() * az.cos(), el.sin(), el.cos() * az.sin())
}

/// Sprite facing the middle of the sky dome, fading to nothing at its rim.
fn halo(m: &mut Mesh, d: Vec3, dist: f32, size: f32, col: Col) {
    let c = d * dist;
    let rt = d.cross(Vec3::Y).normalize();
    let up = rt.cross(d);
    let rim = [col[0], col[1], col[2], 0.0];
    let n = 20;
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        let p = |a: f32| c + (rt * a.cos() + up * a.sin()) * size;
        m.quad_c(c, p(a0), p(a1), c, [col, rim, rim, col]);
    }
}

pub fn sky(md: &Mood) -> Mesh {
    let mut m = Mesh::default();
    let r = 420.0;
    let hor = [md.fog[0], md.fog[1], md.fog[2]];
    let sun = md.sun.normalize();
    let sun_az = sun.z.atan2(sun.x);
    let col_at = |el: f32, az: f32| {
        let t = smooth(0.0, 1.0, (el / 1.1).clamp(0.0, 1.0)).powf(0.7);
        let mut c = mix3(hor, md.zenith, t);
        // The sun's side of the sky is warmer and brighter low down.
        let warm = ((az - sun_az).cos() * 0.5 + 0.5).powi(3) * (1.0 - t).powi(2) * smooth(-0.05, 0.1, el);
        c = mix3(c, md.glow, warm * 0.55);
        [c[0], c[1], c[2], 1.0]
    };
    let (rings, segs) = (14, 48);
    for ri in 0..rings {
        let e0 = -0.25 + (1.57 + 0.25) * ri as f32 / rings as f32;
        let e1 = -0.25 + (1.57 + 0.25) * (ri + 1) as f32 / rings as f32;
        for s in 0..segs {
            let a0 = s as f32 / segs as f32 * TAU;
            let a1 = (s + 1) as f32 / segs as f32 * TAU;
            m.quad_c(
                dir(e0, a0) * r,
                dir(e0, a1) * r,
                dir(e1, a1) * r,
                dir(e1, a0) * r,
                [col_at(e0, a0), col_at(e0, a1), col_at(e1, a1), col_at(e1, a0)],
            );
        }
    }
    // The sun: wide glow, tight glow, disc. Under overcast only the glow.
    let g = md.glow;
    halo(&mut m, sun, 404.0, 150.0, [g[0], g[1], g[2], 0.22 + 0.1 * md.disc]);
    halo(&mut m, sun, 403.0, 48.0, [1.0, 0.94, 0.78, 0.25 + 0.3 * md.disc]);
    if md.disc > 0.0 {
        let c = sun * 402.0;
        let rt = sun.cross(Vec3::Y).normalize();
        let up = rt.cross(sun);
        let n = 20;
        for k in 0..n {
            let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
            m.tri(c, c + (rt * a0.cos() + up * a0.sin()) * 15.0, c + (rt * a1.cos() + up * a1.sin()) * 15.0, [1.0, 0.97, 0.86, md.disc]);
        }
    }
    // Far ranges: two ridgelines, the nearer one darker, both sinking into
    // the haze at their feet.
    for layer in 0..2u32 {
        let dist = 398.0 - layer as f32 * 3.0;
        let n = 120;
        let top = |k: u32| {
            let k = k % n;
            let a = k as f32 / n as f32 * TAU;
            let big = vnoise(a * 2.2 + layer as f32 * 9.0, 0.5, 31 + layer);
            let small = hash01(k, 77 + layer);
            (0.035 + layer as f32 * -0.012) + 0.075 * big + 0.018 * small * big
        };
        let lit = |a: f32| 0.5 + 0.5 * (a - sun_az).cos();
        for k in 0..n {
            let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
            let depth = 0.30 + 0.22 * layer as f32;
            let c = mix3(hor, mix3(md.range, md.glow, 0.25 * lit(a0)), depth);
            let face = [c[0], c[1], c[2], 1.0];
            let foot = [hor[0], hor[1], hor[2], 1.0];
            m.quad_c(
                dir(-0.06, a0) * dist,
                dir(-0.06, a1) * dist,
                dir(top(k + 1), a1) * dist,
                dir(top(k), a0) * dist,
                [foot, foot, face, face],
            );
        }
    }
    // Slab clouds in small stacks: a lit top over a darker belly. With many
    // of them the stacks merge into an overcast.
    let overcast = md.clouds > 30;
    for k in 0..md.clouds {
        let az = hash01(k, 1) * TAU;
        let el = if overcast { 0.10 + hash01(k, 2) * 1.2 } else { 0.14 + hash01(k, 2) * 0.5 };
        let p = dir(el, az) * r * 0.9;
        let grow = if overcast { 1.7 } else { 1.0 };
        let sx = (38.0 + hash01(k, 3) * 60.0) * grow;
        let sz = (18.0 + hash01(k, 4) * 26.0) * grow;
        // Clouds toward the sun catch more of its colour.
        let near = ((az - sun_az).cos() * 0.5 + 0.5).powi(2);
        let top = mix3(md.cloud, md.glow, 0.3 * near);
        let a = if overcast { 0.62 } else { 0.84 };
        m.cuboid(p - vec3(sx, 3.0, sz), p + vec3(sx, 2.0, sz), [md.belly[0], md.belly[1], md.belly[2], a]);
        m.cuboid(p + vec3(-sx * 0.86, 2.0, -sz * 0.84), p + vec3(sx * 0.9, 5.0, sz * 0.86), [top[0], top[1], top[2], a]);
        for j in 0..2u32 {
            let o = vec3((hash01(k, 10 + j) - 0.5) * sx, 5.0, (hash01(k, 12 + j) - 0.5) * sz);
            let (hx, hz) = (sx * (0.3 + 0.25 * hash01(k, 14 + j)), sz * (0.35 + 0.25 * hash01(k, 16 + j)));
            m.cuboid(p + o - vec3(hx, 0.0, hz), p + o + vec3(hx, 4.0 + 4.0 * hash01(k, 18 + j), hz), [top[0], top[1], top[2], a]);
        }
    }
    m
}

// --------------------------------------------------------------- biome ----

struct Biome {
    /// Attempts in one half of the field; each success is planted twice.
    bushes: usize,
    bush: [[f32; 3]; 3],
    /// Share of open ground under tall grass (0 = none), and its tint.
    tall: f32,
    tall_tint: [f32; 3],
    flowers: usize,
    petals: &'static [[f32; 3]],
    /// Chance of ferns at the foot of a tree.
    ferns: f32,
    scrub: usize,
    reeds: usize,
    deadwood: usize,
    litter: bool,
    /// Chance that a concrete wall carries ivy, and that a rock carries moss.
    ivy: f32,
    moss: f32,
    saplings: usize,
    /// Extra trees on the slopes outside the fence, and how many are pines.
    backdrop: usize,
    pine: f32,
    /// Weeds per metre along the foot of walls.
    weeds: f32,
}

/// CANOPY: lush, broad-leaved and wet; coral flowers, ferns everywhere, a
/// jungle wall beyond the fence.
static CANOPY_BIOME: Biome = Biome {
    bushes: 560,
    bush: [[0.16, 0.38, 0.20], [0.24, 0.46, 0.22], [0.12, 0.30, 0.22]],
    tall: 0.28,
    tall_tint: [0.80, 1.05, 0.78],
    flowers: 700,
    petals: &[[0.96, 0.46, 0.42], [0.98, 0.78, 0.30], [0.95, 0.92, 0.86], [0.86, 0.36, 0.56]],
    ferns: 0.95,
    scrub: 60,
    reeds: 0,
    deadwood: 40,
    litter: true,
    ivy: 0.8,
    moss: 0.7,
    saplings: 90,
    backdrop: 900,
    pine: 0.0,
    weeds: 1.0,
};

static BIOMES: [Biome; 6] = [
    // FOUNDRY VALLEY: an overgrown autumn meadow.
    Biome {
        bushes: 520,
        bush: [[0.33, 0.42, 0.18], [0.58, 0.44, 0.16], [0.46, 0.30, 0.16]],
        tall: 0.36,
        tall_tint: [1.12, 1.0, 0.62],
        flowers: 900,
        petals: &[[0.95, 0.85, 0.30], [0.92, 0.92, 0.88], [0.85, 0.42, 0.20], [0.62, 0.45, 0.80]],
        ferns: 0.5,
        scrub: 40,
        reeds: 0,
        deadwood: 60,
        litter: true,
        ivy: 0.55,
        moss: 0.35,
        saplings: 110,
        backdrop: 700,
        pine: 0.4,
        weeds: 0.9,
    },
    // QUARRY: scrub holding on in the dust.
    Biome {
        bushes: 170,
        bush: [[0.47, 0.48, 0.33], [0.54, 0.50, 0.31], [0.40, 0.43, 0.30]],
        tall: 0.12,
        tall_tint: [1.15, 1.02, 0.66],
        flowers: 90,
        petals: &[[0.95, 0.88, 0.45], [0.90, 0.60, 0.30]],
        ferns: 0.0,
        scrub: 520,
        reeds: 0,
        deadwood: 45,
        litter: false,
        ivy: 0.1,
        moss: 0.0,
        saplings: 14,
        backdrop: 260,
        pine: 0.55,
        weeds: 0.5,
    },
    // RIDGELINE: deep forest floor.
    Biome {
        bushes: 620,
        bush: [[0.15, 0.33, 0.20], [0.21, 0.38, 0.20], [0.28, 0.41, 0.18]],
        tall: 0.26,
        tall_tint: [0.86, 1.06, 0.72],
        flowers: 520,
        petals: &[[0.90, 0.92, 0.95], [0.55, 0.50, 0.90], [0.95, 0.75, 0.85]],
        ferns: 0.9,
        scrub: 0,
        reeds: 0,
        deadwood: 130,
        litter: true,
        ivy: 0.5,
        moss: 0.8,
        saplings: 190,
        backdrop: 1000,
        pine: 0.9,
        weeds: 0.8,
    },
    // DEPOT: weeds through the cracks, a hedge gone wild along the fence.
    Biome {
        bushes: 230,
        bush: [[0.30, 0.37, 0.24], [0.37, 0.41, 0.22], [0.26, 0.33, 0.24]],
        tall: 0.14,
        tall_tint: [1.0, 1.02, 0.7],
        flowers: 160,
        petals: &[[0.95, 0.88, 0.30], [0.92, 0.92, 0.90]],
        ferns: 0.2,
        scrub: 90,
        reeds: 0,
        deadwood: 10,
        litter: true,
        ivy: 0.45,
        moss: 0.0,
        saplings: 40,
        backdrop: 420,
        pine: 0.3,
        weeds: 1.6,
    },
    // SPILLWAY: everything damp; reeds where the water used to stand.
    Biome {
        bushes: 380,
        bush: [[0.25, 0.40, 0.28], [0.32, 0.45, 0.26], [0.22, 0.36, 0.30]],
        tall: 0.30,
        tall_tint: [0.92, 1.05, 0.78],
        flowers: 300,
        petals: &[[0.95, 0.90, 0.40], [0.88, 0.90, 0.95], [0.60, 0.55, 0.85]],
        ferns: 0.7,
        scrub: 0,
        reeds: 700,
        deadwood: 40,
        litter: true,
        ivy: 0.7,
        moss: 0.7,
        saplings: 90,
        backdrop: 620,
        pine: 0.35,
        weeds: 1.2,
    },
    // RELAY: highland heather and wind-bent grass.
    Biome {
        bushes: 420,
        bush: [[0.47, 0.31, 0.49], [0.37, 0.40, 0.30], [0.53, 0.37, 0.53]],
        tall: 0.30,
        tall_tint: [1.02, 1.0, 0.74],
        flowers: 260,
        petals: &[[0.70, 0.50, 0.80], [0.92, 0.92, 0.90]],
        ferns: 0.3,
        scrub: 120,
        reeds: 0,
        deadwood: 20,
        litter: false,
        ivy: 0.0,
        moss: 0.5,
        saplings: 0,
        backdrop: 260,
        pine: 0.85,
        weeds: 0.4,
    },
];

/// Which square metres have something built on them.
struct Occ {
    nx: usize,
    nz: usize,
    cells: Vec<bool>,
}

const OCC_X: f32 = HALF_X + 10.0;
const OCC_Z: f32 = HALF_Z + 10.0;

impl Occ {
    fn new(w: &World, margin: f32) -> Occ {
        let (nx, nz) = ((OCC_X * 2.0) as usize, (OCC_Z * 2.0) as usize);
        let mut cells = vec![false; nx * nz];
        for b in &w.blocks {
            if b.kind == BlockKind::Trunk {
                continue;
            }
            let x0 = ((b.min.x - margin + OCC_X).floor().max(0.0)) as usize;
            let x1 = ((b.max.x + margin + OCC_X).ceil().max(0.0) as usize).min(nx);
            let z0 = ((b.min.z - margin + OCC_Z).floor().max(0.0)) as usize;
            let z1 = ((b.max.z + margin + OCC_Z).ceil().max(0.0) as usize).min(nz);
            for z in z0..z1 {
                for x in x0..x1 {
                    cells[z * nx + x] = true;
                }
            }
        }
        Occ { nx, nz, cells }
    }

    fn free(&self, x: f32, z: f32) -> bool {
        let (gx, gz) = (x + OCC_X, z + OCC_Z);
        if gx < 0.0 || gz < 0.0 {
            return true;
        }
        let (ix, iz) = (gx as usize, gz as usize);
        ix >= self.nx || iz >= self.nz || !self.cells[iz * self.nx + ix]
    }
}

struct Field<'a> {
    w: &'a World,
    occ: Occ,
    /// Height of the lowest objective: what "low ground" is measured from.
    low: f32,
}

impl Field<'_> {
    fn objective(&self, x: f32, z: f32) -> f32 {
        let mut near = f32::MAX;
        for p in self.w.points.iter().chain(self.w.bases.iter()) {
            near = near.min(((x - p.x).powi(2) + (z - p.z).powi(2)).sqrt());
        }
        near
    }

    fn ok(&self, x: f32, z: f32, flat: f32, clear: f32) -> bool {
        x.abs() < HALF_X + 5.0
            && z.abs() < HALF_Z + 5.0
            && self.occ.free(x, z)
            && self.w.terrain_normal(x, z).y >= flat
            && self.objective(x, z) > clear
            && !self.w.lava(x, z, 1.5)
            && !(self.w.map == super::fallen_giant::ID && super::fallen_giant::bare(x, z))
    }

    fn at(&self, x: f32, z: f32) -> Vec3 {
        vec3(x, self.w.height(x, z), z)
    }

    /// A random spot in the west half and its mirror image, if both are
    /// open ground no steeper than `flat` and `clear` metres from any
    /// objective.
    fn pair(&self, r: &mut Rng, flat: f32, clear: f32) -> Option<[Vec3; 2]> {
        let x = r.range(-HALF_X - 4.0, 0.0);
        let z = r.range(-HALF_Z - 4.0, HALF_Z + 4.0);
        (self.ok(x, z, flat, clear) && self.ok(-x, -z, flat, clear)).then(|| [self.at(x, z), self.at(-x, -z)])
    }
}

fn col3(c: [f32; 3], k: f32) -> Col {
    rgb(c[0] * k, c[1] * k, c[2] * k)
}

/// A lump of leaves: seven-sided, lit from above, its top moving in the wind.
fn blob(m: &mut Mesh, c: Vec3, r: Vec3, rot: f32, col: Col, ground: f32, sway: f32, k: u32) {
    const N: usize = 7;
    let ring = |i: usize, rad: f32, y: f32, tw: f32| {
        let a = (i % N) as f32 / N as f32 * TAU + rot + tw;
        c + vec3(a.cos() * rad * r.x, y * r.y, a.sin() * rad * r.z)
    };
    let top = c + vec3(0.0, r.y, 0.0);
    let sw = |p: Vec3| ((p.y - ground) * sway).max(0.0);
    let face = |m: &mut Mesh, a: Vec3, b: Vec3, d: Vec3, s: f32| {
        let n = ((a + b + d) / 3.0 - c + vec3(0.0, r.y * 0.4, 0.0)).normalize_or_zero();
        m.tri_w([a, b, d], n, [sw(a), sw(b), sw(d)], shade(col, s));
    };
    for i in 0..N {
        let j = 0.9 + 0.2 * hash01(i as u32, k);
        let (u0, u1) = (ring(i, 0.58, 0.62, 0.3), ring(i + 1, 0.58, 0.62, 0.3));
        let (s0, s1) = (ring(i, 0.92, 0.26, 0.15), ring(i + 1, 0.92, 0.26, 0.15));
        let (m0, m1) = (ring(i, 1.0, -0.12, 0.0), ring(i + 1, 1.0, -0.12, 0.0));
        let (l0, l1) = (ring(i, 0.7, -0.62, -0.3), ring(i + 1, 0.7, -0.62, -0.3));
        face(m, u0, top, u1, 1.1 * j);
        face(m, s0, u0, u1, 1.02 * j);
        face(m, s0, u1, s1, 1.02 * j);
        face(m, m0, s0, s1, 0.94 * j);
        face(m, m0, s1, m1, 0.94 * j);
        face(m, l0, m0, m1, 0.8 * j);
        face(m, l0, m1, l1, 0.8 * j);
    }
}

/// One blade: two thin triangles rooted at `base`, bending over towards
/// its swaying tip.
#[allow(clippy::too_many_arguments)]
fn blade(m: &mut Mesh, base: Vec3, a: f32, h: f32, wd: f32, lean: f32, n: Vec3, col: Col) {
    let side = vec3(-a.sin(), 0.0, a.cos()) * wd;
    let tip = base + vec3(a.cos() * lean, h, a.sin() * lean);
    let mid = base + vec3(a.cos() * lean * 0.28, h * 0.58, a.sin() * lean * 0.28) + side * 0.55;
    m.tri_w([base - side, base + side, mid], n, [0.0, 0.0, h * 0.14], col);
    m.tri_w([base - side, mid, tip], n, [0.0, h * 0.14, h * 0.42], shade(col, 1.07));
}

fn bush(m: &mut Mesh, p: Vec3, r: &mut Rng, pal: &[[f32; 3]; 3], scale: f32) {
    let col = col3(pal[r.below(3) as usize], r.range(0.85, 1.12));
    let n = 2 + r.below(3);
    let size = r.range(0.42, 0.72) * scale;
    for k in 0..n {
        let a = r.range(0.0, TAU);
        let d = if k == 0 { 0.0 } else { size * r.range(0.6, 1.0) };
        let s = if k == 0 { size } else { size * r.range(0.55, 0.85) };
        let hy = s * r.range(0.55, 0.75);
        let c = p + vec3(a.cos() * d, hy * 0.62 - 0.03, a.sin() * d);
        blob(m, c, vec3(s, hy, s), r.range(0.0, TAU), shade(col, r.range(0.9, 1.1)), p.y, 0.07, r.below(1000));
    }
}

/// Dry scrub: a fan of bare twigs, sometimes over a knot of sage.
fn scrub(m: &mut Mesh, p: Vec3, r: &mut Rng) {
    let twig = rgb(0.42, 0.35, 0.27);
    let n = 5 + r.below(4);
    for _ in 0..n {
        let a = r.range(0.0, TAU);
        let h = r.range(0.35, 0.85);
        blade(m, p, a, h, 0.016, h * r.range(0.3, 0.8), Vec3::Y, shade(twig, r.range(0.8, 1.2)));
    }
    if r.chance(0.6) {
        let s = r.range(0.2, 0.34);
        blob(m, p + vec3(0.0, s * 0.45, 0.0), vec3(s, s * 0.6, s), r.range(0.0, TAU), rgb(0.50, 0.50, 0.37), p.y, 0.05, r.below(1000));
    }
}

fn fern(m: &mut Mesh, p: Vec3, r: &mut Rng) {
    let col = rgb(0.17, 0.36, 0.17);
    let n = 5 + r.below(3);
    let len = r.range(0.45, 0.85);
    let a0 = r.range(0.0, TAU);
    for k in 0..n {
        let a = a0 + k as f32 / n as f32 * TAU + r.range(-0.2, 0.2);
        let l = len * r.range(0.8, 1.15);
        let out = vec3(a.cos(), 0.0, a.sin());
        let side = vec3(-a.sin(), 0.0, a.cos()) * l * 0.2;
        let mid = p + out * l * 0.55 + vec3(0.0, l * 0.5, 0.0);
        let tip = p + out * l + vec3(0.0, l * 0.22, 0.0);
        let n1 = (Vec3::Y + out * 0.5).normalize();
        let c = shade(col, r.range(0.85, 1.2));
        m.tri_w([p, mid - side, mid + side], n1, [0.0, l * 0.12, l * 0.12], c);
        m.tri_w([mid - side, mid + side, tip], (Vec3::Y - out * 0.3).normalize(), [l * 0.12, l * 0.12, l * 0.22], shade(c, 1.1));
    }
}

fn reeds(m: &mut Mesh, p: Vec3, r: &mut Rng) {
    let n = 6 + r.below(5);
    for _ in 0..n {
        let a = r.range(0.0, TAU);
        let base = p + vec3(r.range(-0.3, 0.3), 0.0, r.range(-0.3, 0.3));
        let h = r.range(0.8, 1.45);
        let lean = r.range(0.0, 0.22);
        let col = rgb(0.36, 0.46, 0.24);
        blade(m, base, a, h, 0.022, lean, Vec3::Y, shade(col, r.range(0.8, 1.15)));
        if r.chance(0.3) {
            // A seed head just under the tip, swaying with it.
            let c = base + vec3(a.cos() * lean, h, a.sin() * lean) * 0.86;
            let c = vec3(c.x, base.y + h * 0.86, c.z);
            let (s, w) = (vec3(0.028, 0.0, 0.0), h * 0.36);
            let head = rgb(0.34, 0.22, 0.14);
            for side in [s, vec3(0.0, 0.0, 0.028)] {
                m.tri_w([c - side, c + side, c + vec3(0.0, 0.16, 0.0)], Vec3::Y, [w; 3], head);
                m.tri_w([c - side, c + side, c - vec3(0.0, 0.08, 0.0)], Vec3::Y, [w; 3], head);
            }
        }
    }
}

fn flowers(m: &mut Mesh, w: &World, p: Vec3, r: &mut Rng, petals: &[[f32; 3]]) {
    let pc = petals[r.below(petals.len() as u32) as usize];
    let n = 3 + r.below(4);
    for _ in 0..n {
        let (dx, dz) = (r.range(-0.45, 0.45), r.range(-0.45, 0.45));
        let base = vec3(p.x + dx, w.height(p.x + dx, p.z + dz), p.z + dz);
        let h = r.range(0.16, 0.38);
        let a = r.range(0.0, TAU);
        blade(m, base, a, h, 0.012, 0.03, Vec3::Y, rgb(0.28, 0.42, 0.2));
        let c = base + vec3(a.cos() * 0.03, h, a.sin() * 0.03);
        let s = r.range(0.028, 0.048);
        let sw = [h * 0.42; 3];
        let col = [pc[0], pc[1], pc[2], 0.12];
        m.tri_w([c + vec3(-s, 0.0, -s), c + vec3(s, 0.0, -s), c + vec3(0.0, s * 0.5, s)], Vec3::Y, sw, col);
        m.tri_w([c + vec3(-s, 0.0, s), c + vec3(s, 0.0, s), c + vec3(0.0, s * 0.5, -s)], Vec3::Y, sw, shade(col, 0.88));
    }
}

/// A stump, or a windfall branch lying beside one.
fn deadwood(m: &mut Mesh, p: Vec3, r: &mut Rng) {
    let bark = rgb(0.33, 0.25, 0.19);
    if r.chance(0.5) {
        let (s, h) = (r.range(0.16, 0.28), r.range(0.18, 0.4));
        taper(m, p + vec3(-s, -0.3, -s), p + vec3(s, h, s), 1.15, 0.85, Vec3::ZERO, Vec3::ZERO, bark);
        let t = s * 0.8;
        let y = p.y + h + 0.006;
        m.quad(
            vec3(p.x - t, y, p.z - t),
            vec3(p.x - t, y, p.z + t),
            vec3(p.x + t, y, p.z + t),
            vec3(p.x + t, y, p.z - t),
            rgb(0.62, 0.50, 0.34),
        );
    } else {
        let rad = r.range(0.09, 0.17);
        let g = Mat4::from_translation(p + vec3(0.0, rad * 0.8, 0.0)) * Mat4::from_rotation_y(r.range(0.0, TAU));
        m.tube(&g, Vec3::ZERO, rad, 0.0, r.range(0.7, 1.5), 6, shade(bark, r.range(0.85, 1.15)));
        // A snapped limb still on it.
        let a = r.range(-0.9, 0.9);
        m.obox(&(g * Mat4::from_rotation_x(-0.9) * Mat4::from_rotation_y(a)), vec3(0.0, 0.0, 0.22), vec3(0.025, 0.025, 0.26), bark);
    }
}

/// A young tree, too thin to stop anything.
fn sapling(m: &mut Mesh, p: Vec3, r: &mut Rng, pine: bool, pal: &[[f32; 3]; 3]) {
    let h = r.range(1.5, 2.7);
    let trunk = rgb(0.36, 0.27, 0.2);
    m.cuboid(p + vec3(-0.035, -0.2, -0.035), p + vec3(0.035, h * 0.7, 0.035), trunk);
    if pine {
        let c = shade(rgb(0.17, 0.33, 0.25), r.range(0.9, 1.15));
        for k in 0..3 {
            let f = k as f32 / 3.0;
            pyramid(m, p + vec3(0.0, h * (0.22 + 0.24 * f), 0.0), h * 0.26 * (1.0 - 0.25 * f), h * 0.42, f, shade(c, 1.0 + 0.1 * f));
        }
    } else {
        let lc = if r.chance(0.5) { LEAF[r.below(5) as usize] } else { pal[r.below(3) as usize] };
        let s = h * r.range(0.22, 0.3);
        blob(m, p + vec3(0.0, h * 0.72, 0.0), vec3(s, s * 1.25, s), r.range(0.0, TAU), col3(lc, 1.0), p.y, 0.035, r.below(1000));
        blob(
            m,
            p + vec3(s * 0.5, h * 0.52, s * 0.3),
            vec3(s * 0.6, s * 0.6, s * 0.6),
            r.range(0.0, TAU),
            col3(lc, 0.9),
            p.y,
            0.035,
            r.below(1000),
        );
    }
}

/// Small flat leaves lying on the ground around `p`.
#[allow(clippy::too_many_arguments)]
fn litter(m: &mut Mesh, f: &Field, p: Vec3, r: &mut Rng, reach: f32, n: u32, col: [f32; 3], size: f32) {
    for _ in 0..n {
        let a = r.range(0.0, TAU);
        let d = reach * r.range(0.0f32, 1.0).sqrt();
        let (x, z) = (p.x + a.cos() * d, p.z + a.sin() * d);
        if !f.occ.free(x, z) {
            continue;
        }
        let (s, t) = (size * r.range(0.7, 1.4), r.range(0.0, TAU));
        let pt = |k: f32| {
            let (px, pz) = (x + (t + k).cos() * s, z + (t + k).sin() * s);
            vec3(px, f.w.height(px, pz) + 0.03, pz)
        };
        m.tri_n(pt(0.0), pt(2.0), pt(4.2), f.w.terrain_normal(x, z), col3(col, r.range(0.75, 1.1)));
    }
}

/// Ivy climbing one face of a wall from the ground.
fn ivy(m: &mut Mesh, f: &Field, b: &Block, r: &mut Rng) {
    let h = b.max.y - b.min.y;
    for face in 0..4 {
        if !r.chance(0.45) {
            continue;
        }
        // Face: the axis it runs along, the fixed coordinate and its normal.
        let along_x = face < 2;
        let out = if face % 2 == 0 { -1.0 } else { 1.0 };
        let (lo, hi) = if along_x { (b.min.x, b.max.x) } else { (b.min.z, b.max.z) };
        if hi - lo < 1.0 {
            continue;
        }
        let fixed = match face {
            0 => b.min.z,
            1 => b.max.z,
            2 => b.min.x,
            _ => b.max.x,
        } + out * 0.075;
        let n = if along_x { vec3(0.0, 0.0, out) } else { vec3(out, 0.0, 0.0) };
        let at = |u: f32, y: f32| if along_x { vec3(u, y, fixed) } else { vec3(fixed, y, u) };
        let root = r.range(lo + 0.3, hi - 0.3);
        let probe = at(root, 0.0) + n * 0.6;
        if !f.occ.free(probe.x, probe.z) && f.occ.free(probe.x + n.x * 2.0, probe.z + n.z * 2.0) {
            continue; // another wall stands right against this face
        }
        let ground = f.w.height(probe.x, probe.z).max(b.min.y);
        let reach = (b.max.y - ground) * r.range(0.45, 0.95);
        if reach < 0.6 {
            continue;
        }
        let spread = r.range(0.7, 2.0).min((hi - lo) * 0.5);
        let leaves = (reach * spread * 16.0) as u32 + 8;
        let green = [0.20 + r.range(0.0, 0.08), 0.38, 0.17];
        let shading = (n + Vec3::Y * 0.6).normalize();
        for _ in 0..leaves.min(90) {
            let v = r.range(0.0f32, 1.0).powf(1.5);
            let u = root + r.range(-1.0, 1.0) * spread * (1.0 - 0.75 * v);
            if u < lo + 0.05 || u > hi - 0.05 {
                continue;
            }
            let c = at(u, ground + 0.05 + v * reach);
            let (s, t) = (r.range(0.07, 0.15), r.range(0.0, TAU));
            let pt = |k: f32| {
                let (du, dy) = ((t + k).cos() * s, (t + k).sin() * s);
                c + if along_x { vec3(du, dy, 0.0) } else { vec3(0.0, dy, du) }
            };
            let autumn = r.chance(0.12);
            let col = if autumn { rgb(0.62, 0.30, 0.14) } else { col3(green, r.range(0.75, 1.2)) };
            m.tri_n(pt(0.0), pt(2.1), pt(4.2), shading, col);
        }
        let _ = h;
    }
}

/// Green on top of a boulder, matching the taper the rock is drawn with.
fn moss(m: &mut Mesh, b: &Block, i: usize, r: &mut Rng) {
    let c = (b.min + b.max) * 0.5;
    let h = (b.max - b.min) * 0.5;
    let skew = vec3(hash01(i as u32, 3) - 0.5, 0.0, hash01(i as u32, 4) - 0.5) * 0.5;
    let (hx, hz) = (h.x * 0.45 * r.range(0.7, 0.98), h.z * 0.45 * r.range(0.7, 0.98));
    let o = vec3(c.x + skew.x, b.max.y + 0.03, c.z + skew.z);
    let col = rgb(0.30, 0.42, 0.20);
    m.quad(o + vec3(-hx, 0.0, -hz), o + vec3(-hx, 0.0, hz), o + vec3(hx, 0.0, hz), o + vec3(hx, 0.0, -hz), shade(col, r.range(0.85, 1.15)));
    // A tuft or two growing out of it.
    for _ in 0..r.below(4) {
        let base = o + vec3(r.range(-hx, hx), 0.0, r.range(-hz, hz));
        blade(m, base, r.range(0.0, TAU), r.range(0.1, 0.22), 0.02, 0.05, Vec3::Y, shade(col, 1.2));
    }
}

/// Plant a map's undergrowth into its static mesh.
pub fn grow(m: &mut Mesh, w: &World) {
    let look = ferrite_shared::map::doc::look_of(w.map);
    let Some(b) = (if look == super::canopy::ID { Some(&CANOPY_BIOME) } else { BIOMES.get(look as usize) }) else { return };
    let seed = (w.seed as u32) ^ 0xF10A;
    // The ground under each objective, not the objective: Spillway's C stands
    // on a deck over the channel.
    let low = w.points.iter().map(|p| w.height(p.x, p.z)).fold(f32::MAX, f32::min);
    let f = Field { w, occ: Occ::new(w, 0.35), low };
    let rng_for = |salt: u32, k: usize| Rng::new(((seed ^ salt) as u64) << 20 | k as u64);
    let relay = w.map == super::relay::ID;

    // Bushes, half of them gathered where something already stands: at the
    // foot of a tree or beside a rock.
    let anchors: Vec<Vec3> = w
        .trees
        .iter()
        .filter(|t| t.base.x < 0.0 && t.base.x.abs() < HALF_X && t.base.z.abs() < HALF_Z)
        .map(|t| t.base)
        .chain(
            w.blocks
                .iter()
                .filter(|b| b.kind == BlockKind::Rock && b.min.x < 0.0)
                .map(|b| vec3((b.min.x + b.max.x) * 0.5, 0.0, b.max.z + 0.4)),
        )
        .collect();
    let mut site = Rng::new(seed as u64 ^ 0xB05);
    for k in 0..b.bushes {
        let spots = if k % 2 == 0 && !anchors.is_empty() {
            let a = anchors[site.below(anchors.len() as u32) as usize];
            let (x, z) = (a.x + site.range(-2.2, 2.2), a.z + site.range(-2.2, 2.2));
            (f.ok(x, z, 0.9, 10.5) && f.ok(-x, -z, 0.9, 10.5)).then(|| [f.at(x, z), f.at(-x, -z)])
        } else {
            f.pair(&mut site, 0.9, 10.5)
        };
        let Some(spots) = spots else { continue };
        for p in spots {
            bush(m, p, &mut rng_for(1, k), &b.bush, 1.0);
        }
    }

    // Tall grass in drifts, thinning toward the edge of each drift.
    if b.tall > 0.0 {
        let cut = 1.0 - b.tall;
        let tries = (30000.0 * b.tall / 0.3) as usize;
        for k in 0..tries {
            let Some(spots) = f.pair(&mut site, 0.92, 10.0) else { continue };
            let drift = vnoise(spots[0].x / 15.0, spots[0].z / 15.0, seed ^ 0x71) * 0.75
                + vnoise(spots[0].x / 4.0, spots[0].z / 4.0, seed ^ 0x72) * 0.25;
            if drift < cut {
                continue;
            }
            let rise = 0.45 + 0.55 * smooth(cut, cut + 0.12, drift);
            for p in spots {
                let mut r = rng_for(2, k);
                let n = w.terrain_normal(p.x, p.z);
                let gc = ground_col(w, p, n, r.range(0.0, 1.0));
                let t = b.tall_tint;
                let col = rgb(gc[0] * t[0], gc[1] * t[1], gc[2] * t[2]);
                for _ in 0..4 + r.below(3) {
                    let base = p + vec3(r.range(-0.22, 0.22), 0.0, r.range(-0.22, 0.22));
                    let h = r.range(0.34, 0.68) * rise;
                    blade(m, base, r.range(0.0, TAU), h, 0.028, h * r.range(0.1, 0.4), n, shade(col, r.range(0.8, 1.2)));
                }
            }
        }
    }

    // Flowers keep to their own patches.
    for k in 0..b.flowers * 3 {
        let Some(spots) = f.pair(&mut site, 0.94, 9.0) else { continue };
        if vnoise(spots[0].x / 11.0, spots[0].z / 11.0, seed ^ 0x81) < 0.6 {
            continue;
        }
        for p in spots {
            flowers(m, w, p, &mut rng_for(3, k), b.petals);
        }
    }

    for k in 0..b.scrub {
        let Some(spots) = f.pair(&mut site, 0.86, 8.0) else { continue };
        for p in spots {
            scrub(m, p, &mut rng_for(4, k));
        }
    }

    // Reeds stand in the lowest ground.
    for k in 0..b.reeds * 4 {
        let Some(spots) = f.pair(&mut site, 0.95, 9.0) else { continue };
        if spots[0].y > f.low + 0.6 && vnoise(spots[0].x / 9.0, spots[0].z / 9.0, seed ^ 0x91) < 0.72 {
            continue;
        }
        if spots[0].y > f.low + 2.5 {
            continue;
        }
        for p in spots {
            reeds(m, p, &mut rng_for(5, k));
        }
    }

    for k in 0..b.deadwood {
        let Some(spots) = f.pair(&mut site, 0.975, 11.0) else { continue };
        for p in spots {
            deadwood(m, p, &mut rng_for(6, k));
        }
    }

    for k in 0..b.saplings {
        let Some(spots) = f.pair(&mut site, 0.9, 13.0) else { continue };
        let pine = site.chance(b.pine);
        for p in spots {
            sapling(m, p, &mut rng_for(7, k), pine, &b.bush);
        }
    }

    // Around each tree on the field: ferns at its foot, its own leaves on
    // the ground beneath it.
    for (i, t) in w.trees.iter().enumerate() {
        let inside = t.base.x.abs() < HALF_X + 4.0 && t.base.z.abs() < HALF_Z + 4.0;
        // Trees come in mirrored pairs; seeding by pair keeps the two alike.
        let mut r = rng_for(8, i / 2);
        if inside && r.chance(b.ferns) {
            for _ in 0..1 + r.below(3) {
                let a = r.range(0.0, TAU);
                let d = r.range(0.55, 1.7);
                let (x, z) = (t.base.x + a.cos() * d, t.base.z + a.sin() * d);
                if f.occ.free(x, z) && w.terrain_normal(x, z).y > 0.88 {
                    fern(m, f.at(x, z), &mut r);
                }
            }
        }
        if b.litter && inside {
            if t.pine {
                litter(m, &f, t.base, &mut r, t.radius * 1.1, 9, [0.40, 0.29, 0.18], 0.1);
            } else {
                litter(m, &f, t.base, &mut r, t.radius * 1.5, 18, LEAF[t.tint as usize % 5], 0.085);
            }
        }
    }

    // Ivy on standing concrete, moss on rock, weeds along the foot of walls.
    let open = Occ::new(w, 0.0);
    for (i, blk) in w.blocks.iter().enumerate() {
        // Moving to the destroyed state must not leave floating static ivy.
        if blk.dest.is_some() {
            continue;
        }
        let mut r = rng_for(9, i);
        if relay && super::relay::owns(blk) {
            continue;
        }
        let ground = w.height((blk.min.x + blk.max.x) * 0.5, (blk.min.z + blk.max.z) * 0.5);
        let standing = blk.min.y < ground + 0.4;
        match blk.kind {
            BlockKind::Concrete if standing && blk.max.y - blk.min.y > 1.8 && r.chance(b.ivy) => ivy(m, &f, blk, &mut r),
            BlockKind::Rock if r.chance(b.moss) => moss(m, blk, i, &mut r),
            _ => {}
        }
        if !standing || !matches!(blk.kind, BlockKind::Concrete | BlockKind::Container | BlockKind::Metal | BlockKind::Rock) {
            continue;
        }
        let (sx, sz) = (blk.max.x - blk.min.x, blk.max.z - blk.min.z);
        let n = ((sx + sz) * 2.0 * b.weeds) as u32;
        for _ in 0..n.min(60) {
            // A point just outside the footprint, somewhere along its edge.
            let e = r.range(0.0, (sx + sz) * 2.0);
            let off = r.range(0.1, 0.38);
            let (x, z) = if e < sx {
                (blk.min.x + e, blk.min.z - off)
            } else if e < sx * 2.0 {
                (blk.min.x + e - sx, blk.max.z + off)
            } else if e < sx * 2.0 + sz {
                (blk.min.x - off, blk.min.z + e - sx * 2.0)
            } else {
                (blk.max.x + off, blk.min.z + e - sx * 2.0 - sz)
            };
            if !open.free(x, z) || x.abs() > HALF_X + 5.0 || z.abs() > HALF_Z + 5.0 || f.objective(x, z) < 6.0 {
                continue;
            }
            let p = f.at(x, z);
            let nrm = w.terrain_normal(x, z);
            if nrm.y < 0.9 {
                continue;
            }
            let gc = ground_col(w, p, nrm, r.range(0.0, 1.0));
            let col = rgb(gc[0] * 0.78, gc[1] * 1.0, gc[2] * 0.7);
            for _ in 0..3 + r.below(3) {
                let base = p + vec3(r.range(-0.12, 0.12), 0.0, r.range(-0.12, 0.12));
                let h = r.range(0.18, 0.5);
                blade(m, base, r.range(0.0, TAU), h, 0.024, h * r.range(0.1, 0.5), nrm, shade(col, r.range(0.8, 1.2)));
            }
        }
    }

    // The forest beyond the fence: thicker and taller than what stands on
    // the field, so the map ends in a wall of trees and not a bare slope.
    let mut far = Rng::new(seed as u64 ^ 0xFA5);
    for k in 0..b.backdrop {
        let x = far.range(TER_X0 + 6.0, 0.0);
        let z = far.range(TER_Z0 + 6.0, -TER_Z0 - 6.0);
        if x.abs() < HALF_X + 7.0 && z.abs() < HALF_Z + 7.0 {
            continue;
        }
        let height = far.range(6.0, 12.5);
        let radius = far.range(1.7, 3.1);
        let tint = far.below(5) as u8;
        let pine = far.chance(b.pine);
        for (px, pz) in [(x, z), (-x, -z)] {
            let t = Tree { base: vec3(px, w.height(px, pz), pz), height, radius, tint, pine };
            // Monuments stand on these slopes; nothing grows through them.
            if !super::monuments::hides_tree(w, &t) {
                super::tree(m, &t, 100_000 + k * 2, super::leaf_for(w.map));
            }
        }
        // Brush between the trunks.
        if k % 3 == 0 {
            for (px, pz) in [(x + 2.5, z + 1.5), (-x - 2.5, -z - 1.5)] {
                if !super::monuments::hides(w, px, pz, 1.5) {
                    bush(m, vec3(px, w.height(px, pz), pz), &mut rng_for(10, k), &b.bush, 1.5);
                }
            }
        }
    }
}

// ------------------------------------------------------------ ambience ----

/// What moves in the air around `eye` this frame.
#[allow(clippy::too_many_arguments)]
pub fn ambience(opaque: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, w: &World, eye: Vec3, right: Vec3, up: Vec3, time: f32) {
    let Some(md) = mood(w.map) else { return };
    let seed = w.seed as u32;

    // Leaves coming down from the broadleaf trees nearby: each tree keeps
    // three in the air, and each lies a moment where it lands.
    if md.leaves {
        for (i, t) in w.trees.iter().enumerate() {
            if t.pine || (t.base - eye).length_squared() > 42.0 * 42.0 {
                continue;
            }
            let lc = LEAF[t.tint as usize % 5];
            for k in 0..3u32 {
                let id = i as u32 * 3 + k;
                let period = 7.0 + 5.0 * hash01(id, 1);
                let f = (time / period + hash01(id, 2)).fract();
                let a = hash01(id, 3) * TAU;
                let d = t.radius * (0.3 + 0.7 * hash01(id, 4));
                let fall = (f / 0.86).min(1.0);
                let drift = vec3(1.0, 0.0, 0.35) * fall * 1.6;
                let wob =
                    vec3((time * 1.7 + a).sin(), 0.0, (time * 1.3 + a * 2.0).cos()) * 0.35 * (1.0 - (fall * 2.0 - 1.0).abs()).max(0.0);
                let xz = t.base + vec3(a.cos() * d, 0.0, a.sin() * d) + drift + wob;
                let ground = w.height(xz.x, xz.z) + 0.04;
                let top = t.base.y + t.height * 0.7;
                let y = (top - (top - ground) * fall).max(ground);
                let p = vec3(xz.x, y, xz.z);
                let s = 0.075 * smooth(0.0, 0.04, f) * smooth(1.0, 0.95, f);
                let spin = if fall < 1.0 { time * (3.0 + 2.0 * hash01(id, 5)) + a } else { a };
                let tilt = if fall < 1.0 { 0.9 } else { 0.0 };
                let u = vec3(spin.cos(), spin.sin() * tilt, spin.sin()) * s;
                let v = vec3(-spin.sin(), spin.cos() * tilt, spin.cos()) * s;
                let col = col3(lc, 0.9 + 0.2 * hash01(id, 6));
                opaque.tri_n(p - u - v, p + u - v, p + u + v, Vec3::Y, col);
                opaque.tri_n(p - u - v, p + u + v, p - u + v, Vec3::Y, col);
            }
        }
    }

    // Motes: pollen, dust or damp, catching the light a few metres out.
    if md.motes[3] > 0.0 {
        const CELL_M: f32 = 5.0;
        let c0 = (eye / CELL_M).floor();
        for cz in -3..=3i32 {
            for cy in -1..=1i32 {
                for cx in -3..=3i32 {
                    let cell = c0 + vec3(cx as f32, cy as f32, cz as f32);
                    let id = (cell.x as i32 as u32).wrapping_mul(73_856_093)
                        ^ (cell.y as i32 as u32).wrapping_mul(19_349_663)
                        ^ (cell.z as i32 as u32).wrapping_mul(83_492_791);
                    for k in 0..2u32 {
                        let h = |s: u32| hash01(id ^ k.wrapping_mul(0x9E37), s);
                        // Each drifts slowly and wraps inside its own cell.
                        let q = vec3(
                            (h(1) + time * 0.035 * (0.5 + h(4))).fract(),
                            (h(2) + time * 0.012 * (h(5) - 0.4)).rem_euclid(1.0),
                            (h(3) + time * 0.02 * (h(6) - 0.5)).rem_euclid(1.0),
                        );
                        let edge = (q * (Vec3::ONE - q)).min_element() * 8.0;
                        let p = (cell + q) * CELL_M;
                        let dist = (p - eye).length();
                        let a = md.motes[3]
                            * smooth(16.0, 8.0, dist)
                            * smooth(0.6, 2.0, dist)
                            * edge.min(1.0)
                            * (0.55 + 0.45 * (time * (1.0 + h(7)) + h(8) * TAU).sin());
                        if a < 0.02 || p.y < w.height(p.x, p.z) + 0.1 {
                            continue;
                        }
                        let s = 0.012 + 0.012 * h(9);
                        let col = [md.motes[0], md.motes[1], md.motes[2], a * 0.75];
                        add.quad(p - right * s - up * s, p + right * s - up * s, p + right * s + up * s, p - right * s + up * s, col);
                    }
                }
            }
        }
    }

    // Ground mist: soft sheets lying in the low ground, drifting slowly.
    // They fade out near the eye so walking through one never pops.
    if md.mist.0 > 0.0 {
        const STEP: f32 = 15.0;
        let low = w.points.iter().map(|p| w.height(p.x, p.z)).fold(f32::MAX, f32::min);
        let (x0, z0) = ((eye.x / STEP).floor() as i32, (eye.z / STEP).floor() as i32);
        for gz in z0 - 9..=z0 + 9 {
            for gx in x0 - 9..=x0 + 9 {
                let id = (gx as u32).wrapping_mul(0x9E37_79B1) ^ (gz as u32).wrapping_mul(0x85EB_CA77) ^ seed;
                let h = |s: u32| hash01(id, s);
                let x = (gx as f32 + h(1)) * STEP + (time * 0.06 + h(3) * TAU).sin() * 5.0 + time * 0.25 % STEP;
                let z = (gz as f32 + h(2)) * STEP + (time * 0.045 + h(4) * TAU).cos() * 4.0;
                if x.abs() > -TER_X0 - 10.0 || z.abs() > -TER_Z0 - 10.0 {
                    continue;
                }
                let g = w.height(x, z);
                let lie = smooth(low + md.mist.1 + 3.0, low + md.mist.1 - 1.0, g);
                let d = ((x - eye.x).powi(2) + (z - eye.z).powi(2)).sqrt();
                let a =
                    md.mist.0 * lie * 0.17 * smooth(7.0, 26.0, d) * smooth(135.0, 95.0, d) * (0.7 + 0.3 * (time * 0.21 + h(5) * TAU).sin());
                if a < 0.012 {
                    continue;
                }
                let (hw, hh) = (9.0 + 6.0 * h(6), 1.3 + 1.2 * h(7));
                let c = vec3(x, g + hh * 0.75, z);
                let col = [md.fog[0] * 1.06, md.fog[1] * 1.06, md.fog[2] * 1.06, a];
                let rim = [col[0], col[1], col[2], 0.0];
                let turn = h(8) * PI;
                for axis in [vec3(turn.cos(), 0.0, turn.sin()), vec3(-turn.sin(), 0.0, turn.cos())] {
                    let (r, u) = (axis * hw, Vec3::Y * hh);
                    let corners = [c - r - u, c + r - u, c + r + u, c - r + u];
                    for e in 0..4 {
                        alpha.quad_c(c, corners[e], corners[(e + 1) % 4], c, [col, rim, rim, col]);
                    }
                }
            }
        }
    }

    // Blown dust: long faint streaks running with the wind near the ground.
    if md.dust > 0.0 {
        const STEP: f32 = 11.0;
        let wind = vec3(0.95, 0.0, 0.31);
        let (x0, z0) = ((eye.x / STEP).floor() as i32, (eye.z / STEP).floor() as i32);
        for gz in z0 - 6..=z0 + 6 {
            for gx in x0 - 6..=x0 + 6 {
                let id = (gx as u32).wrapping_mul(0x2C1B_3C6D) ^ (gz as u32).wrapping_mul(0x9E37_79B1) ^ seed;
                let h = |s: u32| hash01(id, s);
                let f = (time * (0.05 + 0.05 * h(1)) + h(2)).fract();
                let c0 = vec3((gx as f32 + h(3)) * STEP, 0.0, (gz as f32 + h(4)) * STEP) + wind * (f - 0.5) * 22.0;
                let g = w.height(c0.x, c0.z);
                let c = vec3(c0.x, g + 0.25 + 1.3 * h(5), c0.z);
                let d = (c - eye).length();
                let a = md.dust * 0.11 * (f * PI).sin().powi(2) * smooth(4.0, 14.0, d) * smooth(70.0, 45.0, d);
                if a < 0.01 {
                    continue;
                }
                let (len, hh) = (3.5 + 4.0 * h(6), 0.22 + 0.3 * h(7));
                let col = [md.fog[0] * 0.98, md.fog[1] * 0.93, md.fog[2] * 0.84, a];
                let rim = [col[0], col[1], col[2], 0.0];
                let (r, u) = (wind * len, Vec3::Y * hh);
                let corners = [c - r - u, c + r - u, c + r + u, c - r + u];
                for e in 0..4 {
                    alpha.quad_c(c, corners[e], corners[(e + 1) % 4], c, [col, rim, rim, col]);
                }
            }
        }
    }

    // Birds wheeling high over the field in loose flocks.
    for fl in 0..md.birds {
        let h = |s: u32| hash01(fl * 31 + 7, s ^ seed);
        let centre = vec3((h(1) - 0.5) * HALF_X * 1.5, 0.0, (h(2) - 0.5) * HALF_Z * 1.5);
        let alt = w.height(centre.x, centre.z) + 36.0 + 26.0 * h(3);
        let rad = 22.0 + 26.0 * h(4);
        let turn = if h(5) < 0.5 { 1.0 } else { -1.0 };
        let n = 3 + (h(6) * 5.0) as u32;
        for k in 0..n {
            let hb = |s: u32| hash01(fl * 131 + k, s ^ 0xB1D);
            let a = turn * time * (5.5 / rad) + h(7) * TAU + k as f32 * 0.13 + hb(1) * 0.2;
            let r = rad + (hb(2) - 0.5) * 9.0;
            let p = centre + vec3(a.cos() * r, alt + (hb(3) - 0.5) * 6.0 + (time * 0.4 + hb(4) * TAU).sin() * 1.2, a.sin() * r);
            if (p - eye).length_squared() > 260.0 * 260.0 {
                continue;
            }
            let fwd = vec3(-a.sin(), 0.0, a.cos()) * turn;
            let side = vec3(fwd.z, 0.0, -fwd.x);
            // Mostly gliding, with a few beats now and then.
            let beat = (time * 0.6 + hb(5) * TAU).sin().max(0.0);
            let flap = (time * 9.0 + hb(6) * TAU).sin() * 0.3 * beat + 0.12;
            let col = rgb(0.10, 0.10, 0.12);
            for sgn in [-1.0f32, 1.0] {
                let tip = p + side * sgn * 0.75 + vec3(0.0, flap, 0.0) - fwd * 0.15;
                opaque.tri_n(p + fwd * 0.28, p - fwd * 0.3, tip, Vec3::Y, col);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finite(mesh: Mesh) -> usize {
        let mut tris = 0;
        for (v, i) in mesh.finish() {
            tris += i.len() / 3;
            for p in &v {
                assert!(p.pos.iter().chain(p.nrm.iter()).chain(p.col.iter()).all(|x| x.is_finite()), "non-finite vertex {:?}", p.pos);
            }
        }
        tris
    }

    #[test]
    fn every_classic_map_grows_foliage_within_budget() {
        for map in 0..6u8 {
            let w = World::with_map(7, map);
            let mut m = Mesh::default();
            grow(&mut m, &w);
            let tris = finite(m);
            eprintln!("{}: {} foliage triangles", MAPS[map as usize].name, tris);
            assert!(tris > 40_000, "{} is bare: {tris}", MAPS[map as usize].name);
            assert!(tris < 840_000, "{} over budget: {tris}", MAPS[map as usize].name);
        }
    }

    #[test]
    fn foliage_keeps_off_objectives_and_out_of_buildings() {
        for map in 0..6u8 {
            let w = World::with_map(7, map);
            let f = Field { w: &w, occ: Occ::new(&w, 0.35), low: 0.0 };
            let mut r = Rng::new(3);
            let mut found = 0;
            for _ in 0..4000 {
                let Some(spots) = f.pair(&mut r, 0.9, 10.5) else { continue };
                found += 1;
                // The two spots mirror each other through the centre.
                assert!((spots[0].x + spots[1].x).abs() < 1e-3 && (spots[0].z + spots[1].z).abs() < 1e-3);
                for p in spots {
                    assert!(f.objective(p.x, p.z) > 10.5);
                    assert!(!w.blocks.iter().any(|b| b.kind != BlockKind::Trunk
                        && p.x > b.min.x
                        && p.x < b.max.x
                        && p.z > b.min.z
                        && p.z < b.max.z));
                }
            }
            assert!(found > 400, "map {map}: only {found} open spots");
        }
    }

    #[test]
    fn skies_and_air_stay_finite_and_small() {
        for map in 0..5u8 {
            let md = mood(map).unwrap();
            assert!(finite(sky(md)) < 6_000);
            let w = World::with_map(7, map);
            for (eye, time) in [(vec3(0.0, 3.0, 0.0), 0.0f32), (vec3(-120.0, 8.0, 40.0), 913.7), (vec3(150.0, 62.0, -90.0), 86_400.0)] {
                let (mut o, mut a, mut d) = (Mesh::default(), Mesh::default(), Mesh::default());
                ambience(&mut o, &mut a, &mut d, &w, eye, Vec3::X, Vec3::Y, time);
                let n = finite(o) + finite(a) + finite(d);
                assert!(n < 9_000, "map {map}: {n} triangles of air");
            }
        }
        assert!(mood(5).is_none() && mood(6).is_none());
    }
}
