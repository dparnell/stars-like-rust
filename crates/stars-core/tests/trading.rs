//! Going through a wormhole, and trading with the Mystery Trader.
//!
//! The two things the wanderers are *for*, both run from a generated turn: a
//! fleet ordered to a wormhole comes out of the far end during movement, and a
//! fleet that has come to rest on the Trader is absorbed by it in exchange for
//! technology. See `docs/formulas/wanderers.md`.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::message::id;
use stars_core::movement::Point;
use stars_core::wormhole::{part, tech_levels, Gift, MysteryTrader, Wormhole};
use stars_core::{generate_turn, GameState, Player, Race, Rng};

/// A one-player galaxy with a single ship that can fly.
fn a_galaxy() -> GameState {
    let mut state = GameState::new(7);
    state.players = vec![Player::new(Race::humanoid())];
    state.designs = vec![vec![ShipDesign {
        name: "Scout".to_string(),
        picture: 0,
        stored_armor: 0,
        hull_id: 0,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 1,
            count: 1,
        }],
    }]];
    state.galaxy_planets = 2;
    state
}

/// A fleet of one ship, at `at`.
fn a_fleet(at: Point) -> Fleet {
    Fleet {
        name: None,
        repeat_orders: false,
        id: 1,
        owner: 0,
        position: at,
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            fuel: 10_000,
            ..Cargo::default()
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: at,
            target: None,
            target_class: 4,
            warp: 0,
            task: stars_formats::task::NONE,
            transport: None,
            task_data: Vec::new(),
        }],
    }
}

/// A wormhole end.
fn an_end(id: u16, at: Point, partner: u16) -> Wormhole {
    Wormhole {
        id,
        position: at,
        stability: 0,
        years_still: 0,
        dest_known: false,
        include: true,
        detected_by: 0,
        traversed_by: 0,
        partner,
        turn: 0,
    }
}

/// The Trader, carrying `part`, part way across a long crossing.
///
/// Warp 13 so that it never changes course — the roll is only made below that
/// — and a destination far enough away that it does not arrive, which would
/// end the pass and quite possibly the Trader.
fn a_trader(at: Point, part: u16) -> MysteryTrader {
    MysteryTrader {
        id: 1,
        position: at,
        destination: Point::new(at.x + 5000, at.y),
        warp: 13,
        include: true,
        detected_by: 0,
        part,
        turn: 0,
    }
}

/// Where a Trader will be at the end of the year, which is where a fleet has
/// to be to meet it: the Trader flies before anything else happens.
fn meeting_point(trader: &MysteryTrader) -> Point {
    stars_core::movement::advance(trader.position, trader.destination, trader.range())
}

/// A fleet ordered to a wormhole comes out of the far end.
#[test]
fn a_fleet_goes_through_a_wormhole() {
    let mut state = a_galaxy();
    let near = Point::new(1000, 1000);
    let far = Point::new(3000, 3000);
    state.wormholes = vec![an_end(1, near, 2), an_end(2, far, 1)];

    let mut fleet = a_fleet(Point::new(950, 1000));
    fleet.warp = Some(8);
    fleet.waypoints.push(Waypoint {
        position: near,
        // A thing's id in a waypoint carries its kind in the top three bits:
        // 2 is a wormhole.
        target: Some(1 | (2 << 13)),
        // `grobj` 8: the waypoint names a thing, not a planet.
        target_class: 8,
        warp: 8,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(report.wormhole_trips, vec![(1, 1, 2)]);
    // It came out where the far end was when it went in. The ends wander
    // afterwards; the fleet does not go with them.
    assert_eq!(state.fleets[0].position, far);
    assert_eq!(state.fleets[0].orbiting, None);
    // Both ends remember the traveller, and the far end is now in view.
    assert_eq!(state.wormholes[0].traversed_by, 1);
    assert_eq!(state.wormholes[1].traversed_by, 1);
    assert_eq!(state.wormholes[1].detected_by, 1);
}

/// A fleet that stops short of the wormhole stays where it is.
#[test]
fn a_fleet_that_has_not_arrived_stays_put() {
    let mut state = a_galaxy();
    let near = Point::new(2000, 1000);
    state.wormholes = vec![an_end(1, near, 2), an_end(2, Point::new(3000, 3000), 1)];

    let mut fleet = a_fleet(Point::new(1000, 1000));
    fleet.warp = Some(4);
    fleet.waypoints.push(Waypoint {
        position: near,
        target: Some(1 | (2 << 13)),
        target_class: 8,
        warp: 4,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);
    assert!(report.wormhole_trips.is_empty());
    assert!(state.fleets[0].position.x < 2000);
    assert_eq!(state.wormholes[0].traversed_by, 0);
}

/// The Trader hands over the technology it is carrying, and keeps the fleet.
#[test]
fn the_trader_gives_what_it_carries() {
    let mut state = a_galaxy();
    let trader = a_trader(Point::new(2000, 2000), part::SHIELD);
    let at = meeting_point(&trader);
    state.traders = vec![trader];
    let mut fleet = a_fleet(at);
    fleet.cargo.minerals = [2000, 2000, 1000];
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(report.trades, vec![(1, Gift::Part(part::SHIELD))]);
    assert_eq!(state.players[0].trader_parts, part::SHIELD);
    // The fleet is gone: the Trader absorbed it, cargo and all.
    assert!(state.fleets[0].stacks.is_empty());
    assert_eq!(state.fleets[0].cargo.minerals, [0, 0, 0]);
    // And the Trader will not trade with this player again.
    assert_eq!(state.traders[0].detected_by, 1);

    let message = state
        .messages
        .iter()
        .find(|m| m.id == id::TRADER_GAVE_PART)
        .expect("the player is told what they were given");
    // 0xC206: a shield, the seventh of its kind.
    assert_eq!(message.object as u16, 0xC206);
}

/// A Trader carrying nothing in particular gives research instead.
#[test]
fn the_trader_gives_technology() {
    let mut state = a_galaxy();
    let trader = a_trader(Point::new(2000, 2000), 0);
    let at = meeting_point(&trader);
    state.traders = vec![trader];
    let mut fleet = a_fleet(at);
    // Ten thousand kilotons: five thousand buys the audience and the rest buys
    // four more levels.
    fleet.cargo.minerals = [4000, 3000, 3000];
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(tech_levels(10_000, 0), 10);
    assert_eq!(report.trades, vec![(1, Gift::Tech(10))]);
    let levels: i16 = state.players[0]
        .research
        .levels
        .iter()
        .map(|l| i16::from(*l))
        .sum();
    assert_eq!(levels, 10, "ten levels, spread over the six fields");
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == id::TRADER_GAVE_TECH && m.params.get(1) == Some(&10)));
}

/// One Trader trades once with each player, and turns away a fleet that has
/// not brought enough.
#[test]
fn the_trader_trades_once_and_wants_paying() {
    let mut state = a_galaxy();
    let trader = a_trader(Point::new(2000, 2000), part::MINER);
    let at = meeting_point(&trader);
    state.traders = vec![trader];

    let mut rich = a_fleet(at);
    rich.cargo.minerals = [2000, 2000, 1000];
    let mut poor = a_fleet(at);
    poor.id = 2;
    poor.cargo.minerals = [2000, 2000, 1000];
    let mut empty = a_fleet(at);
    empty.id = 3;
    empty.cargo.minerals = [10, 10, 10];
    // It arrived this year, so it is told; a fleet already sitting here is not
    // told again.
    empty.warp = Some(5);
    state.fleets = vec![rich, poor, empty];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(report.trades[0], (1, Gift::Part(part::MINER)));
    assert_eq!(report.trades[1], (2, Gift::AlreadyMet));
    assert_eq!(report.trades[2], (3, Gift::Refused));
    // The second fleet was not taken.
    assert!(!state.fleets[1].stacks.is_empty());
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == id::TRADER_REFUSED && m.params.first() == Some(&3)));
}

/// With nothing left to give, the Trader gives ships of its own.
#[test]
fn the_trader_gives_ships_when_it_has_nothing_else() {
    use stars_core::startup::{ship::MT_LIFEBOAT, SHIPS};

    let mut given = 0;
    let mut empty_handed = 0;
    // Whether it finds anything in the hold is a one-in-five roll, so this
    // walks a spread of seeds rather than hunting for a lucky one.
    for seed in 1..=25u32 {
        let mut state = a_galaxy();
        let trader = a_trader(Point::new(2000, 2000), 0);
        let at = meeting_point(&trader);
        state.traders = vec![trader];
        // A player who has researched everything and been given every part:
        // the Trader has nothing to sell them.
        state.players[0].research.levels = [26; 6];
        state.players[0].trader_parts = part::ALL;
        let mut fleet = a_fleet(at);
        fleet.cargo.minerals = [2000, 2000, 1000];
        state.fleets = vec![fleet];

        // Seeded the way the game seeds itself: two raw seeds a fixed
        // distance apart stay in step with each other, and a one-in-five roll
        // then comes out the same every time.
        let mut rng = Rng::randomize(seed);
        let report = generate_turn(&mut state, &mut rng);

        match report.trades.as_slice() {
            [(1, Gift::Nothing)] => empty_handed += 1,
            [(1, Gift::Ship { design, ships })] => {
                given += 1;
                // One of the Trader's own three designs, which nobody can
                // build.
                assert!((MT_LIFEBOAT..=MT_LIFEBOAT + 2).contains(design));
                assert!((1..=10).contains(ships), "{ships} ships");

                let gift = state.fleets.last().expect("a new fleet");
                assert_eq!(gift.owner, 0);
                assert_eq!(gift.stacks[0].count, *ships);
                assert_eq!(gift.position, at);
                assert!(gift.cargo.fuel > 0, "the Trader fuels what it gives");

                // The design went into a slot of the player's own, leaving the
                // one they were already using alone.
                let slot = usize::from(gift.stacks[0].design);
                assert_eq!(slot, 1);
                assert_eq!(state.designs[0][slot].name, SHIPS[*design].name);
                assert_eq!(state.designs[0][0].name, "Scout");

                assert!(state
                    .messages
                    .iter()
                    .any(|m| m.id == id::TRADER_GAVE_SHIP
                        && m.params.get(1) == Some(&(*ships as i16))));
            }
            other => panic!("unexpected trade {other:?}"),
        }
    }
    assert!(given > 0, "no ships in 25 tries");
    assert!(empty_handed > 0, "the Trader always found something");
    eprintln!("{given} of 25 meetings ended in ships, {empty_handed} in nothing");
}

/// A computer player is given nothing, and not told either.
#[test]
fn the_trader_does_not_bother_with_the_ai() {
    let mut state = a_galaxy();
    let trader = a_trader(Point::new(2000, 2000), 0);
    let at = meeting_point(&trader);
    state.traders = vec![trader];
    state.players[0].research.levels = [26; 6];
    state.players[0].trader_parts = part::ALL;
    state.players[0].control = stars_core::ai::Control::Computer {
        personality: None,
        skill_bits: 1,
    };
    let mut fleet = a_fleet(at);
    fleet.cargo.minerals = [2000, 2000, 1000];
    state.fleets = vec![fleet];

    // A seed that gives a human player ships.
    let ship_seed = (1..=25u32)
        .find(|seed| {
            let mut human = a_galaxy();
            human.traders = vec![a_trader(Point::new(2000, 2000), 0)];
            human.players[0].research.levels = [26; 6];
            human.players[0].trader_parts = part::ALL;
            let mut fleet = a_fleet(at);
            fleet.cargo.minerals = [2000, 2000, 1000];
            human.fleets = vec![fleet];
            let report = generate_turn(&mut human, &mut Rng::randomize(*seed));
            matches!(report.trades.as_slice(), [(_, Gift::Ship { .. })])
        })
        .expect("some seed gives ships");

    let mut rng = Rng::randomize(ship_seed);
    generate_turn(&mut state, &mut rng);
    // The fleet was still taken; the AI simply gets nothing back.
    assert_eq!(state.fleets.len(), 1);
    assert!(state
        .messages
        .iter()
        .all(|m| m.id != id::TRADER_GAVE_SHIP && m.id != id::TRADER_TRIED_SHIP));
}

/// A Trader that reaches its destination either turns round or leaves, and a
/// fleet that was following one that left is told and given a plain position.
#[test]
fn a_trader_that_arrives_turns_round_or_leaves() {
    use stars_core::wormhole::Event;

    let mut turned = 0;
    let mut left = 0;
    for seed in 1..=20u32 {
        let mut state = a_galaxy();
        let at = Point::new(2000, 2000);
        let mut trader = a_trader(at, 0);
        // Standing on its destination: this year ends the pass.
        trader.destination = at;
        trader.warp = 10;
        state.traders = vec![trader];

        // A fleet on its way to the Trader, which will need new orders if it
        // goes.
        let mut fleet = a_fleet(Point::new(1000, 1000));
        fleet.warp = Some(1);
        fleet.waypoints.push(Waypoint {
            position: at,
            // 3 is the Mystery Trader.
            target: Some(1 | (3 << 13)),
            target_class: 8,
            warp: 1,
            task: stars_formats::task::NONE,
            transport: None,
            task_data: Vec::new(),
        });
        state.fleets = vec![fleet];

        let mut rng = Rng::randomize(seed);
        let report = generate_turn(&mut state, &mut rng);

        match report.trader_events.as_slice() {
            [(1, Event::AnotherPass)] => {
                turned += 1;
                let trader = &state.traders[0];
                assert_eq!(trader.position, at, "it turns round where it arrived");
                // Two slower than the pass it flew, then the shared step up.
                assert_eq!(trader.warp, 9);
                assert_ne!(trader.destination, at, "with somewhere new to go");
                // Everybody hears about it.
                assert!(state
                    .messages
                    .iter()
                    .any(|m| m.id == id::TRADER_ANOTHER_PASS));
                // The fleet's orders still point at a Trader that is still
                // there.
                assert_eq!(state.fleets[0].waypoints[1].target, Some(1 | (3 << 13)));
            }
            [(1, Event::Departed)] => {
                left += 1;
                assert!(state.traders.is_empty());
                // The orders that were following it now point at where it was.
                let waypoint = &state.fleets[0].waypoints[1];
                assert_eq!(waypoint.target, None);
                assert_eq!(waypoint.target_class, 4);
                assert_eq!(waypoint.position, at);
                assert!(state.messages.iter().any(|m| m.id == id::TRADER_VANISHED));
            }
            // It changed its mind on the doorstep: the one-in-twenty-five
            // roll comes first, and a new destination means it never arrived.
            [(1, Event::ChangedCourse)] => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(turned > 0 && left > 0, "{turned} turned, {left} left");
    eprintln!("{turned} of 20 arrivals turned round, {left} left the galaxy");
}

/// With another Trader in the galaxy, an arriving one always leaves.
#[test]
fn a_second_trader_is_never_needed() {
    use stars_core::wormhole::Event;

    for seed in 1..=10u32 {
        let mut state = a_galaxy();
        let at = Point::new(2000, 2000);
        let mut first = a_trader(at, 0);
        first.destination = at;
        // The second is nowhere near its own destination, so it just flies.
        let mut second = a_trader(Point::new(5000, 5000), 0);
        second.id = 2;
        state.traders = vec![first, second];

        let mut rng = Rng::randomize(seed);
        let report = generate_turn(&mut state, &mut rng);
        assert_eq!(
            report.trader_events,
            vec![(1, Event::Departed)],
            "seed {seed}"
        );
        assert_eq!(state.traders.len(), 1);
        assert_eq!(state.traders[0].id, 2);
    }
}
