use ferrite_server::{Config, Game};
use std::net::UdpSocket;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("ferrite-server {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let mut cfg = Config { log: true, lobby_secs: 30, ..Default::default() };
    let mut bench: Option<u32> = None;
    // Player records: a file beside the executable unless told otherwise.
    let mut stats: Option<String> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--port" => cfg.port = val.parse().expect("--port <u16>"),
            "--bots" => cfg.per_team = val.parse().expect("--bots <players per team>"),
            "--max-bots" => cfg.max_bots = val.parse().expect("--max-bots <n>"),
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
            "--mode" => {
                cfg.mode = ferrite_shared::mode_by_name(&val).unwrap_or_else(|| {
                    eprintln!("unknown mode '{val}'. Try --modes for the list.");
                    std::process::exit(2);
                })
            }
            "--modes" => {
                for (i, m) in ferrite_shared::MODES.iter().enumerate() {
                    println!("{i}  {:<11} {:<7} {}", m.key, m.pace.to_lowercase(), m.blurb);
                }
                return;
            }
            "--mode-rotation" => {
                // "all", "off", or a comma-separated list of mode names.
                cfg.mode_rotation = match val.as_str() {
                    "off" | "none" => Vec::new(),
                    "all" => (0..ferrite_shared::MODES.len() as u8).collect(),
                    list => list
                        .split(',')
                        .map(|n| {
                            ferrite_shared::mode_by_name(n).unwrap_or_else(|| {
                                eprintln!("unknown mode '{n}' in --mode-rotation. Try --modes for the list.");
                                std::process::exit(2);
                            })
                        })
                        .collect(),
                };
            }
            "--score" => cfg.score_limit = val.parse().expect("--score <points that win a round outside conquest>"),
            "--time" => cfg.time_limit = val.parse().expect("--time <round length in seconds outside conquest>"),
            "--no-drops" => {
                cfg.drops = false;
                i += 1;
                continue;
            }
            "--lobby" => cfg.lobby_secs = val.parse().expect("--lobby <seconds, 0 for none>"),
            "--name" => cfg.name = val.chars().filter(|c| !c.is_control()).take(24).collect(),
            "--no-discovery" => {
                cfg.discover = false;
                i += 1;
                continue;
            }
            "--stats" => stats = Some(val.clone()),
            "--tickets" => cfg.tickets = val.parse().expect("--tickets <n>"),
            "--skill" => cfg.bot_skill = val.parse().expect("--skill <f32>"),
            "--difficulty" => {
                cfg.bot_skill = ferrite_server::difficulty_by_name(&val).unwrap_or_else(|| {
                    eprintln!("unknown difficulty '{val}'. One of: recruit, regular, veteran, elite.");
                    std::process::exit(2);
                })
            }
            "--bench" => bench = Some(val.parse().expect("--bench <simulated seconds>")),
            "-h" | "--help" => {
                println!("ferrite-server [--version] [--port 27500] [--map NAME|N] [--maps] [--rotation all|off|a,b,c] [--mode conquest|tdm|ctf|stronghold] [--modes] [--mode-rotation all|off|a,b,c] [--score N] [--time SECS] [--no-drops] [--lobby 30] [--name NAME] [--no-discovery] [--bots 12] [--max-bots N] [--seed N] [--tickets 250] [--stats FILE|off] [--difficulty recruit|regular|veteran|elite] [--skill 1.0] [--bench SECS]");
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
    if cfg.mode_rotation.len() > 1 && !args.iter().any(|a| a == "--mode") {
        cfg.mode = cfg.mode_rotation[0];
    }
    cfg.stats_file = match stats.as_deref() {
        Some("off") | Some("none") => None,
        Some(f) => Some(f.into()),
        None if bench.is_some() => None,
        None => std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("ferrite-stats.txt"))),
    };
    if let Some(secs) = bench {
        cfg.lobby_secs = 0;
        // Headless soak: simulate as fast as possible and report cost.
        let mut g = Game::new(cfg);
        println!("map: {} - mode: {}", ferrite_shared::map::map_def(g.world.map).name, ferrite_shared::mode_def(g.mode).name);
        let n = secs * 60;
        let t0 = std::time::Instant::now();
        let mut worst = 0.0f64;
        for _ in 0..n {
            let s = std::time::Instant::now();
            g.tick();
            if g.tick % 1800 == 0 {
                let o: String = g.points.iter().map(|p| ['E', 'C', '-'][p.owner as usize]).collect();
                let alive = |t: u8| g.players.iter().filter(|p| p.team == t && p.alive).count();
                println!("  t={:>4}s  line [{}]  score {:>3}-{:<3}  alive {}v{}  kills {}  pickups {}  rounds {}", g.tick / 60, o, g.tickets[0], g.tickets[1], alive(0), alive(1), g.stats.kills, g.pickups.len(), g.stats.rounds);
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
