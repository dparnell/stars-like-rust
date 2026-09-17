//! What can go wrong on the way, and what a ramscoop gathers.
//!
//! The first-pass checks of `MoveFleets` (`10b0:3496`–`423d`): balky
//! Cheap Engines, an Alternate Reality fleet's colonists dying of
//! acceleration, and the warp-10 reactor accident; the radiation of a
//! Radiating Hydro-Ram Scoop after the leg (`10b0:4b2d`); and the fuel
//! ramscoops gather on the way (`10b0:484b`,
//! `LCalcFuelGainFromRamScoops` `1038:56b8`). See
//! `docs/formulas/movement.md`, *Mishaps on the way* and *Ramscoop fuel*.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::message::id;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::{lrt, Prt, Race, RaceStat};
use stars_core::{generate_turn, GameState, Player, Rng};

/// `ships` ships of one design — hull 1, the Medium Freighter (a 450 mg
/// tank), on the engine at `engine` in the engine table — with `colonists` hundreds aboard and
/// `fuel` mg, ordered `far` light years east at `warp`.
fn a_fleet(engine: u8, ships: i32, fuel: i32, colonists: i32, far: i16, warp: u8) -> GameState {
    let mut state = GameState::new(11);
    state.players = vec![Player::new(Race::humanoid())];
    state.designs = vec![vec![ShipDesign {
        name: "Runner".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: 1,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: engine,
            count: 1,
        }],
    }]];
    let mut planet = Planet::unowned(1);
    planet.owner = Some(0);
    planet.position = Some(Point::new(1000, 1000));
    planet.pop = 25_000;
    state.planets = vec![planet];
    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id: 1,
        owner: 0,
        position: Point::new(1000, 1000),
        orbiting: Some(1),
        stacks: vec![ShipStack {
            design: 0,
            count: ships,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            minerals: [0, 0, 0],
            colonists,
            fuel,
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![
            Waypoint {
                position: Point::new(1000, 1000),
                target: Some(1),
                target_class: 1,
                warp: 0,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            },
            Waypoint {
                position: Point::new(1000 + far, 1000),
                target: None,
                target_class: 0,
                warp,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            },
        ],
    }];
    state
}

fn count(state: &GameState, id: u16) -> usize {
    state.messages.iter().filter(|m| m.id == id).count()
}

/// A Cheap Engines race's fleet above warp 6 stays put one year in ten
/// (`Random(10) == 0`), and is told its engines would not start.
#[test]
fn cheap_engines_balk_one_year_in_ten() {
    let mut stayed = 0;
    let mut moved = 0;
    for seed in 0..40u32 {
        // Long Hump 6 (index 3) at warp 7, with fuel to spare.
        let mut state = a_fleet(3, 1, 5000, 0, 200, 7);
        state.players[0].race.lrt_bits |= 1 << lrt::CHEAP_ENGINES;
        let mut rng = Rng::randomize(seed);
        generate_turn(&mut state, &mut rng);
        let balked = count(&state, id::BALKY_ENGINES);
        if state.fleets[0].position == Point::new(1000, 1000) {
            stayed += 1;
            assert_eq!(balked, 1, "seed {seed}: stayed without being told");
        } else {
            moved += 1;
            assert_eq!(balked, 0, "seed {seed}: told, but moved");
        }
    }
    assert!(
        stayed > 0 && moved > stayed,
        "stayed {stayed}, moved {moved}"
    );

    // Not at warp 6 or below, and not without the trait.
    let mut state = a_fleet(3, 1, 5000, 0, 200, 6);
    state.players[0].race.lrt_bits |= 1 << lrt::CHEAP_ENGINES;
    for seed in 0..40u32 {
        let mut s = state.clone();
        generate_turn(&mut s, &mut Rng::randomize(seed));
        assert_eq!(count(&s, id::BALKY_ENGINES), 0, "seed {seed}");
    }
    state = a_fleet(3, 1, 5000, 0, 200, 7);
    for seed in 0..40u32 {
        let mut s = state.clone();
        generate_turn(&mut s, &mut Rng::randomize(seed));
        assert_eq!(count(&s, id::BALKY_ENGINES), 0, "seed {seed}");
    }
}

/// An Alternate Reality fleet carrying more than 1,000 colonists loses
/// `(colonists × 3 + 33) / 100` of them (in hundreds) to acceleration.
#[test]
fn alternate_reality_colonists_die_of_acceleration() {
    let mut state = a_fleet(3, 1, 5000, 100, 100, 6);
    state.players[0].race.attrs[RaceStat::MajorAdv as usize] = Prt::Ar as i16;
    generate_turn(&mut state, &mut Rng::randomize(1));
    // (100 × 3 + 33) / 100 = 3 hundred.
    assert_eq!(state.fleets[0].cargo.colonists, 97);
    let told: Vec<_> = state
        .messages
        .iter()
        .filter(|m| m.id == id::WARP_ACCELERATION_KILLED)
        .collect();
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].params, vec![3, 0, 1]);

    // Ten hundred or fewer are spared, and so is everybody else's race.
    let mut state = a_fleet(3, 1, 5000, 10, 100, 6);
    state.players[0].race.attrs[RaceStat::MajorAdv as usize] = Prt::Ar as i16;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.colonists, 10);
    let mut state = a_fleet(3, 1, 5000, 100, 100, 6);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.colonists, 100);
}

/// At warp 10 every ship whose engine is not rated for it has one chance
/// in ten of blowing up; a fleet that loses them all is gone.
#[test]
fn warp_ten_strains_engines_not_built_for_it() {
    // Alpha Drive 8 (index 5): not one of the five engines safe at 10.
    let mut lost_total = 0;
    for seed in 0..20u32 {
        let mut state = a_fleet(5, 50, 100_000, 0, 100, 10);
        generate_turn(&mut state, &mut Rng::randomize(seed));
        let left = state.fleets.first().map_or(0, |f| f.stacks[0].count);
        let lost = 50 - left;
        lost_total += lost;
        let one = count(&state, id::WARP_TEN_LOST_ONE);
        let some = count(&state, id::WARP_TEN_LOST_SHIPS);
        match lost {
            0 => assert_eq!((one, some), (0, 0), "seed {seed}"),
            1 => assert_eq!((one, some), (1, 0), "seed {seed}"),
            _ => {
                assert_eq!((one, some), (0, 1), "seed {seed}");
                let told = state
                    .messages
                    .iter()
                    .find(|m| m.id == id::WARP_TEN_LOST_SHIPS)
                    .expect("told");
                assert_eq!(told.params, vec![lost as i16, 1]);
            }
        }
    }
    assert!(
        lost_total > 50 && lost_total < 150,
        "{lost_total} lost of 1000"
    );

    // The Trans-Star 10 (index 9) is built for it.
    for seed in 0..20u32 {
        let mut state = a_fleet(9, 50, 100_000, 0, 100, 10);
        generate_turn(&mut state, &mut Rng::randomize(seed));
        assert_eq!(state.fleets[0].stacks[0].count, 50, "seed {seed}");
    }

    // A lone ship that fails takes the fleet with it.
    let mut gone = 0;
    for seed in 0..60u32 {
        let mut state = a_fleet(5, 1, 100_000, 0, 100, 10);
        generate_turn(&mut state, &mut Rng::randomize(seed));
        if state.fleets.is_empty() {
            gone += 1;
            assert_eq!(count(&state, id::WARP_TEN_LOST_FLEET), 1, "seed {seed}");
        }
    }
    assert!(gone > 0, "no accident in sixty tries");
}

/// A Radiating Hydro-Ram Scoop (index 10) kills a share of the colonists
/// aboard every year the fleet moves: `(86 − mid) / 2` percent, `mid` the
/// middle of the race's radiation range, none for a race immune to it or
/// whose range sits high enough.
#[test]
fn a_radiating_engine_kills_colonists_on_the_way() {
    let mut state = a_fleet(10, 1, 5000, 200, 100, 6);
    // Humanoid radiation 15..=85: mid 50, (86 − 50) / 2 = 18% of 200.
    state.players[0].race.env_min[2] = 15;
    state.players[0].race.env_max[2] = 85;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.colonists, 200 - 36);
    let told: Vec<_> = state
        .messages
        .iter()
        .filter(|m| m.id == id::ENGINE_RADIATION_KILLED)
        .collect();
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].params, vec![36, 1]);

    // Immune to radiation: nobody dies.
    let mut state = a_fleet(10, 1, 5000, 200, 100, 6);
    state.players[0].race.env_max[2] = -1;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.colonists, 200);

    // A fleet that did not move is untouched.
    let mut state = a_fleet(10, 1, 5000, 200, 100, 0);
    state.players[0].race.env_min[2] = 15;
    state.players[0].race.env_max[2] = 85;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.colonists, 200);
}

/// A ramscoop flying at a free warp gathers fuel: per light year, the
/// engine count times 1, plus 2 if the next warp is free too, 3 for the
/// one after, 4 for the one after that — as much as the tank will hold,
/// though the message says what was gathered.
#[test]
fn ramscoops_gather_fuel_at_free_warps() {
    // Galaxy Scoop (index 15) is free to warp 9: at warp 5 every warp to
    // 8 is free, so 1 + 2 + 3 + 4 = 10 mg per light year, 25 ly a year:
    // 250 mg.
    let mut state = a_fleet(15, 1, 0, 0, 100, 5);
    generate_turn(&mut state, &mut Rng::randomize(1));
    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, Point::new(1025, 1000));
    assert_eq!(fleet.cargo.fuel, 250, "{:?}", state.messages);
    let told: Vec<_> = state
        .messages
        .iter()
        .filter(|m| m.id == id::RAMSCOOP_FUEL)
        .collect();
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].params, vec![1, 250]);

    // At warp 9, the top free warp, only the 1 counts: 81 × 1 = 81.
    let mut state = a_fleet(15, 1, 0, 0, 200, 9);
    generate_turn(&mut state, &mut Rng::randomize(1));
    let told = state
        .messages
        .iter()
        .find(|m| m.id == id::RAMSCOOP_FUEL)
        .expect("told");
    assert_eq!(told.params, vec![1, 81]);

    // A tank with room for less takes only that, though the message says
    // what was gathered; a full tank takes nothing and nothing is said; an
    // ordinary engine gathers nothing.
    let mut state = a_fleet(15, 1, 400, 0, 100, 5);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.fuel, 450);
    assert_eq!(count(&state, id::RAMSCOOP_FUEL), 1);
    let mut state = a_fleet(15, 1, 450, 0, 100, 5);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.fleets[0].cargo.fuel, 450);
    assert_eq!(count(&state, id::RAMSCOOP_FUEL), 0);
    let mut state = a_fleet(3, 1, 5000, 0, 100, 5);
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert!(state.fleets[0].cargo.fuel < 5000);
    assert_eq!(count(&state, id::RAMSCOOP_FUEL), 0);
}
