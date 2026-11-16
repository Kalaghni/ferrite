//! Underground tunnels: a walkable floor below the visible terrain.
//!
//! The heightfield stays the visible surface - the hill over a tunnel. A
//! tunnel adds a cavity under it. Inside the tunnel's footprint and below
//! the surface, the floor is the tunnel's own, a flat roof stops heads, and
//! the rock around the cavity stops bodies and bullets. Each end is an open
//! cut, carved into the heightfield, that slopes down to the portal.
//!
//! Quarry's mine runs from an adit in the pit wall, under the terraces, up
//! an incline to an exit beside the headframe. Ridgeline's cave bends
//! through the ridge. Each tunnel has a partner mirrored under a half turn.
use super::{smooth, World};
use crate::glam::{vec2, vec3, Vec2, Vec3, Vec3Swizzles};

/// Clear width either side of the centreline (collision).
pub const HALF_WIDTH: f32 = 2.3;
/// Clear height above the floor (collision). The drawn roof is higher.
pub const HEIGHT: f32 = 3.3;
/// Least rock over the roof.
pub const COVER: f32 = 1.6;
/// Usual length of the open cut at each end, outside the portal.
pub const CUT: f32 = 9.0;
/// Beyond each portal the cavity reaches this far into the cut, so the
/// terrain cells that cross the portal never decide where feet go.
pub const MOUTH: f32 = 3.0;
/// Steepest floor, rise over run.
const MAX_SLOPE: f32 = 0.3;
/// Spacing of centreline samples.
const STEP: f32 = 0.75;
/// Zone around a tunnel where steep ground counts as rock wall.
const ZONE: f32 = 9.0;

/// One tunnel, as sampled centreline points.
#[derive(Clone, Debug)]
pub struct Tunnel {
    /// x, floor height, z.
    pub pts: Vec<Vec3>,
    /// Arc length at each point.
    pub s: Vec<f32>,
    /// Arc lengths of the two portals; between them the tunnel is roofed.
    pub portals: [f32; 2],
    lo: Vec2,
    hi: Vec2,
}

/// Where a point lies relative to a tunnel.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    /// Arc length of the nearest centreline point.
    pub s: f32,
    /// Signed distance from the centreline (positive to the left).
    pub d: f32,
    /// How far past either end, along the tunnel.
    pub beyond: f32,
    pub floor: f32,
    /// Unit direction of travel at `s`.
    pub dir: Vec2,
}

impl Tunnel {
    pub fn length(&self) -> f32 {
        *self.s.last().unwrap_or(&0.0)
    }

    /// The centreline at arc length `s`: (x, floor, z) and direction.
    pub fn at(&self, s: f32) -> (Vec3, Vec2) {
        let s = s.clamp(0.0, self.length());
        let i = self.s.partition_point(|&v| v <= s).clamp(1, self.pts.len() - 1) - 1;
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        let t = ((s - self.s[i]) / (self.s[i + 1] - self.s[i]).max(1e-6)).clamp(0.0, 1.0);
        (a.lerp(b, t), vec2(b.x - a.x, b.z - a.z).normalize_or_zero())
    }

    /// The nearest centreline point to (x, z), if it is within `reach`.
    pub fn locate(&self, x: f32, z: f32, reach: f32) -> Option<Spot> {
        if x < self.lo.x - reach || x > self.hi.x + reach || z < self.lo.y - reach || z > self.hi.y + reach {
            return None;
        }
        let p = vec2(x, z);
        let mut best = (f32::MAX, 0usize, 0.0f32);
        for i in 0..self.pts.len() - 1 {
            let (a, b) = (self.pts[i].xz(), self.pts[i + 1].xz());
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
            let d2 = (a + ab * t).distance_squared(p);
            if d2 < best.0 {
                best = (d2, i, t);
            }
        }
        if best.0 > reach * reach {
            return None;
        }
        let (_, i, t) = best;
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        let dir = vec2(b.x - a.x, b.z - a.z).normalize_or_zero();
        let q = a.xz().lerp(b.xz(), t);
        let rel = p - q;
        // Along-track excess past an end; lateral offset otherwise.
        let along = rel.dot(dir);
        let beyond =
            if (i == 0 && t == 0.0 && along < 0.0) || (i + 2 == self.pts.len() && t == 1.0 && along > 0.0) { along.abs() } else { 0.0 };
        Some(Spot { s: self.s[i] + (self.s[i + 1] - self.s[i]) * t, d: dir.perp_dot(rel), beyond, floor: a.y + (b.y - a.y) * t, dir })
    }

    /// Whether arc length `s` is inside the cavity (portals plus mouths).
    pub fn in_cavity(&self, s: f32) -> bool {
        s > self.portals[0] - MOUTH && s < self.portals[1] + MOUTH
    }

    /// Whether arc length `s` has rock overhead.
    pub fn roofed(&self, s: f32) -> bool {
        s > self.portals[0] && s < self.portals[1]
    }

    fn mirrored(&self) -> Tunnel {
        Tunnel {
            pts: self.pts.iter().map(|p| vec3(-p.x, p.y, -p.z)).collect(),
            s: self.s.clone(),
            portals: self.portals,
            lo: -self.hi,
            hi: -self.lo,
        }
    }
}

/// An authored route: centreline control points and how far the middle of
/// the floor sinks below a straight line between the two ends.
struct Plan {
    pts: &'static [(f32, f32)],
    dip: f32,
    /// Length of the open cut at each end.
    cuts: [f32; 2],
}

fn plan(map: u8) -> Option<Plan> {
    match map {
        // An adit from the first terrace into the face of the second, round
        // under the second terrace and the headframe's shaft, then a long
        // incline up to daylight on that terrace.
        1 => Some(Plan {
            pts: &[(-12.0, -45.6), (-18.4, -49.9), (-32.6, -51.3), (-50.5, -42.6), (-62.9, -32.3), (-70.6, -21.9)],
            dip: 0.0,
            cuts: [9.0, 17.0],
        }),
        // Through the ridge between the saddle and the southern pass, with a
        // bend so it is never one straight sightline.
        2 => Some(Plan { pts: &[(-37.0, -29.0), (-26.0, -29.5), (-13.0, -37.5), (0.5, -45.0), (12.5, -47.5)], dip: 0.0, cuts: [CUT, CUT] }),
        _ => None,
    }
}

fn catmull(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * p1 + (p2 - p0) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// The tunnels of a map (both halves), with floors set from `ground`: the
/// terrain as it would be without tunnels.
pub fn layout(map: u8, ground: impl Fn(f32, f32) -> f32) -> Vec<Tunnel> {
    let Some(plan) = plan(map) else { return Vec::new() };
    let c: Vec<Vec2> = plan.pts.iter().map(|&(x, z)| vec2(x, z)).collect();
    // A dense curve, then resampled at equal arc lengths.
    let mut dense = vec![c[0]];
    for i in 0..c.len() - 1 {
        let p0 = if i == 0 { c[0] * 2.0 - c[1] } else { c[i - 1] };
        let p3 = if i + 2 >= c.len() { c[i + 1] * 2.0 - c[i] } else { c[i + 2] };
        for k in 1..=40 {
            dense.push(catmull(p0, c[i], c[i + 1], p3, k as f32 / 40.0));
        }
    }
    let mut acc = vec![0.0f32];
    for i in 1..dense.len() {
        acc.push(acc[i - 1] + dense[i].distance(dense[i - 1]));
    }
    let total = *acc.last().unwrap();
    let n = (total / STEP).ceil() as usize;
    let (f0, f1) = (ground(c[0].x, c[0].y), ground(c[c.len() - 1].x, c[c.len() - 1].y));
    let mut pts = Vec::with_capacity(n + 1);
    let mut s = Vec::with_capacity(n + 1);
    let mut j = 0;
    for k in 0..=n {
        let want = total * k as f32 / n as f32;
        while j + 2 < acc.len() && acc[j + 1] < want {
            j += 1;
        }
        let t = ((want - acc[j]) / (acc[j + 1] - acc[j]).max(1e-6)).clamp(0.0, 1.0);
        let xz = dense[j].lerp(dense[j + 1], t);
        let u = want / total;
        let floor = f0 + (f1 - f0) * u - plan.dip * (u * std::f32::consts::PI).sin();
        pts.push(vec3(xz.x, floor, xz.y));
        s.push(want);
    }
    // Sink the roofed stretch until the natural ground covers it, then
    // limit the floor's slope so it stays walkable; the cuts ramp down to
    // it. Whatever cover is still missing becomes a mound in `sculpt`.
    let roofed = |want: f32| want > plan.cuts[0] - 1.0 && want < total - plan.cuts[1] + 1.0;
    for (k, p) in pts.iter_mut().enumerate() {
        if roofed(s[k]) {
            p.y = p.y.min(ground(p.x, p.z) - HEIGHT - COVER - 0.6);
        }
    }
    let rise = MAX_SLOPE * total / n as f32;
    let last = pts.len() - 1;
    for _ in 0..4 {
        for k in 1..last {
            pts[k].y = pts[k].y.clamp(pts[k - 1].y - rise, pts[k - 1].y + rise);
        }
        for k in (1..last).rev() {
            pts[k].y = pts[k].y.clamp(pts[k + 1].y - rise, pts[k + 1].y + rise);
        }
    }
    let raw: Vec<f32> = pts.iter().map(|p| p.y).collect();
    for k in 2..last.saturating_sub(1) {
        pts[k].y = (raw[k - 2] + raw[k - 1] + raw[k] + raw[k + 1] + raw[k + 2]) / 5.0;
    }
    let lo = pts.iter().fold(Vec2::MAX, |m, p| m.min(p.xz()));
    let hi = pts.iter().fold(Vec2::MIN, |m, p| m.max(p.xz()));
    let t = Tunnel { pts, s, portals: [plan.cuts[0], total - plan.cuts[1]], lo, hi };
    let m = t.mirrored();
    vec![t, m]
}

/// Shape the terrain for the tunnels: rock over every roofed stretch (a
/// natural mound where the ground is too low) and an open cut down to the
/// floor at each end. Called while sampling terrain.
pub(super) fn sculpt(x: f32, z: f32, mut h: f32, tunnels: &[Tunnel]) -> f32 {
    for t in tunnels {
        let Some(sp) = t.locate(x, z, HALF_WIDTH + 16.0) else { continue };
        let d = sp.d.abs();
        let [a, b] = t.portals;
        // Rock over the roof, falling away to the sides and past the portals.
        let outside = (a - sp.s).max(sp.s - b).max(0.0) + sp.beyond;
        let need = sp.floor + HEIGHT + COVER + 0.5 - (d - HALF_WIDTH - 1.0).max(0.0) * 0.6 - outside * 0.9;
        h = h.max(need);
        // The cut: level floor between steep sides, fading past the ends.
        if !t.roofed(sp.s) {
            let w = (1.0 - smooth(HALF_WIDTH + 0.4, HALF_WIDTH + 3.2, d)) * (1.0 - smooth(0.0, 5.0, sp.beyond));
            h += (sp.floor - h) * w;
        }
    }
    h
}

impl World {
    /// The tunnel cavity at (x, z), if any: its floor.
    fn cavity(&self, x: f32, z: f32) -> Option<f32> {
        self.tunnels.iter().find_map(|t| t.locate(x, z, HALF_WIDTH).filter(|sp| sp.beyond == 0.0 && t.in_cavity(sp.s)).map(|sp| sp.floor))
    }

    /// Keep random cover and trees out of a tunnel's cuts and off its line.
    pub(super) fn tunnel_reserved(&self, x: f32, z: f32, r: f32) -> bool {
        self.tunnels.iter().any(|t| t.locate(x, z, HALF_WIDTH + 5.0 + r).is_some_and(|sp| sp.beyond < 5.0 + r))
    }

    fn near_tunnel(&self, x: f32, z: f32) -> bool {
        self.tunnels.iter().any(|t| x > t.lo.x - ZONE && x < t.hi.x + ZONE && z > t.lo.y - ZONE && z < t.hi.y + ZONE)
    }

    /// What someone at height `y` stands on at (x, z): the terrain, or a
    /// tunnel floor when they are inside a tunnel. Inside means within the
    /// cavity's footprint and below its roof, whatever the terrain above
    /// does, so the cells sloping over a portal never lift anyone out.
    pub fn ground(&self, x: f32, z: f32, y: f32) -> f32 {
        let h = self.height(x, z);
        if self.tunnels.is_empty() {
            return h;
        }
        match self.cavity(x, z) {
            Some(f) if y > f - 0.8 && y < f + HEIGHT - 0.3 && y < h - 0.02 => f,
            _ => h,
        }
    }

    /// The tunnel roof over someone at height `y`, if they are in a tunnel.
    pub fn roof(&self, x: f32, z: f32, y: f32) -> Option<f32> {
        if self.tunnels.is_empty() {
            return None;
        }
        let h = self.height(x, z);
        self.cavity(x, z).filter(|&f| y > f - 0.8 && y < f + HEIGHT - 0.3 && y < h - 0.02).map(|f| f + HEIGHT)
    }

    /// Whether a body with feet at `p`, radius `r` and height `h` overlaps
    /// the rock around a tunnel: ground well above its feet that is not part
    /// of a cavity. Only near tunnels, so slopes elsewhere are unaffected.
    pub fn in_rock(&self, p: Vec3, r: f32, h: f32) -> bool {
        if self.tunnels.is_empty() || !self.near_tunnel(p.x, p.z) {
            return false;
        }
        let _ = h;
        [
            (0.0, 0.0),
            (r, 0.0),
            (-r, 0.0),
            (0.0, r),
            (0.0, -r),
            (r * 0.7, r * 0.7),
            (-r * 0.7, r * 0.7),
            (r * 0.7, -r * 0.7),
            (-r * 0.7, -r * 0.7),
        ]
        .iter()
        .any(|&(dx, dz)| {
            let (x, z) = (p.x + dx, p.z + dz);
            p.y + 0.45 < self.height(x, z) && !self.cavity(x, z).is_some_and(|f| p.y > f - 0.8)
        })
    }

    /// Whether a point is solid ground: under the terrain and not in a cavity.
    pub fn underground(&self, p: Vec3) -> bool {
        if p.y >= self.height(p.x, p.z) {
            return false;
        }
        if self.tunnels.is_empty() {
            return true;
        }
        !self.cavity(p.x, p.z).is_some_and(|f| p.y > f - 0.05 && p.y < f + HEIGHT + 0.05)
    }

    /// The direction a walker underground in a tunnel can actually go:
    /// `want` turned along the tunnel. Unchanged anywhere else.
    pub fn tunnel_steer(&self, pos: Vec3, want: Vec2) -> Vec2 {
        if self.tunnels.is_empty() || pos.y > self.height(pos.x, pos.z) - 0.25 {
            return want;
        }
        for t in &self.tunnels {
            if let Some(sp) = t.locate(pos.x, pos.z, HALF_WIDTH + 0.5) {
                if t.in_cavity(sp.s) {
                    let sign = if want.dot(sp.dir) < 0.0 { -1.0 } else { 1.0 };
                    // Hold the middle, so corners do not snag on the wall.
                    let centre = vec2(-sp.dir.y, sp.dir.x) * (-sp.d * 0.25);
                    return (sp.dir * sign + centre).normalize_or_zero() * want.length().max(1e-3);
                }
            }
        }
        want
    }

    /// Whether a terrain triangle crosses a tunnel's cavity: the cells that
    /// span a portal, which the renderer leaves out for the rock collar.
    pub fn tunnel_carves(&self, centre: Vec3, low: f32, high: f32) -> bool {
        self.tunnels.iter().any(|t| {
            t.locate(centre.x, centre.z, HALF_WIDTH + 0.9).is_some_and(|sp| {
                // Behind the portal, the whole width the vault spans; in
                // front of it only the opening, so the cut's sides stay whole.
                let reach = if t.roofed(sp.s) { HALF_WIDTH + 0.9 } else { HALF_WIDTH + 0.2 };
                sp.beyond == 0.0 && sp.d.abs() < reach && t.in_cavity(sp.s) && low < sp.floor + HEIGHT + 0.6 && high > sp.floor + 0.45
            })
        })
    }
}
