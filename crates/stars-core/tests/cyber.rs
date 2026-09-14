//! The Cybertron on the tutorial's world, in the Berserkers' seat. See
//! `docs/formulas/ai.md`, *The Cybertron's turn*.

use stars_core::ai::personality::{Profile, Shape};
use stars_core::ai::{AiPersonality, Control};

fn tutorial_world(skill_bits: u8) -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let mut state = stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state;
    state.players[1].control = Control::Computer {
        personality: Some(AiPersonality::Cyber),
        skill_bits,
    };
    state
}

/// The Cybertron's turn is its own: the starting scouts are broken up in
/// the first year, the designs it draws land in the slots the routine
/// owns on the hulls it names, and its warships go hunting.
#[test]
fn the_cyber_draws_its_groups_and_hunts() {
    assert_eq!(Profile::of(AiPersonality::Cyber).shape, Shape::Cyber);
    let mut state = tutorial_world(3);
    let mut rng = stars_core::rng::Rng::randomize(11);
    let mut scrapped_first_year = 0;
    let mut designed = Vec::new();
    let mut attacking = 0;
    let mut queued_warships = 0;
    for year in 0..80 {
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        for (player, did) in &report.ai {
            if *player != 1 {
                continue;
            }
            if year == 0 {
                scrapped_first_year += did.scrapped.len();
            }
            designed.extend(did.designed.iter().copied());
            attacking += did.attacking.len();
            queued_warships += did
                .queued
                .iter()
                .filter(|(_, d, _)| (4..14).contains(d))
                .count();
        }
    }
    assert!(scrapped_first_year > 0, "the starting scouts are scrapped");
    for (slot, hull) in &designed {
        let ok = match slot {
            0 => *hull == 5,
            2 | 3 => *hull == 11,
            4 | 5 => *hull == 6,
            6 | 7 | 10 | 11 => matches!(hull, 7 | 9 | 29),
            8 | 12 => matches!(hull, 9 | 29 | 7),
            9 | 13 => matches!(hull, 29 | 9 | 19),
            14 | 15 => matches!(hull, 9 | 7 | 6),
            _ => false,
        };
        assert!(ok, "slot {slot} got hull {hull}");
    }
    assert!(
        designed.iter().any(|(s, _)| *s == 4),
        "a Destroyer is drawn into slot 4: {designed:?}"
    );
    assert!(
        designed.iter().any(|(s, _)| *s == 6),
        "the first warship group is drawn: {designed:?}"
    );
    assert!(queued_warships > 0, "warships are queued");
    assert!(attacking > 0, "and sent after the enemy");
    assert_eq!(state.players[1].research_pct, 17, "seventeen percent");
    assert!(!state.players[1].dead);
    assert_eq!(
        state.players[1].cyber_words.len(),
        state.planets.len(),
        "the lasting words cover the galaxy"
    );
}

/// The turn is deterministic: the same seed gives the same world.
#[test]
fn the_cyber_is_deterministic() {
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
    assert_eq!(a.players[1].cyber_words, b.players[1].cyber_words);
}

/// The Cybertron's armada reads the shared potency table, which only the
/// other personalities set: with the Cybertron alone it stays at zero.
#[test]
fn the_shared_potencies_are_what_the_others_left() {
    let mut state = tutorial_world(1);
    let mut rng = stars_core::rng::Rng::randomize(3);
    stars_core::turn::generate_turn(&mut state, &mut rng);
    assert_eq!(state.ai_armada_potency, [0; 4]);
    let mut state = tutorial_world(1);
    state.players[1].control = Control::Computer {
        personality: Some(AiPersonality::Robotoid),
        skill_bits: 1,
    };
    stars_core::turn::generate_turn(&mut state, &mut rng);
    assert_eq!(
        state.ai_armada_potency,
        stars_core::ai::robotoid::potency(1)
    );
}
