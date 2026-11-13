//! Client-only eye candy: tracers, sparks, smoke puffs and cube debris.

use crate::gfx::Mesh;
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

pub struct Fx {
    pub tracers: Vec<Tracer>,
    pub parts: Vec<Particle>,
    rng: Rng,
}

impl Fx {
    pub fn new() -> Fx {
        Fx { tracers: Vec::new(), parts: Vec::new(), rng: Rng::new(99) }
    }

    fn rv(&mut self) -> Vec3 {
        vec3(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0))
    }

    fn push(&mut self, p: Particle) {
        if self.parts.len() < 1800 {
            self.parts.push(p);
        }
    }

    pub fn tracer(&mut self, a: Vec3, b: Vec3, col: [f32; 3]) {
        let d = b - a;
        let len = d.length();
        if len > 1.5 && self.tracers.len() < 300 {
            self.tracers.push(Tracer { a, dir: d / len, len, head: 1.2, col });
        }
    }

    pub fn impact(&mut self, pos: Vec3, col: [f32; 3]) {
        for _ in 0..4 {
            let v = self.rv() * 2.5 + vec3(0.0, 2.5, 0.0);
            let s = self.rng.range(0.03, 0.07);
            self.push(Particle { pos, vel: v, life: 0.5, max: 0.5, size: s, col, kind: Kind::Cube, spin: 9.0 });
        }
        let v = self.rv() * 0.3 + vec3(0.0, 0.6, 0.0);
        self.push(Particle { pos, vel: v, life: 0.45, max: 0.45, size: 0.1, col: [0.6, 0.56, 0.5], kind: Kind::Smoke, spin: 0.0 });
    }

    pub fn hit_burst(&mut self, pos: Vec3, col: [f32; 3]) {
        for _ in 0..6 {
            let v = self.rv() * 3.0 + vec3(0.0, 2.0, 0.0);
            self.push(Particle { pos, vel: v, life: 0.45, max: 0.45, size: 0.05, col, kind: Kind::Cube, spin: 12.0 });
        }
    }

    /// A fallen soldier bursts into team-coloured blocks.
    pub fn shatter(&mut self, pos: Vec3, col: [f32; 3]) {
        for k in 0..26 {
            let p = pos + vec3(self.rng.range(-0.3, 0.3), self.rng.range(0.1, 1.7), self.rng.range(-0.3, 0.3));
            let v = self.rv() * 3.2 + vec3(0.0, 4.5, 0.0);
            let s = self.rng.range(0.07, 0.17);
            let c = if k % 3 == 0 { [0.2, 0.2, 0.22] } else { [col[0] * self.rng.range(0.6, 1.0), col[1] * 0.9, col[2] * 0.9] };
            self.push(Particle { pos: p, vel: v, life: 2.6, max: 2.6, size: s, col: c, kind: Kind::Cube, spin: 7.0 });
        }
    }

    pub fn crate_break(&mut self, pos: Vec3) {
        for _ in 0..18 {
            let p = pos + self.rv() * 0.4;
            let v = self.rv() * 3.5 + vec3(0.0, 4.0, 0.0);
            let s = self.rng.range(0.08, 0.2);
            self.push(Particle { pos: p, vel: v, life: 2.0, max: 2.0, size: s, col: [0.66, 0.47, 0.25], kind: Kind::Cube, spin: 8.0 });
        }
        self.push(Particle { pos, vel: vec3(0.0, 0.8, 0.0), life: 0.8, max: 0.8, size: 0.5, col: [0.7, 0.62, 0.5], kind: Kind::Smoke, spin: 0.0 });
    }

    pub fn explosion(&mut self, pos: Vec3) {
        for _ in 0..40 {
            let v = self.rv().normalize_or_zero() * self.rng.range(6.0, 20.0) + vec3(0.0, 5.0, 0.0);
            let l = self.rng.range(0.25, 0.7);
            let sz = self.rng.range(0.1, 0.22);
            self.push(Particle { pos, vel: v, life: l, max: l, size: sz, col: [1.0, 0.62, 0.2], kind: Kind::Spark, spin: 0.0 });
        }
        for _ in 0..7 {
            let v = self.rv() * 2.5 + vec3(0.0, 2.5, 0.0);
            let l = self.rng.range(1.0, 2.0);
            let off = self.rv() * 1.2;
            let sz = self.rng.range(0.5, 1.0);
            self.push(Particle { pos: pos + off, vel: v, life: l, max: l, size: sz, col: [0.3, 0.28, 0.27], kind: Kind::Smoke, spin: 0.0 });
        }
        for _ in 0..22 {
            let v = self.rv() * 6.0 + vec3(0.0, 8.0, 0.0);
            let s = self.rng.range(0.06, 0.16);
            self.push(Particle { pos, vel: v, life: 2.2, max: 2.2, size: s, col: [0.36, 0.3, 0.24], kind: Kind::Cube, spin: 10.0 });
        }
        self.push(Particle { pos: pos + vec3(0.0, 0.6, 0.0), vel: Vec3::ZERO, life: 0.14, max: 0.14, size: 5.0, col: [1.0, 0.85, 0.55], kind: Kind::Spark, spin: 0.0 });
    }

    pub fn muzzle(&mut self, pos: Vec3) {
        self.push(Particle { pos, vel: Vec3::ZERO, life: 0.05, max: 0.05, size: 0.3, col: [1.0, 0.8, 0.45], kind: Kind::Spark, spin: 0.0 });
    }

    pub fn update(&mut self, dt: f32, w: &World) {
        for t in self.tracers.iter_mut() {
            t.head += 330.0 * dt;
        }
        self.tracers.retain(|t| t.head - 14.0 < t.len);
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
                    p.pos += p.vel * dt;
                    p.size += p.size * 1.1 * dt;
                }
            }
        }
        self.parts.retain(|p| p.life > 0.0);
    }

    pub fn build(&self, opaque: &mut Mesh, alpha: &mut Mesh, add: &mut Mesh, cam: Vec3, right: Vec3, up: Vec3) {
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
                    let s = p.size;
                    let c = [p.col[0], p.col[1], p.col[2], 0.3 * f];
                    alpha.quad(p.pos - right * s - up * s, p.pos + right * s - up * s, p.pos + right * s + up * s, p.pos - right * s + up * s, c);
                }
            }
        }
    }
}
