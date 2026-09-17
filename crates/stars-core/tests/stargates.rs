//! Stargate jumps.
//!
//! The stargate branch of `MoveFleets` (`10b0:354f`), `FStargateJump`
//! (`1080:0cfe`) and `MdCalcStargateDamage` (`1080:152e`). See
//! `docs/formulas/stargates.md`.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::message::id;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::{Race, RaceStat};
use stars_core::stargate::{self, Verdict};
use stars_core::{generate_turn, GameState, Player, Rng};

/// A Medium Freighter on a Long Hump 6 — hull 1, well under a 100 kT gate's
/// limit — and a Space Station carrying a `Stargate 100/250` in its
/// first orbital slot.
fn designs(gate: u8) -> Vec<ShipDesign> {
    let mut designs = vec![ShipDesign {
        name: "Scout".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: 1,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 3,
            count: 1,
        }],
    }];
    while designs.len() < usize::from(stars_core::startup::FIRST_STARBASE_SLOT) {
        designs.push(ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: -1,
            slots: Vec::new(),
        });
    }
    designs.push(ShipDesign {
        name: "Gated Station".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: 34,
        slots: vec![DesignSlot {
            category: slot::SPECIAL_SB,
            item: gate,
            count: 1,
        }],
    });
    designs
}

fn gated_planet(id: i16, at: Point, owner: i16) -> Planet {
    let mut planet = Planet::unowned(id);
    planet.owner = Some(owner);
    planet.position = Some(at);
    planet.pop = 25_000;
    planet.starbase = true;
    planet.starbase_design = Some(0);
    planet
}

fn waypoint(at: Point, planet: i16, warp: u8) -> Waypoint {
    Waypoint {
        position: at,
        target: Some(planet as u16),
        target_class: 1,
        warp,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    }
}

/// One player, two gated planets `apart` light years apart, and a fleet of
/// `ships` freighters at the first ordered through the gate to the second.
fn a_jump(apart: i16, ships: i32, gate: u8) -> GameState {
    let mut state = GameState::new(11);
    state.players = vec![Player::new(Race::humanoid())];
    state.designs = vec![designs(gate)];
    let a = Point::new(1000, 1000);
    let b = Point::new(1000 + apart, 1000);
    state.planets = vec![gated_planet(1, a, 0), gated_planet(2, b, 0)];
    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id: 1,
        owner: 0,
        position: a,
        orbiting: Some(1),
        stacks: vec![ShipStack {
            design: 0,
            count: ships,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            minerals: [0, 0, 0],
            colonists: 0,
            fuel: 50,
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![waypoint(a, 1, 0), waypoint(b, 2, stargate::WARP)],
    }];
    state
}

fn messages(state: &GameState, id: u16) -> Vec<&stars_core::message::Message> {
    state.messages.iter().filter(|m| m.id == id).collect()
}

/// `MdCalcStargateDamage` on the 100/250 gate: safe inside both limits,
/// then a linear cut for each limit exceeded, the cuts multiplied, and
/// refusal at five times either.
#[test]
fn the_damage_follows_the_gates_limits() {
    // Gate 0 is the 100/250: 100 kT, 250 ly.
    assert_eq!(stargate::verdict(0, 0, 250, 100), Verdict::Damage(0));
    // 500 ly: (1250 − 500) × 2500 / 250 = 7500 of 10000 survive.
    assert_eq!(stargate::verdict(0, 0, 500, 100), Verdict::Damage(25));
    // 200 kT: (500 − 200) × 2500 / 100 = 7500 too, from the source gate
    // and again from the destination: 7500 × 7500 / 10000 = 5625.
    assert_eq!(stargate::verdict(0, 0, 100, 200), Verdict::Damage(43));
    // All three: 5625 × 7500 / 10000 = 4218.
    assert_eq!(stargate::verdict(0, 0, 500, 200), Verdict::Damage(57));
    // Right at five times: nothing survives.
    assert_eq!(stargate::verdict(0, 0, 1250, 100), Verdict::Lost);
    assert_eq!(stargate::verdict(0, 0, 100, 500), Verdict::Lost);
    // Past five times: refused.
    assert_eq!(stargate::verdict(0, 0, 1251, 100), Verdict::TooFar);
    assert_eq!(stargate::verdict(0, 0, 100, 501), Verdict::TooMassive);
    // The any/any gate (6) has no limit either way.
    assert_eq!(stargate::verdict(6, 6, 5000, 5000), Verdict::Damage(0));
    // The range is the source gate's: any/300 (1) to 100/250 (0) at 300
    // ly is safe, the other way round it is not.
    assert_eq!(stargate::verdict(1, 0, 300, 50), Verdict::Damage(0));
    assert_eq!(stargate::verdict(0, 1, 300, 50), Verdict::Damage(5));
}

/// A safe jump: the fleet is at the far planet the same year, its leg
/// done, its fuel untouched, and nobody is told anything.
#[test]
fn a_fleet_jumps_the_whole_leg_at_once() {
    let mut state = a_jump(200, 3, 0);
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, Point::new(1200, 1000));
    assert_eq!(fleet.orbiting, Some(2));
    assert_eq!(fleet.waypoints.len(), 1, "the leg is consumed");
    // A jump burns nothing — and the station at the far end then fills
    // the tank, as any dock does.
    assert!(fleet.cargo.fuel >= 50, "{}", fleet.cargo.fuel);
    assert_eq!(fleet.stacks[0].count, 3);
    assert_eq!(fleet.stacks[0].damage_pct, 0);
    assert!(
        messages(&state, id::STARGATE_LOST_FEW).is_empty()
            && messages(&state, id::STARGATE_TOO_FAR).is_empty(),
        "{:?}",
        state.messages
    );
}

/// Past the range: every ship rolls to be lost, the survivors are damaged,
/// and the player is told how many did not arrive.
#[test]
fn an_overlong_jump_costs_ships_and_damages_the_rest() {
    // 500 ly on a 250 ly gate: 25% damage, so each ship has 8 in 100 of
    // being lost.
    let mut state = a_jump(500, 100, 0);
    let mut rng = Rng::randomize(7);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, Point::new(1500, 1000), "it arrived");
    let left = fleet.stacks[0].count;
    assert!(left < 100 && left > 60, "lost some: {left} left");
    assert_eq!(fleet.stacks[0].damaged_pct, 100, "every survivor damaged");
    assert!(fleet.stacks[0].damage_pct > 0, "{:?}", fleet.stacks[0]);
    let told = messages(&state, id::STARGATE_LOST_FEW);
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].params, vec![1, 1, 2, 100 - left as i16]);
}

/// An Interstellar Traveler's ships take the damage but never the loss.
#[test]
fn an_interstellar_traveler_loses_no_ships_to_a_bad_jump() {
    let mut state = a_jump(500, 100, 0);
    state.players[0].race.attrs[RaceStat::MajorAdv as usize] = stars_core::race::Prt::It as i16;
    let mut rng = Rng::randomize(7);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.stacks[0].count, 100);
    assert_eq!(fleet.stacks[0].damaged_pct, 100);
    assert!(messages(&state, id::STARGATE_LOST_FEW).is_empty());
}

/// Five times the range and more: the gates refuse the jump, the fleet
/// stays, and the player is told why.
#[test]
fn a_jump_beyond_five_times_the_range_is_refused() {
    let mut state = a_jump(1300, 2, 0);
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, Point::new(1000, 1000), "stayed");
    assert_eq!(fleet.waypoints.len(), 2);
    assert_eq!(fleet.stacks[0].count, 2);
    let told = messages(&state, id::STARGATE_TOO_FAR);
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].params, vec![1, 1, 2]);
}

/// Cargo cannot go through: an ordinary race's fleet puts its minerals
/// and colonists down on the planet it leaves from and says so, then
/// jumps.
#[test]
fn cargo_is_unloaded_before_the_jump() {
    let mut state = a_jump(100, 1, 0);
    state.fleets[0].cargo.minerals = [30, 20, 10];
    state.fleets[0].cargo.colonists = 5;
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, Point::new(1100, 1000));
    assert_eq!(fleet.cargo.minerals, [0, 0, 0]);
    assert_eq!(fleet.cargo.colonists, 0);
    let home = &state.planets[0];
    assert_eq!(home.surface_min, [30, 20, 10]);
    let told = messages(&state, id::STARGATE_UNLOADED_BOTH);
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].params, vec![1, 5, 0, 60, 0, 1]);
}

/// No gate at the far end: the fleet stays and hears why.
#[test]
fn a_destination_without_a_gate_refuses_the_jump() {
    let mut state = a_jump(100, 1, 0);
    state.planets[1].starbase = false;
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    assert_eq!(state.fleets[0].position, Point::new(1000, 1000));
    let told = messages(&state, id::STARGATE_NONE_THERE);
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].params, vec![1, 1, -1, 2]);
}

/// A gate that belongs to somebody who is not a friend cannot be used.
#[test]
fn an_enemys_gate_is_no_use() {
    let mut state = a_jump(100, 1, 0);
    state.players.push(Player::new(Race::humanoid()));
    state.designs.push(designs(0));
    state.planets[1].owner = Some(1);
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    assert_eq!(state.fleets[0].position, Point::new(1000, 1000));
    let told = messages(&state, id::STARGATE_BLOCKED_THERE);
    assert_eq!(told.len(), 1, "{:?}", state.messages);
}
