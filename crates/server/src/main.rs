use ferrite_server::{Config, Game};
use std::net::UdpSocket;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    let mut cfg = Config { log: true, lobby_secs: 30, ..Default::default() };
    let mut bench: Option<u32> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--port" => cfg.port = val.parse().expect("--port <u16>"),
            "--bots" => cfg.per_team = val.parse().expect("--bots <players per team>"),
            "--seed" => cfg.seed = val.parse().expect("--seed <u64>"),
            "--map" => match ferrite_shared::map::map_by_name(&val) {
                Some(m) => cfg.map = m,
                None => {
                    eprintln!("unknown map '{val}'. Try --maps for the list.");
                    std::process::exit(2);
                }
            },
            "--maps" => {
                for (i, m) in ferrite_shared::map::MAPS.iter().enumerate() {
                    println!("{i}  {:<15} {}", m.name.to_lowercase(), m.blurb);
                }
                return;
            }
            "--rotation" => {
                // "all", "off", or a comma-separated list of map names.
                cfg.rotation = match val.as_str() {
                    "off" | "none" => vec![u8::MAX],
                    "all" => (0..ferrite_shared::map::MAPS.len() as u8).collect(),
                    list => list
                        .split(',')
                        .map(|n| {
                            ferrite_shared::map::map_by_name(n.trim()).unwrap_or_else(|| {
                                eprintln!("unknown map '{n}' in --rotation. Try --maps for the list.");
                                std::process::exit(2);
                            })
                        })
                        .collect(),
                };
            }
            "--lobby" => cfg.lobby_secs = val.parse().expect("--lobby <seconds, 0 for none>"),
            "--name" => cfg.name = val.chars().filter(|c| !c.is_control()).take(24).collect(),
            "--no-discovery" => {
                cfg.discover = false;
                i += 1;
                continue;
            }
            "--tickets" => cfg.tickets = val.parse().expect("--tickets <n>"),
            "--skill" => cfg.bot_skill = val.parse().expect("--skill <f32>"),
            "--bench" => bench = Some(val.parse().expect("--bench <simulated seconds>")),
            "-h" | "--help" => {
                println!("ferrite-server [--port 27500] [--map NAME|N] [--maps] [--rotation all|off|a,b,c] [--lobby 30] [--name NAME] [--no-discovery] [--bots 12] [--seed N] [--tickets 250] [--skill 1.0] [--bench SECS]");
                return;
            }
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    // A dedicated server plays every map in turn unless told otherwise.
    if cfg.rotation.is_empty() {
        cfg.rotation = (0..ferrite_shared::map::MAPS.len() as u8).collect();
    } else if cfg.rotation == [u8::MAX] {
        cfg.rotation.clear();
    } else if !args.iter().any(|a| a == "--map") {
        cfg.map = cfg.rotation[0];
    }
    if let Some(secs) = bench {
        cfg.lobby_secs = 0;
        // Headless soak: simulate as fast as possible and report cost.
        let mut g = Game::new(cfg);
        println!("map: {}", ferrite_shared::map::map_def(g.world.map).name);
        let n = secs * 60;
        let t0 = std::time::Instant::now();
        let mut worst = 0.0f64;
        for _ in 0..n {
            let s = std::time::Instant::now();
            g.tick();
            if g.tick % 1800 == 0 {
                let o: String = g.points.iter().map(|p| ['E', 'C', '-'][p.owner as usize]).collect();
                let alive = |t: u8| g.players.iter().filter(|p| p.team == t && p.alive).count();
                println!("  t={:>4}s  line [{}]  tickets {:>3}-{:<3}  alive {}v{}  kills {}", g.tick / 60, o, g.tickets[0], g.tickets[1], alive(0), alive(1), g.stats.kills);
            }
            worst = worst.max(s.elapsed().as_secs_f64() * 1000.0);
        }
        let el = t0.elapsed().as_secs_f64();
        println!(
            "simulated {} s of {}v{} in {:.2} s ({:.0}x realtime) | avg {:.3} ms/tick, worst {:.3} ms (budget 16.67)",
            secs,
            g.cfg.per_team,
            g.cfg.per_team,
            el,
            secs as f64 / el,
            el * 1000.0 / n as f64,
            worst
        );
        println!("kills {} shots {} captures {} rounds {} tickets {:?}", g.stats.kills, g.stats.shots, g.stats.captures, g.stats.rounds, g.tickets);
        return;
    }
    let sock = UdpSocket::bind(("0.0.0.0", cfg.port)).expect("could not bind UDP port");
    ferrite_server::run(cfg, sock, Arc::new(AtomicBool::new(false)));
}
