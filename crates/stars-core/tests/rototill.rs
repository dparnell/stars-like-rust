//! The Rototill on the tutorial's world, in the Berserkers' seat. See
//! `docs/formulas/ai.md`, *The Rototill's turn*.

use stars_core::ai::personality::{Profile, Shape};
use stars_core::ai::{AiPersonality, Control};

fn tutorial_world(skill_bits: u8) -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let mut state = stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state;
    state.players[1].control = Control::Computer {
        personality: Some(AiPersonality::Rototill),
        skill_bits,
    };
    state
}

/// The Rototill's turn is its own: it designs nothing, ever; its scouts
/// go out, and its starting colony ships settle and are replaced one at
/// a time while there is somewhere to settle.
#[test]
fn the_rototill_designs_nothing_and_settles() {
    assert_eq!(Profile::of(AiPersonality::Rototill).shape, Shape::Rototill);
    let mut state = tutorial_world(1);
    let mut rng = stars_core::rng::Rng::randomize(11);
    let mut designed = 0;
    let mut scouted = 0;
    let mut colonising = 0;
    let mut colony_queued = 0;
    for _ in 0..40 {
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        for (player, did) in &report.ai {
            if *player != 1 {
                continue;
            }
            designed += did.designed.len();
            scouted += did.scouted.len();
            colonising += did.colonising.len();
            colony_queued += did.queued.iter().filter(|(_, d, _)| *d == 1).count();
        }
    }
    assert_eq!(designed, 0, "the Rototill draws no designs");
    assert!(scouted > 0, "its scouts go out");
    assert!(colonising > 0, "its colony ships settle");
    assert!(colony_queued > 0, "and are replaced");
    assert!(
        state.planets.iter().filter(|p| p.owner == Some(1)).count() > 1,
        "so it holds more than its homeworld"
    );
    assert_eq!(
        state.players[1].research_pct, 15,
        "fifteen percent from turn 20"
    );
    assert!(!state.players[1].dead);
}

/// The turn is deterministic: the same seed gives the same world.
#[test]
fn the_rototill_is_deterministic() {
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
