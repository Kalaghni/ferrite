//! Small flat-shaded renderer on raw miniquad pipelines (macroquad is only
//! used for the window, input, audio and 2D HUD). Static world geometry is
//! uploaded once; everything that moves is rebuilt into stream buffers each
//! frame, which is cheap at this polygon budget.

use ferrite_shared::glam::{Mat4, Vec3};
use macroquad::miniquad::*;
use macroquad::window::get_internal_gl;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub col: [f32; 4],
}

pub type Col = [f32; 4];

pub fn rgb(r: f32, g: f32, b: f32) -> Col {
    [r, g, b, 0.0]
}

pub fn shade(c: Col, k: f32) -> Col {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

const MAX_V: usize = 65000;
/// Normals longer than this carry a wind-sway weight (see `Mesh::tri_w`).
pub const SWAY_BASE: f32 = 2.0;
/// How many facets each facet of a round shape (tube, lathe) is cut into.
pub const ROUND: usize = 2;

/// Colour of the light: direct sun, sky ambient, ground ambient.
pub type Tone = [[f32; 3]; 3];
pub const TONE: Tone = [[1.12, 0.98, 0.76], [0.46, 0.57, 0.74], [0.30, 0.27, 0.34]];

#[derive(Default)]
pub struct Mesh {
    pub v: Vec<Vertex>,
    pub i: Vec<u16>,
    pub chunks: Vec<(Vec<Vertex>, Vec<u16>)>,
}

impl Mesh {
    pub fn triangles(&self) -> usize {
        (self.i.len() + self.chunks.iter().map(|(_, i)| i.len()).sum::<usize>()) / 3
    }

    pub fn clear(&mut self) {
        self.v.clear();
        self.i.clear();
        self.chunks.clear();
    }

    fn reserve(&mut self, n: usize) -> u16 {
        if self.v.len() + n > MAX_V {
            let v = std::mem::take(&mut self.v);
            let i = std::mem::take(&mut self.i);
            self.chunks.push((v, i));
        }
        self.v.len() as u16
    }

    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Col) {
        let base = self.reserve(3);
        let n = (b - a).cross(c - a).normalize_or_zero().to_array();
        for p in [a, b, c] {
            self.v.push(Vertex { pos: p.to_array(), nrm: n, col });
        }
        self.i.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// Flat triangle with a colour per corner.
    pub fn tri_c(&mut self, a: Vec3, b: Vec3, c: Vec3, cols: [Col; 3]) {
        let base = self.reserve(3);
        let n = (b - a).cross(c - a).normalize_or_zero().to_array();
        for (p, col) in [a, b, c].into_iter().zip(cols) {
            self.v.push(Vertex { pos: p.to_array(), nrm: n, col });
        }
        self.i.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// Triangle with a given normal (grass takes the ground's shading).
    pub fn tri_n(&mut self, a: Vec3, b: Vec3, c: Vec3, n: Vec3, col: Col) {
        let base = self.reserve(3);
        for p in [a, b, c] {
            self.v.push(Vertex { pos: p.to_array(), nrm: n.to_array(), col });
        }
        self.i.extend_from_slice(&[base, base + 1, base + 2]);
    }

    /// Triangle that moves in the wind: `w` is how far, in metres at a
    /// strong gust, each corner sways (0 = rooted). Carried in the length
    /// of the normal, which the vertex shader reads back.
    pub fn tri_w(&mut self, p: [Vec3; 3], n: Vec3, w: [f32; 3], col: Col) {
        let base = self.reserve(3);
        for k in 0..3 {
            let nn = if w[k] > 0.0 { n * (SWAY_BASE + w[k].min(4.0)) } else { n };
            self.v.push(Vertex { pos: p[k].to_array(), nrm: nn.to_array(), col });
        }
        self.i.extend_from_slice(&[base, base + 1, base + 2]);
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Col) {
        self.quad_c(a, b, c, d, [col; 4]);
    }

    pub fn quad_c(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, cols: [Col; 4]) {
        let base = self.reserve(4);
        let n = (b - a).cross(d - a).normalize_or_zero().to_array();
        for (p, col) in [a, b, c, d].into_iter().zip(cols) {
            self.v.push(Vertex { pos: p.to_array(), nrm: n, col });
        }
        self.i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn corners(&mut self, p: [Vec3; 8], col: Col) {
        // p index = x + 2*y + 4*z
        self.quad(p[1], p[3], p[7], p[5], col); // +X
        self.quad(p[0], p[4], p[6], p[2], col); // -X
        self.quad(p[2], p[6], p[7], p[3], col); // +Y
        self.quad(p[0], p[1], p[5], p[4], col); // -Y
        self.quad(p[4], p[5], p[7], p[6], col); // +Z
        self.quad(p[0], p[2], p[3], p[1], col); // -Z
    }

    /// Quad with explicit per-vertex normals (for smooth-shaded curves).
    pub fn quad_n(&mut self, p: [Vec3; 4], n: [Vec3; 4], col: Col) {
        let base = self.reserve(4);
        for k in 0..4 {
            self.v.push(Vertex { pos: p[k].to_array(), nrm: n[k].to_array(), col });
        }
        self.i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Smooth-shaded cylinder along the local Z axis of `m`. `r1 > 0` makes
    /// it a hollow tube you can see through; `r1 == 0` is a solid rod.
    #[allow(clippy::too_many_arguments)]
    pub fn tube(&mut self, m: &Mat4, c: Vec3, r0: f32, r1: f32, half_len: f32, segs: usize, col: Col) {
        // Callers give the count the art was drawn at; every tube is cut
        // `ROUND` times as fine as that.
        let segs = segs * ROUND;
        let tau = std::f32::consts::TAU;
        let nz = m.transform_vector3(Vec3::Z).normalize_or_zero();
        for k in 0..segs {
            let (a0, a1) = (k as f32 / segs as f32 * tau, (k + 1) as f32 / segs as f32 * tau);
            let (d0, d1) = (Vec3::new(a0.cos(), a0.sin(), 0.0), Vec3::new(a1.cos(), a1.sin(), 0.0));
            let (n0, n1) = (m.transform_vector3(d0).normalize_or_zero(), m.transform_vector3(d1).normalize_or_zero());
            let pt = |d: Vec3, r: f32, z: f32| m.transform_point3(c + d * r + Vec3::new(0.0, 0.0, z));
            let (f, b) = (-half_len, half_len);
            self.quad_n([pt(d0, r0, f), pt(d1, r0, f), pt(d1, r0, b), pt(d0, r0, b)], [n0, n1, n1, n0], col);
            if r1 > 0.0 {
                let dark = [col[0] * 0.55, col[1] * 0.55, col[2] * 0.55, col[3]];
                self.quad_n([pt(d1, r1, f), pt(d0, r1, f), pt(d0, r1, b), pt(d1, r1, b)], [-n1, -n0, -n0, -n1], dark);
            }
            self.quad_n([pt(d0, r1, f), pt(d1, r1, f), pt(d1, r0, f), pt(d0, r0, f)], [-nz; 4], col);
            self.quad_n([pt(d1, r1, b), pt(d0, r1, b), pt(d0, r0, b), pt(d1, r0, b)], [nz; 4], col);
        }
    }

    pub fn cuboid(&mut self, min: Vec3, max: Vec3, col: Col) {
        let mut p = [Vec3::ZERO; 8];
        for (k, c) in p.iter_mut().enumerate() {
            *c = Vec3::new(
                if k & 1 != 0 { max.x } else { min.x },
                if k & 2 != 0 { max.y } else { min.y },
                if k & 4 != 0 { max.z } else { min.z },
            );
        }
        self.corners(p, col);
    }

    /// Box of half-extents `half` centred at `center` in the space of `m`.
    pub fn obox(&mut self, m: &Mat4, center: Vec3, half: Vec3, col: Col) {
        let mut p = [Vec3::ZERO; 8];
        for (k, c) in p.iter_mut().enumerate() {
            let s =
                Vec3::new(if k & 1 != 0 { 1.0 } else { -1.0 }, if k & 2 != 0 { 1.0 } else { -1.0 }, if k & 4 != 0 { 1.0 } else { -1.0 });
            *c = m.transform_point3(center + half * s);
        }
        self.corners(p, col);
    }

    pub fn finish(mut self) -> Vec<(Vec<Vertex>, Vec<u16>)> {
        if !self.v.is_empty() {
            self.chunks.push((self.v, self.i));
        }
        self.chunks
    }
}

#[repr(C)]
struct Uniforms {
    vp: [f32; 16],
    cam: [f32; 3],
    sun: [f32; 3],
    fog: [f32; 4],
    params: [f32; 4],
    light: [f32; 16],
    lights: [[f32; 4]; 4],
    lcols: [[f32; 4]; 4],
    sh: [f32; 4],
    tone: [[f32; 4]; 3],
    post: [f32; 4],
}

const VS: &str = r#"#version 100
attribute vec3 in_pos;
attribute vec3 in_nrm;
attribute vec4 in_col;
uniform mat4 u_vp;
uniform vec4 u_params;
varying vec4 v_col;
varying vec3 v_nrm;
varying vec3 v_pos;
void main() {
    v_col = in_col;
    v_nrm = in_nrm;
    vec3 p = in_pos;
    // Foliage: a normal longer than 2 carries how far the vertex sways.
    float w = max(length(in_nrm) - 2.0, 0.0);
    if (w > 0.0 && u_params.w > 0.0) {
        float t = u_params.w;
        float gust = 0.55 + 0.45 * sin(t * 0.31 + p.x * 0.021 + p.z * 0.017);
        float a = sin(t * 1.9 + p.x * 0.37 + p.z * 0.29) * 0.6 + sin(t * 0.83 + p.x * 0.11 - p.z * 0.07) * 0.4;
        float b = sin(t * 1.3 + p.z * 0.41 - p.x * 0.13);
        p.x += (0.35 + a) * gust * w * 0.26;
        p.z += b * gust * w * 0.12;
    }
    v_pos = p;
    gl_Position = u_vp * vec4(p, 1.0);
}"#;

// u_params.x: 0 = lit opaque (alpha channel = emissive), 1 = alpha blended,
// 2 = additive. u_params.y scales fog density, .z scales direct sunlight
// (the first-person weapon dims when you stand in shade), .w is the wind
// clock (0 = still air). u_tone: sun colour, sky ambient, ground ambient.
// u_sh: x = shadow texel size, y = shadows on, z = depth bias, w = grain on.
// u_post.x: 0 = draw to the screen, 1 = opaque alpha is the distance to the
// eye (depth of field), 2 = opaque alpha is -1 (never blurred).
const FS: &str = r#"#version 100
precision highp float;
varying vec4 v_col;
varying vec3 v_nrm;
varying vec3 v_pos;
uniform vec3 u_cam;
uniform vec3 u_sun;
uniform vec4 u_fog;
uniform vec4 u_params;
uniform mat4 u_light;
uniform vec4 u_lights[4];
uniform vec4 u_lcols[4];
uniform vec4 u_sh;
uniform vec4 u_tone[3];
uniform vec4 u_post;
uniform sampler2D u_shadow;

float unpack(vec4 c) {
    return dot(c, vec4(1.0, 1.0 / 255.0, 1.0 / 65025.0, 1.0 / 16581375.0));
}
float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}
float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}
float shadow(vec3 p, vec3 n, float ndl) {
    vec4 lp = u_light * vec4(p + n * (0.04 + 0.1 * (1.0 - ndl)), 1.0);
    vec3 q = lp.xyz * 0.5 + 0.5;
    if (q.x < 0.002 || q.x > 0.998 || q.y < 0.002 || q.y > 0.998 || q.z > 0.999) return 1.0;
    float z = q.z - u_sh.z;
    // Rotate the 3x3 kernel per pixel so the edge dithers instead of banding.
    float a = hash(gl_FragCoord.xy) * 6.2831853;
    vec2 r0 = vec2(cos(a), sin(a)) * u_sh.x * 1.1;
    vec2 r1 = vec2(-r0.y, r0.x);
    float s = 0.0;
    for (int i = -1; i <= 1; i++) {
        for (int j = -1; j <= 1; j++) {
            vec2 uv = q.xy + r0 * float(i) + r1 * float(j);
            s += step(z, unpack(texture2D(u_shadow, uv)));
        }
    }
    return s / 9.0;
}
void main() {
    vec3 to = v_pos - u_cam;
    float dist = length(to);
    // Haze pools in the valley and thins with height.
    float hk = mix(1.0, clamp(1.25 - (u_cam.y + v_pos.y) * 0.011, 0.35, 1.25), u_sh.w);
    float x = dist * u_fog.w * u_params.y * hk;
    float f = 1.0 - exp(-x * sqrt(x));
    float glare = pow(max(dot(to / max(dist, 0.001), u_sun), 0.0), 6.0);
    vec3 fogc = mix(u_fog.rgb, vec3(1.0, 0.93, 0.76), glare * 0.45 * u_sh.w);
    vec3 c;
    float a = 1.0;
    if (u_params.x < 0.5) {
        vec3 n = normalize(v_nrm);
        float ndl = dot(n, u_sun);
        float d = max(ndl, 0.0);
        if (d > 0.0 && u_sh.y > 0.5) {
            d *= shadow(v_pos, n, d);
        }
        d *= u_params.z;
        float hemi = n.y * 0.5 + 0.5;
        vec3 amb = mix(u_tone[2].rgb, u_tone[1].rgb, hemi);
        // Bounce: faces turned away from the sun pick up warm ground light.
        amb += vec3(0.10, 0.08, 0.05) * max(-ndl, 0.0) * (1.0 - hemi * 0.6);
        // Surface grain so a large face is not one flat colour.
        vec3 an = abs(n);
        vec2 gp = an.y > 0.6 ? v_pos.xz : (an.x > an.z ? v_pos.zy : v_pos.xy);
        float grain = (vnoise(gp * 0.9) - 0.5) * 0.10 + (vnoise(gp * 5.3) - 0.5) * 0.07;
        grain *= clamp(1.0 - dist / 70.0, 0.0, 1.0) * u_sh.w;
        vec3 base = v_col.rgb * (1.0 + grain);
        vec3 light = amb + u_tone[0].rgb * d;
        for (int i = 0; i < 4; i++) {
            vec3 lv = u_lights[i].xyz - v_pos;
            float r = u_lights[i].w;
            float at = clamp(1.0 - dot(lv, lv) / (r * r), 0.0, 1.0);
            float nl = 0.35 + 0.65 * max(dot(n, normalize(lv + vec3(0.0001))), 0.0);
            light += u_lcols[i].rgb * (at * at * nl * u_lcols[i].a);
        }
        vec3 lit = base * light;
        // Gentle highlight roll-off so bright faces do not clip.
        lit = lit / (1.0 + max(lit - 0.75, 0.0) * 0.6);
        c = mix(lit, v_col.rgb, v_col.a);
        c = mix(c, fogc, f);
        // Drawing for depth of field: alpha carries the distance to the
        // eye, or -1 for what is never blurred (the weapon in hand).
        if (u_post.x > 1.5) {
            a = -1.0;
        } else if (u_post.x > 0.5) {
            a = dist;
        }
    } else if (u_params.x < 1.5) {
        c = mix(v_col.rgb, fogc, f);
        a = v_col.a;
    } else {
        c = v_col.rgb * v_col.a * (1.0 - f);
    }
    gl_FragColor = vec4(c, a);
}"#;

const SHADOW_VS: &str = r#"#version 100
attribute vec3 in_pos;
uniform mat4 u_vp;
varying float v_depth;
void main() {
    gl_Position = u_vp * vec4(in_pos, 1.0);
    v_depth = gl_Position.z * 0.5 + 0.5;
}"#;

const SHADOW_FS: &str = r#"#version 100
precision highp float;
varying float v_depth;
void main() {
    vec4 enc = fract(clamp(v_depth, 0.0, 0.9999) * vec4(1.0, 255.0, 65025.0, 16581375.0));
    enc -= enc.yzww * vec4(1.0 / 255.0, 1.0 / 255.0, 1.0 / 255.0, 0.0);
    gl_FragColor = enc;
}"#;

struct DynBuf {
    pages: Vec<DynPage>,
    cap_hint: usize,
    tex: TextureId,
    n: i32,
}

struct DynPage {
    b: Bindings,
    cap_v: usize,
    cap_i: usize,
    n: i32,
}

impl DynPage {
    fn new(ctx: &mut dyn RenderingBackend, cap_v: usize, tex: TextureId) -> Self {
        let cap_i = cap_v * 3 / 2 + 6;
        let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Stream, BufferSource::empty::<Vertex>(cap_v));
        let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Stream, BufferSource::empty::<u16>(cap_i));
        Self { b: Bindings { vertex_buffers: vec![vb], index_buffer: ib, images: vec![tex] }, cap_v, cap_i, n: 0 }
    }
}

/// Mesh pages retain local u16 indices. Submit each page independently so
/// a full squad's detailed equipment cannot silently drop other soldiers.
fn mesh_pages(m: &Mesh) -> impl Iterator<Item = (&[Vertex], &[u16])> {
    m.chunks
        .iter()
        .map(|(v, i)| (v.as_slice(), i.as_slice()))
        .chain(std::iter::once((m.v.as_slice(), m.i.as_slice())))
        .filter(|(v, _)| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_dynamic_mesh_keeps_every_page_and_the_tail() {
        let mut mesh = Mesh::default();
        // A detailed platoon exceeds the u16 page limit, with geometry
        // remaining in the current page after the first two are sealed.
        let quads = MAX_V / 2 + 17;
        for k in 0..quads {
            let a = Vec3::new(k as f32, 0.0, 0.0);
            mesh.quad(a, a + Vec3::X, a + Vec3::X + Vec3::Y, a + Vec3::Y, rgb(0.5, 0.5, 0.5));
        }
        let pages: Vec<_> = mesh_pages(&mesh).collect();
        assert!(pages.len() >= 3);
        assert_eq!(pages.iter().map(|(v, _)| v.len()).sum::<usize>(), quads * 4);
        assert_eq!(pages.iter().map(|(_, i)| i.len()).sum::<usize>(), quads * 6);
        assert_eq!(mesh.triangles(), quads * 2);
        for (v, i) in pages {
            assert!(i.iter().all(|&n| (n as usize) < v.len()));
        }
        mesh.clear();
        assert_eq!(mesh_pages(&mesh).count(), 0);
    }
}

impl DynBuf {
    fn new(ctx: &mut dyn RenderingBackend, cap_v: usize, tex: TextureId) -> Self {
        Self { pages: vec![DynPage::new(ctx, cap_v, tex)], cap_hint: cap_v, tex, n: 0 }
    }

    fn upload(&mut self, ctx: &mut dyn RenderingBackend, m: &Mesh) {
        self.n = 0;
        for page in &mut self.pages {
            page.n = 0;
        }
        for (k, (v, i)) in mesh_pages(m).enumerate() {
            let capacity = self.cap_hint.max(v.len());
            if k == self.pages.len() {
                self.pages.push(DynPage::new(ctx, capacity, self.tex));
            }
            let page = &mut self.pages[k];
            if v.len() > page.cap_v || i.len() > page.cap_i {
                for vb in &page.b.vertex_buffers {
                    ctx.delete_buffer(*vb);
                }
                ctx.delete_buffer(page.b.index_buffer);
                *page = DynPage::new(ctx, capacity.max(i.len() * 2 / 3 + 1), self.tex);
            }
            ctx.buffer_update(page.b.vertex_buffers[0], BufferSource::slice(v));
            ctx.buffer_update(page.b.index_buffer, BufferSource::slice(i));
            page.n = i.len() as i32;
            self.n += page.n;
        }
    }

    fn draw(&self, ctx: &mut dyn RenderingBackend) {
        for page in self.pages.iter().filter(|p| p.n > 0) {
            ctx.apply_bindings(&page.b);
            ctx.draw(0, page.n, 1);
        }
    }
}

/// miniquad disables depth testing entirely for pipelines that do not
/// write depth, so blended pipelines are created depth-writing and the
/// write mask is switched off by hand around their draws.
fn tone4(t: &Tone) -> [[f32; 4]; 3] {
    [[t[0][0], t[0][1], t[0][2], 0.0], [t[1][0], t[1][1], t[1][2], 0.0], [t[2][0], t[2][1], t[2][2], 0.0]]
}

fn depth_mask(on: bool) {
    unsafe { macroquad::miniquad::gl::glDepthMask(on as _) }
}

/// A short-lived point light (muzzle flash, explosion).
#[derive(Clone, Copy, Default)]
pub struct Light {
    pub pos: Vec3,
    pub radius: f32,
    pub col: [f32; 3],
    pub power: f32,
}

pub struct Frame<'a> {
    pub vp: Mat4,
    pub sky_vp: Mat4,
    pub cam: Vec3,
    pub sun: Vec3,
    pub fog: [f32; 4],
    pub tone: Tone,
    /// Wind clock in seconds; foliage sways with it.
    pub time: f32,
    pub lights: [Light; 4],
    pub opaque: &'a Mesh,
    pub alpha: &'a Mesh,
    pub add: &'a Mesh,
    pub view_vp: Mat4,
    pub view_sun: Vec3,
    /// 0..1: how much direct sun reaches the first-person weapon.
    pub view_lit: f32,
    pub view_opaque: &'a Mesh,
    pub view_add: &'a Mesh,
}

/// Depth of field for the cinematic camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dof {
    /// Distance in focus, in metres.
    pub focus: f32,
    /// Blur radius of the far distance, as a fraction of the image height.
    /// Things nearer than the focus blur more, up to `MAX_BLUR`.
    pub blur: f32,
}

/// Largest blur radius, as a fraction of the image height.
pub const MAX_BLUR: f32 = 0.045;

/// What the depth-of-field pass reads for the sky: far away.
const SKY_DIST: f32 = 5000.0;

/// Gather blur over the scene drawn off screen. Alpha holds the distance
/// from the eye (negative: never blurred). Each pixel walks a golden-angle
/// spiral out to the largest blur radius; a sample counts where its own
/// blur circle reaches back to this pixel, so a blurred foreground spreads
/// over what is behind it while a sharp subject does not bleed into its
/// blurred background (after Dennis Gustafsson's single-pass bokeh).
const DOF_VS: &str = r#"#version 100
attribute vec2 in_pos;
varying vec2 v_uv;
void main() {
    v_uv = in_pos * 0.5 + 0.5;
    gl_Position = vec4(in_pos, 0.0, 1.0);
}"#;

// u_dof: x = focus distance (m), y = blur radius of the far distance (px),
// z = largest blur radius (px), w = samples (0 = plain copy).
// u_px: xy = one texel in uv, z = spiral step.
const DOF_FS: &str = r#"#version 100
precision highp float;
varying vec2 v_uv;
uniform sampler2D u_scene;
uniform vec4 u_dof;
uniform vec4 u_px;
float coc(float d) {
    if (d < 0.0) return 0.0;
    return min(abs(1.0 - u_dof.x / max(d, 0.05)) * u_dof.y, u_dof.z);
}
float hash(vec2 p) {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}
void main() {
    vec4 c0 = texture2D(u_scene, v_uv);
    c0.rgb = clamp(c0.rgb, 0.0, 3.0);
    if (u_dof.w < 0.5) {
        gl_FragColor = vec4(c0.rgb, 1.0);
        return;
    }
    float cs = coc(c0.a);
    vec3 acc = c0.rgb;
    float tot = 1.0;
    // Start the spiral at a different angle per pixel: noise, not rings.
    float ang = hash(gl_FragCoord.xy) * 6.2831853;
    float r = 0.5;
    for (int i = 0; i < 256; i++) {
        if (float(i) >= u_dof.w) break;
        vec4 s = texture2D(u_scene, v_uv + vec2(cos(ang), sin(ang)) * u_px.xy * r);
        // Highlights may be far past white (or past what half floats hold):
        // a little headroom makes bokeh of them, too much a solid disc.
        s.rgb = clamp(s.rgb, 0.0, 3.0);
        float ss = coc(s.a);
        // Something behind this pixel cannot blur over it by more than
        // this pixel's own blur.
        if (s.a > c0.a) ss = min(ss, cs * 2.0);
        acc += mix(acc / tot, s.rgb, smoothstep(r - 0.5, r + 0.5, ss));
        tot += 1.0;
        ang += 2.39996323;
        r += u_px.z / r;
    }
    gl_FragColor = vec4(acc / tot, 1.0);
}"#;

#[repr(C)]
struct PostUniforms {
    dof: [f32; 4],
    px: [f32; 4],
}

/// The scene drawn off screen for depth of field or a capture: multisampled
/// colour (alpha = distance) and depth, resolved into `color`. A capture
/// also has `out`, an image the size of the target to read back.
struct Target {
    w: u32,
    h: u32,
    textures: Vec<TextureId>,
    color: TextureId,
    pass: RenderPass,
    out: Option<(TextureId, RenderPass)>,
}

impl Target {
    fn new(ctx: &mut dyn RenderingBackend, w: u32, h: u32, samples: i32, out: bool) -> Target {
        let params = |format, sample_count| TextureParams {
            format,
            width: w,
            height: h,
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            wrap: TextureWrap::Clamp,
            sample_count,
            ..Default::default()
        };
        let ms_color = ctx.new_render_texture(params(TextureFormat::RGBA16F, samples));
        let ms_depth = ctx.new_render_texture(params(TextureFormat::Depth32, samples));
        let (color, pass, textures) = if samples > 1 {
            let color = ctx.new_render_texture(params(TextureFormat::RGBA16F, 1));
            (color, ctx.new_render_pass_mrt(&[ms_color], Some(&[color]), Some(ms_depth)), vec![ms_color, ms_depth, color])
        } else {
            (ms_color, ctx.new_render_pass(ms_color, Some(ms_depth)), vec![ms_color, ms_depth])
        };
        let out = out.then(|| {
            let t = ctx.new_render_texture(params(TextureFormat::RGBA8, 1));
            (t, ctx.new_render_pass(t, None))
        });
        Target { w, h, textures, color, pass, out }
    }

    fn free(self, ctx: &mut dyn RenderingBackend) {
        ctx.delete_render_pass(self.pass);
        for t in self.textures {
            ctx.delete_texture(t);
        }
        if let Some((t, p)) = self.out {
            ctx.delete_render_pass(p);
            ctx.delete_texture(t);
        }
    }
}

/// The depth-of-field pass and the screen-sized target it reads, made the
/// first time they are needed: a frame without depth of field never
/// touches them.
struct Post {
    pipe: Pipeline,
    shader: ShaderId,
    quad: Bindings,
    screen: Option<Target>,
}

struct Shadow {
    pass: RenderPass,
    color: TextureId,
    depth: TextureId,
    pipe: Pipeline,
    shader: ShaderId,
    vp: Mat4,
    texel: f32,
    bias: f32,
}

pub struct Gpu {
    opaque: Pipeline,
    alpha: Pipeline,
    add: Pipeline,
    shader: ShaderId,
    dummy: TextureId,
    shadow: Option<Shadow>,
    statics: Vec<(Bindings, i32)>,
    /// Static geometry that is drawn but casts no shadow (a map's distant
    /// high scenery, which would otherwise shade the whole field).
    scenery: Vec<(Bindings, i32)>,
    tex: TextureId,
    sky: Vec<(Bindings, i32)>,
    d_opaque: DynBuf,
    d_alpha: DynBuf,
    d_add: DynBuf,
    d_view: DynBuf,
    d_view_add: DynBuf,
    d_overlay: DynBuf,
    pub tris: usize,
    /// Depth of field for the next `render`. None draws straight to the
    /// screen, exactly as if this did not exist.
    pub dof: Option<Dof>,
    post: Option<Post>,
}

fn upload_static(ctx: &mut dyn RenderingBackend, mesh: Mesh, tex: TextureId) -> Vec<(Bindings, i32)> {
    mesh.finish()
        .into_iter()
        .map(|(v, i)| {
            let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(&v));
            let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&i));
            (Bindings { vertex_buffers: vec![vb], index_buffer: ib, images: vec![tex] }, i.len() as i32)
        })
        .collect()
}

fn uniform_meta(images: Vec<String>) -> ShaderMeta {
    ShaderMeta {
        images,
        uniforms: UniformBlockLayout {
            uniforms: vec![
                UniformDesc::new("u_vp", UniformType::Mat4),
                UniformDesc::new("u_cam", UniformType::Float3),
                UniformDesc::new("u_sun", UniformType::Float3),
                UniformDesc::new("u_fog", UniformType::Float4),
                UniformDesc::new("u_params", UniformType::Float4),
                UniformDesc::new("u_light", UniformType::Mat4),
                UniformDesc::new("u_lights", UniformType::Float4).array(4),
                UniformDesc::new("u_lcols", UniformType::Float4).array(4),
                UniformDesc::new("u_sh", UniformType::Float4),
                UniformDesc::new("u_tone", UniformType::Float4).array(3),
                UniformDesc::new("u_post", UniformType::Float4),
            ],
        },
    }
}

impl Gpu {
    /// Add static scenery that is drawn but casts no shadow.
    pub fn with_scenery(mut self, mesh: Mesh) -> Self {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        self.tris += mesh.chunks.iter().map(|c| c.1.len()).sum::<usize>() / 3 + mesh.i.len() / 3;
        let more = upload_static(ctx, mesh, self.tex);
        self.scenery.extend(more);
        self
    }

    /// Free the GPU objects of a finished match.
    pub fn destroy(self) {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let dynamic = [&self.d_opaque, &self.d_alpha, &self.d_add, &self.d_view, &self.d_view_add, &self.d_overlay];
        for b in self
            .statics
            .iter()
            .chain(self.scenery.iter())
            .chain(self.sky.iter())
            .map(|(b, _)| b)
            .chain(dynamic.iter().flat_map(|d| d.pages.iter().map(|p| &p.b)))
        {
            for v in &b.vertex_buffers {
                ctx.delete_buffer(*v);
            }
            ctx.delete_buffer(b.index_buffer);
        }
        for p in [self.opaque, self.alpha, self.add] {
            ctx.delete_pipeline(p);
        }
        ctx.delete_shader(self.shader);
        if let Some(s) = self.shadow {
            ctx.delete_pipeline(s.pipe);
            ctx.delete_shader(s.shader);
            ctx.delete_render_pass(s.pass);
            ctx.delete_texture(s.color);
            ctx.delete_texture(s.depth);
        }
        ctx.delete_texture(self.dummy);
        if let Some(p) = self.post {
            ctx.delete_pipeline(p.pipe);
            ctx.delete_shader(p.shader);
            for v in &p.quad.vertex_buffers {
                ctx.delete_buffer(*v);
            }
            ctx.delete_buffer(p.quad.index_buffer);
            for t in p.quad.images {
                ctx.delete_texture(t);
            }
            if let Some(t) = p.screen {
                t.free(ctx);
            }
        }
    }

    /// `shadow_res`: side of the sun shadow map in texels, 0 for none.
    /// `bounds`: world-space box the shadow map has to cover.
    pub fn new(world_mesh: Mesh, sky_mesh: Mesh, shadow_res: u32, sun: Vec3, bounds: (Vec3, Vec3)) -> Gpu {
        let gl = unsafe { get_internal_gl() };
        let ctx = gl.quad_context;
        let shader = ctx
            .new_shader(ShaderSource::Glsl { vertex: VS, fragment: FS }, uniform_meta(vec!["u_shadow".to_string()]))
            .unwrap_or_else(|e| panic!("shader compile failed: {e:?}"));
        let attrs = [
            VertexAttribute::new("in_pos", VertexFormat::Float3),
            VertexAttribute::new("in_nrm", VertexFormat::Float3),
            VertexAttribute::new("in_col", VertexFormat::Float4),
        ];
        // Blended draws leave the alpha underneath alone: off screen it
        // holds the distance the depth-of-field pass reads.
        let keep_alpha = BlendState::new(Equation::Add, BlendFactor::Zero, BlendFactor::One);
        let mk = |ctx: &mut dyn RenderingBackend, shader: ShaderId, blend: Option<BlendState>| {
            ctx.new_pipeline(
                &[BufferLayout::default()],
                &attrs,
                shader,
                PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: true,
                    color_blend: blend,
                    alpha_blend: blend.map(|_| keep_alpha),
                    cull_face: CullFace::Nothing,
                    ..Default::default()
                },
            )
        };
        let opaque = mk(ctx, shader, None);
        let alpha = mk(
            ctx,
            shader,
            Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
        );
        let add = mk(ctx, shader, Some(BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One)));
        let dummy = ctx.new_texture_from_rgba8(1, 1, &[255, 255, 255, 255]);

        let shadow = if shadow_res > 0 {
            let params = |format| TextureParams {
                format,
                width: shadow_res,
                height: shadow_res,
                min_filter: FilterMode::Nearest,
                mag_filter: FilterMode::Nearest,
                wrap: TextureWrap::Clamp,
                ..Default::default()
            };
            let color = ctx.new_render_texture(params(TextureFormat::RGBA8));
            let depth = ctx.new_render_texture(params(TextureFormat::Depth));
            let pass = ctx.new_render_pass(color, Some(depth));
            let sh = ctx
                .new_shader(ShaderSource::Glsl { vertex: SHADOW_VS, fragment: SHADOW_FS }, uniform_meta(vec![]))
                .unwrap_or_else(|e| panic!("shadow shader compile failed: {e:?}"));
            let pipe = mk(ctx, sh, None);
            // Fit an orthographic frustum, seen from the sun, around the valley.
            let centre = (bounds.0 + bounds.1) * 0.5;
            let view = Mat4::look_at_rh(centre + sun * 400.0, centre, Vec3::Y);
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for k in 0..8 {
                let p = Vec3::new(
                    if k & 1 != 0 { bounds.1.x } else { bounds.0.x },
                    if k & 2 != 0 { bounds.1.y } else { bounds.0.y },
                    if k & 4 != 0 { bounds.1.z } else { bounds.0.z },
                );
                let q = view.transform_point3(p);
                lo = lo.min(q);
                hi = hi.max(q);
            }
            let proj = Mat4::orthographic_rh_gl(lo.x, hi.x, lo.y, hi.y, -hi.z - 5.0, -lo.z + 5.0);
            let depth_range = hi.z - lo.z + 10.0;
            let texel_m = (hi.x - lo.x).max(hi.y - lo.y) / shadow_res as f32;
            Some(Shadow {
                pass,
                color,
                depth,
                pipe,
                shader: sh,
                vp: proj * view,
                texel: 1.0 / shadow_res as f32,
                bias: (0.04 + texel_m * 0.8) / depth_range,
            })
        } else {
            None
        };
        let tex = shadow.as_ref().map_or(dummy, |s| s.color);
        let tris = world_mesh.chunks.iter().map(|c| c.1.len()).sum::<usize>() / 3 + world_mesh.i.len() / 3;
        Gpu {
            opaque,
            alpha,
            add,
            shader,
            dummy,
            statics: upload_static(ctx, world_mesh, tex),
            scenery: Vec::new(),
            tex,
            sky: upload_static(ctx, sky_mesh, tex),
            d_opaque: DynBuf::new(ctx, 64000, tex),
            d_alpha: DynBuf::new(ctx, 30000, tex),
            d_add: DynBuf::new(ctx, 30000, tex),
            d_view: DynBuf::new(ctx, 40000, tex),
            d_view_add: DynBuf::new(ctx, 400, tex),
            d_overlay: DynBuf::new(ctx, 60000, tex),
            shadow,
            tris,
            dof: None,
            post: None,
        }
    }

    /// Lit models drawn over the 2D HUD (the weapons in the loadout
    /// panel). `mesh` is in pixels, +Y up, origin at the bottom left.
    /// Whatever the HUD has drawn so far goes underneath.
    pub fn overlay(&mut self, mesh: &Mesh, w: f32, h: f32) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        self.d_overlay.upload(ctx, mesh);
        if self.d_overlay.n == 0 {
            return;
        }
        let u = Uniforms {
            vp: Mat4::orthographic_rh_gl(0.0, w, 0.0, h, -4000.0, 4000.0).to_cols_array(),
            cam: [0.0; 3],
            sun: Vec3::new(0.35, 0.75, 0.56).normalize().to_array(),
            fog: [0.0; 4],
            params: [0.0, 0.0, 1.0, 0.0],
            light: Mat4::IDENTITY.to_cols_array(),
            lights: [[0.0, 0.0, 0.0, 0.01]; 4],
            lcols: [[0.0; 4]; 4],
            sh: [0.0; 4],
            tone: tone4(&TONE),
            post: [0.0; 4],
        };
        ctx.begin_default_pass(PassAction::Clear { color: None, depth: Some(1.0), stencil: None });
        ctx.apply_pipeline(&self.opaque);
        depth_mask(true);
        ctx.apply_uniforms(UniformsSource::table(&u));
        self.d_overlay.draw(ctx);
        ctx.end_render_pass();
    }

    pub fn shadows(&self) -> bool {
        self.shadow.is_some()
    }

    /// Draw a frame to the screen, through the depth-of-field pass when
    /// `dof` is set.
    pub fn render(&mut self, f: &Frame) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        self.prepare(ctx, f);
        let Some(dof) = self.dof else {
            self.draw_world(ctx, f, None, 0.0);
            self.draw_view(ctx, f, None, 0.0);
            return;
        };
        let (w, h) = window::screen_size();
        let (w, h) = (w.max(1.0) as u32, h.max(1.0) as u32);
        let mut post = self.post.take().unwrap_or_else(|| Post::new(ctx));
        if post.screen.as_ref().is_some_and(|t| (t.w, t.h) != (w, h)) {
            if let Some(t) = post.screen.take() {
                t.free(ctx);
            }
        }
        let target = post.screen.take().unwrap_or_else(|| Target::new(ctx, w, h, 4, false));
        self.draw_world(ctx, f, Some(target.pass), 1.0);
        self.draw_view(ctx, f, Some(target.pass), 2.0);
        ctx.begin_default_pass(PassAction::Nothing);
        post.apply(ctx, target.color, w, h, Some(dof), 64.0);
        ctx.end_render_pass();
        post.screen = Some(target);
        self.post = Some(post);
    }

    /// Draw a frame off screen at `w` x `h` (the screen's aspect ratio, a
    /// whole multiple of its size for a sharper picture) and read it back:
    /// RGBA, bottom row first, as `macroquad::texture::Image` holds it.
    pub fn capture(&mut self, f: &Frame, w: u32, h: u32) -> Vec<u8> {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        self.prepare(ctx, f);
        let mut post = self.post.take().unwrap_or_else(|| Post::new(ctx));
        // Multisampling a very large picture costs a lot of memory; past a
        // 4K picture it is supersampled enough once scaled down.
        let samples = if w as u64 * h as u64 <= 3840 * 2160 { 4 } else { 1 };
        let target = Target::new(ctx, w, h, samples, true);
        self.draw_world(ctx, f, Some(target.pass), 1.0);
        self.draw_view(ctx, f, Some(target.pass), 2.0);
        let mut px = vec![0u8; w as usize * h as usize * 4];
        if let Some((out, out_pass)) = target.out {
            ctx.begin_pass(Some(out_pass), PassAction::Nothing);
            post.apply(ctx, target.color, w, h, self.dof, 256.0);
            ctx.end_render_pass();
            ctx.texture_read_pixels(out, &mut px);
        }
        target.free(ctx);
        self.post = Some(post);
        px
    }

    /// Upload this frame's moving geometry and draw the sun's shadow map.
    fn prepare(&mut self, ctx: &mut dyn RenderingBackend, f: &Frame) {
        self.d_opaque.upload(ctx, f.opaque);
        self.d_alpha.upload(ctx, f.alpha);
        self.d_add.upload(ctx, f.add);
        self.d_view.upload(ctx, f.view_opaque);
        self.d_view_add.upload(ctx, f.view_add);
        // Sun shadow map: everything solid, seen from the sun.
        if let Some(s) = &self.shadow {
            ctx.begin_pass(Some(s.pass), PassAction::Clear { color: Some((1.0, 1.0, 1.0, 1.0)), depth: Some(1.0), stencil: None });
            ctx.apply_pipeline(&s.pipe);
            ctx.apply_uniforms(UniformsSource::table(&self.uni(f, &s.vp, Vec3::ZERO, f.sun, [0.0, 0.0, 1.0], false, 0.0)));
            for (b, n) in &self.statics {
                ctx.apply_bindings(b);
                ctx.draw(0, *n, 1);
            }
            if self.d_opaque.n > 0 {
                self.d_opaque.draw(ctx);
            }
            ctx.end_render_pass();
        }
    }

    /// `m`: draw mode, fog scale, sunlight scale (see the fragment shader).
    /// `world`: lit by the map's lights and shadows, swaying in the wind.
    #[allow(clippy::too_many_arguments)]
    fn uni(&self, f: &Frame, vp: &Mat4, cam: Vec3, sun: Vec3, m: [f32; 3], world: bool, post: f32) -> Uniforms {
        let (light_vp, sh) = match &self.shadow {
            Some(s) => (s.vp, [s.texel, 1.0, s.bias, 1.0]),
            None => (Mat4::IDENTITY, [0.0, 0.0, 0.0, 1.0]),
        };
        let mut lights = [[0.0f32, 0.0, 0.0, 0.01]; 4];
        let mut lcols = [[0.0f32; 4]; 4];
        if world {
            for (k, l) in f.lights.iter().enumerate() {
                lights[k] = [l.pos.x, l.pos.y, l.pos.z, l.radius.max(0.01)];
                lcols[k] = [l.col[0], l.col[1], l.col[2], l.power];
            }
        }
        Uniforms {
            vp: vp.to_cols_array(),
            cam: cam.to_array(),
            sun: sun.to_array(),
            fog: f.fog,
            params: [m[0], m[1], m[2], if world { f.time.max(0.001) } else { 0.0 }],
            tone: tone4(&f.tone),
            light: light_vp.to_cols_array(),
            lights,
            lcols,
            sh: if world { sh } else { [0.0; 4] },
            post: [post, 0.0, 0.0, 0.0],
        }
    }

    /// Sky and world, into `pass` (None = the screen); `post` is `u_post.x`.
    fn draw_world(&self, ctx: &mut dyn RenderingBackend, f: &Frame, pass: Option<RenderPass>, post: f32) {
        let sky_a = if post > 0.0 { SKY_DIST } else { 1.0 };
        ctx.begin_pass(pass, PassAction::Clear { color: Some((f.fog[0], f.fog[1], f.fog[2], sky_a)), depth: Some(1.0), stencil: None });
        // Sky: camera-centred dome, no fog, no depth write.
        ctx.apply_pipeline(&self.alpha);
        depth_mask(false);
        ctx.apply_uniforms(UniformsSource::table(&self.uni(f, &f.sky_vp, Vec3::ZERO, f.sun, [1.0, 0.0, 1.0], false, 0.0)));
        for (b, n) in &self.sky {
            ctx.apply_bindings(b);
            ctx.draw(0, *n, 1);
        }
        ctx.apply_pipeline(&self.opaque);
        depth_mask(true);
        ctx.apply_uniforms(UniformsSource::table(&self.uni(f, &f.vp, f.cam, f.sun, [0.0, 1.0, 1.0], true, post)));
        for (b, n) in self.statics.iter().chain(self.scenery.iter()) {
            ctx.apply_bindings(b);
            ctx.draw(0, *n, 1);
        }
        if self.d_opaque.n > 0 {
            self.d_opaque.draw(ctx);
        }
        if self.d_alpha.n > 0 {
            ctx.apply_pipeline(&self.alpha);
            depth_mask(false);
            ctx.apply_uniforms(UniformsSource::table(&self.uni(f, &f.vp, f.cam, f.sun, [1.0, 1.0, 1.0], true, post)));
            self.d_alpha.draw(ctx);
        }
        if self.d_add.n > 0 {
            ctx.apply_pipeline(&self.add);
            depth_mask(false);
            ctx.apply_uniforms(UniformsSource::table(&self.uni(f, &f.vp, f.cam, f.sun, [2.0, 0.6, 1.0], true, post)));
            self.d_add.draw(ctx);
        }
        depth_mask(true);
        ctx.end_render_pass();
    }

    /// First-person weapon: own projection, depth cleared so it never
    /// clips into the world.
    fn draw_view(&self, ctx: &mut dyn RenderingBackend, f: &Frame, pass: Option<RenderPass>, post: f32) {
        if self.d_view.n == 0 {
            return;
        }
        ctx.begin_pass(pass, PassAction::Clear { color: None, depth: Some(1.0), stencil: None });
        ctx.apply_pipeline(&self.opaque);
        let lit = 0.3 + 0.7 * f.view_lit;
        ctx.apply_uniforms(UniformsSource::table(&self.uni(f, &f.view_vp, Vec3::ZERO, f.view_sun, [0.0, 0.0, lit], false, post)));
        self.d_view.draw(ctx);
        if self.d_view_add.n > 0 {
            ctx.apply_pipeline(&self.add);
            depth_mask(false);
            ctx.apply_uniforms(UniformsSource::table(&self.uni(f, &f.view_vp, Vec3::ZERO, f.view_sun, [2.0, 0.0, 1.0], false, post)));
            self.d_view_add.draw(ctx);
            depth_mask(true);
        }
        ctx.end_render_pass();
    }
}

impl Post {
    fn new(ctx: &mut dyn RenderingBackend) -> Post {
        let shader = ctx
            .new_shader(
                ShaderSource::Glsl { vertex: DOF_VS, fragment: DOF_FS },
                ShaderMeta {
                    images: vec!["u_scene".to_string()],
                    uniforms: UniformBlockLayout {
                        uniforms: vec![UniformDesc::new("u_dof", UniformType::Float4), UniformDesc::new("u_px", UniformType::Float4)],
                    },
                },
            )
            .unwrap_or_else(|e| panic!("depth of field shader compile failed: {e:?}"));
        let pipe = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[VertexAttribute::new("in_pos", VertexFormat::Float2)],
            shader,
            PipelineParams::default(),
        );
        let corners: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(&corners));
        let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&[0u16, 1, 2, 0, 2, 3]));
        let blank = ctx.new_texture_from_rgba8(1, 1, &[0, 0, 0, 255]);
        Post { pipe, shader, quad: Bindings { vertex_buffers: vec![vb], index_buffer: ib, images: vec![blank] }, screen: None }
    }

    /// Draw `scene` (w x h) over the current pass, blurred by `dof` with up
    /// to `most` samples a pixel (fewer on screen, more for a picture).
    fn apply(&mut self, ctx: &mut dyn RenderingBackend, scene: TextureId, w: u32, h: u32, dof: Option<Dof>, most: f32) {
        let (far, max, samples, step) = match dof {
            Some(d) if d.blur > 0.0 => {
                let far = d.blur.min(MAX_BLUR) * h as f32;
                let max = (far * 2.5).min(MAX_BLUR * h as f32).max(1.0);
                // Enough samples to cover the blur circle without gaps.
                let samples = (max * max * 0.45).clamp(24.0, most);
                (far, max, samples, max * max / (2.0 * samples))
            }
            _ => (0.0, 0.0, 0.0, 0.0),
        };
        let u = PostUniforms {
            dof: [dof.map_or(10.0, |d| d.focus.max(0.1)), far, max, samples],
            px: [1.0 / w as f32, 1.0 / h as f32, step, 0.0],
        };
        let blank = std::mem::replace(&mut self.quad.images[0], scene);
        ctx.apply_pipeline(&self.pipe);
        ctx.apply_bindings(&self.quad);
        ctx.apply_uniforms(UniformsSource::table(&u));
        ctx.draw(0, 6, 1);
        self.quad.images[0] = blank;
    }
}
