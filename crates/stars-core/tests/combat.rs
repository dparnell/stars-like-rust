//! `DoBattles` on the tutorial's world: see `docs/formulas/combat.md`,
//! *The battle around the board*.

use stars_core::combat;

fn tutorial_world() -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state
}

/// The index of a player's fleet carrying an armed design.
fn armed_fleet(state: &stars_core::GameState, player: i16) -> usize {
    state
        .fleets
        .iter()
        .position(|f| {
            f.owner == player
                && f.stacks.iter().any(|s| {
                    state.designs[usize::try_from(player).unwrap()]
                        .get(usize::from(s.design))
                        .is_some_and(stars_core::design::ShipDesign::is_armed)
                })
        })
        .expect("an armed fleet")
}

/// Two fleets sharing a point in space, one of them armed and set to
/// attack everyone, fight: the recording has both, shots are fired, and
/// the loser's ships are gone from its fleet.
#[test]
fn an_armed_fleet_attacks_what_it_meets() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(9);
    let ours = armed_fleet(&state, 0);
    let theirs = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 0)
        .expect("a Berserker scout");
    let at = stars_core::movement::Point::new(1300, 1300);
    for index in [ours, theirs] {
        let fleet = &mut state.fleets[index];
        fleet.position = at;
        fleet.orbiting = None;
        fleet.waypoints.truncate(1);
        fleet.waypoints[0].position = at;
        fleet.waypoints[0].target = None;
    }
    let our_id = state.fleets[ours].id;
    let their_id = state.fleets[theirs].id;
    let our_ships = state.fleets[ours].ships();
    // Our plan attacks everyone.
    for plan in &mut state.players[0].battle_plans {
        plan.attack_who = combat::attack_who::EVERYONE;
    }
    // Without a plan that attacks, nothing happens.
    let quiet = {
        let mut state = state.clone();
        for plan in &mut state.players[0].battle_plans {
            plan.attack_who = combat::attack_who::NOBODY;
        }
        combat::do_battles(&mut state, &mut rng)
    };
    assert!(quiet.is_empty(), "nobody attacks, nobody fights");

    let outcomes = combat::do_battles(&mut state, &mut rng);
    assert_eq!(outcomes.len(), 1, "{outcomes:?}");
    let battle = &outcomes[0];
    assert_eq!(battle.record.players, 2);
    assert_eq!(battle.record.player_mask & 0b11, 0b11);
    assert_eq!(battle.record.position, (1300, 1300));
    assert_eq!(battle.record.tokens.len(), 2, "{:?}", battle.record.tokens);
    assert!(
        battle.record.actions.iter().any(|a| !a.kills.is_empty()),
        "shots were fired: {:?}",
        battle.record.actions
    );
    // A recording for the players, and a message each.
    assert_eq!(state.battles.len(), 1);
    assert_eq!(
        state
            .messages
            .iter()
            .filter(|m| m.id == stars_core::message::id::BATTLE)
            .count(),
        2
    );
    // The unarmed scout dies or runs; either way ours are all there.
    let ours_after = state
        .fleets
        .iter()
        .find(|f| f.owner == 0 && f.id == our_id)
        .expect("our fleet survives");
    assert_eq!(ours_after.ships(), our_ships);
    let theirs_after = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == their_id);
    let lost: i32 = battle
        .losses
        .iter()
        .filter(|(p, _, _)| *p == 1)
        .map(|(_, _, n)| n)
        .sum();
    assert!(
        lost == 1 || theirs_after.is_some(),
        "the scout was destroyed or got away: {:?}",
        battle.losses
    );
    if lost == 1 {
        assert!(theirs_after.is_none(), "a fleet with no ships is gone");
    }
}

/// The same fight through the turn: the battle report reaches the turn
/// report and the recording the state.
#[test]
fn battles_happen_in_the_turn() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(9);
    let ours = armed_fleet(&state, 0);
    let theirs = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 0)
        .expect("a Berserker scout");
    let at = stars_core::movement::Point::new(1300, 1300);
    for index in [ours, theirs] {
        let fleet = &mut state.fleets[index];
        fleet.position = at;
        fleet.orbiting = None;
        fleet.waypoints.truncate(1);
        fleet.waypoints[0].position = at;
        fleet.waypoints[0].target = None;
    }
    for plan in &mut state.players[0].battle_plans {
        plan.attack_who = combat::attack_who::EVERYONE;
    }
    // The Berserkers' scout would otherwise be sent away by their turn.
    state.players[1].control = stars_core::ai::Control::Human;
    let report = stars_core::generate_turn(&mut state, &mut rng);
    assert_eq!(report.battles.len(), 1, "{:?}", report.battles);
    assert_eq!(state.battles.len(), 1);
}

/// The recording is written into each participant's turn file, where the
/// VCR reads it from.
#[test]
fn the_recording_goes_into_the_turn_file() {
    use stars_formats::battle::battle_records;
    use stars_formats::StarsFile;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(9);
    let ours = armed_fleet(&state, 0);
    let theirs = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 0)
        .expect("a Berserker scout");
    let at = stars_core::movement::Point::new(1300, 1300);
    for index in [ours, theirs] {
        let fleet = &mut state.fleets[index];
        fleet.position = at;
        fleet.orbiting = None;
        fleet.waypoints.truncate(1);
        fleet.waypoints[0].position = at;
        fleet.waypoints[0].target = None;
    }
    for plan in &mut state.players[0].battle_plans {
        plan.attack_who = combat::attack_who::EVERYONE;
    }
    let outcomes = combat::do_battles(&mut state, &mut rng);
    assert_eq!(outcomes.len(), 1);
    for player in 0..2 {
        let bytes = stars_core::save::player_file(&state, player).expect("writes");
        let file = StarsFile::decode(&bytes).expect("decodes");
        let records = battle_records(&file);
        assert_eq!(records.len(), 1, "player {player}");
        assert_eq!(records[0].tokens, outcomes[0].record.tokens);
        assert_eq!(records[0].actions, outcomes[0].record.actions);
    }
}

/// `DoBombing`: a bomber in orbit of an enemy planet with no starbase
/// kills colonists and knocks down installations; over a starbase it
/// does nothing.
#[test]
fn bombers_bomb_an_undefended_planet() {
    use stars_core::components::slot;
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_core::fleet::ShipStack;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(3);
    // A bomber design for the human: two Lady Finger Bombs.
    state.designs[0].push(ShipDesign {
        hull_id: 18,
        slots: vec![
            DesignSlot {
                category: slot::ENGINE,
                item: 1,
                count: 1,
            },
            DesignSlot {
                category: slot::BOMB,
                item: 0,
                count: 2,
            },
        ],
        name: "Firecracker".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
    });
    let bomber_slot = u8::try_from(state.designs[0].len() - 1).expect("a slot");
    let target = state
        .planets
        .iter()
        .position(|p| p.owner == Some(1))
        .expect("the Berserkers' home");
    let at = state.planets[target].position.expect("placed");
    let target_id = state.planets[target].id;
    let pop_before = state.planets[target].pop;
    let mut bomber = state.fleets[0].clone();
    bomber.id = 20;
    bomber.owner = 0;
    bomber.position = at;
    bomber.orbiting = Some(target_id as u16);
    bomber.waypoints.truncate(1);
    bomber.waypoints[0].position = at;
    bomber.waypoints[0].target = Some(target_id as u16);
    bomber.stacks = vec![ShipStack {
        design: bomber_slot,
        count: 4,
        damaged_pct: 0,
        damage_pct: 0,
    }];
    state.fleets.push(bomber);
    for plan in &mut state.players[0].battle_plans {
        plan.attack_who = combat::attack_who::EVERYONE;
    }

    // The starbase stands: nothing.
    assert!(state.planets[target].starbase);
    let none = stars_core::bombing::do_bombing(&mut state, &mut rng);
    assert!(none.is_empty(), "{none:?}");

    state.planets[target].starbase = false;
    let done = stars_core::bombing::do_bombing(&mut state, &mut rng);
    assert_eq!(done.len(), 1, "{done:?}");
    assert_eq!(done[0].planet, target_id);
    assert_eq!(done[0].fleets, vec![20]);
    assert!(done[0].result.colonists > 0, "{:?}", done[0].result);
    assert!(state.planets[target].pop < pop_before);
    assert!(!done[0].depopulated);
    assert!(state
        .messages
        .iter()
        .any(|m| m.player == 0 && m.id == stars_core::message::id::BOMBED));
    assert!(state
        .messages
        .iter()
        .any(|m| m.player == 1 && m.id == stars_core::message::id::BOMBED_YOU));

    // A second call in the same year does nothing more: the fleet has
    // bombed.
    let pop_after = state.planets[target].pop;
    let again = stars_core::bombing::do_bombing(&mut state, &mut rng);
    assert_eq!(again.len(), 1, "a new call is a new year: it bombs again");
    assert!(state.planets[target].pop < pop_after);
}
