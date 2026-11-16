//! Client-only eye candy: tracers, sparks, smoke, debris, shell casings,
//! bullet holes, scorch marks and short-lived lights.

use crate::gfx::{Light, Mesh};
use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::World;
use ferrite_shared::rng::Rng;

pub struct Tracer {
    a: Vec3,
    dir: Vec3,
    len: f32,
    head: f32,
    col: [f32; 3],
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Cube,
    Spark,
    Smoke,
    /// Expanding ring on the ground.
    Ring,
}

pub struct Particle {
    pos: Vec3,
    vel: Vec3,
    life: f32,
    max: f32,
    size: f32,
    col: [f32; 3],
    kind: Kind,
    spin: f32,
}

/// A mark left on a surface: bullet hole or scorch.
struct Decal {
    pos: Vec3,
    u: Vec3,
    v: Vec3,
    col: [f32; 4],
    life: f32,
}

struct Glow {
    l: Light,
    life: f32,
    max: f32,
}

pub struct Fx {
    pub tracers: Vec<Tracer>,
    pub parts: Vec<Particle>,
    decals: Vec<Decal>,
    glows: Vec<Glow>,
    rng: Rng,
}

impl Fx {
    pub fn new() -> Fx {
        Fx { tracers: Vec::new(), parts: Vec::new(), decals: Vec::new(), glows: Vec::new(), rng: Rng::new(99) }
    }

    fn rv(&mut self) -> Vec3 {
        vec3(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0))
    }

    /// Persistent state, rather than a possibly lost debris event, removes
    /// bullet marks attached to surfaces that no longer exist.
    pub fn remove_destroyed_decals(&mut self, world: &World, changed: u64) {
        if changed == 0 { return; }
        self.decals.retain(|d| !world.blocks.iter().any(|b| {
            b.dest.is_some_and(|bit| changed & (1u64 << bit) != 0)
                && d.pos.cmpge(b.min - Vec3::splat(0.08)).all()
                && d.pos.cmple(b.max + Vec3::splat(0.08)).all()
        }));
    }

    fn push(&mut self, p: Particle) {
        if self.parts.len() < 2200 {
            self.parts.push(p);
        }
    }

    pub fn light(&mut self, pos: Vec3, radius: f32, col: [f32; 3], power: f32, life: f32) {
        if self.glows.len() < 24 {
            self.glows.push(Glow { l: Light { pos, radius, col, power }, life, max: life });
        }
    }

    /// The four lights that matter most to a viewer at `cam`.
    pub fn lights(&self, cam: Vec3) -> [Light; 4] {
        let mut ranked: Vec<(f32, Light)> = self
            .glows
            .iter()
            .map(|g| {
                let f = (g.life / g.max).clamp(0.0, 1.0);
                let mut l = g.l;
                l.power *= f;
                (l.power * l.radius / (1.0 + (l.pos - cam).length() * 0.05), l)
            })
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out = [Light::default(); 4];
        for (k, (_, l)) in ranked.into_iter().take(4).enumerate() {
            out[k] = l;
        }
        out
    }

    pub fn tracer(&mut self, a: Vec3, b: Vec3, col: [f32; 3]) {
        let d = b - a;
        let len = d.length();
        if len > 1.5 && self.tracers.len() < 300 {
            self.tracers.push(Tracer { a, dir: d / len, len, head: 1.2, col });
        }
    }

    fn decal(&mut self, pos: Vec3, n: Vec3, size: f32, col: [f32; 4], life: f32) {
        let n = n.normalize_or_zero();
        if n == Vec3::ZERO {
            return;
        }
        let t = if n.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let u = n.cross(t).normalize() * size;
        let v = n.cross(u).normalize() * size;
        if self.decals.len() >= 260 {
            self.decals.remove(0);
        }
        self.decals.push(Decal { pos: pos + n * 0.012, u, v, col, life });
    }

    /// A bullet striking the world: chips fly off along the surface normal
    /// and a hole is left behind.
    pub fn impact(&mut self, pos: Vec3, n: Vec3, col: [f32; 3]) {
        for _ in 0..4 {
            let v = self.rv() * 1.8 + n * 3.0 + vec3(0.0, 1.2, 0.0);
            let s = self.rng.range(0.03, 0.07);
            self.push(Particle { pos, vel: v, life: 0.5, max: 0.5, size: s, col, kind: Kind::Cube, spin: 9.0 });
        }
        let v = self.rv() * 0.3 + n * 0.8 + vec3(0.0, 0.3, 0.0);
        self.push(Particle { pos: pos + n * 0.05, vel: v, life: 0.55, max: 0.55, size: 0.12, col: [0.62, 0.58, 0.52], kind: Kind::Smoke, spin: 0.0 });
        self.push(Particle { pos: pos + n * 0.03, vel: Vec3::ZERO, life: 0.06, max: 0.06, size: 0.1, col: [1.0, 0.85, 0.5], kind: Kind::Spark, spin: 0.0 });
        let s = self.rng.range(0.018, 0.032);
        self.decal(pos, n, s, [0.06, 0.055, 0.05, 0.85], 26.0);
    }

    pub fn hit_burst(&mut self, pos: Vec3, col: [f32; 3]) {
        for _ in 0..7 {
            let v = self.rv() * 3.0 + vec3(0.0, 2.0, 0.0);
            self.push(Particle { pos, vel: v, life: 0.45, max: 0.45, size: 0.05, col, kind: Kind::Cube, spin: 12.0 });
        }
        self.push(Particle { pos, vel: Vec3::ZERO, life: 0.08, max: 0.08, size: 0.16, col, kind: Kind::Spark, spin: 0.0 });
    }

    /// Bits knocked off a soldier as they go down.
    pub fn shatter(&mut self, pos: Vec3, col: [f32; 3]) {
        for k in 0..12 {
            let p = pos + vec3(self.rng.range(-0.3, 0.3), self.rng.range(0.6, 1.6), self.rng.range(-0.3, 0.3));
            let v = self.rv() * 2.6 + vec3(0.0, 3.5, 0.0);
            let s = self.rng.range(0.05, 0.12);
            let c = if k % 3 == 0 { [0.2, 0.2, 0.22] } else { [col[0] * self.rng.range(0.6, 1.0), col[1] * 0.9, col[2] * 0.9] };
            self.push(Particle { pos: p, vel: v, life: 2.2, max: 2.2, size: s, col: c, kind: Kind::Cube, spin: 7.0 });
        }
    }

    pub fn crate_break(&mut self, pos: Vec3) {
        for _ in 0..22 {
            let p = pos + self.rv() * 0.4;
            let v = self.rv() * 3.5 + vec3(0.0, 4.0, 0.0);
            let s = self.rng.range(0.08, 0.2);
            self.push(Particle { pos: p, vel: v, life: 2.4, max: 2.4, size: s, col: [0.66, 0.47, 0.25], kind: Kind::Cube, spin: 8.0 });
        }
        for _ in 0..3 {
            let v = self.rv() * 0.8 + vec3(0.0, 0.8, 0.0);
            self.push(Particle { pos, vel: v, life: 1.0, max: 1.0, size: 0.45, col: [0.7, 0.62, 0.5], kind: Kind::Smoke, spin: 0.0 });
        }
    }

    /// Cosmetic fragments for a discrete server-confirmed map change.
    /// Collision changes only when its persistent snapshot bit arrives.
    pub fn map_break(&mut self, pos: Vec3, feature: u8) {
        let metal = feature != 0;
        let color = if metal { [0.39, 0.43, 0.43] } else { [0.58, 0.55, 0.48] };
        for _ in 0..28 {
            let p = pos + self.rv() * 0.6;
            let vel = self.rv() * 4.0 + vec3(0.0, 4.5, 0.0);
            let size = self.rng.range(0.06, 0.19);
            self.push(Particle { pos: p, vel, life: 2.6, max: 2.6, size, col: color, kind: Kind::Cube, spin: 9.0 });
        }
        for _ in 0..7 {
            let vel = self.rv() * 1.4 + vec3(0.0, 1.1, 0.0);
            self.push(Particle { pos, vel, life: 2.0, max: 2.0, size: 0.65, col: [0.40, 0.38, 0.34], kind: Kind::Smoke, spin: 0.0 });
        }
        if metal {
            for _ in 0..16 {
                let vel = self.rv() * 6.5 + vec3(0.0, 3.0, 0.0);
                self.push(Particle { pos, vel, life: 0.55, max: 0.55, size: 0.08, col: [1.0, 0.68, 0.25], kind: Kind::Spark, spin: 0.0 });
            }
            self.light(pos, 9.0, [1.0, 0.58, 0.22], 1.5, 0.25);
        }
    }

    pub fn explosion(&mut self, pos: Vec3, w: &World) {
        for _ in 0..46 {
            let v = self.rv().normalize_or_zero() * self.rng.range(6.0, 22.0) + vec3(0.0, 5.0, 0.0);
            let l = self.rng.range(0.25, 0.75);
            let sz = self.rng.range(0.1, 0.24);
            self.push(Particle { pos, vel: v, life: l, max: l, size: sz, col: [1.0, 0.62, 0.2], kind: Kind::Spark, spin: 0.0 });
        }
        for k in 0..12 {
            let v = self.rv() * 2.8 + vec3(0.0, 2.2 + k as f32 * 0.25, 0.0);
            let l = self.rng.range(1.4, 3.2);
            let off = self.rv() * 1.3;
            let sz = self.rng.range(0.5, 1.1);
            let g = self.rng.range(0.2, 0.34);
            self.push(Particle { pos: pos + off, vel: v, life: l, max: l, size: sz, col: [g, g * 0.94, g * 0.9], kind: Kind::Smoke, spin: 0.0 });
        }
        for _ in 0..30 {
            let v = self.rv() * 6.5 + vec3(0.0, 8.0, 0.0);
            let s = self.rng.range(0.06, 0.17);
            self.push(Particle { pos, vel: v, life: 2.4, max: 2.4, size: s, col: [0.36, 0.3, 0.24], kind: Kind::Cube, spin: 10.0 });
        }
        self.push(Particle { pos: pos + vec3(0.0, 0.6, 0.0), vel: Vec3::ZERO, life: 0.16, max: 0.16, size: 5.5, col: [1.0, 0.85, 0.55], kind: Kind::Spark, spin: 0.0 });
        let g = w.height(pos.x, pos.z);
        if pos.y - g < 2.0 {
            let ground = vec3(pos.x, g, pos.z);
            self.push(Particle { pos: ground + vec3(0.0, 0.12, 0.0), vel: Vec3::ZERO, life: 0.45, max: 0.45, size: 8.0, col: [1.0, 0.8, 0.55], kind: Kind::Ring, spin: 0.0 });
            let s = self.rng.range(2.0, 2.6);
            self.decal(ground, w.terrain_normal(pos.x, pos.z), s, [0.05, 0.045, 0.04, 0.6], 45.0);
        }
        self.light(pos + vec3(0.0, 1.0, 0.0), 26.0, [1.0, 0.62, 0.28], 5.0, 0.5);
    }

    pub fn muzzle(&mut self, pos: Vec3) {
        self.push(Particle { pos, vel: Vec3::ZERO, life: 0.05, max: 0.05, size: 0.3, col: [1.0, 0.8, 0.45], kind: Kind::Spark, spin: 0.0 });
        self.light(pos, 7.0, [1.0, 0.75, 0.4], 1.4, 0.06);
    }

    /// A spent case flicked out of the ejection port.
    pub fn shell(&mut self, pos: Vec3, vel: Vec3, big: bool) {
        let jitter = self.rv() * 0.5;
        let (size, col) = if big { (0.035, [0.75, 0.2, 0.15]) } else { (0.02, [0.85, 0.66, 0.25]) };
        self.push(Particle { pos, vel: vel + jitter, life: 1.6, max: 1.6, size, col, kind: Kind::Cube, spin: 22.0 });
    }

    /// Kicked-up dirt: footfalls, landings, slides.
    pub fn dust(&mut self, pos: Vec3, amount: f32) {
        let n = (amount * 3.0).ceil() as usize;
        for _ in 0..n {
            let v = self.rv() * 0.6 * amount + vec3(0.0, 0.5, 0.0);
            let l = self.rng.range(0.4, 0.8);
            self.push(Particle { pos: pos + vec3(0.0, 0.08, 0.0), vel: v, life: l, max: l, size: 0.14 * amount.max(0.6), col: [0.6, 0.53, 0.42], kind: Kind::Smoke, spin: 0.0 });
        }
    }

    /// Thin smoke left behind a flying grenade.
    pub fn trail(&mut self, pos: Vec3) {
        self.push(Particle { pos, vel: vec3(0.0, 0.3, 0.0), life: 0.5, max: 0.5, size: 0.06, col: [0.75, 0.75, 0.75], kind: Kind::Smoke, spin: 0.0 });
    }

    pub fn update(&mut self, dt: f32, w: &World) {
        for t in self.tracers.iter_mut() {
            t.head += 330.0 * dt;
        }
        self.tracers.retain(|t| t.head - 14.0 < t.len);
        for g in self.glows.iter_mut() {
            g.life -= dt;
        }
        self.glows.retain(|g| g.life > 0.0);
        for d in self.decals.iter_mut() {
            d.life -= dt;
        }
        self.decals.retain(|d| d.life > 0.0);
        for p in self.parts.iter_mut() {
            p.life -= dt;
            match p.kind {
                Kind::Cube => {
                    p.vel.y -= 16.0 * dt;
                    p.pos += p.vel * dt;
                    let g = w.height(p.pos.x, p.pos.z) + p.size * 0.5;
                    if p.pos.y < g {
                        p.pos.y = g;
                        p.vel = vec3(p.vel.x * 0.5, -p.vel.y * 0.35, p.vel.z * 0.5);
                        p.spin *= 0.5;
                    }
                }
                Kind::Spark => {
                    p.vel.y -= 9.0 * dt;
                    p.vel *= 1.0 - 2.5 * dt;
                    p.pos += p.vel * dt;
                }
                Kind::Smoke => {
                    p.vel *= 1.0 - 1.5 * dt;
                    p.vel.y += 0.35 * dt;
                    p.pos += p.vel * dt;
                    p.size += p.size * 0.9 * dt;
                }
                Kind::Ring => {}
            }
        }
        self.parts.retain(|p| p.life > 0.0);
    }

    pub fn build(&self, opaque: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, cam: Vec3, right: Vec3, up: Vec3) {
        for d in &self.decals {
            if (d.pos - cam).length_squared() > 90.0 * 90.0 {
                continue;
            }
            let a = d.col[3] * (d.life / 3.0).min(1.0);
            let c = [d.col[0], d.col[1], d.col[2], a];
            alpha.quad(d.pos - d.u - d.v, d.pos + d.u - d.v, d.pos + d.u + d.v, d.pos - d.u + d.v, c);
        }
        for t in &self.tracers {
            let h = t.head.min(t.len);
            let tail = (t.head - 14.0).max(0.0);
            if h <= tail {
                continue;
            }
            let (p0, p1) = (t.a + t.dir * tail, t.a + t.dir * h);
            let side = t.dir.cross(cam - p1).normalize_or_zero() * (0.025 + 0.0012 * (cam - p1).length());
            let c1 = [t.col[0], t.col[1], t.col[2], 0.95];
            let c0 = [t.col[0], t.col[1], t.col[2], 0.0];
            add.quad_c(p0 - side, p0 + side, p1 + side, p1 - side, [c0, c0, c1, c1]);
        }
        for p in &self.parts {
            let f = (p.life / p.max).clamp(0.0, 1.0);
            match p.kind {
                Kind::Cube => {
                    let s = p.size * (0.4 + 0.6 * f.min(0.3) / 0.3);
                    let a = p.spin * (p.max - p.life);
                    let m = Mat4::from_translation(p.pos) * Mat4::from_rotation_y(a) * Mat4::from_rotation_x(a * 0.7);
                    opaque.obox(&m, Vec3::ZERO, Vec3::splat(s * 0.5), [p.col[0], p.col[1], p.col[2], 0.0]);
                }
                Kind::Spark => {
                    let s = p.size * (0.4 + 0.6 * f);
                    let c = [p.col[0], p.col[1], p.col[2], f];
                    add.quad(p.pos - right * s - up * s, p.pos + right * s - up * s, p.pos + right * s + up * s, p.pos - right * s + up * s, c);
                }
                Kind::Smoke => {
                    // Fade in fast, fade out slow: puffs appear soft, not popped.
                    let s = p.size;
                    let a = 0.34 * f * ((1.0 - f) * 8.0).min(1.0);
                    let c = [p.col[0], p.col[1], p.col[2], a];
                    alpha.quad(p.pos - right * s - up * s, p.pos + right * s - up * s, p.pos + right * s + up * s, p.pos - right * s + up * s, c);
                }
                Kind::Ring => {
                    let r1 = p.size * (1.0 - f * f);
                    let r0 = r1 * 0.82;
                    let c = [p.col[0], p.col[1], p.col[2], 0.5 * f];
                    let n = 24;
                    for k in 0..n {
                        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
                        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
                        let q = |a: f32, r: f32| p.pos + vec3(a.cos() * r, 0.0, a.sin() * r);
                        add.quad(q(a0, r0), q(a0, r1), q(a1, r1), q(a1, r0), c);
                    }
                }
            }
        }
    }
}
