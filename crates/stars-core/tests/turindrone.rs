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
    // The two Potato Bugs are merged (`MergeAllShdefs` over the miners'
    // slots) and then scrapped at year 0, as the routine has it.
    assert_eq!(report.merged, vec![(3, 2)]);
    assert_eq!(report.scrapped, vec![2]);
    let miners = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == 2)
        .expect("the miners");
    assert_eq!(miners.stacks[0].count, 2);
    assert_eq!(miners.waypoints[0].task, stars_formats::task::SCRAP);
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

/// `CheckAiShdefStatus`: a design older than the recycling period with no
/// ship of it left is retired; one with ships is kept (for
/// `SplitOutShdefs`, when that is written).
#[test]
fn old_designs_are_recycled() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(4);
    // Make the Berserkers' miner design ancient and remove its ships, and
    // give the scout the same age but keep its ship.
    state.turn = 60;
    state.designs[1][2].designed = 0;
    state.designs[1][0].designed = 0;
    state
        .fleets
        .retain(|f| !(f.owner == 1 && f.stacks[0].design == 2));
    // Construction 7 is what makes the miner range looked at.
    state.players[1].research.levels[3] = 7;
    turindrone::turn(&mut state, 1, &mut rng);
    assert!(
        state.designs[1][2].obsolete,
        "the Potato Bug, sixty years old and gone"
    );
    assert!(!state.designs[1][0].obsolete, "the Peeping Tom still flies");
}

/// `IdTargetFreighter`: a hauler at home goes to collect what a miner has
/// dug — an unowned planet a miner of ours claims, scored by its mineral
/// worth over the distance — with Load All on the three minerals.
#[test]
fn a_hauler_goes_to_the_miners_planet() {
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_core::fleet::ShipStack;
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(5);
    state.turn = 3;
    // A Small Freighter design in the haulers' slot 8.
    while state.designs[1].len() < 9 {
        let last = state.designs[1].last().expect("a design").clone();
        state.designs[1].push(last);
    }
    state.designs[1][8] = ShipDesign {
        hull_id: 0,
        slots: vec![
            DesignSlot {
                category: stars_core::components::slot::ENGINE,
                item: 1,
                count: 1,
            },
            DesignSlot {
                category: stars_core::components::slot::SCANNER,
                item: 0,
                count: 1,
            },
            DesignSlot {
                category: stars_core::components::slot::SPECIAL_M,
                item: 2,
                count: 1,
            },
        ],
        name: "Boxcar".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 1,
        built: 1,
    };
    // The miner (fleet 2) sits at an unowned planet the Berserkers know.
    let home = state
        .planets
        .iter()
        .find(|p| p.owner == Some(1))
        .expect("home")
        .clone();
    let mined = state
        .planets
        .iter()
        .find(|p| p.owner.is_none() && p.position.is_some())
        .expect("an unowned planet")
        .clone();
    state.players[1].explored.insert(mined.id);
    let miner = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 2)
        .expect("a miner");
    state.fleets[miner].position = mined.position.expect("placed");
    state.fleets[miner].orbiting = Some(mined.id as u16);
    state.fleets[miner].waypoints[0].position = mined.position.expect("placed");
    state.fleets[miner].waypoints[0].target = Some(mined.id as u16);
    // And a hauler at home.
    let mut hauler = state.fleets[0].clone();
    hauler.id = 9;
    hauler.owner = 1;
    hauler.position = home.position.expect("placed");
    hauler.orbiting = Some(home.id as u16);
    hauler.waypoints.truncate(1);
    hauler.waypoints[0].position = hauler.position;
    hauler.waypoints[0].target = Some(home.id as u16);
    hauler.stacks = vec![ShipStack {
        design: 8,
        count: 1,
        damaged_pct: 0,
        damage_pct: 0,
    }];
    hauler.cargo = stars_core::fleet::Cargo::default();
    state.fleets.push(hauler);

    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.hauling, vec![(9, mined.id)]);
    let hauler = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == 9)
        .expect("the hauler");
    let leg = &hauler.waypoints[1];
    assert_eq!(leg.task, stars_formats::task::TRANSPORT);
    let orders = leg.transport.expect("orders");
    for kind in 0..3 {
        assert_eq!(
            orders.items[kind].action,
            stars_formats::XferAction::LoadAll
        );
    }
    assert_eq!(orders.items[3].action, stars_formats::XferAction::None);
}

/// `MergeAllShdefs` and the armada: two bomber fleets at home join into
/// one; the armada waits while it is short of the potency, and once it has
/// the bombers and the battleships it goes for the best of the other
/// players' planets — the human home world, the only one the Berserkers
/// know of.
#[test]
fn bombers_gather_and_then_go() {
    use stars_core::fleet::ShipStack;
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(6);
    state.turn = 5;
    // Bomber and battleship designs in their slots (any hull will do for
    // the fleet pass, which goes by the slot).
    while state.designs[1].len() < 14 {
        let last = state.designs[1].last().expect("a design").clone();
        state.designs[1].push(last);
    }
    let home = state
        .planets
        .iter()
        .find(|p| p.owner == Some(1))
        .expect("home")
        .clone();
    let mut make = |id: u16, slot: u8, count: i32| {
        let mut fleet = state.fleets[0].clone();
        fleet.id = id;
        fleet.owner = 1;
        fleet.position = home.position.expect("placed");
        fleet.orbiting = Some(home.id as u16);
        fleet.waypoints.truncate(1);
        fleet.waypoints[0].position = fleet.position;
        fleet.waypoints[0].target = Some(home.id as u16);
        fleet.stacks = vec![ShipStack {
            design: slot,
            count,
            damaged_pct: 0,
            damage_pct: 0,
        }];
        state.fleets.push(fleet);
    };
    make(20, 13, 3);
    make(21, 13, 2);
    make(22, 4, 1);
    // The human home world is known to them.
    let target = state
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("the human home")
        .id;
    state.players[1].explored.insert(target);

    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.merged.contains(&(21, 20)), "{:?}", report.merged);
    assert!(report.merged.contains(&(22, 20)), "{:?}", report.merged);
    let armada = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.stacks.iter().any(|s| s.design == 13))
        .expect("the armada");
    assert_eq!(
        armada
            .stacks
            .iter()
            .filter(|s| s.design == 13)
            .map(|s| s.count)
            .sum::<i32>(),
        5
    );
    // Six bombers and three battleships are wanted at year 5: it waits.
    assert!(report.attacking.is_empty());
    assert_eq!(armada.waypoints.len(), 1);

    let armada = armada.id;
    let index = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == armada)
        .expect("it");
    state.fleets[index].stacks = vec![
        ShipStack {
            design: 13,
            count: 6,
            damaged_pct: 0,
            damage_pct: 0,
        },
        ShipStack {
            design: 4,
            count: 3,
            damaged_pct: 0,
            damage_pct: 0,
        },
    ];
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.attacking, vec![(armada, target)]);
}
