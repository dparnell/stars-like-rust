//! Setting a waypoint's task, for the waypoint the map has in hand.
//!
//! `DrawShipWayPtOrders` (`1050:0912`) draws the tile and `UpdateOrdersDDs`
//! (`1050:93ee`) fills its dropdowns. Both work on `sel.iwpAct`, so the tile
//! is about whichever waypoint the scanner is holding.

use stars_core::fleet::{grobj, Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_formats::{task, XferAction};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "tasks".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

/// A fleet of ours with two legs, selected, with the second leg in hand.
fn a_fleet_with_legs(app: &mut App) -> usize {
    let at = Point::new(200, 200);
    let index = {
        let game = app.game.as_mut().expect("a game");
        let mut fleet = Fleet {
            id: 77,
            owner: 0,
            position: at,
            orbiting: None,
            stacks: vec![ShipStack {
                design: 0,
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
        };
        for position in [at, Point::new(240, 200), Point::new(280, 200)] {
            fleet.waypoints.push(stars_core::fleet::Waypoint {
                position,
                target: None,
                target_class: grobj::POSITION,
                warp: 5,
                task: task::NONE,
                transport: None,
                task_data: Vec::new(),
            });
        }
        game.fleets.push(fleet);
        game.fleets.len() - 1
    };
    app.select_object(stars_ui::ScanObject::Fleet(index));
    index
}

fn leg(app: &App, fleet: usize, index: usize) -> stars_core::fleet::Waypoint {
    app.game.as_ref().expect("a game").fleets[fleet].waypoints[index].clone()
}

/// The ten tasks are the ten ids, and their captions are the game's own.
#[test]
fn the_task_list_is_the_games_own() {
    assert_eq!(task::ALL.len(), 10);
    for (id, task) in task::ALL.iter().enumerate() {
        assert_eq!(u8::try_from(id).expect("small"), *task);
    }
    assert_eq!(task::caption(task::NONE), "(no task here)");
    assert_eq!(task::caption(task::MERGE), "Merge with Fleet");
    assert_eq!(task::caption(task::LAY_MINES), "Lay Mine Field");
    assert_eq!(task::caption(task::TRANSFER), "Transfer Fleet");
}

/// The tile is about the waypoint in hand, not always the first leg.
#[test]
fn the_tile_follows_the_waypoint_in_hand() {
    let mut app = a_game();
    let fleet = a_fleet_with_legs(&mut app);

    // Nothing in hand falls back to the first leg, which is what the fleet is
    // actually doing.
    app.selection.waypoint = None;
    assert_eq!(app.task_waypoint(), Some(1));

    app.selection.waypoint = Some(2);
    assert_eq!(app.task_waypoint(), Some(2));
    assert!(app.set_waypoint_task(task::COLONIZE));
    assert_eq!(leg(&app, fleet, 2).task, task::COLONIZE);
    assert_eq!(
        leg(&app, fleet, 1).task,
        task::NONE,
        "the other leg is untouched"
    );

    // A waypoint index past the end is no waypoint at all.
    app.selection.waypoint = Some(9);
    assert_eq!(app.task_waypoint(), None);
    assert!(!app.set_waypoint_task(task::SCRAP));
}

/// Changing the task clears the payload, because the ten bytes mean something
/// different under each one.
#[test]
fn a_new_task_starts_with_a_clean_payload() {
    let mut app = a_game();
    let fleet = a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);

    assert!(app.set_waypoint_task(task::PATROL));
    assert!(app.set_waypoint_task_word(1, 4));
    assert_eq!(app.waypoint_task_word(1), 4);

    assert!(app.set_waypoint_task(task::LAY_MINES));
    assert_eq!(
        app.waypoint_task_word(1),
        0,
        "the patrol range did not become a mine setting"
    );
    assert_eq!(leg(&app, fleet, 1).task_data.len(), 10);
    // Setting the same task again is not a change.
    assert!(!app.set_waypoint_task(task::LAY_MINES));
}

/// Each task keeps its one setting in the word the original reads it from.
#[test]
fn each_task_uses_its_own_word() {
    let mut app = a_game();
    let fleet = a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);

    // Lay Mine Field and Transfer Fleet: word 0.
    assert!(app.set_waypoint_task(task::LAY_MINES));
    assert!(app.set_waypoint_task_word(0, 3));
    assert_eq!(&leg(&app, fleet, 1).task_data[0..2], &[3, 0]);

    // Patrol: word 1, because word 0 is the warp it patrols at.
    assert!(app.set_waypoint_task(task::PATROL));
    assert!(app.set_waypoint_task_word(1, 2));
    assert_eq!(&leg(&app, fleet, 1).task_data[0..2], &[0, 0]);
    assert_eq!(&leg(&app, fleet, 1).task_data[2..4], &[2, 0]);
    // Which is where the engine reads it from.
    assert_eq!(stars_core::patrol::patrol_range(2), 150);
}

/// Transport packs a quantity and an action into one word per cargo, and the
/// decoded view is kept beside the raw bytes.
#[test]
fn transport_packs_each_cargo_into_its_word() {
    let mut app = a_game();
    let fleet = a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);
    assert!(app.set_waypoint_task(task::TRANSPORT));

    assert!(app.set_waypoint_transport(0, XferAction::LoadExact, 250));
    assert_eq!(
        app.waypoint_transport(0),
        (XferAction::LoadExact, 250),
        "ironium loads 250"
    );
    // quantity:12, action:4 — 250 | (3 << 12).
    assert_eq!(
        u16::from_le_bytes([
            leg(&app, fleet, 1).task_data[0],
            leg(&app, fleet, 1).task_data[1]
        ]),
        250 | (3 << 12)
    );
    // The decoded view follows.
    let decoded = leg(&app, fleet, 1).transport.expect("a transport task");
    assert_eq!(decoded.items[0].quantity, 250);
    assert_eq!(decoded.items[0].action, XferAction::LoadExact);

    // A quantity cannot outgrow its twelve bits.
    assert!(app.set_waypoint_transport(1, XferAction::UnloadExact, 9999));
    assert_eq!(app.waypoint_transport(1), (XferAction::UnloadExact, 0x0fff));
}

/// The cargo dropdown starts with fuel and the units follow the cargo, except
/// that a percentage action overrides them.
#[test]
fn the_cargo_list_reads_fuel_first() {
    assert_eq!(stars_formats::CARGO_ORDER, [4, 0, 1, 2, 3]);
    assert_eq!(stars_formats::cargo_name(4), "Fuel");
    assert_eq!(stars_formats::cargo_name(0), "Ironium");

    assert_eq!(stars_formats::cargo_unit(0, XferAction::LoadExact), "kT");
    assert_eq!(stars_formats::cargo_unit(3, XferAction::LoadExact), "00");
    assert_eq!(stars_formats::cargo_unit(4, XferAction::LoadExact), "mg");
    assert_eq!(stars_formats::cargo_unit(0, XferAction::FillPercent), "%");
    assert_eq!(stars_formats::cargo_unit(4, XferAction::WaitPercent), "%");
}

/// Four actions need no figure beside them, and `Load Dunnage` is called
/// `Load Optimal` when the cargo is fuel.
#[test]
fn the_action_list_is_the_games_own() {
    assert_eq!(XferAction::ALL.len(), 10);
    for (code, action) in XferAction::ALL.iter().enumerate() {
        assert_eq!(action.to_raw(), u8::try_from(code).expect("small"));
    }
    for action in [
        XferAction::None,
        XferAction::LoadAll,
        XferAction::UnloadAll,
        XferAction::LoadDunnage,
    ] {
        assert!(!action.needs_quantity(), "{action:?} takes no figure");
    }
    assert!(XferAction::LoadExact.needs_quantity());
    assert!(XferAction::FillPercent.needs_quantity());

    assert_eq!(XferAction::LoadDunnage.caption(false), "Load Dunnage");
    assert_eq!(XferAction::LoadDunnage.caption(true), "Load Optimal");
    assert_eq!(XferAction::None.caption(false), "(no action)");
}

/// Merge warns when the waypoint is not on a fleet, which is the one thing
/// that makes the task work at all.
#[test]
fn merge_warns_when_the_waypoint_is_not_on_a_fleet() {
    let mut app = a_game();
    let fleet = a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);
    assert!(app.set_waypoint_task(task::MERGE));

    let (note, warning) = app.waypoint_task_note().expect("a note");
    assert!(warning, "it is a warning: {note}");

    // Point the waypoint at a fleet and the warning goes.
    if let Some(game) = app.game.as_mut() {
        game.fleets[fleet].waypoints[1].target_class = grobj::FLEET;
        game.fleets[fleet].waypoints[1].target = Some(3);
    }
    assert!(app.waypoint_task_note().is_none());
}

/// Scrap and Colonize always have something to say; a bare task does not.
#[test]
fn the_notes_appear_where_the_original_puts_them() {
    let mut app = a_game();
    a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);

    assert!(app.set_waypoint_task(task::SCRAP));
    let (_, warning) = app.waypoint_task_note().expect("a note");
    assert!(!warning, "scrap is a note, not a warning");

    // No colonists aboard, so colonize warns.
    assert!(app.set_waypoint_task(task::COLONIZE));
    let (_, warning) = app.waypoint_task_note().expect("a note");
    assert!(warning);

    assert!(app.set_waypoint_task(task::ROUTE));
    assert!(app.waypoint_task_note().is_none());
}

/// Another player's fleet takes no orders.
#[test]
fn only_your_own_fleets_take_a_task() {
    let mut app = a_game();
    let theirs = {
        let game = app.game.as_mut().expect("a game");
        let mut fleet = game.fleets[0].clone();
        fleet.owner = 1;
        fleet.id = 12;
        game.fleets.push(fleet);
        game.fleets.len() - 1
    };
    app.select_object(stars_ui::ScanObject::Fleet(theirs));
    app.selection.waypoint = Some(1);
    if app.task_waypoint().is_some() {
        assert!(!app.set_waypoint_task(task::SCRAP));
        assert!(!app.set_waypoint_task_word(0, 3));
    }
}

/// Colonize's three notes (`DrawShipWayPtOrders`): no colonisation module
/// aboard, nobody aboard, or the ships are broken up on arrival.
#[test]
fn colonize_looks_for_a_module_before_the_colonists() {
    let mut app = a_game();
    let fleet = a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);
    assert!(app.set_waypoint_task(task::COLONIZE));
    // Design 0 is a scout with nothing to settle with.
    let (note, warning) = app.waypoint_task_note().expect("a note");
    assert!(warning && note.contains("module"), "{note}");

    // A Santa Maria carries the module.
    let santa_maria = app.game.as_ref().expect("a game").designs[0]
        .iter()
        .position(|d| d.name == "Santa Maria")
        .expect("a colony ship design");
    if let Some(game) = app.game.as_mut() {
        game.fleets[fleet].stacks[0].design = u8::try_from(santa_maria).unwrap();
    }
    let (note, warning) = app.waypoint_task_note().expect("a note");
    assert!(warning && note.contains("colonists"), "{note}");
    if let Some(game) = app.game.as_mut() {
        game.fleets[fleet].cargo.colonists = 25;
    }
    let (_, warning) = app.waypoint_task_note().expect("a note");
    assert!(!warning, "with people aboard it is only a note");
}

/// Remote Mining's notes and its rate row: no robots is a warning, a planet
/// not to be mined gets the note, one nobody knows cannot be estimated,
/// and an uninhabited one on file gets `Mining Rate per Year`.
#[test]
fn remote_mining_estimates_the_rate_where_it_can() {
    let mut app = a_game();
    let fleet = a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);
    assert!(app.set_waypoint_task(task::REMOTE_MINING));
    let (note, warning) = app.waypoint_task_note().expect("a note");
    assert!(warning && note.contains("mining module"), "{note}");
    assert!(app.waypoint_mining_rate().is_none());

    // A Potato Bug is a remote miner (the stock template, which a race
    // without Advanced Remote Mining does not start with); the leg points
    // at nothing yet.
    let miner = {
        let game = app.game.as_mut().expect("a game");
        game.designs[0]
            .push(stars_core::startup::SHIPS[stars_core::startup::ship::POTATO_BUG].design());
        game.designs[0].len() - 1
    };
    if let Some(game) = app.game.as_mut() {
        game.fleets[fleet].stacks[0].design = u8::try_from(miner).unwrap();
    }
    let (_, warning) = app.waypoint_task_note().expect("a note");
    assert!(!warning, "only uninhabited planets: a note, not a warning");

    // At an unowned planet on file: the rate.
    let target = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner.is_none() && p.detail == stars_core::planet::Detail::Full)
        .map(|p| p.id)
        .expect("an unowned planet on file");
    if let Some(game) = app.game.as_mut() {
        let leg = &mut game.fleets[fleet].waypoints[1];
        leg.target_class = grobj::PLANET;
        leg.target = Some(u16::try_from(target).unwrap());
    }
    assert!(app.waypoint_task_note().is_none(), "the rate row instead");
    let rate = app.waypoint_mining_rate().expect("a rate");
    assert!(rate.iter().all(|r| *r >= 0));

    // Somebody's planet: the note again.
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .map(|p| p.id)
        .expect("the home world");
    if let Some(game) = app.game.as_mut() {
        game.fleets[fleet].waypoints[1].target = Some(u16::try_from(home).unwrap());
    }
    let (_, warning) = app.waypoint_task_note().expect("a note");
    assert!(!warning);
    assert!(app.waypoint_mining_rate().is_none());

    // A planet only glimpsed: no estimate.
    if let Some(game) = app.game.as_mut() {
        let id = game.planets.iter().position(|p| p.id == target).unwrap();
        game.planets[id].detail = stars_core::planet::Detail::Minimal;
        game.fleets[fleet].waypoints[1].target = Some(u16::try_from(target).unwrap());
    }
    let (note, warning) = app.waypoint_task_note().expect("a note");
    assert!(warning && note.contains("estimated"), "{note}");
}

/// Patrol's warp is task word 0, which the gauge under the Intercept
/// dropdown sets.
#[test]
fn patrol_keeps_its_warp_in_word_zero() {
    let mut app = a_game();
    a_fleet_with_legs(&mut app);
    app.selection.waypoint = Some(1);
    assert!(app.set_waypoint_task(task::PATROL));
    assert!(app.set_waypoint_task_word(0, 7));
    assert_eq!(app.waypoint_task_word(0), 7);
}
