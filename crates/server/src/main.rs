use ferrite_server::{Config, Game};
use std::net::UdpSocket;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    let mut cfg = Config { log: true, ..Default::default() };
    let mut bench: Option<u32> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--port" => cfg.port = val.parse().expect("--port <u16>"),
            "--bots" => cfg.per_team = val.parse().expect("--bots <players per team>"),
            "--seed" => cfg.seed = val.parse().expect("--seed <u64>"),
            "--tickets" => cfg.tickets = val.parse().expect("--tickets <n>"),
            "--skill" => cfg.bot_skill = val.parse().expect("--skill <f32>"),
            "--bench" => bench = Some(val.parse().expect("--bench <simulated seconds>")),
            "-h" | "--help" => {
                println!("ferrite-server [--port 27500] [--bots 12] [--seed N] [--tickets 250] [--skill 1.0] [--bench SECS]");
                return;
            }
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    if let Some(secs) = bench {
        // Headless soak: simulate as fast as possible and report cost.
        let mut g = Game::new(cfg);
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
