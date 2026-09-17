//! Damage repair.
//!
//! `HealShips` (`10b8:444c`): a damaged design mends a share of its
//! armour each year, the share set by where the fleet sits — and none at
//! all in a year it fought, hit a mine or jumped a gate. The player's
//! guide's *Damage Repair* topic gives the rates in percent; the binary
//! keeps them in 500ths, which is what a stack's damage is counted in. See
//! `docs/formulas/fleet.md`, *Damage repair*.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::{Prt, Race, RaceStat};
use stars_core::{generate_turn, GameState, Player, Rng};

fn design(name: &str, hull_id: i16) -> ShipDesign {
    ShipDesign {
        name: name.to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 3,
            count: 1,
        }],
    }
}

/// A player, a planet of theirs (with a starbase whose hull is `base`,
/// when given), and a damaged freighter sitting over it — or `far` light
/// years out in space when `far` is not zero.
fn a_damaged_fleet(base: Option<i16>, far: i16) -> GameState {
    let mut state = GameState::new(11);
    state.players = vec![Player::new(Race::humanoid())];
    let mut designs = vec![design("Freighter", 1), design("Tanker", 25)];
    while designs.len() < usize::from(stars_core::startup::FIRST_STARBASE_SLOT) {
        designs.push(design("", -1));
    }
    designs.push(design("Base", base.unwrap_or(-1)));
    state.designs = vec![designs];
    let mut planet = Planet::unowned(1);
    planet.owner = Some(0);
    planet.position = Some(Point::new(1000, 1000));
    planet.pop = 25_000;
    planet.starbase = base.is_some();
    planet.starbase_design = base.map(|_| 0);
    state.planets = vec![planet];
    let at = Point::new(1000 + far, 1000);
    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id: 1,
        owner: 0,
        position: at,
        orbiting: (far == 0).then_some(1),
        stacks: vec![ShipStack {
            design: 0,
            count: 2,
            damaged_pct: 100,
            damage_pct: 200,
        }],
        cargo: Cargo {
            minerals: [0, 0, 0],
            colonists: 0,
            fuel: 100,
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: at,
            target: (far == 0).then_some(1),
            target_class: u8::from(far == 0),
            warp: 0,
            task: 0,
            transport: None,
            task_data: Vec::new(),
        }],
    }];
    state
}

fn damage_after(mut state: GameState) -> (i32, i32) {
    generate_turn(&mut state, &mut Rng::randomize(1));
    let stack = &state.fleets[0].stacks[0];
    (stack.damaged_pct, stack.damage_pct)
}

/// The rates by place: 2% stopped in space, 5% at a planet of yours
/// without a starbase, 8% at one with a starbase but no dock, 20% at a
/// dock — in 500ths, 10, 25, 40 and 100.
#[test]
fn the_rate_depends_on_where_the_fleet_sits() {
    assert_eq!(damage_after(a_damaged_fleet(None, 50)), (100, 190));
    assert_eq!(damage_after(a_damaged_fleet(None, 0)), (100, 175));
    // Orbital Fort (32) has no dock; Space Station (34) does.
    assert_eq!(damage_after(a_damaged_fleet(Some(32), 0)), (100, 160));
    assert_eq!(damage_after(a_damaged_fleet(Some(34), 0)), (100, 100));
}

/// A fleet that moved this year mends only 1%: five 500ths.
#[test]
fn a_moving_fleet_mends_the_least() {
    let mut state = a_damaged_fleet(None, 0);
    state.fleets[0].waypoints.push(Waypoint {
        position: Point::new(1100, 1000),
        target: None,
        target_class: 0,
        warp: 5,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    });
    assert_eq!(damage_after(state), (100, 195));
}

/// A Fuel Transport in the fleet adds 5%, and an Inner Strength race
/// heals twice as fast — the place's rate doubled, not the transport's.
#[test]
fn fuel_transports_and_inner_strength_mend_faster() {
    let mut state = a_damaged_fleet(None, 0);
    state.fleets[0].stacks.push(ShipStack {
        design: 1,
        count: 1,
        damaged_pct: 0,
        damage_pct: 0,
    });
    // 25 for the planet, 25 for the tanker.
    assert_eq!(damage_after(state.clone()), (100, 150));
    state.players[0].race.attrs[RaceStat::MajorAdv as usize] = Prt::Is as i16;
    // 50 for the planet, 25 for the tanker.
    assert_eq!(damage_after(state), (100, 125));
}

/// Damage no greater than the year's mending is gone altogether, and the
/// stack is no longer counted as damaged.
#[test]
fn a_lightly_damaged_stack_is_made_whole() {
    let mut state = a_damaged_fleet(Some(34), 0);
    state.fleets[0].stacks[0].damage_pct = 100;
    assert_eq!(damage_after(state), (0, 0));
}

/// A starbase mends 10% of its own a year, 15% for Inner Strength.
#[test]
fn a_starbase_mends_itself() {
    let mut state = a_damaged_fleet(Some(34), 0);
    state.planets[0].starbase_damage = 120;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.planets[0].starbase_damage, 70);
    let mut state = a_damaged_fleet(Some(34), 0);
    state.planets[0].starbase_damage = 120;
    state.players[0].race.attrs[RaceStat::MajorAdv as usize] = Prt::Is as i16;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.planets[0].starbase_damage, 45);
    let mut state = a_damaged_fleet(Some(34), 0);
    state.planets[0].starbase_damage = 30;
    generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(state.planets[0].starbase_damage, 0);
}
