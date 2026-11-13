use ferrite::*;
#[path = "client/metrics.rs"]
mod metrics;
use macroquad::prelude::*;
use std::{
    collections::VecDeque,
    env,
    io::ErrorKind,
    net::UdpSocket,
    time::{Duration, Instant},
};

const BLUE_TEAM: Color = Color::new(0.2, 0.68, 1.0, 1.0);
const ORANGE_TEAM: Color = Color::new(1.0, 0.48, 0.18, 1.0);
const INTERPOLATION_DELAY: Duration = Duration::from_millis(120);
const MAX_PENDING_INPUTS: usize = 64;

fn team_color(team: u8) -> Color {
    if team == 0 {
        BLUE_TEAM
    } else {
        ORANGE_TEAM
    }
}
fn settings() -> Conf {
    Conf {
        window_title: format!("FERRITE {} | Conquest", env!("CARGO_PKG_VERSION")),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

struct TimedSnapshot {
    arrived: Instant,
    state: Snapshot,
}

/// Interpolate snapshots of remote players on a short intentional delay.
/// Respawns and discontinuities snap immediately instead of sliding through walls.
fn remote_position(id: u32, history: &VecDeque<TimedSnapshot>, at: Instant) -> Option<(f32, f32)> {
    let render_at = at.checked_sub(INTERPOLATION_DELAY).unwrap_or(at);
    let latest = history.back()?.state.soldiers.iter().find(|s| s.id == id)?;
    let mut earlier: Option<&TimedSnapshot> = None;
    for sample in history {
        if sample.arrived <= render_at {
            earlier = Some(sample);
            continue;
        }
        let Some(old) = earlier else {
            break;
        };
        let a = old.state.soldiers.iter().find(|s| s.id == id);
        let b = sample.state.soldiers.iter().find(|s| s.id == id);
        if let (Some(a), Some(b)) = (a, b) {
            let distance = (a.x - b.x).hypot(a.z - b.z);
            if distance > 5.0 || a.health != b.health {
                return Some((b.x, b.z));
            }
            let span = sample.arrived.duration_since(old.arrived).as_secs_f32();
            let dt = render_at.duration_since(old.arrived).as_secs_f32();
            let blend = if span > 0.001 {
                (dt / span).clamp(0.0, 1.0)
            } else {
                1.0
            };
            return Some((a.x + (b.x - a.x) * blend, a.z + (b.z - a.z) * blend));
        }
        break;
    }
    if let Some(old) = earlier {
        if let Some(p) = old.state.soldiers.iter().find(|s| s.id == id) {
            return Some((p.x, p.z));
        }
    }
    Some((latest.x, latest.z))
}

#[macroquad::main(settings)]
async fn main() {
    let address = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".into());
    let nickname = env::args().nth(2).unwrap_or_else(|| "Soldier".into());
    let sock = UdpSocket::bind("0.0.0.0:0").expect("Cannot bind local UDP port");
    sock.connect(&address)
        .expect("Cannot resolve/connect server address");
    sock.set_nonblocking(true)
        .expect("Cannot make socket nonblocking");

    let mut snapshot: Option<Snapshot> = None;
    let mut snapshot_history: VecDeque<TimedSnapshot> = VecDeque::new();
    let mut predicted: Option<Soldier> = None;
    let mut pending: VecDeque<Input> = VecDeque::new();
    let mut error_x = 0.0_f32;
    let mut error_z = 0.0_f32;
    let mut yaw = std::f32::consts::FRAC_PI_2;
    let mut pitch = 0.0_f32;
    let mut sequence = 0_u64;
    let mut accumulator = 0.0_f32;
    let mut last_receive = Instant::now();
    let mut captured = false;
    let mut last_mouse = (0.0_f32, 0.0_f32);
    let mut muzzle_timer = 0.0_f32;
    let mut hit_timer = 0.0_f32;
    let mut damage_timer = 0.0_f32;
    let mut last_event_id = 0_u64;
    let mut last_hit_label = String::new();
    let mut bytes = [0_u8; 65_535];
    let mut metrics = metrics::Metrics::new();
    let mut fire_sampled_at = Instant::now();
    let mut was_firing = false;
    let mut first_fire_command = false;

    loop {
        let frame_started = Instant::now();
        let dt = get_frame_time().min(0.1);
        if is_key_pressed(KeyCode::Escape) {
            captured = false;
            set_cursor_grab(false);
            show_mouse(true);
        }
        if is_mouse_button_pressed(MouseButton::Left) && !captured {
            captured = true;
            set_cursor_grab(true);
            show_mouse(false);
            last_mouse = mouse_position();
        }
        if captured {
            let (mx, my) = mouse_position();
            yaw -= (mx - last_mouse.0).clamp(-120.0, 120.0) * 0.0024;
            pitch = (pitch - (my - last_mouse.1).clamp(-120.0, 120.0) * 0.0024).clamp(-1.25, 1.25);
            last_mouse = (mx, my);
        }
        let forward = if captured {
            (is_key_down(KeyCode::W) as i32 - is_key_down(KeyCode::S) as i32) as f32
        } else {
            0.0
        };
        let strafe = if captured {
            (is_key_down(KeyCode::D) as i32 - is_key_down(KeyCode::A) as i32) as f32
        } else {
            0.0
        };
        let fire = captured && is_mouse_button_down(MouseButton::Left);
        if fire && !was_firing {
            fire_sampled_at = frame_started;
            first_fire_command = true;
        }
        was_firing = fire;
        metrics.input_sample(fire, frame_started);
        let sprint = captured && is_key_down(KeyCode::LeftShift);

        // Predict immediately in the exact same 20Hz movement routine as the server.
        accumulator += dt;
        while accumulator >= TICK_DT {
            accumulator -= TICK_DT;
            sequence += 1;
            let command = Input {
                version: PROTOCOL_VERSION,
                seq: sequence,
                name: nickname.clone(),
                forward,
                strafe,
                yaw,
                pitch,
                fire,
                sprint,
            };
            if let Some(local) = &mut predicted {
                apply_input(local, &command);
                pending.push_back(command.clone());
                if pending.len() > MAX_PENDING_INPUTS {
                    pending.pop_front();
                }
            }
            if let Ok(packet) = serde_json::to_vec(&command) {
                let _ = sock.send(&packet);
            }
            let sampled = if fire && first_fire_command {
                fire_sampled_at
            } else {
                frame_started
            };
            metrics.command(&command, sampled, Instant::now());
            if fire {
                first_fire_command = false;
            }
            if fire {
                muzzle_timer = 0.05;
            }
        }

        // Reconcile authoritative self; keep timed remote snapshots for interpolation.
        loop {
            match sock.recv(&mut bytes) {
                Ok(n) => {
                    let Ok(incoming) = serde_json::from_slice::<Snapshot>(&bytes[..n]) else {
                        continue;
                    };
                    if incoming.version != PROTOCOL_VERSION
                        || snapshot
                            .as_ref()
                            .is_some_and(|old| old.tick >= incoming.tick)
                    {
                        continue;
                    }
                    if let Some(me) = incoming.soldiers.iter().find(|s| s.id == incoming.you) {
                        if let Some(before) = &predicted {
                            pending.retain(|command| command.seq > incoming.ack_seq);
                            let unacked: Vec<Input> = pending.iter().cloned().collect();
                            let corrected = replay_unacked(me, &unacked, incoming.ack_seq);
                            let delta = (before.x - corrected.x).hypot(before.z - corrected.z);
                            metrics.reconcile(
                                incoming.tick,
                                incoming.ack_seq,
                                delta,
                                pending.len(),
                                delta >= 5.0 || before.health != corrected.health,
                                forward != 0.0 || strafe != 0.0,
                            );
                            if delta < 5.0 && before.health == corrected.health {
                                error_x += before.x - corrected.x;
                                error_z += before.z - corrected.z;
                            } else {
                                error_x = 0.0;
                                error_z = 0.0;
                            }
                            predicted = Some(corrected);
                        } else {
                            // The join command already overwrote the server's spawn yaw.
                            // Derive the initial view from team assignment instead.
                            yaw = spawn_yaw(me.team);
                            predicted = Some(me.clone());
                            pending.clear();
                        }
                    }
                    for e in &incoming.events {
                        if e.id <= last_event_id {
                            continue;
                        }
                        last_event_id = e.id;
                        if e.shooter == incoming.you {
                            metrics.hit(e, Instant::now());
                            hit_timer = 0.22;
                            last_hit_label = if e.killed {
                                "KILL CONFIRMED".into()
                            } else if e.headshot {
                                "HEADSHOT".into()
                            } else {
                                format!("HIT +{}", e.damage)
                            };
                        }
                        if e.victim == incoming.you {
                            damage_timer = 0.35;
                        }
                    }
                    last_receive = Instant::now();
                    snapshot_history.push_back(TimedSnapshot {
                        arrived: last_receive,
                        state: incoming.clone(),
                    });
                    while snapshot_history.len() > 16 {
                        snapshot_history.pop_front();
                    }
                    snapshot = Some(incoming);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        let follow = (-12.0 * dt).exp();
        error_x *= follow;
        error_z *= follow;
        let (render_x, render_z, health, team, kills, deaths) = if let Some(local) = &predicted {
            (
                local.x + error_x,
                local.z + error_z,
                local.health,
                local.team,
                local.kills,
                local.deaths,
            )
        } else {
            (-41.0, 0.0, MAX_HEALTH, 0, 0, 0)
        };

        clear_background(Color::new(0.61, 0.78, 0.88, 1.0));
        let eye = vec3(render_x, 1.75, render_z);
        let direction = vec3(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            yaw.cos() * pitch.cos(),
        );
        set_camera(&Camera3D {
            position: eye,
            target: eye + direction,
            up: vec3(0.0, 1.0, 0.0),
            fovy: 75.0_f32.to_radians(),
            ..Default::default()
        });
        draw_plane(
            vec3(0., -0.11, 0.),
            vec2(MAP_HALF * 2.0, MAP_HALF * 2.0),
            None,
            Color::new(0.27, 0.38, 0.31, 1.0),
        );
        draw_cube(
            vec3(0., -0.07, 0.),
            vec3(MAP_HALF * 2.0, 0.07, 7.0),
            None,
            Color::new(0.30, 0.32, 0.32, 1.0),
        );
        for i in -9..10 {
            draw_cube(
                vec3(i as f32 * 5., -0.018, 0.),
                vec3(2.0, 0.014, 0.13),
                None,
                Color::new(0.78, 0.78, 0.65, 1.0),
            );
        }
        for &(x, z, w, d, h) in &COVER {
            let color = if h > 4.0 {
                Color::new(0.42, 0.45, 0.42, 1.)
            } else {
                Color::new(0.50, 0.47, 0.39, 1.)
            };
            draw_cube(vec3(x, h / 2., z), vec3(w, h, d), None, color);
            draw_cube_wires(
                vec3(x, h / 2., z),
                vec3(w, h, d),
                Color::new(0.22, 0.25, 0.23, 1.),
            );
        }
        let wall = Color::new(0.24, 0.30, 0.31, 1.0);
        draw_cube(
            vec3(0., 1.5, MAP_HALF),
            vec3(MAP_HALF * 2., 3., 1.),
            None,
            wall,
        );
        draw_cube(
            vec3(0., 1.5, -MAP_HALF),
            vec3(MAP_HALF * 2., 3., 1.),
            None,
            wall,
        );
        draw_cube(
            vec3(MAP_HALF, 1.5, 0.),
            vec3(1., 3., MAP_HALF * 2.),
            None,
            wall,
        );
        draw_cube(
            vec3(-MAP_HALF, 1.5, 0.),
            vec3(1., 3., MAP_HALF * 2.),
            None,
            wall,
        );
        if let Some(ref world) = snapshot {
            for z in &world.zones {
                let color = if z.owner < 0 {
                    YELLOW
                } else {
                    team_color(z.owner as u8)
                };
                draw_cube(
                    vec3(z.x, 0.05, z.z),
                    vec3(12., 0.10, 12.),
                    None,
                    Color::new(color.r, color.g, color.b, 0.4),
                );
                draw_cube(vec3(z.x, 3.0, z.z), vec3(0.18, 6., 0.18), None, WHITE);
                draw_cube(vec3(z.x + 1.1, 5.0, z.z), vec3(2.1, 1.1, 0.08), None, color);
            }
            for p in &world.soldiers {
                if p.id == world.you || p.health <= 0 {
                    continue;
                }
                let (x, z) =
                    remote_position(p.id, &snapshot_history, Instant::now()).unwrap_or((p.x, p.z));
                let color = team_color(p.team);
                draw_cube(vec3(x, 0.95, z), vec3(0.85, 1.45, 0.55), None, color);
                draw_cube(
                    vec3(x, 1.88, z),
                    vec3(0.58, 0.58, 0.58),
                    None,
                    Color::new(0.85, 0.78, 0.65, 1.),
                );
                let (fx, fz) = (p.yaw.sin(), p.yaw.cos());
                draw_cube(
                    vec3(x + fx * 0.42, 1.30, z + fz * 0.42),
                    vec3(0.16, 0.17, 0.85),
                    None,
                    DARKGRAY,
                );
            }
        } else {
            for &(x, z) in &ZONES {
                draw_cube(vec3(x, 0.04, z), vec3(12., 0.08, 12.), None, GRAY);
            }
        }
        set_default_camera();
        let (w, h) = (screen_width(), screen_height());
        let accent = team_color(team);
        draw_rectangle(0., 0., w, 64., Color::new(0.04, 0.07, 0.09, 0.79));
        draw_text(
            format!(
                "F E R R I T E  /  C O N Q U E S T  {}",
                env!("CARGO_PKG_VERSION")
            ),
            20.,
            28.,
            24.,
            WHITE,
        );
        if let Some(world) = &snapshot {
            draw_text(
                format!(
                    "BLUE {:03}  |  {:03} ORANGE",
                    world.score[0], world.score[1]
                ),
                w * 0.5 - 122.,
                27.,
                27.,
                WHITE,
            );
            let mut zone_label = String::new();
            for (i, z) in world.zones.iter().enumerate() {
                zone_label.push_str(&format!(
                    " {}:{}% ",
                    ['A', 'B', 'C'][i],
                    z.progress.round() as i32
                ));
            }
            draw_text(&zone_label, w * 0.5 - 120., 52., 19., YELLOW);
            if let Some(win) = world.winner {
                draw_text(
                    if win == 0 {
                        "BLUE TEAM WINS"
                    } else {
                        "ORANGE TEAM WINS"
                    },
                    w * 0.5 - 180.,
                    h * 0.43,
                    42.,
                    team_color(win),
                );
            }
        } else {
            draw_text("CONNECTING...", w * 0.5 - 90., 27., 25., YELLOW);
        }
        if last_receive.elapsed().as_secs_f32() > 3.0 {
            draw_text(
                "SERVER UNREACHABLE - start ferrite-server",
                w * 0.5 - 230.,
                h * 0.3,
                26.,
                RED,
            );
        }
        let middle = vec2(w * 0.5, h * 0.5);
        let crosshair = if hit_timer > 0.0 { YELLOW } else { WHITE };
        draw_line(
            middle.x - 12.,
            middle.y,
            middle.x - 4.,
            middle.y,
            2.,
            crosshair,
        );
        draw_line(
            middle.x + 4.,
            middle.y,
            middle.x + 12.,
            middle.y,
            2.,
            crosshair,
        );
        draw_line(
            middle.x,
            middle.y - 12.,
            middle.x,
            middle.y - 4.,
            2.,
            crosshair,
        );
        draw_line(
            middle.x,
            middle.y + 4.,
            middle.x,
            middle.y + 12.,
            2.,
            crosshair,
        );
        if muzzle_timer > 0.0 {
            draw_circle(middle.x + 80., h - 80., 18., YELLOW);
            muzzle_timer -= dt;
        }
        if hit_timer > 0.0 {
            draw_text(&last_hit_label, middle.x - 68., middle.y + 50., 23., YELLOW);
            hit_timer -= dt;
        }
        metrics.draw_feedback(Instant::now());
        if damage_timer > 0.0 {
            let alpha = (damage_timer * 0.6).clamp(0.0, 0.24);
            draw_rectangle(0., 0., w, h, Color::new(0.95, 0.03, 0.03, alpha));
            damage_timer -= dt;
        }
        if health <= 0 {
            draw_text("KIA - RESPAWNING", w * 0.5 - 150., h * 0.6, 32., RED);
        }
        draw_rectangle(16., h - 88., 300., 72., Color::new(0.04, 0.07, 0.09, 0.8));
        draw_text(
            format!(
                "{} TEAM | HP {}",
                if team == 0 { "BLUE" } else { "ORANGE" },
                health
            ),
            30.,
            h - 57.,
            25.,
            accent,
        );
        draw_text(
            format!(
                "K {} / D {} | {} / {} DMG",
                kills, deaths, BODY_DAMAGE, HEAD_DAMAGE
            ),
            30.,
            h - 29.,
            19.,
            WHITE,
        );
        draw_rectangle(
            w - 360.,
            h - 62.,
            345.,
            47.,
            Color::new(0.04, 0.07, 0.09, 0.8),
        );
        draw_text(
            "WASD MOVE | SHIFT SPRINT | LMB FIRE",
            w - 352.,
            h - 40.,
            16.,
            WHITE,
        );
        draw_text(
            "CLICK LOCK MOUSE | ESC RELEASE",
            w - 352.,
            h - 23.,
            16.,
            LIGHTGRAY,
        );
        if !captured {
            draw_rectangle(
                w * 0.5 - 220.,
                h * 0.5 + 42.,
                440.,
                64.,
                Color::new(0.04, 0.07, 0.09, 0.85),
            );
            draw_text(
                "CLICK TO DEPLOY",
                w * 0.5 - 130.,
                h * 0.5 + 83.,
                32.,
                YELLOW,
            );
        }
        next_frame().await;
    }
}
