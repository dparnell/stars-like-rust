//! The Automitron on the tutorial's world, in the Berserkers' seat. See
//! `docs/formulas/ai.md`, *The Automitron's turn*.

use stars_core::ai::personality::{Profile, Shape};
use stars_core::ai::{AiPersonality, Control};

fn tutorial_world(skill_bits: u8) -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let mut state = stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state;
    state.players[1].control = Control::Computer {
        personality: Some(AiPersonality::Automitron),
        skill_bits,
    };
    state
}

/// The Automitron's turn is its own: its starting colony ship — not a
/// Medium Freighter — is broken up in the first year, its scouts go out
/// to the unknown planets, and once the slot is empty it draws its own
/// Medium Freighter colony design and sends it to settle.
#[test]
fn the_automitron_scouts_and_settles_by_freighter() {
    assert_eq!(
        Profile::of(AiPersonality::Automitron).shape,
        Shape::Automitron
    );
    let mut state = tutorial_world(1);
    let mut rng = stars_core::rng::Rng::randomize(3);
    let mut scouted = 0;
    let mut scrapped_first_year = 0;
    let mut colonising = 0;
    let mut designed = Vec::new();
    for year in 0..40 {
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        for (player, did) in &report.ai {
            if *player != 1 {
                continue;
            }
            scouted += did.scouted.len();
            if year == 0 {
                scrapped_first_year += did.scrapped.len();
            }
            colonising += did.colonising.len();
            designed.extend(did.designed.iter().copied());
        }
    }
    assert!(scouted > 0, "the scouts are sent out");
    assert!(
        scrapped_first_year > 0,
        "the starting colony ship, no Medium Freighter, is scrapped"
    );
    assert!(
        designed.contains(&(1, 1)),
        "a Medium Freighter colony design is drawn into slot 1: {designed:?}"
    );
    assert!(colonising > 0, "and sent to settle");
    for (slot, hull) in &designed {
        let ok = match slot {
            0 => *hull == 4,
            1 => *hull == 1,
            2 => *hull == 17,
            3 => *hull == 19,
            4 => *hull == 1,
            5 => *hull == 3,
            6 => *hull == 11,
            9 => *hull == 9,
            14 => *hull == 6,
            _ => false,
        };
        assert!(ok, "slot {slot} got hull {hull}");
    }
    assert_eq!(
        state.players[1].research_pct, 20,
        "twenty percent from turn 10"
    );
    assert!(!state.players[1].dead);
}

/// The turn is deterministic: the same seed gives the same world.
#[test]
fn the_automitron_is_deterministic() {
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
