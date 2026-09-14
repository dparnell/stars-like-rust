//! The Macinti on the tutorial's world, in the Berserkers' seat. See
//! `docs/formulas/ai.md`, *The Macinti's turn*. The Berserkers are no
//! Alternate Reality race, so the Macinti's colony ships — which carry an
//! Orbital Construction Module — cannot be drawn here; what the test
//! checks is the turn's own shape.

use stars_core::ai::personality::{Profile, Shape};
use stars_core::ai::{AiPersonality, Control};

fn tutorial_world(skill_bits: u8) -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let mut state = stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state;
    state.players[1].control = Control::Computer {
        personality: Some(AiPersonality::Macinti),
        skill_bits,
    };
    state
}

/// The Macinti's turn is its own: the starting scouts and miners are
/// broken up in the first year, its research follows its own plan, its
/// starbase recycling table is kept, and what it draws lands in the slots
/// the routine owns.
#[test]
fn the_macinti_takes_its_own_turn() {
    assert_eq!(Profile::of(AiPersonality::Macinti).shape, Shape::Macinti);
    let mut state = tutorial_world(3);
    let mut rng = stars_core::rng::Rng::randomize(11);
    let mut scrapped_first_year = 0;
    let mut designed = Vec::new();
    for year in 0..60 {
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        for (player, did) in &report.ai {
            if *player != 1 {
                continue;
            }
            if year == 0 {
                scrapped_first_year += did.scrapped.len();
            }
            designed.extend(did.designed.iter().copied());
        }
    }
    assert!(
        scrapped_first_year > 0,
        "the starting scouts and slot-2 ships are scrapped"
    );
    for (slot, hull) in &designed {
        let ok = match slot {
            0 => *hull == 5,
            1 | 7 => *hull == 15,
            2..=4 => *hull == 7,
            5 | 6 => matches!(hull, 9 | 29),
            8 | 9 => matches!(hull, 9 | 19),
            10 | 11 => matches!(hull, 1 | 2),
            12 | 13 => matches!(hull, 6 | 29),
            14 | 15 => matches!(hull, 21..=24),
            _ => false,
        };
        assert!(ok, "slot {slot} got hull {hull}");
    }
    assert_eq!(state.players[1].research_pct, 15, "fifteen percent");
    assert!(
        state.players[1]
            .mac_starbase_recycle
            .iter()
            .any(|v| *v != 0),
        "the starbase recycling table is kept"
    );
    assert_eq!(
        state.ai_armada_potency,
        stars_core::ai::macinti::potency(59)
    );
    assert!(!state.players[1].dead);
}

/// The turn is deterministic: the same seed gives the same world.
#[test]
fn the_macinti_is_deterministic() {
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

/// `PctPlanetCapacity` reads the fill of a planet in percent.
#[test]
fn planet_capacity_is_a_percentage() {
    let state = tutorial_world(1);
    let race = state.players[1].race.clone();
    let planet = state
        .planets
        .iter()
        .find(|p| p.owner == Some(1))
        .expect("a homeworld");
    let pct = stars_core::ai::macinti::pct_planet_capacity(planet, &race);
    assert!((1..=999).contains(&pct), "{pct}");
}
