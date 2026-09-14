//! The Robotoid on the tutorial's world, in the Berserkers' seat. See
//! `docs/formulas/ai.md`, *The Robotoid's turn*.

use stars_core::ai::personality::{Profile, Shape};
use stars_core::ai::{AiPersonality, Control};

fn tutorial_world(skill_bits: u8) -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let mut state = stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state;
    state.players[1].control = Control::Computer {
        personality: Some(AiPersonality::Robotoid),
        skill_bits,
    };
    state
}

fn ships_in_slot(state: &stars_core::GameState, player: i16, slot: u8) -> i32 {
    state
        .fleets
        .iter()
        .filter(|f| f.owner == player)
        .flat_map(|f| f.stacks.iter())
        .filter(|s| s.design == slot)
        .map(|s| s.count)
        .sum()
}

/// The Robotoid's turn is its own, and in its first years it scraps the
/// scouts it began with, sends its colony ships out, and queues colony
/// ships two at a time.
#[test]
fn the_robotoid_opens_by_scrapping_scouts_and_colonising() {
    assert_eq!(Profile::of(AiPersonality::Robotoid).shape, Shape::Robotoid);
    let mut state = tutorial_world(1);
    let scouts_at_start = ships_in_slot(&state, 1, 0);
    assert!(scouts_at_start > 0, "the Berserkers start with scouts");
    let mut rng = stars_core::rng::Rng::randomize(3);
    let mut scrapped = 0;
    let mut colonising = 0;
    let mut queued_colony_ships = 0;
    let mut wandered = 0;
    for _ in 0..12 {
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        for (player, did) in &report.ai {
            if *player != 1 {
                continue;
            }
            scrapped += did.scrapped.len();
            colonising += did.colonising.len();
            wandered += did.scouted.len();
            queued_colony_ships += did
                .queued
                .iter()
                .filter(|(_, design, _)| *design == 1)
                .map(|(_, _, n)| *n)
                .sum::<i32>();
        }
    }
    assert!(scrapped > 0, "the starting scouts are scrapped");
    assert_eq!(ships_in_slot(&state, 1, 0), 0, "and none are left by 2412");
    assert!(colonising > 0, "colony ships are sent out");
    assert!(
        queued_colony_ships >= 2,
        "colony ships are queued two at a time before turn 21 (got {queued_colony_ships})"
    );
    assert_eq!(wandered, 0, "nothing scouts: the Robotoid does not");
    assert_eq!(
        state.players[1].research_pct, 15,
        "fifteen percent to research from turn 10"
    );
}

/// The turn is deterministic: the same seed gives the same world.
#[test]
fn the_robotoid_is_deterministic() {
    let mut a = tutorial_world(2);
    let mut b = tutorial_world(2);
    let mut rng_a = stars_core::rng::Rng::randomize(7);
    let mut rng_b = stars_core::rng::Rng::randomize(7);
    for _ in 0..8 {
        stars_core::turn::generate_turn(&mut a, &mut rng_a);
        stars_core::turn::generate_turn(&mut b, &mut rng_b);
    }
    assert_eq!(a.fleets, b.fleets);
    assert_eq!(a.planets, b.planets);
    assert_eq!(a.designs, b.designs);
}

/// Over a longer run the Robotoid keeps its planets building and, once
/// the tech is there, draws designs into its slots.
#[test]
fn the_robotoid_keeps_building() {
    let mut state = tutorial_world(3);
    let mut rng = stars_core::rng::Rng::randomize(11);
    let mut designed = Vec::new();
    let mut filled = 0;
    for _ in 0..40 {
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        for (player, did) in &report.ai {
            if *player == 1 {
                designed.extend(did.designed.iter().copied());
                filled += did.filled.len();
            }
        }
    }
    assert!(filled > 0, "its queues are filled");
    assert!(
        state.players[1].research.levels.iter().any(|l| *l > 1),
        "it researches: {:?}",
        state.players[1].research.levels
    );
    // Every design drawn sits in a slot the routine owns, on the hull it
    // names for that slot.
    for (slot, hull) in &designed {
        let ok = match slot {
            0 => *hull == 5,
            2..=5 => *hull == 31,
            6 | 7 => *hull == 9,
            9 | 10 => *hull == 9 || *hull == 19,
            11..=13 => *hull == 11 || *hull == 31,
            14 | 15 => *hull == 29 || *hull == 6,
            _ => false,
        };
        assert!(ok, "slot {slot} got hull {hull}");
    }
    assert!(!state.players[1].dead);
}
