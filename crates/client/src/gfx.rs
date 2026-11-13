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

#[derive(Default)]
pub struct Mesh {
    pub v: Vec<Vertex>,
    pub i: Vec<u16>,
    pub chunks: Vec<(Vec<Vertex>, Vec<u16>)>,
}

impl Mesh {
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
            let s = Vec3::new(
                if k & 1 != 0 { 1.0 } else { -1.0 },
                if k & 2 != 0 { 1.0 } else { -1.0 },
                if k & 4 != 0 { 1.0 } else { -1.0 },
            );
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
}

const VS: &str = r#"#version 100
attribute vec3 in_pos;
attribute vec3 in_nrm;
attribute vec4 in_col;
uniform mat4 u_vp;
varying vec4 v_col;
varying vec3 v_nrm;
varying vec3 v_pos;
void main() {
    v_col = in_col;
    v_nrm = in_nrm;
    v_pos = in_pos;
    gl_Position = u_vp * vec4(in_pos, 1.0);
}"#;

// u_params.x: 0 = lit opaque (alpha channel = emissive), 1 = alpha blended,
// 2 = additive. u_params.y scales fog density.
const FS: &str = r#"#version 100
precision highp float;
varying vec4 v_col;
varying vec3 v_nrm;
varying vec3 v_pos;
uniform vec3 u_cam;
uniform vec3 u_sun;
uniform vec4 u_fog;
uniform vec4 u_params;
void main() {
    float dist = length(v_pos - u_cam);
    float x = dist * u_fog.w * u_params.y;
    float f = 1.0 - exp(-x * sqrt(x));
    vec3 c;
    float a = 1.0;
    if (u_params.x < 0.5) {
        vec3 n = normalize(v_nrm);
        float d = max(dot(n, u_sun), 0.0);
        float hemi = n.y * 0.5 + 0.5;
        vec3 amb = mix(vec3(0.34, 0.30, 0.36), vec3(0.50, 0.60, 0.74), hemi);
        vec3 lit = v_col.rgb * (amb + vec3(1.05, 0.93, 0.74) * d);
        c = mix(lit, v_col.rgb, v_col.a);
        c = mix(c, u_fog.rgb, f);
    } else if (u_params.x < 1.5) {
        c = mix(v_col.rgb, u_fog.rgb, f);
        a = v_col.a;
    } else {
        c = v_col.rgb * v_col.a * (1.0 - f);
    }
    gl_FragColor = vec4(c, a);
}"#;

struct DynBuf {
    b: Bindings,
    cap_v: usize,
    cap_i: usize,
    n: i32,
}

impl DynBuf {
    fn new(ctx: &mut dyn RenderingBackend, cap_v: usize) -> DynBuf {
        let cap_i = cap_v * 3 / 2 + 6;
        let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Stream, BufferSource::empty::<Vertex>(cap_v));
        let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Stream, BufferSource::empty::<u16>(cap_i));
        DynBuf { b: Bindings { vertex_buffers: vec![vb], index_buffer: ib, images: vec![] }, cap_v, cap_i, n: 0 }
    }

    fn upload(&mut self, ctx: &mut dyn RenderingBackend, m: &Mesh) {
        self.n = 0;
        // Dynamic geometry is expected to fit one chunk; overflow is dropped.
        let (v, i) = match m.chunks.first() {
            Some((v, i)) => (v.as_slice(), i.as_slice()),
            None => (m.v.as_slice(), m.i.as_slice()),
        };
        if v.is_empty() || v.len() > self.cap_v || i.len() > self.cap_i {
            return;
        }
        ctx.buffer_update(self.b.vertex_buffers[0], BufferSource::slice(v));
        ctx.buffer_update(self.b.index_buffer, BufferSource::slice(i));
        self.n = i.len() as i32;
    }
}

/// miniquad disables depth testing entirely for pipelines that do not
/// write depth, so blended pipelines are created depth-writing and the
/// write mask is switched off by hand around their draws.
fn depth_mask(on: bool) {
    unsafe { macroquad::miniquad::gl::glDepthMask(on as _) }
}

pub struct Frame<'a> {
    pub vp: Mat4,
    pub sky_vp: Mat4,
    pub cam: Vec3,
    pub sun: Vec3,
    pub fog: [f32; 4],
    pub opaque: &'a Mesh,
    pub alpha: &'a Mesh,
    pub add: &'a Mesh,
    pub view_vp: Mat4,
    pub view_sun: Vec3,
    pub view_opaque: &'a Mesh,
    pub view_add: &'a Mesh,
}

pub struct Gpu {
    opaque: Pipeline,
    alpha: Pipeline,
    add: Pipeline,
    statics: Vec<(Bindings, i32)>,
    sky: Vec<(Bindings, i32)>,
    d_opaque: DynBuf,
    d_alpha: DynBuf,
    d_add: DynBuf,
    d_view: DynBuf,
    d_view_add: DynBuf,
    pub tris: usize,
}

fn upload_static(ctx: &mut dyn RenderingBackend, mesh: Mesh) -> Vec<(Bindings, i32)> {
    mesh.finish()
        .into_iter()
        .map(|(v, i)| {
            let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(&v));
            let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&i));
            (Bindings { vertex_buffers: vec![vb], index_buffer: ib, images: vec![] }, i.len() as i32)
        })
        .collect()
}

impl Gpu {
    /// Free the GPU objects of a finished match.
    pub fn destroy(self) {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let dynamic = [&self.d_opaque.b, &self.d_alpha.b, &self.d_add.b, &self.d_view.b, &self.d_view_add.b];
        for b in self.statics.iter().chain(self.sky.iter()).map(|(b, _)| b).chain(dynamic) {
            for v in &b.vertex_buffers {
                ctx.delete_buffer(*v);
            }
            ctx.delete_buffer(b.index_buffer);
        }
        for p in [self.opaque, self.alpha, self.add] {
            ctx.delete_pipeline(p);
        }
    }

    pub fn new(world_mesh: Mesh, sky_mesh: Mesh) -> Gpu {
        let gl = unsafe { get_internal_gl() };
        let ctx = gl.quad_context;
        let meta = ShaderMeta {
            images: vec![],
            uniforms: UniformBlockLayout {
                uniforms: vec![
                    UniformDesc::new("u_vp", UniformType::Mat4),
                    UniformDesc::new("u_cam", UniformType::Float3),
                    UniformDesc::new("u_sun", UniformType::Float3),
                    UniformDesc::new("u_fog", UniformType::Float4),
                    UniformDesc::new("u_params", UniformType::Float4),
                ],
            },
        };
        let shader = ctx
            .new_shader(ShaderSource::Glsl { vertex: VS, fragment: FS }, meta)
            .unwrap_or_else(|e| panic!("shader compile failed: {e:?}"));
        let attrs = [
            VertexAttribute::new("in_pos", VertexFormat::Float3),
            VertexAttribute::new("in_nrm", VertexFormat::Float3),
            VertexAttribute::new("in_col", VertexFormat::Float4),
        ];
        let mk = |ctx: &mut dyn RenderingBackend, write: bool, blend: Option<BlendState>| {
            ctx.new_pipeline(
                &[BufferLayout::default()],
                &attrs,
                shader,
                PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: write,
                    color_blend: blend,
                    cull_face: CullFace::Nothing,
                    ..Default::default()
                },
            )
        };
        let opaque = mk(ctx, true, None);
        let alpha = mk(
            ctx,
            true,
            Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
        );
        let add = mk(ctx, true, Some(BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One)));
        let tris = world_mesh.chunks.iter().map(|c| c.1.len()).sum::<usize>() / 3 + world_mesh.i.len() / 3;
        Gpu {
            opaque,
            alpha,
            add,
            statics: upload_static(ctx, world_mesh),
            sky: upload_static(ctx, sky_mesh),
            d_opaque: DynBuf::new(ctx, 60000),
            d_alpha: DynBuf::new(ctx, 30000),
            d_add: DynBuf::new(ctx, 30000),
            d_view: DynBuf::new(ctx, 24000),
            d_view_add: DynBuf::new(ctx, 400),
            tris,
        }
    }

    pub fn render(&mut self, f: &Frame) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        self.d_opaque.upload(ctx, f.opaque);
        self.d_alpha.upload(ctx, f.alpha);
        self.d_add.upload(ctx, f.add);
        self.d_view.upload(ctx, f.view_opaque);
        self.d_view_add.upload(ctx, f.view_add);

        let uni = |vp: &Mat4, cam: Vec3, sun: Vec3, mode: f32, fog_k: f32| Uniforms {
            vp: vp.to_cols_array(),
            cam: cam.to_array(),
            sun: sun.to_array(),
            fog: f.fog,
            params: [mode, fog_k, 0.0, 0.0],
        };

        ctx.begin_default_pass(PassAction::Clear {
            color: Some((f.fog[0], f.fog[1], f.fog[2], 1.0)),
            depth: Some(1.0),
            stencil: None,
        });
        // Sky: camera-centred dome, no fog, no depth write.
        ctx.apply_pipeline(&self.alpha);
        depth_mask(false);
        ctx.apply_uniforms(UniformsSource::table(&uni(&f.sky_vp, Vec3::ZERO, f.sun, 1.0, 0.0)));
        for (b, n) in &self.sky {
            ctx.apply_bindings(b);
            ctx.draw(0, *n, 1);
        }
        ctx.apply_pipeline(&self.opaque);
        depth_mask(true);
        ctx.apply_uniforms(UniformsSource::table(&uni(&f.vp, f.cam, f.sun, 0.0, 1.0)));
        for (b, n) in &self.statics {
            ctx.apply_bindings(b);
            ctx.draw(0, *n, 1);
        }
        if self.d_opaque.n > 0 {
            ctx.apply_bindings(&self.d_opaque.b);
            ctx.draw(0, self.d_opaque.n, 1);
        }
        if self.d_alpha.n > 0 {
            ctx.apply_pipeline(&self.alpha);
            depth_mask(false);
            ctx.apply_uniforms(UniformsSource::table(&uni(&f.vp, f.cam, f.sun, 1.0, 1.0)));
            ctx.apply_bindings(&self.d_alpha.b);
            ctx.draw(0, self.d_alpha.n, 1);
        }
        if self.d_add.n > 0 {
            ctx.apply_pipeline(&self.add);
            depth_mask(false);
            ctx.apply_uniforms(UniformsSource::table(&uni(&f.vp, f.cam, f.sun, 2.0, 0.6)));
            ctx.apply_bindings(&self.d_add.b);
            ctx.draw(0, self.d_add.n, 1);
        }
        depth_mask(true);
        ctx.end_render_pass();

        // First-person weapon: own projection, depth cleared so it never
        // clips into the world.
        if self.d_view.n > 0 {
            ctx.begin_default_pass(PassAction::Clear { color: None, depth: Some(1.0), stencil: None });
            ctx.apply_pipeline(&self.opaque);
            ctx.apply_uniforms(UniformsSource::table(&uni(&f.view_vp, Vec3::ZERO, f.view_sun, 0.0, 0.0)));
            ctx.apply_bindings(&self.d_view.b);
            ctx.draw(0, self.d_view.n, 1);
            if self.d_view_add.n > 0 {
                ctx.apply_pipeline(&self.add);
                depth_mask(false);
                ctx.apply_uniforms(UniformsSource::table(&uni(&f.view_vp, Vec3::ZERO, f.view_sun, 2.0, 0.0)));
                ctx.apply_bindings(&self.d_view_add.b);
                ctx.draw(0, self.d_view_add.n, 1);
                depth_mask(true);
            }
            ctx.end_render_pass();
        }
    }
}
