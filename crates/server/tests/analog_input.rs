use ferrite_server::{Config, Game};
use ferrite_shared::glam::vec3;
use ferrite_shared::map::World;
use ferrite_shared::proto::{btn2, decode, encode, Cmd};
use ferrite_shared::sim::{self, PState};

#[test]
fn keyboard_commands_keep_their_semantics_and_analog_commands_keep_magnitude() {
    let mut keyboard = Cmd { mx: -128, mz: 127, ..Default::default() };
    keyboard.clamp_movement();
    assert_eq!((keyboard.mx, keyboard.mz), (-1, 1));
    assert_eq!(keyboard.movement(), (-1.0, 1.0));
    let mut analog = Cmd::default();
    analog.set_analog_movement(0.25, -0.5);
    let received: Cmd = decode(&encode(&analog)).unwrap();
    assert_eq!(received, analog);
    let (x, z) = received.movement();
    assert!((x - 0.25).abs() < 1.0 / 127.0 && (z + 0.5).abs() < 1.0 / 127.0);
    analog.set_analog_movement(f32::NAN, f32::INFINITY);
    assert_eq!(analog.movement(), (0.0, 0.0));
    analog.mx = -128;
    analog.mz = 127;
    analog.clamp_movement();
    assert_eq!(analog.movement(), (-1.0, 1.0));
}

#[test]
fn a_half_stick_moves_half_as_far_and_full_stick_cannot_exceed_keyboard_speed() {
    let mut world = World::with_map(0xFE221, 3);
    world.heights.fill(0.0);
    world.blocks.clear();
    let start = vec3(-106.0, 0.0, 0.0);
    let mut keyboard = PState::spawn(start, 0);
    let (mut full, mut half) = (keyboard, keyboard);
    for seq in 1..=120 {
        let c = Cmd { seq, mz: 1, ..Default::default() };
        sim::step(&mut keyboard, &c, &world, 0);
        let mut a = c;
        a.set_analog_movement(0.0, 1.0);
        sim::step(&mut full, &a, &world, 0);
        a.set_analog_movement(0.0, 0.5);
        sim::step(&mut half, &a, &world, 0);
    }
    assert_eq!(full, keyboard);
    let ratio = (half.pos.z - start.z) / (full.pos.z - start.z);
    assert!((0.49..0.52).contains(&ratio), "half stick moved {ratio:.3} of full speed");
}

#[test]
fn analog_commands_predict_exactly_through_authoritative_validation() {
    let mut game = Game::new(Config {
        map: 3,
        per_team: 1,
        max_bots: 0,
        port: 0,
        discover: false,
        lobby_secs: 0,
        stats_file: None,
        ..Default::default()
    });
    let me = game.join("controller-test").unwrap();
    for _ in 0..40 {
        game.tick();
    }
    assert!(game.players[me].alive);
    let mut predicted = game.players[me].st;
    for seq in 1..=360 {
        let phase = seq as f32 * 0.03;
        let mut command = Cmd { seq, yaw: phase * 0.2, ..Default::default() };
        command.set_analog_movement(phase.sin() * 0.7, phase.cos() * 0.7);
        assert_ne!(command.more & btn2::ANALOG_MOVE, 0);
        let wire: Cmd = decode(&encode(&command)).unwrap();
        predicted.surge = game.players[me].st.surge;
        sim::step(&mut predicted, &wire, &game.world, game.dmask);
        game.apply_cmd(me, wire);
        assert_eq!(game.players[me].st, predicted, "analog prediction diverged at {seq}");
        game.tick();
    }
}
