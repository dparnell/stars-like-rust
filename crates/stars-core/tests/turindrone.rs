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

/// `FillProductionQueue`: the Berserkers' home world queues mines and
/// factories every year, mines at the front and factories at the back —
/// the mines-and-factories fill of `docs/formulas/ai.md`, now run from
/// the personality's turn.
#[test]
fn the_home_world_queues_mines_and_factories() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(7);
    state.turn = 1;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    let home = state
        .planets
        .iter()
        .find(|p| p.owner == Some(1))
        .expect("home");
    assert_eq!(report.filled.len(), 1, "{:?}", report.filled);
    let (planet, mines, factories) = report.filled[0];
    assert_eq!(planet, home.id);
    assert!(
        mines > 0 && factories > 0,
        "mines {mines}, factories {factories}"
    );
    let items: Vec<(u16, bool)> = home.queue.iter().map(|q| (q.item, q.ship)).collect();
    assert_eq!(
        items.first(),
        Some(&(stars_core::production::item::MINE, false))
    );
    assert_eq!(
        items.last(),
        Some(&(stars_core::production::item::FACTORY, false))
    );
}

/// `IroEnsureAi`: the Berserkers research down their plan — Propulsion 2
/// first, at fifteen percent — and line up the next field when a level is
/// one away; with everything at 24 they stop, and past the plan they study
/// whatever is lowest.
#[test]
fn the_berserkers_follow_their_research_plan() {
    use stars_core::research::NextField;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(1);
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.research, 0, "the plan's first entry");
    assert_eq!(state.players[1].research.current_field, 2, "Propulsion");
    assert_eq!(state.players[1].research_pct, turindrone::RESEARCH_PCT);
    assert_eq!(state.players[1].research.next_field, NextField::Same);

    // One level short of Propulsion 2: Construction is lined up after it.
    state.players[1].research.levels[2] = 1;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.research, 0);
    assert_eq!(state.players[1].research.current_field, 2);
    assert_eq!(state.players[1].research.next_field, NextField::Field(3));

    // Propulsion 2 reached: on to Construction 4.
    state.players[1].research.levels[2] = 2;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.research, 1);
    assert_eq!(state.players[1].research.current_field, 3);

    // Past the plan, with Weapons the lowest.
    state.players[1].research.levels = [20, 17, 20, 20, 20, 20];
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.research, turindrone::PLAN_DONE);
    assert_eq!(state.players[1].research.current_field, 1);
    assert_eq!(state.players[1].research_pct, turindrone::RESEARCH_PCT);

    // Everything at 24: nothing more to spend on.
    state.players[1].research.levels = [24; 6];
    turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(state.players[1].research_pct, 0);
}

/// Left to themselves, the Berserkers' scouts find a planet worth settling
/// within their first twenty years or so, and a colony ship goes out.
#[test]
fn the_berserkers_settle_a_planet_in_time() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(1);
    let mut colonised = false;
    for _ in 0..25 {
        let report = stars_core::generate_turn(&mut state, &mut rng);
        if report.ai.iter().any(|(_, r)| !r.colonising.is_empty()) {
            colonised = true;
            break;
        }
    }
    assert!(colonised, "a colony ship went out within twenty-five years");
}

/// `FQueueAiDefenses`: a home world of 200,000 people wants twenty-five
/// defences and queues four of them at a time; a smaller one wants none.
#[test]
fn defences_come_with_the_people() {
    use stars_core::production::item;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(7);
    state.turn = 1;
    let home = state
        .planets
        .iter()
        .position(|p| p.owner == Some(1))
        .expect("home");
    let id = state.planets[home].id;
    state.planets[home].pop = 2000;
    state.planets[home].defenses = 0;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.defended, vec![(id, 4)], "{:?}", report.defended);
    let defences: Vec<i32> = state.planets[home]
        .queue
        .iter()
        .filter(|q| !q.ship && q.item == item::DEFENSE)
        .map(|q| q.count)
        .collect();
    assert_eq!(defences, vec![4]);

    // Already covered: with thirty defences up, nothing more is queued.
    let mut state = tutorial_world();
    state.turn = 1;
    state.planets[home].pop = 2000;
    state.planets[home].defenses = 30;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.defended.is_empty(), "{:?}", report.defended);

    // Too few people (the tutorial's Berserkers start with fewer than
    // 160,000): none either.
    let mut state = tutorial_world();
    state.turn = 1;
    state.planets[home].defenses = 0;
    assert!(state.planets[home].pop < 1600);
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.defended.is_empty(), "{:?}", report.defended);
}

/// `AddMinesToBlockedQueues`: factories at the head of a queue on a planet
/// with no minerals on the surface and no mines to dig them are waiting on
/// minerals, and mines go in ahead of them.
#[test]
fn a_blocked_queue_gets_mines_in_front() {
    use stars_core::production::{item, QueueItem};

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(7);
    state.turn = 1;
    let home = state
        .planets
        .iter()
        .position(|p| p.owner == Some(1))
        .expect("home");
    let id = state.planets[home].id;
    state.planets[home].surface_min = [0, 0, 0];
    state.planets[home].mines = 0;
    state.planets[home].queue = vec![QueueItem {
        count: 50,
        item: item::FACTORY,
        ship: false,
        completion: 0,
    }];
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.unblocked.len(), 1, "{:?}", report.unblocked);
    let (planet, mines) = report.unblocked[0];
    assert_eq!(planet, id);
    assert!(mines > 0, "{mines} mines");
    assert_eq!(state.planets[home].queue[0].item, item::MINE);

    // Nothing to fling: the tutorial's Berserkers are of skill 0.
    assert!(report.flung.is_empty());
}

/// `SplitOutShdefs`: from turn 61, a fleet carrying both an old design and
/// a current one is split, the old design's ships going to a fleet of
/// their own with their share of the cargo.
#[test]
fn old_designs_are_split_into_their_own_fleets() {
    use stars_core::fleet::ShipStack;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(4);
    state.turn = 70;
    // The scout design again in the destroyers' slot 10, made at turn 0:
    // seventy years old, with ships still flying.
    while state.designs[1].len() < 11 {
        let last = state.designs[1].last().expect("a design").clone();
        state.designs[1].push(last);
    }
    let mut old = state.designs[1][0].clone();
    old.designed = 0;
    state.designs[1][10] = old;
    let scout = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 0)
        .expect("a scout");
    state.fleets[scout].stacks = vec![
        ShipStack {
            design: 10,
            count: 2,
            damaged_pct: 0,
            damage_pct: 0,
        },
        ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        },
    ];
    state.fleets[scout].cargo.fuel = 300;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.split.len(), 1, "{:?}", report.split);
    let (from, to) = report.split[0];
    assert_eq!(from, 0);
    let kept = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == from)
        .expect("the old fleet");
    let split = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == to)
        .expect("the new fleet");
    assert_eq!(
        kept.stacks,
        vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }]
    );
    assert_eq!(
        split.stacks,
        vec![ShipStack {
            design: 10,
            count: 2,
            damaged_pct: 0,
            damage_pct: 0,
        }]
    );
    assert_eq!(split.position, kept.position);
    assert_eq!(
        split.cargo.fuel, 200,
        "two thirds of the fuel went with them"
    );
    assert_eq!(kept.cargo.fuel, 100);

    // Before turn 61 nothing is split.
    let mut state = tutorial_world();
    state.turn = 60;
    let mut old = state.designs[1][0].clone();
    old.designed = 0;
    while state.designs[1].len() < 11 {
        let last = state.designs[1].last().expect("a design").clone();
        state.designs[1].push(last);
    }
    state.designs[1][10] = old;
    state.fleets[scout].stacks = vec![
        ShipStack {
            design: 10,
            count: 2,
            damaged_pct: 0,
            damage_pct: 0,
        },
        ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        },
    ];
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.split.is_empty());
}

/// The first pass over the fleets: a colony ship bound for a planet
/// another player has since taken has its orders cut — the planet has a
/// starbase, so no colonists are dropped — and a miner at a planet
/// somebody owns likewise.
#[test]
fn stale_orders_are_cut() {
    use stars_core::fleet::{grobj, Waypoint};
    use stars_formats::task;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(4);
    state.turn = 5;
    let theirs = state
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("the human's home")
        .clone();
    assert!(theirs.starbase);
    let colony = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 1)
        .expect("the Santa Maria");
    state.fleets[colony].waypoints.push(Waypoint {
        position: theirs.position.expect("placed"),
        target: Some(theirs.id as u16),
        target_class: grobj::PLANET,
        warp: 6,
        task: task::COLONIZE,
        transport: None,
        task_data: Vec::new(),
    });
    // The miners, with Remote Mining ordered at the planet they orbit —
    // which is home, and owned.
    let miner = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 2)
        .expect("the miners");
    state.fleets[miner].waypoints[0].task = task::REMOTE_MINING;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.cleaned.contains(&1), "{:?}", report.cleaned);
    assert!(report.cleaned.contains(&2), "{:?}", report.cleaned);
    assert!(report.dropping.is_empty());
    let miner = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == 2)
        .expect("the miners");
    assert_eq!(miner.waypoints[0].task, task::NONE);
}

/// `LCheckForColDrop`: a colony ship with colonists aboard, sitting at a
/// planet another player took that is worth something to us and has no
/// starbase, is told to drop the colonists there and go home.
#[test]
fn colonists_are_dropped_on_a_neighbours_new_colony() {
    use stars_formats::{task, XferAction};

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(4);
    state.turn = 5;
    // Planet 14 is habitable to the Berserkers; the human has just taken
    // it.
    let taken = state
        .planets
        .iter()
        .position(|p| p.id == 14)
        .expect("planet 14");
    state.planets[taken].owner = Some(0);
    state.planets[taken].pop = 25;
    state.planets[taken].starbase = false;
    let at = state.planets[taken].position.expect("placed");
    let home = state
        .planets
        .iter()
        .find(|p| p.owner == Some(1))
        .expect("home")
        .clone();
    let colony = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 1)
        .expect("the Santa Maria");
    let fleet = &mut state.fleets[colony];
    fleet.position = at;
    fleet.orbiting = Some(14);
    fleet.waypoints.truncate(1);
    fleet.waypoints[0].position = at;
    fleet.waypoints[0].target = Some(14);
    fleet.cargo.colonists = 25;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.dropping, vec![(1, 14)]);
    let fleet = &state.fleets[colony];
    assert_eq!(fleet.waypoints.len(), 2);
    assert_eq!(fleet.waypoints[0].task, task::TRANSPORT);
    let orders = fleet.waypoints[0].transport.expect("a transport order");
    assert_eq!(orders.items[3].action, XferAction::UnloadAll);
    assert_eq!(fleet.waypoints[1].target, Some(home.id as u16));
    // Laid at warp 4, and then re-speeded by `KeepFleetsMoving`.
    assert!(fleet.waypoints[1].warp > 0);
}

/// A Small Freighter design in the Berserkers' haulers' slot 8, and a
/// hauler of it at home with the given fleet id.
fn a_hauler_at_home(state: &mut stars_core::GameState, id: u16) -> stars_core::planet::Planet {
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_core::fleet::ShipStack;
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
    let home = state
        .planets
        .iter()
        .find(|p| p.owner == Some(1))
        .expect("home")
        .clone();
    let mut hauler = state.fleets[0].clone();
    hauler.id = id;
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
    home
}

/// `ValidateStarbaseHistory`: from turn 20 the Berserkers' home world,
/// with its starbase, is listed, and their hauler is assigned to it;
/// before then the table stays empty.
#[test]
fn the_starbase_history_lists_the_haulers() {
    use stars_core::ai::StarbaseHistoryEntry;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(5);
    state.turn = 19;
    let home = a_hauler_at_home(&mut state, 9);
    turindrone::turn(&mut state, 1, &mut rng);
    assert!(state.players[1].starbase_history.is_empty());

    state.turn = 20;
    turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(
        state.players[1].starbase_history,
        vec![StarbaseHistoryEntry {
            planet: home.id,
            fleets: vec![9],
        }]
    );

    // A planet lost is dropped from the table.
    let index = state
        .planets
        .iter()
        .position(|p| p.id == home.id)
        .expect("home");
    state.planets[index].owner = Some(0);
    turindrone::turn(&mut state, 1, &mut rng);
    assert!(state.players[1].starbase_history.is_empty());
}

/// `FSalvageTargetFreighter2`: a hauler at home with nothing else to do
/// goes for salvage — a stationary packet — within 200 light years, and
/// one sitting on top of salvage empties it into its hold.
#[test]
fn a_hauler_collects_salvage() {
    use stars_core::fleet::grobj;
    use stars_core::movement::Point;
    use stars_core::packet::Packet;

    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(5);
    state.turn = 3;
    let home = a_hauler_at_home(&mut state, 9);
    let at = home.position.expect("placed");
    let nearby = Point::new(at.x + 60, at.y);
    state.packets.push(Packet {
        id: 3,
        owner: 0,
        position: nearby,
        target: 0,
        warp: 0,
        minerals: [120, 40, 0],
        decay_rate: 0,
        moved: false,
        include: true,
        turn: 0,
    });
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.hauling, vec![(9, -1)], "{:?}", report.hauling);
    let hauler = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == 9)
        .expect("the hauler");
    assert_eq!(hauler.waypoints.len(), 2);
    assert_eq!(hauler.waypoints[1].target_class, grobj::THING);
    assert_eq!(hauler.waypoints[1].target, Some(3));
    assert_eq!(hauler.waypoints[1].position, nearby);
    assert_eq!(hauler.waypoints[1].task, stars_formats::task::NONE);

    // On top of it: the minerals come aboard — all of them, home being
    // short of none in particular (with one mineral scarce, only that one
    // would be taken).
    let mut state = tutorial_world();
    state.turn = 3;
    let home = a_hauler_at_home(&mut state, 9);
    let at = home.position.expect("placed");
    let index = state
        .planets
        .iter()
        .position(|p| p.id == home.id)
        .expect("home");
    state.planets[index].surface_min = [500, 500, 500];
    state.packets.push(Packet {
        id: 3,
        owner: 0,
        position: at,
        target: 0,
        warp: 0,
        minerals: [60, 40, 0],
        decay_rate: 0,
        moved: false,
        include: true,
        turn: 0,
    });
    turindrone::turn(&mut state, 1, &mut rng);
    let hauler = state
        .fleets
        .iter()
        .find(|f| f.owner == 1 && f.id == 9)
        .expect("the hauler");
    assert_eq!(hauler.cargo.minerals, [60, 40, 0], "a 120 kT hold");
    assert_eq!(state.packets[0].minerals, [0, 0, 0]);
}

/// `vlpbAiPlanet[+15] = 4`: a planet one colony ship is sent to this turn
/// is claimed, and the next colony ship goes elsewhere.
#[test]
fn colony_ships_of_one_turn_claim_their_planets() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(4);
    state.turn = 5;
    // Planets 14 and 16 are habitable to the Berserkers once known.
    state.players[1].explored.insert(14);
    state.players[1].explored.insert(16);
    let colony = state
        .fleets
        .iter()
        .position(|f| f.owner == 1 && f.id == 1)
        .expect("the Santa Maria");
    let mut second = state.fleets[colony].clone();
    second.id = 7;
    state.fleets.push(second);
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.colonising.len(), 2, "{:?}", report.colonising);
    assert_ne!(
        report.colonising[0].1, report.colonising[1].1,
        "two ships, two planets: {:?}",
        report.colonising
    );
}

/// `FixPlanetsUnderAttack`: an own planet with a bomber fleet of somebody
/// else's in orbit gets defences rushed to the front of its queue —
/// mineral alchemy ahead of them when the minerals are short — but only
/// from turn `20 + 10 × size`, never in a tutorial game, never with
/// defences already queued, and never with under fifty resources.
#[test]
fn a_planet_under_bombers_rushes_its_defences() {
    use stars_core::fleet::{Fleet, ShipStack};
    use stars_core::production::item;

    let setup = || {
        let mut state = tutorial_world();
        state.tutorial_game = false;
        state.turn = 20 + 10 * state.galaxy_size;
        let home = state
            .planets
            .iter()
            .position(|p| p.owner == Some(1))
            .expect("home");
        // Player 0's Gadfly — a Mini Bomber — parked over the Berserkers'
        // home world.
        let slot = state.designs[0].len();
        state.designs[0]
            .push(stars_core::startup::SHIPS[stars_core::startup::ship::GADFLY].design());
        let at = state.planets[home].position.expect("placed");
        state.fleets.push(Fleet {
            id: 200,
            owner: 0,
            position: at,
            orbiting: Some(u16::try_from(state.planets[home].id).unwrap()),
            stacks: vec![ShipStack {
                design: u8::try_from(slot).unwrap(),
                count: 1,
                damaged_pct: 0,
                damage_pct: 0,
            }],
            cargo: Default::default(),
            battle_plan: 0,
            warp: None,
            waypoints: Vec::new(),
            name: None,
            repeat_orders: false,
            direction: None,
        });
        state.planets[home].queue.clear();
        state.planets[home].defenses = 0;
        state.planets[home].surface_min = [5000, 5000, 5000];
        (state, home)
    };

    let (mut state, home) = setup();
    let id = state.planets[home].id;
    let mut rng = stars_core::rng::Rng::randomize(7);
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert_eq!(report.fortified.len(), 1, "{:?}", report.fortified);
    let (planet, defenses, alchemy) = report.fortified[0];
    assert_eq!(planet, id);
    assert!(defenses > 0);
    assert_eq!(alchemy, 0, "minerals aplenty: no alchemy");
    let queue = &state.planets[home].queue;
    assert!(!queue.is_empty());
    assert_eq!(queue[0].item, item::DEFENSE, "at the front: {queue:?}");
    assert_eq!(queue[0].count, defenses);

    // With no minerals at all, alchemy goes in front of the defences: the
    // resources over the tenth set aside buy a defence's worth per 150.
    let (mut state, home) = setup();
    state.planets[home].surface_min = [0, 0, 0];
    state.planets[home].mines = 0;
    // Under 160,000 people, so `FQueueAiDefenses` queues nothing first;
    // the factories make the resources.
    state.planets[home].pop = 1500;
    state.planets[home].factories = 300;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(
        !report.fortified.is_empty(),
        "queue {:?}",
        state.planets[home].queue
    );
    let (_, defenses, alchemy) = report.fortified[0];
    assert!(alchemy > 0, "{:?}", report.fortified);
    assert_eq!(alchemy % 5, 0, "five units a defence");
    let queue = &state.planets[home].queue;
    assert_eq!(queue[0].item, item::ALCHEMY);
    assert_eq!(queue[1].item, item::DEFENSE);
    assert_eq!(queue[1].count, defenses);

    // Too early: nothing.
    let (mut state, _) = setup();
    state.turn -= 1;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.fortified.is_empty());

    // A tutorial game: nothing.
    let (mut state, _) = setup();
    state.tutorial_game = true;
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.fortified.is_empty());

    // No bombers aboard — a scout instead — and the planet is not under
    // attack.
    let (mut state, _) = setup();
    let scout = state.designs[0]
        .iter()
        .position(|d| d.hull_id == 4)
        .expect("a scout design");
    let last = state.fleets.len() - 1;
    state.fleets[last].stacks[0].design = u8::try_from(scout).unwrap();
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(report.fortified.is_empty());
}
