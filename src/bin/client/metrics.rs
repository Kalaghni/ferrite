//! Opt-in real-client measurements. No clock synchronization or mock transport.
use ferrite::{CombatEvent, Input};
use serde_json::json;
use std::{
    collections::VecDeque,
    env,
    fs::File,
    io::{BufWriter, Write},
    time::{Duration, Instant},
};

struct Command {
    seq: u64,
    sampled: Instant,
    trigger: Option<(u64, Instant)>,
}

pub struct Metrics {
    output: Option<BufWriter<File>>,
    started: Instant,
    last_flush: Instant,
    commands: VecDeque<Command>,
    trigger: Option<(u64, Instant)>,
    next_trigger: u64,
    last_confirmed_trigger: u64,
    firing: bool,
    feedback: Vec<(CombatEvent, Instant)>,
}

impl Metrics {
    pub fn new() -> Self {
        let started = Instant::now();
        let output =
            env::var_os("FERRITE_METRICS_PATH").and_then(|path| match File::create(path) {
                Ok(file) => Some(BufWriter::new(file)),
                Err(error) => {
                    eprintln!("Cannot open client metrics: {error}");
                    None
                }
            });
        let mut metrics = Self {
            output,
            started,
            last_flush: started,
            commands: VecDeque::new(),
            trigger: None,
            next_trigger: 0,
            last_confirmed_trigger: 0,
            firing: false,
            feedback: Vec::new(),
        };
        metrics.write(
            json!({"kind":"session", "version":env!("CARGO_PKG_VERSION"),
            "correction_threshold_m":0.01, "hit_clock":"input sampling to HUD draw submission"}),
        );
        metrics
    }

    fn write(&mut self, value: serde_json::Value) {
        let Some(output) = self.output.as_mut() else {
            return;
        };
        let result = serde_json::to_writer(&mut *output, &value)
            .map_err(std::io::Error::other)
            .and_then(|()| output.write_all(b"\n"));
        if let Err(error) = result {
            eprintln!("Client metrics disabled after write error: {error}");
            self.output = None;
        }
    }

    pub fn input_sample(&mut self, fire: bool, now: Instant) {
        if self.output.is_none() {
            return;
        }
        if fire && !self.firing {
            self.next_trigger += 1;
            self.trigger = Some((self.next_trigger, now));
            self.write(json!({"kind":"trigger", "t":now.duration_since(self.started).as_secs_f64(), "id":self.next_trigger}));
        }
        self.firing = fire;
        if !fire {
            self.trigger = None;
        }
    }

    pub fn command(&mut self, input: &Input, sampled: Instant, sent: Instant) {
        if self.output.is_none() {
            return;
        }
        if input.fire {
            self.commands.push_back(Command {
                seq: input.seq,
                sampled,
                trigger: self.trigger,
            });
            self.write(
                json!({"kind":"fire_command", "t":sent.duration_since(self.started).as_secs_f64(),
                "seq":input.seq, "yaw":input.yaw, "pitch":input.pitch,
                "input_queue_ms":sent.duration_since(sampled).as_secs_f64()*1000.0}),
            );
        }
        while self
            .commands
            .front()
            .is_some_and(|c| sent.duration_since(c.sampled) > Duration::from_secs(10))
        {
            self.commands.pop_front();
        }
    }

    pub fn reconcile(
        &mut self,
        tick: u64,
        ack: u64,
        error: f32,
        pending: usize,
        discontinuity: bool,
        moving: bool,
    ) {
        if self.output.is_none() {
            return;
        }
        self.write(
            json!({"kind":"reconcile", "t":self.started.elapsed().as_secs_f64(),
            "tick":tick, "ack":ack, "prediction_error_m":error,
            "corrected":error > 0.01, "pending":pending, "discontinuity":discontinuity,
            "moving":moving}),
        );
    }

    pub fn hit(&mut self, event: &CombatEvent, received: Instant) {
        if self.output.is_some() {
            self.feedback.push((event.clone(), received));
        }
    }

    /// Called where this frame's hit label is submitted to Macroquad.
    pub fn draw_feedback(&mut self, now: Instant) {
        let events = std::mem::take(&mut self.feedback);
        for (event, received) in events {
            let command = event
                .input_seq
                .and_then(|seq| self.commands.iter().find(|c| c.seq == seq));
            let command_latency =
                command.map(|c| now.duration_since(c.sampled).as_secs_f64() * 1000.0);
            let trigger = command.and_then(|c| c.trigger);
            let trigger_latency = trigger
                .filter(|(id, _)| *id > self.last_confirmed_trigger)
                .map(|(id, started)| {
                    self.last_confirmed_trigger = id;
                    now.duration_since(started).as_secs_f64() * 1000.0
                });
            self.write(
                json!({"kind":"hit_feedback", "t":now.duration_since(self.started).as_secs_f64(),
                "event_id":event.id, "input_seq":event.input_seq, "damage":event.damage,
                "killed":event.killed, "headshot":event.headshot,
                "command_to_hud_ms":command_latency, "trigger_to_first_hud_ms":trigger_latency,
                "receive_to_hud_ms":now.duration_since(received).as_secs_f64()*1000.0}),
            );
        }
        if now.duration_since(self.last_flush) >= Duration::from_secs(1) {
            if let Some(output) = self.output.as_mut() {
                if let Err(error) = output.flush() {
                    eprintln!("Client metrics flush: {error}");
                }
            }
            self.last_flush = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite::PROTOCOL_VERSION;

    #[test]
    fn hit_timing_matches_command_and_first_trigger_only_once() {
        let started = Instant::now();
        let path =
            std::env::temp_dir().join(format!("ferrite-metrics-{}.jsonl", std::process::id()));
        let mut metrics = Metrics {
            output: Some(BufWriter::new(File::create(&path).unwrap())),
            started,
            last_flush: started,
            commands: VecDeque::new(),
            trigger: None,
            next_trigger: 0,
            last_confirmed_trigger: 0,
            firing: false,
            feedback: Vec::new(),
        };
        let at = |ms| started + Duration::from_millis(ms);
        metrics.input_sample(true, started);
        let mut input = Input {
            version: PROTOCOL_VERSION,
            seq: 11,
            name: "test".into(),
            forward: 0.0,
            strafe: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            fire: true,
            sprint: false,
        };
        metrics.command(&input, at(20), at(30));
        input.seq = 12;
        metrics.command(&input, at(50), at(60));
        let mut event = CombatEvent {
            id: 1,
            tick: 20,
            shooter: 1,
            victim: 2,
            damage: 25,
            headshot: false,
            killed: false,
            input_seq: Some(11),
        };
        metrics.hit(&event, at(100));
        metrics.draw_feedback(at(110));
        event.id = 2;
        event.input_seq = Some(12);
        metrics.hit(&event, at(150));
        metrics.draw_feedback(at(160));
        event.id = 3;
        event.input_seq = Some(999); // Never infer a timestamp from an unrelated shot.
        metrics.hit(&event, at(180));
        metrics.draw_feedback(at(190));
        drop(metrics);
        let data = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let rows: Vec<serde_json::Value> = data
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .filter(|v: &serde_json::Value| v["kind"] == "hit_feedback")
            .collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["command_to_hud_ms"], 90.0);
        assert_eq!(rows[0]["trigger_to_first_hud_ms"], 110.0);
        assert_eq!(rows[1]["command_to_hud_ms"], 110.0);
        assert!(rows[1]["trigger_to_first_hud_ms"].is_null());
        assert!(rows[2]["command_to_hud_ms"].is_null());
    }
}
