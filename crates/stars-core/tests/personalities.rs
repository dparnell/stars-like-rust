//! The seven computer opponents on the tutorial's world — each in the
//! Berserkers' seat in turn. See `docs/formulas/ai.md`, *The seven
//! personalities*: every one researches by its own plan and share, the
//! Maid's turn is its own, and the other five borrow the TurinDrone's
//! middle until theirs are transcribed.

use stars_core::ai::personality::{Profile, Shape};
use stars_core::ai::{AiPersonality, Control};

const ALL: [AiPersonality; 7] = [
    AiPersonality::Robotoid,
    AiPersonality::TurinDrone,
    AiPersonality::Automitron,
    AiPersonality::Rototill,
    AiPersonality::Cyber,
    AiPersonality::Macinti,
    AiPersonality::Maid,
];

fn tutorial_world(personality: AiPersonality) -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    let mut state = stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state;
    state.players[1].control = Control::Computer {
        personality: Some(personality),
        skill_bits: 1,
    };
    state
}

/// Every personality plays: the year's report carries its turn, and its
/// research is set from its own plan and share.
#[test]
fn every_personality_takes_its_turn() {
    for personality in ALL {
        let mut state = tutorial_world(personality);
        let mut rng = stars_core::rng::Rng::randomize(3);
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        assert!(
            report.ai.iter().any(|(player, _)| *player == 1),
            "{} took a turn",
            personality.name()
        );
        let profile = Profile::of(personality);
        let player = &state.players[1];
        assert_eq!(
            player.research_pct,
            profile.research_pct(0),
            "{}'s share in its first year",
            personality.name()
        );
        // The first entry of the plan not yet reached is the field under
        // study; with no plan, whichever field is lowest.
        let want = profile
            .plan
            .iter()
            .map(|e| (usize::from(e >> 5), e & 0x1f))
            .find(|(field, level)| player.research.levels[*field] < *level)
            .map(|(field, _)| field);
        if let Some(field) = want {
            assert_eq!(
                player.research.current_field,
                field,
                "{} studies its plan's next field",
                personality.name()
            );
        }
    }
}

/// The Maid designs nothing and sends no fleets; its planets still build.
#[test]
fn the_maid_keeps_house_and_no_more() {
    let mut state = tutorial_world(AiPersonality::Maid);
    let mut rng = stars_core::rng::Rng::randomize(3);
    let mut designed = 0;
    let mut scouted = 0;
    let mut filled = 0;
    for _ in 0..6 {
        let report = stars_core::turn::generate_turn(&mut state, &mut rng);
        for (player, did) in &report.ai {
            if *player != 1 {
                continue;
            }
            designed += did.designed.len();
            scouted += did.scouted.len() + did.colonising.len() + did.attacking.len();
            filled += did.filled.len();
        }
    }
    assert_eq!(designed, 0, "the Maid draws no designs");
    assert_eq!(scouted, 0, "and sends nobody anywhere");
    assert!(filled > 0, "but its queues are filled");
    assert_eq!(Profile::of(AiPersonality::Maid).shape, Shape::Basic);
}

/// A stand-in personality plays the TurinDrone's turn under its own plan:
/// the Cyber's first study is Construction 4, where the TurinDrone's is
/// Propulsion 2.
#[test]
fn a_stand_in_researches_its_own_way() {
    let mut cyber = tutorial_world(AiPersonality::Cyber);
    let mut drone = tutorial_world(AiPersonality::TurinDrone);
    let mut rng = stars_core::rng::Rng::randomize(3);
    stars_core::turn::generate_turn(&mut cyber, &mut rng);
    let mut rng = stars_core::rng::Rng::randomize(3);
    stars_core::turn::generate_turn(&mut drone, &mut rng);
    assert_eq!(cyber.players[1].research.current_field, 3, "Construction");
    assert_eq!(drone.players[1].research.current_field, 2, "Propulsion");
    assert_eq!(cyber.players[1].research_pct, 17);
    assert_eq!(drone.players[1].research_pct, 15);
}
