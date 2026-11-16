//! Cinematic camera: a free camera for footage and screenshots, with the
//! HUD hidden, an adjustable field of view, depth of field and pictures
//! saved at a multiple of the screen's resolution. Works while playing,
//! dead or spectating; the soldier stands still while it is on.
//!
//! Also the posing aid for staged shots: a "cast" of soldiers placed by
//! hand (`--cine-cast`), drawn by the client only. They are a picture, not
//! players: nothing hits them and the server never hears of them.

use crate::gfx::Dof;
use crate::scene::PlayerView;
use ferrite_shared::glam::{vec3, Vec3};
use ferrite_shared::map::World;
use ferrite_shared::proto::flag;
use ferrite_shared::sim;
use macroquad::prelude as mq;

/// Field of view the camera starts with, and its range, in degrees.
pub const FOV: f32 = 60.0;
const FOV_MIN: f32 = 8.0;
const FOV_MAX: f32 = 110.0;
/// Free-flight speed in m/s, before Shift (x4) or Alt (x0.15).
const SPEED: f32 = 8.0;
/// Autofocus distance when the screen centre looks at the sky.
const SKY_FOCUS: f32 = 400.0;
/// How long a changed setting stays on screen, in seconds.
const NOTE_SECS: f64 = 1.6;

pub struct Cine {
    /// The free camera is flying.
    pub on: bool,
    /// HUD, crosshair, nameplates and first-person weapon hidden. Toggled on
    /// its own key anywhere; turning the camera on hides them too.
    pub hide_hud: bool,
    /// What `hide_hud` was before the camera came on.
    hud_before: bool,
    /// Where the controls steer, and what is shown (eased toward it when
    /// smoothing is on).
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    fov: f32,
    show_pos: Vec3,
    show_yaw: f32,
    show_pitch: f32,
    pub show_fov: f32,
    /// m/s before modifiers.
    speed: f32,
    /// 0 = off, 1 = soft, 2 = heavy.
    smooth: u8,
    pub dof: bool,
    /// Focus on whatever is under the screen centre.
    autofocus: bool,
    /// Manual focus distance; with autofocus, the distance found this frame.
    focus: f32,
    show_focus: f32,
    /// Blur knob: 1 = a strong blur of the background at a 60 degree view.
    aperture: f32,
    /// The action is held: soldiers, effects and particles stop.
    pub freeze: bool,
    /// Screenshot size, as a multiple of the screen.
    pub scale: u32,
    /// A setting just changed, shown briefly; a screenshot just saved.
    note: Option<(String, f64)>,
    /// Placed by the command line for a scripted shot: no smoothing and
    /// nothing moves it unless someone takes the controls.
    pub scripted: bool,
}

impl Default for Cine {
    fn default() -> Self {
        Cine {
            on: false,
            hide_hud: false,
            hud_before: false,
            pos: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            fov: FOV,
            show_pos: Vec3::ZERO,
            show_yaw: 0.0,
            show_pitch: 0.0,
            show_fov: FOV,
            speed: SPEED,
            smooth: 1,
            dof: false,
            autofocus: true,
            focus: 20.0,
            show_focus: 20.0,
            aperture: 0.5,
            freeze: false,
            scale: 1,
            note: None,
            scripted: false,
        }
    }
}

/// Yaw and pitch that look along `d` (the inverse of `sim::view_dir`).
pub fn angles(d: Vec3) -> (f32, f32) {
    let d = d.normalize_or_zero();
    (d.x.atan2(d.z), d.y.clamp(-1.0, 1.0).asin())
}

/// Ease `cur` toward `to` at `rate` per second (0 = jump there).
fn ease(cur: f32, to: f32, rate: f32, dt: f32) -> f32 {
    if rate <= 0.0 {
        to
    } else {
        cur + (to - cur) * (1.0 - (-rate * dt).exp())
    }
}

/// Parse "x,y,z,tx,ty,tz": an eye and the point it looks at.
pub fn parse_pose(s: &str) -> Option<(Vec3, Vec3)> {
    let v: Vec<f32> = s.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    (v.len() == 6).then(|| (vec3(v[0], v[1], v[2]), vec3(v[3], v[4], v[5])))
}

/// Blur of the far distance, as a fraction of the picture's height, for a
/// given aperture knob and vertical field of view (degrees). A longer lens
/// (a narrower view) blurs more, as a real one does.
pub fn blur_for(aperture: f32, fov: f32) -> f32 {
    let narrow = 30f32.to_radians().tan() / (fov.to_radians() * 0.5).tan().max(0.01);
    (0.025 * aperture * narrow).clamp(0.0, crate::gfx::MAX_BLUR)
}

/// Distance to whatever is under the screen centre: the world, or a
/// soldier (a standing cylinder), or the sky far away.
pub fn focus_distance(w: &World, views: &[PlayerView], eye: Vec3, dir: Vec3, dmask: u64) -> f32 {
    let mut best = w.raycast(eye, dir, SKY_FOCUS, dmask).map_or(SKY_FOCUS, |h| h.t);
    for v in views {
        if let Some(t) = ray_soldier(eye, dir, v.pos) {
            best = best.min(t);
        }
    }
    best.max(0.2)
}

fn ray_soldier(o: Vec3, d: Vec3, feet: Vec3) -> Option<f32> {
    let (r, h) = (0.42, 1.9);
    let (ox, oz) = (o.x - feet.x, o.z - feet.z);
    let a = d.x * d.x + d.z * d.z;
    if a < 1e-6 {
        return None;
    }
    let b = ox * d.x + oz * d.z;
    let c = ox * ox + oz * oz - r * r;
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    let y = o.y + d.y * t - feet.y;
    (t > 0.0 && (0.0..=h).contains(&y)).then_some(t)
}

/// What the free camera reads from the keyboard and mouse this frame.
#[derive(Default, Clone, Copy)]
pub struct Controls {
    /// Mouse movement in pixels (zero while the mouse is not held).
    pub look: (f32, f32),
    pub sens: f32,
    /// -1..1: right, up, forward.
    pub mv: Vec3,
    pub fast: bool,
    pub slow: bool,
}

impl Cine {
    /// Turn the camera on where the game camera is now.
    pub fn start(&mut self, eye: Vec3, dir: Vec3, fov: f32) {
        let (yaw, pitch) = angles(dir);
        self.on = true;
        self.pos = eye;
        self.yaw = yaw;
        self.pitch = pitch;
        self.fov = fov;
        self.show_pos = eye;
        self.show_yaw = yaw;
        self.show_pitch = pitch;
        self.show_fov = fov;
        self.hud_before = self.hide_hud;
        self.hide_hud = true;
    }

    pub fn stop(&mut self) {
        self.on = false;
        self.freeze = false;
        self.scripted = false;
        self.hide_hud = self.hud_before;
    }

    /// A scripted shot from the command line: eye, target, field of view,
    /// focus distance (None = autofocus), aperture (None = no depth of field).
    pub fn script(&mut self, eye: Vec3, target: Vec3, fov: f32, focus: Option<f32>, aperture: Option<f32>) {
        self.start(eye, target - eye, fov.clamp(FOV_MIN, FOV_MAX));
        self.hud_before = false;
        self.scripted = true;
        self.smooth = 0;
        if let Some(f) = focus {
            self.autofocus = false;
            self.focus = f.max(0.2);
            self.show_focus = self.focus;
        }
        self.dof = focus.is_some() || aperture.is_some();
        if let Some(a) = aperture {
            self.aperture = a.max(0.0);
        }
    }

    /// Eye and look direction to draw from.
    pub fn view(&self) -> (Vec3, Vec3) {
        (self.show_pos, sim::view_dir(self.show_yaw, self.show_pitch))
    }

    /// The fixed camera keys, then movement, look and easing.
    pub fn update(&mut self, c: &Controls, dt: f32, now: f64) {
        use mq::KeyCode as K;
        let key = mq::is_key_pressed;
        let held = mq::is_key_down;
        let say = |s: &mut Self, text: String| s.note = Some((text, now));
        // Field of view: wheel or - / =, Backspace resets.
        let wheel = mq::mouse_wheel().1;
        let zoom = if wheel != 0.0 { -wheel.signum() } else { 0.0 } + if held(K::Minus) { dt * 8.0 } else { 0.0 }
            - if held(K::Equal) { dt * 8.0 } else { 0.0 };
        if zoom != 0.0 || key(K::Backspace) {
            self.fov = if key(K::Backspace) { FOV } else { (self.fov * 1.06f32.powf(zoom)).clamp(FOV_MIN, FOV_MAX) };
            say(self, format!("FIELD OF VIEW {:.0}", self.fov));
        }
        // Speed: , and .
        if key(K::Comma) || key(K::Period) {
            self.speed = (self.speed * if key(K::Period) { 1.5 } else { 1.0 / 1.5 }).clamp(0.5, 120.0);
            say(self, format!("CAMERA SPEED {:.1} M/S", self.speed));
        }
        if key(K::N) {
            self.smooth = (self.smooth + 1) % 3;
            say(self, format!("SMOOTHING {}", ["OFF", "SOFT", "HEAVY"][self.smooth as usize]));
        }
        if key(K::K) {
            self.freeze = !self.freeze;
            say(self, if self.freeze { "ACTION HELD" } else { "ACTION RUNNING" }.to_string());
        }
        // Depth of field: B on/off, Z / X focus (manual), O autofocus, [ ] aperture.
        if key(K::B) {
            self.dof = !self.dof;
            say(self, if self.dof { "DEPTH OF FIELD ON" } else { "DEPTH OF FIELD OFF" }.to_string());
        }
        let pull = if held(K::Z) { -1.0 } else { 0.0 } + if held(K::X) { 1.0 } else { 0.0 };
        if pull != 0.0 {
            if self.autofocus {
                self.autofocus = false;
                self.focus = self.show_focus;
            }
            self.focus = (self.focus * (1.0 + 1.2 * dt).powf(pull)).clamp(0.3, SKY_FOCUS);
            self.dof = true;
            say(self, format!("FOCUS {:.1} M", self.focus));
        }
        if key(K::O) {
            self.autofocus = true;
            self.dof = true;
            say(self, "AUTOFOCUS".to_string());
        }
        if key(K::LeftBracket) || key(K::RightBracket) {
            self.aperture = (self.aperture * if key(K::RightBracket) { 1.25 } else { 0.8 }).clamp(0.05, 4.0);
            self.dof = true;
            say(self, format!("APERTURE {:.2}", self.aperture));
        }
        for (k, n) in [(K::Key1, 1), (K::Key2, 2), (K::Key3, 3), (K::Key4, 4)] {
            if key(k) {
                self.scale = n;
                say(self, format!("SCREENSHOT SIZE x{n}"));
            }
        }

        // Look and fly.
        let s = c.sens * self.fov / 70.0;
        if c.look != (0.0, 0.0) || c.mv != Vec3::ZERO {
            self.scripted = false;
        }
        self.yaw = ferrite_shared::wrap_angle(self.yaw - c.look.0 * s);
        self.pitch = (self.pitch - c.look.1 * s).clamp(-1.55, 1.55);
        let speed = self.speed * if c.fast { 4.0 } else { 1.0 } * if c.slow { 0.15 } else { 1.0 };
        let fwd = sim::view_dir(self.yaw, self.pitch);
        let right = sim::right_of(self.yaw);
        self.pos += (right * c.mv.x + Vec3::Y * c.mv.y + fwd * c.mv.z) * speed * dt;

        let rate = [0.0, 7.0, 2.2][self.smooth as usize];
        self.show_pos = self.show_pos + (self.pos - self.show_pos) * if rate > 0.0 { 1.0 - (-rate * dt).exp() } else { 1.0 };
        // (yaw eased the short way round)
        let dyaw = ferrite_shared::wrap_angle(self.yaw - self.show_yaw);
        self.show_yaw = ferrite_shared::wrap_angle(self.yaw - dyaw + ease(0.0, dyaw, rate, dt));
        self.show_pitch = ease(self.show_pitch, self.pitch, rate, dt);
        self.show_fov = ease(self.show_fov, self.fov, rate.max(9.0), dt);
    }

    /// Depth of field for this frame, given what lies under the screen
    /// centre (only looked for with autofocus on).
    pub fn dof(&mut self, centre: impl FnOnce() -> f32, dt: f32) -> Option<Dof> {
        if !self.dof || !self.on {
            return None;
        }
        let target = if self.autofocus { centre() } else { self.focus };
        // Focus eases in distance ratio, so near and far pulls feel alike;
        // a scripted shot is in focus at once.
        let k = if self.scripted { 1.0 } else { 1.0 - (-5.0 * dt).exp() };
        self.show_focus = (self.show_focus.ln() + (target.ln() - self.show_focus.ln()) * k).exp();
        Some(Dof { focus: self.show_focus, blur: blur_for(self.aperture, self.show_fov) })
    }

    /// Show a line briefly (a saved screenshot).
    pub fn notify(&mut self, text: String, now: f64) {
        self.note = Some((text, now));
    }

    /// The brief note, bottom left. Drawn after any screenshot is taken, so
    /// it is never in the picture.
    pub fn draw_note(&self, now: f64) {
        let Some((text, at)) = &self.note else { return };
        let age = now - at;
        if age > NOTE_SECS + 0.4 {
            return;
        }
        let a = (1.0 - ((age - NOTE_SECS) / 0.4).max(0.0)) as f32;
        let h = mq::screen_height();
        let w = crate::ui::width(text, 20.0);
        mq::draw_rectangle(18.0, h - 58.0, w + 28.0, 38.0, crate::ui::fade(crate::ui::color::GLASS_STRONG, a));
        crate::ui::text(text, 32.0, h - 32.0, 20.0, crate::ui::fade(crate::ui::color::WHITE, a));
    }
}

/// Where screenshots go: `screenshots/` next to the game (and its
/// `ferrite-client.txt`), named by the time, UTC.
pub fn screenshot_path() -> std::path::PathBuf {
    let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("screenshots"))).unwrap_or_else(|| "screenshots".into());
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let stamp = timestamp(secs);
    let mut path = dir.join(format!("ferrite-{stamp}.png"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("ferrite-{stamp}-{n}.png"));
        n += 1;
    }
    path
}

/// "YYYY-MM-DD_HH-MM-SS" for seconds since 1970 (UTC).
fn timestamp(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

/// Save a picture (RGBA, bottom row first) without holding up the game.
pub fn save_png(path: std::path::PathBuf, bytes: Vec<u8>, w: u32, h: u32) {
    std::thread::spawn(move || {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let img = mq::Image { bytes, width: w as u16, height: h as u16 };
        let shown = path.display().to_string();
        if std::panic::catch_unwind(|| img.export_png(&shown)).is_err() {
            eprintln!("could not save {shown}");
        }
    });
}

/// One soldier placed by hand for a staged shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extra {
    pub team: u8,
    pub kit: u8,
    /// Feet; y None = standing on the ground there.
    pub x: f32,
    pub y: Option<f32>,
    pub z: f32,
    /// Facing, in degrees (0 = +Z, 90 = +X), and aim up (+) or down.
    pub yaw: f32,
    pub pitch: f32,
    pub pose: Pose,
    pub skin: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Idle,
    Walk,
    Run,
    Sprint,
    Crouch,
    Aim,
    Fire,
    Reload,
    Slide,
    Air,
    Down,
}

impl Pose {
    fn parse(s: &str) -> Option<Pose> {
        Some(match s {
            "idle" => Pose::Idle,
            "walk" => Pose::Walk,
            "run" => Pose::Run,
            "sprint" => Pose::Sprint,
            "crouch" => Pose::Crouch,
            "aim" => Pose::Aim,
            "fire" => Pose::Fire,
            "reload" => Pose::Reload,
            "slide" => Pose::Slide,
            "air" => Pose::Air,
            "down" => Pose::Down,
            _ => return None,
        })
    }
}

/// Read a cast list: one soldier a line,
/// `team kit x y z yaw pose [pitch] [look]`, with team `ember`/`cobalt`,
/// kit 0-2 (Vanguard, Breacher, Marksman), y `-` for the ground, yaw and
/// pitch in degrees, pose one of idle walk run sprint crouch aim fire
/// reload slide air down, look 0-5. `#` starts a comment.
pub fn parse_cast(text: &str) -> Result<Vec<Extra>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let bad = || format!("cast line {}: {line}", n + 1);
        if f.len() < 7 {
            return Err(bad());
        }
        let num = |s: &str| s.parse::<f32>().map_err(|_| bad());
        let team = match f[0] {
            "ember" | "0" => 0,
            "cobalt" | "1" => 1,
            _ => return Err(bad()),
        };
        out.push(Extra {
            team,
            kit: f[1].parse::<u8>().map_err(|_| bad())?.min(2),
            x: num(f[2])?,
            y: if f[3] == "-" { None } else { Some(num(f[3])?) },
            z: num(f[4])?,
            yaw: num(f[5])?,
            pose: Pose::parse(f[6]).ok_or_else(bad)?,
            pitch: f.get(7).map_or(Ok(0.0), |s| num(s))?,
            skin: f.get(8).map_or(Ok(0), |s| s.parse::<u8>().map_err(|_| bad()))? % sim::SKINS.len() as u8,
        });
    }
    Ok(out)
}

impl Extra {
    /// The soldier as the scene draws one. `t`: seconds, for the stride.
    pub fn view(&self, k: usize, w: &World, t: f32) -> PlayerView {
        let wid = sim::kit(self.kit).prims[0];
        let ground = w.height(self.x, self.z);
        let mut v = PlayerView {
            id: 200u8.wrapping_add(k as u8),
            pos: vec3(self.x, self.y.unwrap_or(ground), self.z),
            yaw: self.yaw.to_radians(),
            pitch: self.pitch.to_radians(),
            flags: flag::GROUND,
            hp: 100,
            team: self.team,
            kit: self.kit,
            gun: sim::weapon_def(wid).arch,
            wid,
            skin: self.skin,
            reload_phase: -1.0,
            // (each one a little out of step with the others)
            phase: t * 3.2 + k as f32 * 1.7,
            ..Default::default()
        };
        match self.pose {
            Pose::Idle | Pose::Aim => {}
            Pose::Walk => {
                v.moving = 0.6;
                v.vel = (3.0, 0.0);
            }
            Pose::Run => {
                v.moving = 1.0;
                v.vel = (6.0, 0.0);
            }
            Pose::Sprint => {
                v.moving = 1.0;
                v.vel = (8.4, 0.0);
            }
            Pose::Crouch => v.crouch = 1.0,
            Pose::Fire => v.fire = 0.8,
            Pose::Reload => {
                v.reload = 1.0;
                v.reload_phase = 0.4;
            }
            Pose::Slide => {
                v.slide = 1.0;
                v.crouch = 1.0;
                v.vel = (8.0, 0.0);
            }
            Pose::Air => {
                v.air = 1.0;
                v.flags = 0;
                v.pos.y += 0.5;
            }
            Pose::Down => v.fall = 1.0,
        }
        if self.pose == Pose::Crouch && self.pitch != 0.0 {
            v.fire = 0.3;
        }
        if matches!(self.pose, Pose::Aim | Pose::Fire | Pose::Crouch) {
            v.flags |= flag::ADS;
        }
        v
    }

    /// Where a shot leaves the barrel, roughly, and which way it goes.
    pub fn muzzle(&self, v: &PlayerView) -> (Vec3, Vec3) {
        let d = sim::view_dir(v.yaw, v.pitch);
        let up = if v.crouch > 0.5 { 0.95 } else { 1.42 };
        (v.pos + vec3(0.0, up, 0.0) + d * 1.0 - sim::right_of(v.yaw) * 0.12, d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_invert_the_view_direction() {
        for (yaw, pitch) in [(0.0, 0.0), (1.2, -0.4), (-2.5, 0.9), (3.0, 0.1)] {
            let (y, p) = angles(sim::view_dir(yaw, pitch));
            assert!((y - yaw).abs() < 1e-4 && (p - pitch).abs() < 1e-4, "{yaw} {pitch} -> {y} {p}");
        }
    }

    #[test]
    fn a_pose_is_six_numbers() {
        assert_eq!(parse_pose("1,2,3, 4,5,6"), Some((vec3(1.0, 2.0, 3.0), vec3(4.0, 5.0, 6.0))));
        assert_eq!(parse_pose("1,2,3,4,5"), None);
        assert_eq!(parse_pose("1,2,3,4,5,x"), None);
    }

    #[test]
    fn a_longer_lens_blurs_more_and_blur_is_bounded() {
        assert!(blur_for(0.5, 30.0) > blur_for(0.5, 60.0));
        assert!(blur_for(1.0, 60.0) > blur_for(0.5, 60.0));
        assert_eq!(blur_for(0.0, 60.0), 0.0);
        assert!(blur_for(4.0, 8.0) <= crate::gfx::MAX_BLUR);
    }

    #[test]
    fn a_ray_finds_a_soldier_in_front_and_not_behind() {
        let eye = vec3(0.0, 1.6, 0.0);
        let t = ray_soldier(eye, Vec3::Z, vec3(0.0, 0.0, 10.0)).unwrap();
        assert!((t - 9.58).abs() < 0.01);
        assert_eq!(ray_soldier(eye, Vec3::Z, vec3(0.0, 0.0, -10.0)), None);
        assert_eq!(ray_soldier(eye, Vec3::Z, vec3(2.0, 0.0, 10.0)), None);
        // over the head
        assert_eq!(ray_soldier(vec3(0.0, 3.0, 0.0), Vec3::Z, vec3(0.0, 0.0, 10.0)), None);
    }

    #[test]
    fn timestamps_are_calendar_dates() {
        assert_eq!(timestamp(0), "1970-01-01_00-00-00");
        assert_eq!(timestamp(951_782_400), "2000-02-29_00-00-00");
        assert_eq!(timestamp(1_794_744_245), "2026-11-15_12-04-05");
    }

    #[test]
    fn a_cast_list_reads_soldiers_and_rejects_bad_lines() {
        let cast = parse_cast("# staged\nember 0 10 - -5 90 run\ncobalt 2 1.5 12 3 -45 fire -5 3 # aiming down\n\n").unwrap();
        assert_eq!(cast.len(), 2);
        assert_eq!(cast[0].team, 0);
        assert_eq!(cast[0].y, None);
        assert_eq!(cast[0].pose, Pose::Run);
        assert_eq!(cast[1].y, Some(12.0));
        assert_eq!(cast[1].pitch, -5.0);
        assert_eq!(cast[1].skin, 3);
        assert!(parse_cast("ember 0 1 - 2 0 dance").is_err());
        assert!(parse_cast("red 0 1 - 2 0 run").is_err());
        assert!(parse_cast("ember 0 1 - 2").is_err());
    }
}
