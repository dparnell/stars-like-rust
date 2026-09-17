//! Messages: written by a turn, carried by a file, read back.
//!
//! The format itself is checked in `stars-formats` against the 1,053 real
//! messages in the fixtures; this is about the engine producing them and the
//! writers carrying them. See `docs/formats/message.md`.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::message::id;
use stars_core::minefield::Minefield;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::Race;
use stars_core::{generate_turn, save, GameState, Player, Rng};
use stars_formats::StarsFile;

/// One player, one planet, one fleet that lays mines and carries lasers.
fn a_game() -> GameState {
    let mut state = GameState::new(11);
    state.players = vec![Player::new(Race::humanoid()), Player::new(Race::humanoid())];
    state.designs = vec![
        vec![ShipDesign {
            name: "Layer".to_string(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: 4,
            slots: vec![
                DesignSlot {
                    category: slot::MINES,
                    item: 0,
                    count: 1,
                },
                DesignSlot {
                    category: slot::BEAM,
                    item: 0,
                    count: 4,
                },
            ],
        }],
        Vec::new(),
    ];
    let mut planet = Planet::unowned(1);
    planet.owner = Some(0);
    planet.position = Some(Point::new(1000, 1000));
    planet.pop = 25_000;
    state.planets = vec![planet];
    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id: 1,
        owner: 0,
        position: Point::new(1000, 1000),
        orbiting: Some(1),
        stacks: vec![ShipStack {
            design: 0,
            count: 2,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo::default(),
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: Point::new(1000, 1000),
            target: Some(1),
            target_class: 1,
            warp: 0,
            task: stars_formats::task::LAY_MINES,
            transport: None,
            task_data: vec![5, 0, 5, 0],
        }],
    }];
    state
}

/// A year that lays mines and sweeps somebody else's tells both sides about it.
#[test]
fn a_turn_writes_the_news() {
    let mut state = a_game();
    // Player 1 has a small field right where our fleet is sitting.
    state.minefields.push(Minefield {
        id: 0,
        owner: 1,
        position: Point::new(1000, 1000),
        mines: 5_000,
        kind: 0,
        detonating: false,
        detected_by: 0,
        visible_to: 0,
        turn: 0,
    });

    let mut rng = Rng::from_seeds(1, 2);
    generate_turn(&mut state, &mut rng);

    let ids: Vec<u16> = state.messages.iter().map(|m| m.id).collect();
    assert!(ids.contains(&id::MINES_LAID), "{ids:?}");
    assert!(ids.contains(&id::FLEET_SWEPT), "{ids:?}");
    assert!(
        ids.contains(&id::YOUR_FIELD_SWEPT),
        "the field's owner is told too: {ids:?}"
    );

    let laid = state
        .messages
        .iter()
        .find(|m| m.id == id::MINES_LAID)
        .expect("a laying message");
    assert_eq!(laid.player, 0);
    assert!(laid.summary().contains("80 mines"), "{}", laid.summary());

    let theirs = state
        .messages
        .iter()
        .find(|m| m.id == id::YOUR_FIELD_SWEPT)
        .expect("a sweeping message");
    assert_eq!(theirs.player, 1, "sent to whoever owns the field");

    // Next year's news replaces this year's.
    let before = state.messages.len();
    assert!(before > 0);
    generate_turn(&mut state, &mut rng);
    assert!(
        state.messages.iter().all(|m| m.id != id::FLEET_SWEPT) || state.messages.len() <= before,
        "the queue is rebuilt, not appended to"
    );
}

/// A player's messages travel in their turn file and come back.
#[test]
fn messages_survive_a_player_file() {
    let mut state = a_game();
    let mut rng = Rng::from_seeds(3, 4);
    generate_turn(&mut state, &mut rng);
    let mine: Vec<_> = state
        .messages
        .iter()
        .filter(|m| m.player == 0)
        .cloned()
        .collect();
    assert!(!mine.is_empty(), "the year produced news");

    let bytes = save::player_file(&state, 0).expect("writes");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let (back, _) = GameState::from_file(&file);

    assert_eq!(back.messages.len(), mine.len());
    for (read, sent) in back.messages.iter().zip(&mine) {
        assert_eq!(read.id, sent.id);
        assert_eq!(read.object, sent.object);
        assert_eq!(read.params, sent.params);
        assert_eq!(read.player, 0, "the file's own player");
    }

    // And the block is a single run of records, as the original writes it.
    let blocks = file
        .blocks
        .iter()
        .filter(|b| b.type_id == stars_formats::MESSAGE_BLOCK)
        .count();
    assert_eq!(blocks, 1, "one block holding every message");
}

/// A fleet with colonists aboard cannot be given away, and the player is told
/// why rather than left wondering.
#[test]
fn a_refused_gift_says_why() {
    let mut state = a_game();
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = stars_formats::task::TRANSFER;
    fleet.waypoints[0].target = Some(0); // player 1, counted among the others
    fleet.cargo.colonists = 100;

    let (done, _) = stars_core::orders::execute_arrival_tasks(&mut state);
    assert!(done.is_empty(), "the gift was refused");
    let told = state
        .messages
        .iter()
        .find(|m| m.id == id::GIFT_HAS_COLONISTS)
        .expect("and said so");
    assert_eq!(told.player, 0);
    assert!(told.summary().contains("colonists"), "{}", told.summary());
}

/// Every message the engine sends carries as many parameters as the
/// original's table says its id does (`PARAMETER_COUNT`): a record with
/// fewer would misalign every message after it in the block, and one with
/// more would be read short. Checked over a decade of a real game, which
/// exercises most of the engine.
#[test]
fn every_message_sent_matches_its_parameter_count() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let host = root.join("games/all-computer-players/2450/Game.hst");
    let universe = root.join("games/all-computer-players/2450/Game.xy");
    if !host.is_file() || !universe.is_file() {
        eprintln!("skipping: fixture absent");
        return;
    }
    let file = StarsFile::decode(&std::fs::read(&host).expect("read")).expect("decode");
    let (mut state, _) = GameState::from_file(&file);
    let xy = stars_formats::Universe::decode(&std::fs::read(&universe).expect("read"))
        .expect("universe");
    state.apply_universe(&xy);
    let mut rng = Rng::from_seeds(9, 10);
    let mut seen = std::collections::BTreeSet::new();
    let mut wrong: Vec<(u16, usize, usize)> = Vec::new();
    for _ in 0..10 {
        generate_turn(&mut state, &mut rng);
        for message in &state.messages {
            let want = stars_formats::message::parameter_count(message.id);
            seen.insert(message.id);
            if message.params.len() != want && !wrong.iter().any(|w| w.0 == message.id) {
                wrong.push((message.id, message.params.len(), want));
            }
        }
    }
    assert!(seen.len() > 10, "the decade said little: {seen:?}");
    assert!(wrong.is_empty(), "(id, sent, table): {wrong:x?}");
}
