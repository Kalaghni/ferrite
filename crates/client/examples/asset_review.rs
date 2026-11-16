//! Deterministic visual checks without a server or saved settings.
//! cargo run -p ferrite --example asset_review -- players target/players.png run
//! Modes: players, weapons, skins, sights, finishes, finish, or a map name.
//! Optional player pose: run/crouch/slide/air/reload/melee/fall; weapons accepts a finish ID.
//! finishes: every weapon finish on one weapon (third argument: weapon ID).
//! finish: one finish (third argument: finish ID) on eight weapon types.
//! Slag Cathedral views: overview / nave / gallery / furnace / approach.
//! Classic map views: workshop / interior / passage / landmark / breach / walkway.
//! Ridgeline aircraft views: wreck / cargo / tail.
//! Quarry's mine and Ridgeline's cave: portal / exit from outside, enter /
//! leave from inside looking out of each end, tunnel along the inside.
//! Monument views: foundry viaduct / train / crossing, quarry excavator /
//! bucket / dragline, spillway dam / gates / crest.
//! Spillway's gate deck: platform / outfall / channel / deck; give a fourth
//! argument in a flooding window (220 to 275 for this seed) to see the water.
//! A fourth argument freezes machinery and wind at that second.
//! A fifth argument selects a destruction mask (decimal/0xhex), or "broken"
//! to show all designated map features in their destroyed state.

#[allow(dead_code)]
#[path = "../src/gfx.rs"]
mod gfx;
#[allow(dead_code)]
#[path = "../src/scene.rs"]
mod scene;

use ferrite_shared::glam::{vec3, Mat4, Vec3};
use ferrite_shared::map::{map_by_name, World, HALF_X, HALF_Z};
use ferrite_shared::proto::flag;
use ferrite_shared::sim::{self, SKINS};
use macroquad::prelude as mq;

fn conf() -> mq::Conf {
    mq::Conf {
        window_title: "FERRITE / Asset review".into(),
        window_width: 1600,
        window_height: 900,
        sample_count: 4,
        ..Default::default()
    }
}

fn label(s: &str, x: f32, y: f32, size: f32, c: mq::Color) {
    mq::draw_text(s, x, y, size, c);
}

fn sight_icon(m: &mut gfx::Mesh, wid: u8, sight: u8, cx: f32, cy: f32, w: f32, h: f32) {
    let mut probe = gfx::Mesh::default();
    scene::sight_model(&mut probe, wid, sight, Mat4::IDENTITY);
    let rotation = Mat4::from_rotation_x(0.23) * Mat4::from_rotation_y(0.65);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for v in &probe.v {
        let p = rotation.transform_point3(Vec3::from(v.pos));
        lo = lo.min(p);
        hi = hi.max(p);
    }
    if probe.v.is_empty() {
        return;
    }
    let scale = (w / (hi.x - lo.x).max(0.01)).min(h / (hi.y - lo.y).max(0.01));
    let g = Mat4::from_translation(vec3(cx, cy, 0.0))
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-(lo + hi) * 0.5)
        * rotation;
    scene::sight_model(m, wid, sight, g);
}

#[macroquad::main(conf)]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("players");
    let output = args.get(1);
    let pose = args.get(2).map(String::as_str).unwrap_or("idle");
    let map = map_by_name(mode).unwrap_or(0);
    let w = World::with_map(0xFE221, map);
    let dmask = match args.get(4).map(String::as_str) {
        Some("broken") => w.features.iter().fold(0, |mask, f| mask | (1u64 << f.bit)),
        Some(mask) => mask.strip_prefix("0x").map_or_else(|| mask.parse().ok(), |hex| u64::from_str_radix(hex, 16).ok()).unwrap_or(0),
        None => 0,
    };
    let models = mode == "players";
    let weapons = mode == "weapons";
    let skins = mode == "skins";
    let sights = mode == "sights";
    let finishes = mode == "finishes";
    let finish = mode == "finish";
    let gallery = models || weapons || skins || sights || finishes || finish;
    let mut stage = if gallery { gfx::Mesh::default() } else { scene::build_world(&w) };
    if models {
        stage.cuboid(vec3(-8.0, -0.2, -4.0), vec3(8.0, -0.08, 4.0), gfx::rgb(0.17, 0.19, 0.23));
        for k in 0..6 {
            let x = (k as f32 - 2.5) * 2.2;
            for z in [-1.8, 1.8] {
                stage.cuboid(vec3(x - 0.75, -0.08, z - 0.75), vec3(x + 0.75, 0.0, z + 0.75), gfx::rgb(0.26, 0.29, 0.32));
            }
        }
    }
    let bounds = if gallery { (vec3(-HALF_X, -14.0, -HALF_Z), vec3(HALF_X, 40.0, HALF_Z)) } else { scene::shadow_bounds(map) };
    let mut gpu = gfx::Gpu::new(
        stage,
        if gallery { gfx::Mesh::default() } else { scene::build_sky_for(map) },
        if gallery { 0 } else { 2048 },
        scene::sun_for(map).normalize(),
        bounds,
    );
    let ids: Vec<u8> = (0..sim::WEAPONS.len() as u8).filter(|id| !matches!(*id, 3 | 17)).collect();
    let empty = gfx::Mesh::default();
    let mut frames = 0;
    loop {
        let time = if output.is_some() {
            // An optional fourth argument freezes animation at that second.
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.65)
        } else {
            mq::get_time() as f32
        };
        let mut opaque = gfx::Mesh::default();
        let mut alpha = gfx::Mesh::default();
        let mut add = gfx::Mesh::default();
        if models {
            for team in 0..2 {
                for skin in 0..6 {
                    let x = (skin as f32 - 2.5) * 2.2;
                    let kit = skin % 3;
                    let wid = sim::kit(kit).prims[0];
                    let mut p = scene::PlayerView {
                        id: team * 6 + skin,
                        pos: vec3(x, 0.0, if team == 0 { 1.8 } else { -1.8 }),
                        yaw: if team == 0 { -0.3 } else { std::f32::consts::PI - 0.3 },
                        flags: flag::GROUND,
                        hp: 100,
                        team,
                        kit,
                        gun: sim::weapon_def(wid).arch,
                        wid,
                        skin,
                        reload_phase: -1.0,
                        phase: time * 3.2,
                        ..Default::default()
                    };
                    match pose {
                        "run" => {
                            p.moving = 1.0;
                            p.vel = (8.2, 1.2);
                        }
                        "crouch" => {
                            p.crouch = 1.0;
                        }
                        "slide" => {
                            p.slide = 1.0;
                            p.crouch = 1.0;
                            p.vel = (8.0, 0.0);
                        }
                        "air" => {
                            p.air = 1.0;
                            p.flags = 0;
                            p.pos.y = 0.4;
                        }
                        "reload" => {
                            p.reload = 1.0;
                            p.reload_phase = 0.4;
                        }
                        "melee" => {
                            p.melee = 0.7;
                        }
                        "fall" => {
                            p.fall = 1.0;
                        }
                        _ => {}
                    }
                    scene::soldier(&mut opaque, &mut alpha, &w, &p, time, false);
                }
            }
        } else if !gallery {
            scene::crates(&mut opaque, &w, dmask);
            scene::beacons(
                &mut opaque,
                &mut alpha,
                &mut add,
                &w,
                &[(0, 0.0, false), (0, 0.0, false), (2, 0.0, false), (1, 0.0, false), (1, 0.0, false)],
                time,
                dmask,
            );
        }
        let aspect = mq::screen_width() / mq::screen_height();
        let monument = if gallery { None } else { scene::monuments::review_view(&w, pose).or_else(|| scene::spillway::view(&w, pose)) };
        let (eye, target) = if models {
            (vec3(2.7, 5.0, 13.0), vec3(0.0, 0.6, 0.0))
        } else if let Some(view) = monument {
            view
        } else if !gallery && map == 2 && matches!(pose, "wreck" | "cargo" | "tail") {
            let g = w.height(0.0, 0.0);
            match pose {
                "cargo" => (vec3(-39.0, g + 1.8, -1.8), vec3(30.0, g + 3.0, 0.0)),
                "tail" => (vec3(-62.0, w.height(-62.0, -35.0) + 9.0, -35.0), vec3(-26.0, g + 14.0, 0.0)),
                _ => (vec3(75.0, w.height(75.0, -78.0) + 35.0, -78.0), vec3(0.0, g + 8.0, 0.0)),
            }
        } else if !gallery && matches!(map, 1 | 2) && matches!(pose, "passage" | "tunnel" | "portal" | "exit" | "leave" | "enter") {
            // Quarry's mine and Ridgeline's cave run underground.
            let t = &w.tunnels[0];
            let [a, b] = t.portals;
            let lift = |p: Vec3, dy: f32| p + Vec3::Y * dy;
            match pose {
                "portal" => {
                    let (from, _) = t.at(0.0);
                    let (to, _) = t.at(a);
                    (vec3(from.x, w.height(from.x, from.z) + 2.6, from.z) + (from - to).normalize() * 4.0, lift(to, 1.6))
                }
                "exit" => {
                    let (from, _) = t.at(t.length());
                    let (to, _) = t.at(b);
                    (vec3(from.x, w.height(from.x, from.z) + 2.6, from.z) + (from - to).normalize() * 4.0, lift(to, 1.6))
                }
                // From inside, looking out of the exit end / the entry end.
                "leave" => {
                    let (eye, _) = t.at(b - 5.0);
                    let (to, _) = t.at((b + 12.0).min(t.length()));
                    (lift(eye, 1.6), lift(to, 1.6))
                }
                "enter" => {
                    let (eye, _) = t.at(a + 5.0);
                    let (to, _) = t.at((a - 12.0).max(0.0));
                    (lift(eye, 1.6), lift(to, 1.6))
                }
                _ => {
                    let (eye, _) = t.at(a + 6.0);
                    let (to, _) = t.at(a + 18.0);
                    (lift(eye, 1.7), lift(to, 1.4))
                }
            }
        } else if !gallery && map < 4 && matches!(pose, "workshop" | "interior" | "passage" | "landmark" | "breach" | "walkway") {
            use ferrite_shared::map::landmarks::{floor, walkway_site, PASSAGES, WORKSHOPS};
            let site = match pose {
                "passage" => PASSAGES[map as usize],
                "walkway" => walkway_site(map),
                _ => WORKSHOPS[map as usize],
            };
            let c = vec3(site.x, floor(&w, site), site.z);
            let above_ground = |dx: f32, dy: f32, dz: f32| {
                let (x, z) = (c.x + dx, c.z + dz);
                vec3(x, w.height(x, z) + dy, z)
            };
            match pose {
                "breach" => {
                    let panel = w.features.iter().find(|f| f.kind == ferrite_shared::map::FeatureKind::BreachPanel).unwrap();
                    let outward = (panel.pos.z - site.z).signum();
                    let eye = panel.pos + vec3(-3.5, 2.0, 7.0 * outward);
                    (vec3(eye.x, w.height(eye.x, eye.z) + 2.4, eye.z), panel.pos + Vec3::Y * 0.8)
                }
                "walkway" => (above_ground(-14.0, 5.0, -9.0), c + Vec3::Y),
                "interior" => (c + vec3(-site.hx + 1.0, 1.7, -site.hz + 1.0), c + vec3(2.0, 1.7, 1.0)),
                "passage" => (above_ground(-site.hx - 3.0, 2.0, 0.0), c + vec3(2.0, 1.8, 0.0)),
                "landmark" if map == 3 => (vec3(-53.0, 14.0, -59.0), vec3(0.0, 17.0, 0.0)),
                _ => (above_ground(-site.hx - 18.0, 9.0, -site.hz - 20.0), c + vec3(0.0, 2.8, 0.0)),
            }
        } else if !gallery && map == 5 && pose == "mast" {
            let root = vec3(0.0, w.height(0.0, 126.0), 126.0);
            (vec3(-37.0, 16.0, 40.0), root + vec3(0.0, 36.0, 0.0))
        } else if map == ferrite_shared::map::slag_cathedral::MAP_ID && !gallery {
            match pose {
                "volcano" => (vec3(56.0, 24.0, -85.0), vec3(0.0, w.height(0.0, -160.0) + 36.0, -160.0)),
                "nave" => (vec3(-31.5, 4.8, 0.0), vec3(10.0, 14.0, 0.0)),
                "gallery" => (vec3(-22.0, 5.1, 15.0), vec3(11.0, 9.0, -3.0)),
                "furnace" => (vec3(-51.0, 5.0, -49.0), vec3(-17.0, 19.0, -31.0)),
                "approach" => (vec3(-57.0, 3.2, -7.0), vec3(0.0, 15.0, 0.0)),
                // "ex,ey,ez,tx,ty,tz": an arbitrary camera for art review.
                custom if custom.contains(',') => {
                    let v: Vec<f32> = custom.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    if v.len() == 6 {
                        (vec3(v[0], v[1], v[2]), vec3(v[3], v[4], v[5]))
                    } else {
                        (vec3(96.0, 59.0, -99.0), vec3(0.0, 9.0, 0.0))
                    }
                }
                _ => (vec3(96.0, 59.0, -99.0), vec3(0.0, 9.0, 0.0)),
            }
        } else {
            let v: Vec<f32> = pose.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if !gallery && v.len() == 6 {
                (vec3(v[0], v[1], v[2]), vec3(v[3], v[4], v[5]))
            } else {
                (vec3(110.0, 135.0, 150.0), vec3(0.0, 0.0, 0.0))
            }
        };
        let view = Mat4::look_at_rh(eye, target, Vec3::Y);
        let projection = if models {
            Mat4::orthographic_rh_gl(-8.0, 8.0, -8.0 / aspect, 8.0 / aspect, 0.01, 100.0)
        } else {
            Mat4::perspective_rh_gl(58f32.to_radians(), aspect, 0.1, 900.0)
        };
        gpu.render(&gfx::Frame {
            vp: projection * view,
            sky_vp: projection * Mat4::look_to_rh(Vec3::ZERO, (target - eye).normalize(), Vec3::Y),
            cam: eye,
            sun: scene::sun_for(map).normalize(),
            tone: scene::tone_for(map),
            time,
            fog: if gallery { [0.055, 0.07, 0.1, 0.0] } else { scene::fog_for(map) },
            lights: if gallery { [gfx::Light::default(); 4] } else { scene::map_lights(&w, eye, [gfx::Light::default(); 4]) },
            opaque: &opaque,
            alpha: &alpha,
            add: &add,
            view_vp: Mat4::IDENTITY,
            view_sun: scene::sun_for(map),
            view_lit: 1.0,
            view_opaque: &empty,
            view_add: &empty,
        });
        if gallery || (map != ferrite_shared::map::slag_cathedral::MAP_ID && !pose.contains(',')) {
            label("FERRITE / ASSET REVIEW", 40.0, 48.0, 30.0, mq::WHITE);
            label(&format!("{} / {}", mode.to_uppercase(), pose.to_uppercase()), 40.0, 77.0, 20.0, mq::LIGHTGRAY);
        }
        if weapons {
            let cw = (mq::screen_width() - 80.0) / 4.0;
            let ch = (mq::screen_height() - 150.0) / 4.0;
            for (k, &id) in ids.iter().enumerate() {
                let x = 40.0 + k as f32 % 4.0 * cw;
                let y = 110.0 + (k / 4) as f32 * ch;
                mq::draw_rectangle(x + 3.0, y + 3.0, cw - 6.0, ch - 6.0, mq::Color::new(0.1, 0.125, 0.17, 1.0));
                label(sim::weapon_def(id).name, x + 16.0, y + 29.0, 21.0, mq::WHITE);
                let mut gun = gfx::Mesh::default();
                scene::gun_icon(
                    &mut gun,
                    id,
                    1,
                    pose.parse::<u8>().unwrap_or(0),
                    x + cw * 0.5,
                    mq::screen_height() - y - ch * 0.57,
                    cw - 40.0,
                    ch - 60.0,
                    0.3,
                );
                gpu.overlay(&gun, mq::screen_width(), mq::screen_height());
            }
        } else if finishes || finish {
            // Every finish on one weapon, or one finish across the families.
            let (cols, rows) = if finishes { (4, 4) } else { (4, 2) };
            let cw = (mq::screen_width() - 80.0) / cols as f32;
            let ch = (mq::screen_height() - 150.0) / rows as f32;
            let arg = pose.parse::<u8>().unwrap_or(0);
            let cells: Vec<(u8, u8)> = if finishes {
                (0..sim::WEAPON_SKINS.len() as u8).map(|s| (arg, s)).collect()
            } else {
                [0u8, 8, 5, 6, 1, 2, 4, 10].iter().map(|&wid| (wid, arg)).collect()
            };
            for (k, &(wid, skin)) in cells.iter().enumerate() {
                let x = 40.0 + (k % cols) as f32 * cw;
                let y = 110.0 + (k / cols) as f32 * ch;
                mq::draw_rectangle(x + 3.0, y + 3.0, cw - 6.0, ch - 6.0, mq::Color::new(0.1, 0.125, 0.17, 1.0));
                let mut probe = gfx::Mesh::default();
                scene::gun_model(&mut probe, wid, 1, skin, Mat4::IDENTITY);
                let title = if finishes { sim::weapon_skin(skin).name } else { sim::weapon_def(wid).name };
                label(title, x + 14.0, y + 26.0, 20.0, mq::WHITE);
                label(&format!("{} tris", probe.triangles()), x + cw - 90.0, y + 26.0, 15.0, mq::LIGHTGRAY);
                let mut gun = gfx::Mesh::default();
                scene::gun_icon(&mut gun, wid, 1, skin, x + cw * 0.5, mq::screen_height() - y - ch * 0.56, cw - 30.0, ch - 50.0, 0.3);
                gpu.overlay(&gun, mq::screen_width(), mq::screen_height());
            }
        } else if skins || sights {
            let cw = (mq::screen_width() - 80.0) / 3.0;
            let ch = (mq::screen_height() - 150.0) / 2.0;
            for k in 0..6 {
                let x = 40.0 + (k % 3) as f32 * cw;
                let y = 110.0 + (k / 3) as f32 * ch;
                mq::draw_rectangle(x + 3.0, y + 3.0, cw - 6.0, ch - 6.0, mq::Color::new(0.1, 0.125, 0.17, 1.0));
                let mut mesh = gfx::Mesh::default();
                if skins {
                    let skin = (k + 1) as u8;
                    let finish = sim::weapon_skin(skin);
                    label(finish.name, x + 20.0, y + 32.0, 24.0, mq::WHITE);
                    label(finish.blurb, x + 20.0, y + 58.0, 16.0, mq::LIGHTGRAY);
                    for (slot, c) in [finish.body, finish.furniture, finish.accent, finish.pattern_color].iter().enumerate() {
                        if let Some(c) = c {
                            mq::draw_rectangle(
                                x + cw - 107.0 + slot as f32 * 20.0,
                                y + 20.0,
                                14.0,
                                9.0,
                                mq::Color::new(c[0], c[1], c[2], 1.0),
                            );
                        }
                    }
                    scene::gun_icon(&mut mesh, 0, 1, skin, x + cw * 0.5, mq::screen_height() - y - ch * 0.40, cw - 44.0, ch * 0.30, 0.3);
                    scene::gun_icon(&mut mesh, 4, 1, skin, x + cw * 0.52, mq::screen_height() - y - ch * 0.78, cw * 0.42, ch * 0.30, 0.3);
                } else {
                    let sight = k as u8;
                    let wid = if sight >= scene::SCOPE4 { 2 } else { 0 };
                    label(scene::SIGHT_NAMES[k], x + 20.0, y + 32.0, 24.0, mq::WHITE);
                    label("ATTACHMENT / MOUNTED PROFILE", x + 20.0, y + 55.0, 16.0, mq::LIGHTGRAY);
                    sight_icon(&mut mesh, wid, sight, x + cw * 0.5, mq::screen_height() - y - ch * 0.40, cw * 0.65, ch * 0.35);
                    scene::gun_icon_sighted(
                        &mut mesh,
                        wid,
                        1,
                        0,
                        sight,
                        x + cw * 0.5,
                        mq::screen_height() - y - ch * 0.79,
                        cw - 44.0,
                        ch * 0.25,
                        0.3,
                    );
                }
                gpu.overlay(&mesh, mq::screen_width(), mq::screen_height());
            }
        } else if models {
            for (k, (name, _)) in SKINS.iter().enumerate() {
                label(name, 120.0 + k as f32 * 238.0, mq::screen_height() - 58.0, 22.0, mq::LIGHTGRAY);
            }
            label("EMBER / FRONT     COBALT / BACK", 40.0, mq::screen_height() - 22.0, 18.0, mq::LIGHTGRAY);
        }
        frames += 1;
        if frames == 24 {
            if let Some(path) = output {
                mq::get_screen_data().export_png(path);
                println!("Saved {path}");
                break;
            }
        }
        if mq::is_key_pressed(mq::KeyCode::Escape) {
            break;
        }
        mq::next_frame().await;
    }
}
