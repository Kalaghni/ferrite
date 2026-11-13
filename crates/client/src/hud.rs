//! 2D overlay, drawn with macroquad on top of the 3D frame.

use crate::scene::{team_col, NEUTRAL_COL};
use crate::App;
use ferrite_shared::glam::{Mat4, Vec3, Vec4};
use ferrite_shared::map::{BlockKind, HALF_X, HALF_Z};
use ferrite_shared::proto::{flag, SPECTATOR_ID};
use ferrite_shared::sim;
use ferrite_shared::{eligible, NEUTRAL, POINT_NAMES, POINT_RADIUS, TEAM_NAMES};
use macroquad::prelude::{
    draw_circle, draw_circle_lines, draw_line, draw_poly, draw_rectangle, draw_rectangle_lines, draw_text, draw_triangle,
    is_key_down, measure_text, screen_height, screen_width, Color, KeyCode, Vec2,
};

fn col(c: [f32; 3], a: f32) -> Color {
    Color::new(c[0], c[1], c[2], a)
}

const WHITE: [f32; 3] = [0.97, 0.95, 0.9];
const DARK: [f32; 3] = [0.07, 0.06, 0.07];

fn text(s: &str, x: f32, y: f32, size: f32, c: Color) {
    draw_text(s, x + 1.5, y + 1.5, size, Color::new(0.0, 0.0, 0.0, 0.6 * c.a));
    draw_text(s, x, y, size, c);
}

fn text_c(s: &str, cx: f32, y: f32, size: f32, c: Color) {
    let w = measure_text(s, None, size as u16, 1.0).width;
    text(s, cx - w * 0.5, y, size, c);
}

fn text_r(s: &str, rx: f32, y: f32, size: f32, c: Color) {
    let w = measure_text(s, None, size as u16, 1.0).width;
    text(s, rx - w, y, size, c);
}

fn project(vp: &Mat4, p: Vec3, w: f32, h: f32) -> Option<(f32, f32, f32)> {
    let c = *vp * Vec4::new(p.x, p.y, p.z, 1.0);
    if c.w < 0.2 {
        return None;
    }
    Some(((c.x / c.w * 0.5 + 0.5) * w, (1.0 - (c.y / c.w * 0.5 + 0.5)) * h, c.w))
}

pub fn draw(app: &App, vp: &Mat4, now: f64, scoped: bool, tris: usize) {
    let (w, h) = (screen_width(), screen_height());
    let (cx, cy) = (w * 0.5, h * 0.5);
    let Some(snap) = app.net.latest() else {
        text_c("waiting for server...", cx, cy, 30.0, col(WHITE, 1.0));
        return;
    };
    let playing = app.alive && !app.spectator;
    let my_pos = if app.spectator {
        app.views.iter().find(|v| v.id == app.spec_target).map_or(Vec3::ZERO, |v| v.pos)
    } else {
        app.st.pos
    };
    let owners = [snap.points[0].owner, snap.points[1].owner, snap.points[2].owner, snap.points[3].owner, snap.points[4].owner];

    // ---- scope ----
    if scoped {
        let r = h * 0.46;
        let dark = col(DARK, 1.0);
        draw_rectangle(0.0, 0.0, cx - r, h, dark);
        draw_rectangle(cx + r, 0.0, w - cx - r, h, dark);
        draw_rectangle(cx - r, 0.0, 2.0 * r, cy - r, dark);
        draw_rectangle(cx - r, cy + r, 2.0 * r, cy - r, dark);
        for k in 0..128 {
            let a0 = k as f32 / 128.0 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / 128.0 * std::f32::consts::TAU;
            let p = |a: f32, rr: f32| Vec2::new(cx + a.cos() * rr, cy + a.sin() * rr);
            draw_triangle(p(a0, r), p(a1, r), p(a0, r * 1.5), dark);
            draw_triangle(p(a1, r), p(a1, r * 1.5), p(a0, r * 1.5), dark);
        }
        draw_circle_lines(cx, cy, r, 3.0, col(DARK, 1.0));
        draw_line(cx - r, cy, cx - 14.0, cy, 1.5, col(DARK, 0.9));
        draw_line(cx + 14.0, cy, cx + r, cy, 1.5, col(DARK, 0.9));
        draw_line(cx, cy + 14.0, cx, cy + r, 1.5, col(DARK, 0.9));
        draw_circle(cx, cy, 1.5, Color::new(1.0, 0.2, 0.15, 0.9));
    }

    // ---- world-space markers ----
    for (k, p) in app.world.points.iter().enumerate() {
        if let Some((sx, sy, d)) = project(vp, *p + Vec3::new(0.0, 9.3, 0.0), w, h) {
            let pt = &snap.points[k];
            let c = team_col(pt.owner);
            let contested = pt.n[0] > 0 && pt.n[1] > 0;
            let pulse = if contested { 0.75 + 0.25 * (now as f32 * 8.0).sin() } else { 1.0 };
            draw_poly(sx, sy, 4, 15.0, 45.0, col(DARK, 0.75));
            draw_poly(sx, sy, 4, 12.0, 45.0, col(c, 0.92 * pulse));
            text_c(POINT_NAMES[k], sx, sy + 6.0, 21.0, col(DARK, 1.0));
            text_c(&format!("{:.0}m", d), sx, sy + 30.0, 16.0, col(WHITE, 0.85));
        }
    }
    let view_dir = sim::view_dir(app.yaw, app.pitch);
    let eye = my_pos + Vec3::new(0.0, sim::eye_h(app.st.crouch), 0.0);
    for v in &app.views {
        let friendly = v.team == app.my_team && !app.spectator;
        let head = v.pos + Vec3::new(0.0, 2.05, 0.0);
        let Some((sx, sy, d)) = project(vp, head, w, h) else { continue };
        let c = team_col(v.team);
        if friendly || app.spectator {
            if d < 70.0 {
                let s = (7.0 - d * 0.04).max(3.5);
                draw_triangle(Vec2::new(sx - s, sy - s * 1.4), Vec2::new(sx + s, sy - s * 1.4), Vec2::new(sx, sy), col(c, 0.9));
                if d < 28.0 {
                    text_c(&app.name(v.id), sx, sy - s * 1.4 - 5.0, 15.0, col(WHITE, 0.85));
                }
            }
        } else if v.flags & flag::MARKED != 0 {
            // Spotted by a Marksman: visible to the whole team.
            draw_poly(sx, sy - 8.0, 4, 8.0, 0.0, Color::new(1.0, 0.2, 0.15, 0.95));
            text_c(&format!("{:.0}m", d), sx, sy - 20.0, 14.0, Color::new(1.0, 0.5, 0.4, 0.9));
        } else if playing {
            let to = (v.pos + Vec3::new(0.0, 1.1, 0.0) - eye).normalize_or_zero();
            if to.dot(view_dir) > 0.9992 && d < 90.0 && app.world.los(eye, v.pos + Vec3::new(0.0, 1.3, 0.0), app.dmask) {
                text_c(&app.name(v.id), sx, sy - 6.0, 15.0, Color::new(1.0, 0.45, 0.35, 0.95));
            }
        }
    }

    // ---- damage feedback ----
    if app.dmg_flash > 0.0 {
        let a = app.dmg_flash * 0.35;
        let red = Color::new(0.8, 0.05, 0.02, a);
        let t = 70.0;
        draw_rectangle(0.0, 0.0, w, t, red);
        draw_rectangle(0.0, h - t, w, t, red);
        draw_rectangle(0.0, t, t, h - 2.0 * t, red);
        draw_rectangle(w - t, t, t, h - 2.0 * t, red);
    }
    if playing {
        for (from, at) in &app.dmg_dirs {
            let d = *from - my_pos;
            let world_a = d.x.atan2(d.z);
            let rel = ferrite_shared::wrap_angle(app.yaw - world_a); // + = to the right
            let a = rel - std::f32::consts::FRAC_PI_2;
            let fade = (1.0 - (now - at) as f32 / 1.6).clamp(0.0, 1.0);
            let r = 130.0;
            let (px, py) = (cx + a.cos() * r, cy + a.sin() * r);
            let (tx, ty) = (-a.sin(), a.cos());
            draw_triangle(
                Vec2::new(px + tx * 34.0, py + ty * 34.0),
                Vec2::new(px - tx * 34.0, py - ty * 34.0),
                Vec2::new(px + a.cos() * 26.0, py + a.sin() * 26.0),
                Color::new(1.0, 0.15, 0.1, 0.8 * fade),
            );
        }
    }

    // ---- crosshair & hit marker ----
    if playing {
        let kit = sim::wid(&app.st);
        let sight = app.sight();
        let optic = (crate::scene::RED_DOT..=crate::scene::OPTIC).contains(&sight);
        if optic && app.ads > 0.8 && app.st.reload == 0 && app.st.swap == 0 {
            // Illuminated reticle, drawn where the optic's window sits.
            let a = ((app.ads - 0.8) / 0.15).clamp(0.0, 1.0);
            let red = Color::new(1.0, 0.16, 0.1, a);
            let glow = Color::new(1.0, 0.2, 0.1, 0.22 * a);
            match sight {
                crate::scene::RED_DOT => {
                    draw_circle(cx, cy, 7.0, glow);
                    draw_circle(cx, cy, 3.0, red);
                }
                crate::scene::HOLO => {
                    draw_circle_lines(cx, cy, 22.0, 2.0, red);
                    draw_circle(cx, cy, 2.2, red);
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        draw_line(cx + dx * 22.0, cy + dy * 22.0, cx + dx * 29.0, cy + dy * 29.0, 2.0, red);
                    }
                }
                _ => {
                    draw_line(cx, cy, cx - 9.0, cy + 13.0, 2.5, red);
                    draw_line(cx, cy, cx + 9.0, cy + 13.0, 2.5, red);
                    for (i, wd) in [7.0f32, 5.0, 3.5].iter().enumerate() {
                        let y = cy + 24.0 + i as f32 * 12.0;
                        draw_line(cx - wd, y, cx + wd, y, 1.5, col(DARK, 0.85 * a));
                    }
                    draw_line(cx, cy + 16.0, cx, cy + 52.0, 1.5, col(DARK, 0.85 * a));
                }
            }
        }
        if !scoped && !(optic && app.ads > 0.5) {
            let ads = app.ads > 0.5;
            let cone = sim::spread(kit, ads, app.hspeed(), !app.st.on_ground, app.st.crouch);
            let px_per_rad = h / app.fov.to_radians();
            let gap = (cone * px_per_rad + 3.0).min(140.0);
            let c = col(WHITE, 0.9);
            let len = 9.0;
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                draw_line(cx + dx * gap + 1.0, cy + dy * gap + 1.0, cx + dx * (gap + len) + 1.0, cy + dy * (gap + len) + 1.0, 2.0, col(DARK, 0.5));
                draw_line(cx + dx * gap, cy + dy * gap, cx + dx * (gap + len), cy + dy * (gap + len), 2.0, c);
            }
            draw_circle(cx, cy, 1.5, c);
        }
        if app.hitmarker > 0.0 {
            let a = (app.hitmarker / 0.22).min(1.0);
            let c = if app.hit_kill > 0.0 {
                Color::new(1.0, 0.25, 0.15, a)
            } else if app.hit_head {
                Color::new(1.0, 0.85, 0.2, a)
            } else {
                col(WHITE, a)
            };
            let (r0, r1) = (7.0 + (1.0 - a) * 5.0, 17.0 + (1.0 - a) * 5.0);
            for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                draw_line(cx + dx * r0, cy + dy * r0, cx + dx * r1, cy + dy * r1, 2.5, c);
            }
        }
    }

    // ---- top bar: tickets and the front line ----
    let bar_w = 190.0;
    let slot = 46.0;
    let line_w = slot * 5.0;
    let top = 14.0;
    draw_rectangle(cx - line_w * 0.5 - bar_w - 24.0, top - 6.0, line_w + 2.0 * bar_w + 48.0, 50.0, col(DARK, 0.55));
    for t in 0..2usize {
        let c = team_col(t as u8);
        let frac = (snap.tickets[t] as f32 / 250.0).clamp(0.0, 1.0);
        let x = if t == 0 { cx - line_w * 0.5 - 12.0 - bar_w } else { cx + line_w * 0.5 + 12.0 };
        draw_rectangle(x, top + 4.0, bar_w, 12.0, col(DARK, 0.8));
        if t == 0 {
            draw_rectangle(x + bar_w * (1.0 - frac), top + 4.0, bar_w * frac, 12.0, col(c, 0.95));
            text(&format!("{}", snap.tickets[t]), x, top + 38.0, 24.0, col(c, 1.0));
            text_r(TEAM_NAMES[t], x + bar_w, top + 36.0, 18.0, col(WHITE, if app.my_team == 0 { 1.0 } else { 0.6 }));
        } else {
            draw_rectangle(x, top + 4.0, bar_w * frac, 12.0, col(c, 0.95));
            text_r(&format!("{}", snap.tickets[t]), x + bar_w, top + 38.0, 24.0, col(c, 1.0));
            text(TEAM_NAMES[t], x, top + 36.0, 18.0, col(WHITE, if app.my_team == 1 { 1.0 } else { 0.6 }));
        }
    }
    for k in 0..5 {
        let pt = &snap.points[k];
        let x = cx - line_w * 0.5 + slot * (k as f32 + 0.5);
        let y = top + 19.0;
        let c = team_col(pt.owner);
        if k < 4 {
            draw_line(x + 15.0, y, x + slot - 15.0, y, 3.0, col(WHITE, 0.25));
        }
        let hot = eligible(&owners, 0, k) && pt.owner != 0 || eligible(&owners, 1, k) && pt.owner != 1;
        let contested = pt.n[0] > 0 && pt.n[1] > 0;
        let pulse = if contested { 0.6 + 0.4 * (now as f32 * 8.0).sin() } else { 1.0 };
        draw_poly(x, y, 4, 17.0, 45.0, col(if hot { WHITE } else { DARK }, 0.9));
        draw_poly(x, y, 4, 14.0, 45.0, col(DARK, 1.0));
        draw_poly(x, y, 4, 13.0, 45.0, col(c, 0.35));
        // progress fill, growing from the leading team's colour
        let prog = pt.progress as f32 / 100.0;
        if prog.abs() > 0.02 {
            let lead = team_col(if prog > 0.0 { 1 } else { 0 });
            draw_poly(x, y, 4, 13.0 * prog.abs().sqrt(), 45.0, col(lead, pulse));
        }
        text_c(POINT_NAMES[k], x, y + 6.0, 20.0, col(WHITE, 1.0));
    }

    // ---- banners ----
    if let Some((msg, c, at)) = &app.banner {
        let age = (now - at) as f32;
        if age < 3.5 {
            let a = (1.0 - (age - 2.6).max(0.0) / 0.9).clamp(0.0, 1.0) * (age * 5.0).clamp(0.0, 1.0);
            let tw = measure_text(msg, None, 34, 1.0).width;
            draw_rectangle(cx - tw * 0.5 - 22.0, 92.0, tw + 44.0, 46.0, col(DARK, 0.6 * a));
            draw_rectangle(cx - tw * 0.5 - 22.0, 92.0, 6.0, 46.0, col(*c, a));
            text_c(msg, cx, 125.0, 34.0, col(WHITE, a));
        }
    }
    if let Some((msg, at)) = &app.sub_banner {
        let age = (now - at) as f32;
        if age < 2.5 {
            let a = (1.0 - (age - 1.8).max(0.0) / 0.7).clamp(0.0, 1.0);
            text_c(msg, cx, cy + 96.0, 24.0, col([1.0, 0.86, 0.45], a));
        }
    }

    // ---- kill feed ----
    for (i, k) in app.feed.iter().rev().enumerate() {
        let age = (now - k.at) as f32;
        if age > 7.0 {
            continue;
        }
        let a = (1.0 - (age - 6.0).max(0.0)).clamp(0.0, 1.0);
        let y = 92.0 + i as f32 * 24.0;
        let mid = if k.head { format!(" [{} HS] ", k.weapon) } else { format!(" [{}] ", k.weapon) };
        let wv = measure_text(&k.victim, None, 18, 1.0).width;
        let wm = measure_text(&mid, None, 18, 1.0).width;
        let wk = measure_text(&k.killer, None, 18, 1.0).width;
        let x = w - 18.0 - wv - wm - wk;
        draw_rectangle(x - 8.0, y - 17.0, wv + wm + wk + 16.0, 22.0, col(if k.mine { [0.3, 0.26, 0.12] } else { DARK }, 0.6 * a));
        text(&k.killer, x, y, 18.0, col(team_col(k.kteam), a));
        text(&mid, x + wk, y, 18.0, col(WHITE, 0.8 * a));
        text(&k.victim, x + wk + wm, y, 18.0, col(team_col(k.vteam), a));
    }

    // ---- minimap ----
    {
        let scale = 0.92;
        let (mw, mh) = (2.0 * HALF_X * scale, 2.0 * HALF_Z * scale);
        let (mx, my) = (16.0, 16.0);
        // Team 1 sees the map rotated so "forward" is always up-right consistent.
        let flip = if app.my_team == 1 && !app.spectator { -1.0 } else { 1.0 };
        let to = |p: Vec3| (mx + (p.x * flip + HALF_X) * scale, my + (p.z * flip + HALF_Z) * scale);
        draw_rectangle(mx, my, mw, mh, col([0.13, 0.14, 0.11], 0.72));
        for b in &app.world.blocks {
            if matches!(b.kind, BlockKind::Trunk | BlockKind::Crate) {
                continue;
            }
            let (x0, y0) = to(if flip > 0.0 { b.min } else { b.max });
            draw_rectangle(x0, y0, (b.max.x - b.min.x) * scale, (b.max.z - b.min.z) * scale, col(WHITE, 0.22));
        }
        draw_rectangle_lines(mx, my, mw, mh, 2.0, col(WHITE, 0.35));
        for (k, p) in app.world.points.iter().enumerate() {
            let (x, y) = to(*p);
            let pt = &snap.points[k];
            draw_circle(x, y, POINT_RADIUS * scale, col(team_col(pt.owner), 0.3));
            draw_circle_lines(x, y, POINT_RADIUS * scale, 1.5, col(team_col(pt.owner), 0.9));
            text_c(POINT_NAMES[k], x, y + 5.0, 16.0, col(WHITE, 0.95));
        }
        for v in &app.views {
            let friendly = v.team == app.my_team || app.spectator;
            if friendly || v.flags & (flag::MARKED | flag::FIRING) != 0 {
                let (x, y) = to(v.pos);
                draw_circle(x, y, 2.6, col(team_col(v.team), if friendly { 0.95 } else { 0.8 }));
            }
        }
        if playing {
            let (x, y) = to(my_pos);
            let f = sim::view_dir(app.yaw, 0.0) * flip;
            let (fx, fy) = (f.x, f.z);
            draw_triangle(
                Vec2::new(x + fx * 7.0, y + fy * 7.0),
                Vec2::new(x - fx * 4.0 - fy * 4.0, y - fy * 4.0 + fx * 4.0),
                Vec2::new(x - fx * 4.0 + fy * 4.0, y - fy * 4.0 - fx * 4.0),
                col(WHITE, 1.0),
            );
        }
    }

    // ---- capture panel ----
    if playing || app.spectator {
        for (k, p) in app.world.points.iter().enumerate() {
            let d = *p - my_pos;
            if d.x * d.x + d.z * d.z > POINT_RADIUS * POINT_RADIUS {
                continue;
            }
            let pt = &snap.points[k];
            let prog = pt.progress as f32 / 100.0;
            let (bw, bh) = (300.0, 12.0);
            let y = 158.0;
            let me = app.my_team;
            let status = if pt.n[0] > 0 && pt.n[1] > 0 {
                "CONTESTED"
            } else if pt.owner == me && prog.abs() > 0.98 {
                "HOLDING"
            } else if pt.owner != me && !eligible(&owners, me, k) {
                "LOCKED - take the point before it first"
            } else if pt.n[me as usize] > 0 && pt.owner != me {
                "CAPTURING"
            } else if pt.n[me as usize] > 0 {
                "SECURING"
            } else {
                "LOSING"
            };
            text_c(&format!("POINT {}  -  {}", POINT_NAMES[k], status), cx, y - 8.0, 20.0, col(WHITE, 0.95));
            draw_rectangle(cx - bw * 0.5 - 2.0, y - 2.0, bw + 4.0, bh + 4.0, col(DARK, 0.8));
            draw_rectangle(cx - bw * 0.5, y, bw, bh, col(NEUTRAL_COL, 0.25));
            if prog < 0.0 {
                draw_rectangle(cx + prog * bw * 0.5, y, -prog * bw * 0.5, bh, col(team_col(0), 0.95));
            } else {
                draw_rectangle(cx, y, prog * bw * 0.5, bh, col(team_col(1), 0.95));
            }
            draw_line(cx, y - 3.0, cx, y + bh + 3.0, 2.0, col(WHITE, 0.9));
            text_r(&format!("{}", pt.n[0]), cx - bw * 0.5 - 10.0, y + 12.0, 20.0, col(team_col(0), 1.0));
            text(&format!("{}", pt.n[1]), cx + bw * 0.5 + 10.0, y + 12.0, 20.0, col(team_col(1), 1.0));
        }
    }

    // ---- vitals & weapon ----
    if playing {
        let kd = sim::kit(app.st.kit);
        let max = kd.hp;
        let frac = (app.hp as f32 / max).clamp(0.0, 1.0);
        let (x, y) = (24.0, h - 44.0);
        draw_rectangle(x - 4.0, y - 4.0, 248.0, 28.0, col(DARK, 0.6));
        let hc = if frac > 0.5 { WHITE } else if frac > 0.25 { [1.0, 0.75, 0.2] } else { [1.0, 0.25, 0.15] };
        draw_rectangle(x, y, 240.0 * frac, 20.0, col(hc, 0.92));
        text(&format!("{}", app.hp), x + 6.0, y + 16.0, 20.0, col(DARK, 1.0));
        text(kd.name, x, y - 12.0, 20.0, col(team_col(app.my_team), 1.0));
        if app.st.surge {
            text("SURGE", x + 110.0, y - 12.0, 20.0, col([1.0, 0.86, 0.45], 0.7 + 0.3 * (now as f32 * 10.0).sin()));
        }
        let wd = sim::weapon_def(sim::wid(&app.st));
        let ammo = format!("{}", app.st.ammo);
        let low = app.st.ammo as f32 <= wd.mag as f32 * 0.25;
        text_r(&ammo, w - 96.0, h - 26.0, 52.0, if low { Color::new(1.0, 0.35, 0.2, 1.0) } else { col(WHITE, 1.0) });
        text(&format!("/ {}", wd.mag), w - 90.0, h - 28.0, 24.0, col(WHITE, 0.6));
        text_r(&format!("{}  -  {}", wd.name, crate::scene::SIGHT_NAMES[app.sight() as usize % 6]), w - 24.0, h - 74.0, 20.0, col(WHITE, 0.75));
        text_r("[T] change sight", w - 24.0, h - 146.0, 16.0, col(WHITE, 0.4));
        let other = if app.st.slot == 1 { kd.weapon.name } else { sim::SIDEARM.name };
        text_r(&format!("[Q] {}  {}", other, app.st.ammo_alt), w - 24.0, h - 128.0, 16.0, col(WHITE, 0.5));
        for g in 0..app.st.grenades {
            draw_poly(w - 34.0 - g as f32 * 22.0, h - 104.0, 6, 8.0, 0.0, col([0.75, 0.8, 0.5], 0.95));
        }
        if app.st.reload > 0 {
            let f = 1.0 - app.st.reload as f32 / wd.reload as f32;
            draw_rectangle(cx - 60.0, cy + 46.0, 120.0, 6.0, col(DARK, 0.7));
            draw_rectangle(cx - 60.0, cy + 46.0, 120.0 * f, 6.0, col(WHITE, 0.9));
            text_c("RELOADING", cx, cy + 72.0, 18.0, col(WHITE, 0.8));
        } else if app.st.ammo == 0 {
            text_c("EMPTY", cx, cy + 72.0, 18.0, Color::new(1.0, 0.35, 0.2, 1.0));
        }
    }

    // ---- death screen ----
    if !app.alive && !app.spectator {
        draw_rectangle(0.0, h * 0.62, w, 150.0, col(DARK, 0.6));
        let who = if app.killer == SPECTATOR_ID || app.killer == app.my_id {
            "DOWN".to_string()
        } else {
            format!("ELIMINATED BY {}", app.name(app.killer))
        };
        text_c(&who, cx, h * 0.62 + 40.0, 34.0, col(WHITE, 1.0));
        let rs = if app.respawn_in > 0.05 { format!("Reinforcing in {:.1}s", app.respawn_in) } else { "Deploying...".to_string() };
        text_c(&rs, cx, h * 0.62 + 70.0, 22.0, col(WHITE, 0.8));
        let total_w = 3.0 * 300.0;
        for k in 0..3u8 {
            let kd = sim::kit(k);
            let x = cx - total_w * 0.5 + k as f32 * 300.0;
            let sel = app.kit_choice == k;
            let c = if sel { team_col(app.my_team) } else { WHITE };
            text(&format!("[{}] {}", k + 1, kd.name), x + 10.0, h * 0.62 + 108.0, 24.0, col(c, if sel { 1.0 } else { 0.65 }));
            text(kd.blurb, x + 10.0, h * 0.62 + 130.0, 15.0, col(WHITE, if sel { 0.9 } else { 0.5 }));
        }
    }
    if app.spectator {
        text_c(&format!("SPECTATING  {}", app.name(app.spec_target)), cx, h - 30.0, 24.0, col(team_col(app.team(app.spec_target)), 1.0));
    }

    // ---- round over ----
    if snap.phase == 1 && snap.winner != NEUTRAL {
        draw_rectangle(0.0, cy - 80.0, w, 130.0, col(DARK, 0.7));
        let c = team_col(snap.winner);
        text_c(&format!("{} HOLDS THE FIELD", TEAM_NAMES[snap.winner as usize % 2]), cx, cy - 20.0, 56.0, col(c, 1.0));
        let res = if app.spectator { "" } else if snap.winner == app.my_team { "VICTORY" } else { "DEFEAT" };
        text_c(&format!("{}   next round in {:.0}s", res, snap.phase_secs), cx, cy + 22.0, 24.0, col(WHITE, 0.9));
    }

    // ---- scoreboard ----
    if is_key_down(KeyCode::Tab) || snap.phase == 1 {
        let (bw, bh) = (760.0, 400.0);
        let (x0, y0) = (cx - bw * 0.5, if snap.phase == 1 { cy + 70.0 } else { cy - bh * 0.5 });
        let bh = if snap.phase == 1 { (h - y0 - 10.0).min(bh) } else { bh };
        draw_rectangle(x0, y0, bw, bh, col(DARK, 0.82));
        for t in 0..2u8 {
            let x = x0 + 16.0 + t as f32 * bw * 0.5;
            text(TEAM_NAMES[t as usize], x, y0 + 30.0, 26.0, col(team_col(t), 1.0));
            text("K", x + 220.0, y0 + 30.0, 16.0, col(WHITE, 0.6));
            text("D", x + 260.0, y0 + 30.0, 16.0, col(WHITE, 0.6));
            text("SCORE", x + 300.0, y0 + 30.0, 16.0, col(WHITE, 0.6));
            let mut rows: Vec<_> = app.net.roster.iter().filter(|r| r.team == t).collect();
            rows.sort_by(|a, b| b.score.cmp(&a.score));
            for (i, r) in rows.iter().enumerate() {
                let y = y0 + 56.0 + i as f32 * 22.0;
                if y > y0 + bh - 6.0 {
                    break;
                }
                let me = r.id == app.my_id;
                if me {
                    draw_rectangle(x - 6.0, y - 16.0, bw * 0.5 - 20.0, 21.0, col(WHITE, 0.14));
                }
                let c = if r.bot { col(WHITE, 0.62) } else { col(WHITE, 1.0) };
                text(&r.name, x, y, 18.0, c);
                text(&sim::kit(r.kit).name[..3], x + 160.0, y, 14.0, col(WHITE, 0.4));
                text(&r.kills.to_string(), x + 220.0, y, 18.0, c);
                text(&r.deaths.to_string(), x + 260.0, y, 18.0, c);
                text(&r.score.to_string(), x + 300.0, y, 18.0, c);
            }
        }
    }

    // ---- pause / help ----
    if !app.captured && !app.auto && !app.spectator {
        draw_rectangle(0.0, 0.0, w, h, col(DARK, 0.62));
        text_c("FERRITE", cx, cy - 150.0, 96.0, col(team_col(app.my_team), 1.0));
        text_c("hold the line - five points, one front", cx, cy - 112.0, 22.0, col(WHITE, 0.8));
        let lines = [
            "CLICK to deploy            ESC release mouse          Q leave match (while released)",
            "",
            "WASD move     SHIFT sprint     SPACE jump     CTRL / C crouch     MOUSE aim",
            "LMB fire    RMB aim    R reload    Q / wheel swap to sidearm    G frag grenade",
            "T change sight    1 / 2 / 3 kit for next deploy    TAB scoreboard    F3 net stats",
            "",
            "Capture points in order - only the front line can be contested.",
            "Standing together captures faster. Every capture grants your team a SURGE.",
            "Teams that hold more ground reinforce more slowly. Crates can be destroyed.",
        ];
        for (i, l) in lines.iter().enumerate() {
            text_c(l, cx, cy - 50.0 + i as f32 * 26.0, 20.0, col(WHITE, 0.92));
        }
        text_c(&format!("mouse sensitivity {:.2}   ( - / = to adjust )", app.sens * 1000.0), cx, cy + 210.0, 18.0, col(WHITE, 0.7));
        if let Some(hs) = &app.hosting {
            text_c(&format!("{hs}  -  friends on your LAN join with:  ferrite --connect <your-ip>"), cx, cy + 240.0, 18.0, col(WHITE, 0.7));
        }
        if !app.spectator {
            text_c(&format!("you fight for {}", TEAM_NAMES[app.my_team as usize % 2]), cx, cy + 270.0, 20.0, col(team_col(app.my_team), 1.0));
        }
    }

    if app.show_net {
        let s = &app.stats;
        let l = format!(
            "{:.0} fps | ping {:.0} ms | corrections {} (max {:.2} m) | {} tris | {} players drawn",
            s.frames as f64 / s.frame_sum.max(0.001),
            app.net.ping_ms,
            s.corrections,
            s.max_err,
            tris,
            app.views.len()
        );
        text(&l, 16.0, h - 78.0, 16.0, col(WHITE, 0.8));
    }
    if now - app.net.last_recv > 2.0 {
        text_c("CONNECTION LOST - waiting for server", cx, cy - 60.0, 30.0, Color::new(1.0, 0.4, 0.2, 1.0));
    }
}
