//! Triangle counts per scene module for every map, with no window or GPU.
//! cargo run --release -p ferrite --example triangle_profile
//!
//! The first table is the static world mesh, uploaded once per map. The
//! second is the per-frame geometry `beacons` rebuilds (machinery, beacons,
//! smoke, objectives), as the peak over two minutes of the visual clock
//! sampled every quarter second; the ambience row (leaves, motes, mist,
//! birds) is the peak over a few camera positions. An optional argument
//! picks the world seed (decimal or 0xhex; default 0xFE221, as asset_review).

#[allow(dead_code)]
#[path = "../src/gfx.rs"]
mod gfx;
#[allow(dead_code, unused_imports)]
#[path = "../src/scene.rs"]
mod scene;

use ferrite_shared::glam::{vec3, Vec3};
use ferrite_shared::map::{World, MAPS};

const SHORT: [&str; 7] = ["foundry", "quarry", "ridgeline", "depot", "spillway", "relay", "slag"];

fn table(title: &str, rows: &[scene::Profile]) {
    let mut names: Vec<&'static str> = Vec::new();
    for p in rows {
        for (name, _) in p {
            if !names.contains(name) {
                names.push(name);
            }
        }
    }
    println!("\n{title}\n");
    print!("| {:<22} |", "module");
    for s in SHORT {
        print!(" {s:>9} |");
    }
    println!();
    print!("| {:-<22} |", "");
    for _ in SHORT {
        print!(" {:->9} |", "");
    }
    println!();
    for name in names.iter().chain(std::iter::once(&"TOTAL")) {
        print!("| {name:<22} |");
        for p in rows {
            let n = if *name == "TOTAL" { p.iter().map(|r| r.1).sum() } else { p.iter().find(|r| r.0 == *name).map_or(0, |r| r.1) };
            if n == 0 && *name != "TOTAL" {
                print!(" {:>9} |", "-");
            } else {
                print!(" {n:>9} |");
            }
        }
        println!();
    }
}

fn main() {
    let seed = std::env::args()
        .nth(1)
        .and_then(|s| s.strip_prefix("0x").map_or_else(|| s.parse().ok(), |h| u64::from_str_radix(h, 16).ok()))
        .unwrap_or(0xFE221);
    let owners = [(0, 0.0, false), (0, 0.0, false), (2, 0.0, false), (1, 0.0, false), (1, 0.0, false)];
    let mut statics = Vec::new();
    let mut dynamics = Vec::new();
    for id in 0..MAPS.len() as u8 {
        let w = World::with_map(seed, id);
        let (_, prof) = scene::build_world_profiled(&w);
        statics.push(prof);
        // Peak per module over the sampled clock.
        let mut peak = scene::Profile::new();
        // The first two minutes, then a whole siren window in which the dam is
        // tested, so Spillway's flood is counted.
        let tested = (0..64).find(|&c| ferrite_shared::events::siren_tested(seed, c)).unwrap_or(0) as f32;
        let window = tested * ferrite_shared::events::SIREN_PERIOD;
        for k in 0..480 + 360 {
            let time = if k < 480 { k as f32 * 0.25 } else { window + (k - 480) as f32 * 0.25 };
            let (mut m, mut alpha, mut add) = (gfx::Mesh::default(), gfx::Mesh::default(), gfx::Mesh::default());
            let mut p = scene::Profile::new();
            scene::beacons_profiled(&mut m, &mut alpha, &mut add, &w, &owners, time, 0, &mut p);
            for (name, n) in p {
                scene::tally(&mut peak, name, 0);
                let row = peak.iter_mut().find(|r| r.0 == name).unwrap();
                row.1 = row.1.max(n);
            }
        }
        let (mut m, mut alpha, mut add) = (gfx::Mesh::default(), gfx::Mesh::default(), gfx::Mesh::default());
        scene::crates(&mut m, &w, 0);
        scene::tally(&mut peak, "crates", m.triangles());
        let mut amb = 0;
        for eye in [w.points[2], w.bases[0], w.bases[1], w.points[0]] {
            for k in 0..8 {
                m.clear();
                alpha.clear();
                add.clear();
                let eye = eye + Vec3::Y * 1.7;
                scene::ambience(&mut m, &mut alpha, &mut add, &w, eye, vec3(1.0, 0.0, 0.0), Vec3::Y, k as f32 * 7.3);
                amb = amb.max(m.triangles() + alpha.triangles() + add.triangles());
            }
        }
        scene::tally(&mut peak, "ambience (flora)", amb);
        dynamics.push(peak);
    }
    println!("Triangle profile, seed {seed:#x}. Columns are maps 0-6.");
    table("Static world (uploaded once)", &statics);
    table("Per frame, peak (rebuilt every frame; soldiers, effects and the HUD are extra)", &dynamics);
}
