//! The TurinDrone's turn on the tutorial's world — the Berserkers, whom
//! `tutorial.hst` marks `0x27`. See `docs/formulas/ai.md`, *The TurinDrone
//! turn*. Only what is written so far is asked about.

use stars_core::ai::turindrone;

fn tutorial_world() -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state
}

/// In year 0 the Berserkers send their scouts to planets they have not
/// seen, and queue a scout — one per thirty planets of a twenty-four
/// planet universe — at home.
#[test]
fn year_zero_sends_the_scouts_out() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(1);
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(
        !report.scouted.is_empty(),
        "the scouts have somewhere to go"
    );
    for (fleet, planet) in &report.scouted {
        let fleet = state
            .fleets
            .iter()
            .find(|f| f.owner == 1 && f.id == *fleet)
            .expect("a Berserker fleet");
        assert_eq!(fleet.waypoints.len(), 2);
        assert_eq!(fleet.waypoints[1].target, Some(*planet as u16));
        assert!(fleet.waypoints[1].warp > 0, "at a warp of its own");
        // Not one of the planets in their scanner's view.
        assert!(!state.players[1].explored.contains(planet));
    }
    assert_eq!(
        report
            .queued
            .iter()
            .filter(|(_, d, _)| *d == turindrone::SCOUT_SLOT)
            .map(|(_, _, n)| n)
            .sum::<i32>(),
        1,
        "one scout for twenty-four planets"
    );
}

/// The turn runs inside the year: after a few years the Berserkers' scouts
/// are out among the planets and their knowledge has grown.
#[test]
fn the_berserkers_explore_as_the_years_go_by() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(1);
    let home = state.players[1].explored.len();
    for _ in 0..6 {
        stars_core::generate_turn(&mut state, &mut rng);
    }
    let berserker_fleets: Vec<_> = state.fleets.iter().filter(|f| f.owner == 1).collect();
    assert!(
        berserker_fleets.iter().any(|f| f.orbiting != Some(10)),
        "somebody has left home: {:?}",
        berserker_fleets
            .iter()
            .map(|f| (f.id, f.position, f.orbiting))
            .collect::<Vec<_>>()
    );
    assert!(
        state.players[1].explored.len() > home.max(1),
        "and seen more of the galaxy"
    );
}

/// The new game gives the Berserkers what `tutorial.hst` gives them: three
/// designs — a Smaugarian Peeping Tom, a Santa Maria, a Potato Bug — and
/// four fleets, one scout, one colony ship and two miners.
#[test]
fn the_berserkers_start_as_the_host_file_has_them() {
    let state = tutorial_world();
    let designs = &state.designs[1];
    let names: Vec<&str> = designs.iter().take(3).map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        ["Smaugarian Peeping Tom", "Santa Maria", "Potato Bug"]
    );
    assert!(designs
        .get(3)
        .is_none_or(|d| d.hull().is_none() || d.is_starbase()));
    let mut fleets: Vec<(u16, u8)> = state
        .fleets
        .iter()
        .filter(|f| f.owner == 1)
        .map(|f| (f.id, f.stacks[0].design))
        .collect();
    fleets.sort_unstable();
    assert_eq!(fleets, [(0, 0), (1, 1), (2, 2), (3, 2)]);
    assert_eq!(state.players[1].research.levels, [0, 0, 0, 0, 5, 0]);
}

/// `FGetAIPart` and `FCreateAiShdef` on the Berserkers: at Electronics 5
/// alone nothing can be fitted, because the engine class wants the
/// Trans-Star 10, a scoop or the Fuel Mizer; at Propulsion 2 their
/// Improved Fuel Efficiency admits the Fuel Mizer and the colony ship and
/// the Frigate scout come out fitted to every slot's capacity.
#[test]
fn the_fittings_wait_for_an_engine() {
    use stars_core::ai::parts::{create_design, fitting, pick_part};
    use stars_core::components::slot;
    let state = tutorial_world();
    let mut player = state.players[1].clone();
    let who = stars_core::parts::Builder::player(&player);
    assert_eq!(pick_part(8, &who), None, "no engine the class allows");
    assert!(create_design(15, fitting::COLONY_SHIP, &who).is_none());

    player.research.levels[2] = 2;
    let who = stars_core::parts::Builder::player(&player);
    assert_eq!(
        pick_part(8, &who),
        Some((slot::ENGINE, 2)),
        "the Fuel Mizer"
    );
    let colony = create_design(15, fitting::COLONY_SHIP, &who).expect("a colony ship");
    assert_eq!(colony.hull_id, 15);
    assert_eq!(
        colony
            .slots
            .iter()
            .map(|s| (s.category, s.item, s.count))
            .collect::<Vec<_>>(),
        [(slot::ENGINE, 2, 1), (slot::SPECIAL_M, 0, 1)],
        "an engine and the Colonization Module, the Orbital Construction Module being AR's"
    );
    // The Frigate hull itself wants Construction 6.
    assert!(create_design(5, fitting::SCOUT, &who).is_none());
    player.research.levels[3] = 6;
    let who = stars_core::parts::Builder::player(&player);
    let scout = create_design(5, fitting::SCOUT, &who).expect("a Frigate scout");
    assert_eq!(scout.slots.len(), 4);
    assert_eq!(
        scout.slots[0],
        stars_core::design::DesignSlot {
            category: slot::ENGINE,
            item: 2,
            count: 1
        }
    );
    assert_eq!(
        scout.slots[2].category,
        slot::TORPEDO,
        "an Alpha Torpedo in the weapon slot"
    );
}

/// `EnsureTurinDroneShdefs` on the Berserkers over the years: once
/// Propulsion 2 admits the Fuel Mizer and Construction 6 the Frigate, the
/// scout and colony slots are refilled with designs of the personality's
/// own, named from the game's lists, and the starting Santa Maria is
/// retired when its last ship is gone.
#[test]
fn designs_come_as_the_tech_arrives() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(3);
    // Nothing yet: no engine the fittings allow.
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.designed.is_empty(), "{:?}", report.designed);
    assert!(
        !state.designs[1][1].obsolete,
        "the Santa Maria stays while one exists"
    );

    // The colony ship gone (settled), Propulsion 2: a new colony design.
    state.fleets.retain(|f| !(f.owner == 1 && f.id == 1));
    state.players[1].research.levels[2] = 2;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.designed, vec![(1, 15)]);
    let colony = &state.designs[1][1];
    assert!(!colony.obsolete);
    assert!(stars_core::ai::parts::names::EGG
        .iter()
        .any(|n| colony.name.starts_with(n)));

    // And the scout slot, once Construction reaches 6 and the Peeping Tom
    // is no more: retired, and a Frigate in its place. (The colony design
    // is made afresh each year until a ship of it exists — the routine
    // asks for `cExist == 0`, and a new design has none.)
    state.fleets.retain(|f| !(f.owner == 1 && f.id == 0));
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.designed, vec![(1, 15)]);
    assert!(
        state.designs[1][0].obsolete,
        "retired before the Frigate can be made"
    );
    state.players[1].research.levels[3] = 6;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.designed, vec![(1, 15), (0, 5)]);
    assert_eq!(state.designs[1][0].hull_id, 5);
}
