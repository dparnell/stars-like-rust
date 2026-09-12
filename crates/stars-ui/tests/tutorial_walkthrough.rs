//! The tutorial, played as its pages say to play it.
//!
//! Each year is driven through the same calls the panes make — Goto, a
//! shift-click on the map, Next, the production dialog, the research
//! dialog, F9 — and after each task the page has to turn exactly as
//! `AdvanceTutor` would turn it. So this fails if any one of those stops
//! doing what the page says it does, or if the tutorial's world stops
//! playing out the way the pages assume.
//!
//! See `docs/ui/tutorial.md`.

use stars_core::production::item;
use stars_formats::task;
use stars_ui::{App, ScanObject};

// The worlds the pages send you to by name, which `tutorial_seed.rs` pins to
// these ids.
const PRUNE: i16 = 0x0c;
const STOVE_TOP: i16 = 0x0d;
const ALEXANDER: i16 = 0x0f;
const PLANET_90210: i16 = 0x10;
const HIHO: i16 = 0x09;
const NO_VACANCY: i16 = 0x03;
const SLIME: i16 = 0x08;
const WALLABY: i16 = 0x05;
const OXYGEN: i16 = 0x02;
const DWARTE: i16 = 0x15;
const MOBIUS: i16 = 0x13;
const CASTLE: i16 = 0x14;
const MOHOLDI: i16 = 0x07;
const SHAGGY_DOG: i16 = 0x0e;
const SEA_SQUARED: i16 = 0x11;
const RED_STORM: i16 = 0x12;
const BLOOP: i16 = 0x17;
const KALAMAZOO: i16 = 0x16;

fn page(app: &App) -> usize {
    app.tutor.as_ref().expect("running").page()
}

fn bold(app: &App) -> Option<usize> {
    app.tutor.as_ref().expect("running").bold_line()
}

fn selected_fleet_id(app: &App) -> Option<u16> {
    app.selection
        .fleet
        .map(|f| app.game.as_ref().expect("a game").fleets[f].id)
}

/// A shift-click on a planet: a waypoint snapped onto it.
fn shift_click(app: &mut App, planet: i16) {
    let at = {
        let game = app.game.as_ref().expect("a game");
        game.planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.id == planet)
            .and_then(|p| p.position)
            .expect("a placed planet")
    };
    assert!(app.add_waypoint(at.x, at.y, 20.0), "a leg to {planet:#x}");
}

/// The Production dialog: pick an item in the inventory and Add it, `count`
/// at a time, `presses` times, under the queue row in hand.
fn queue(app: &mut App, item: u16, count: i32, presses: usize, under: Option<usize>) {
    let index = app
        .production_inventory()
        .iter()
        .position(|row| !row.ship && row.item == item)
        .expect("the item is on offer");
    let dialog = app.production.as_mut().expect("the dialog is open");
    dialog.inventory_index = index;
    dialog.queue_index = under;
    for _ in 0..presses {
        app.production_add(count);
    }
}

/// Years zero to three: pages one to sixteen.
#[test]
fn the_first_four_years_play_through_from_the_pages() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");

    // --- 2400 -------------------------------------------------------------
    // Page 1: read the five messages.
    assert_eq!(page(&app), 1);
    assert_eq!(bold(&app), Some(5), "\"Read all of your messages now.\"");
    for press in 0..4 {
        assert!(!app.advance_tutor(), "not yet, after {press} presses");
        app.show_next_message();
    }
    assert!(app.advance_tutor(), "the last message is in front");
    assert_eq!(page(&app), 2);

    // Page 2: Goto Armed Probe #1, shift-click Prune.
    assert!(app.goto_fleet(0), "Armed Probe #1 is fleet 0");
    assert!(!app.advance_tutor(), "selecting it is only the first rung");
    assert_eq!(
        bold(&app),
        Some(7),
        "the emphasis moves on to the shift-click"
    );
    shift_click(&mut app, PRUNE);
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 3);
    // "It will take 2 years to get to Prune": 49 light years at the warp
    // the client picked.
    {
        let fleet = &app.game.as_ref().expect("a game").fleets[0];
        let warp = i32::from(fleet.waypoints[1].warp);
        assert!(warp * warp < 49, "warp {warp} takes two years to Prune");
    }

    // Page 3: n, shift-click 90210.
    assert!(app.select_adjacent_fleet(1));
    assert_eq!(selected_fleet_id(&app), Some(1));
    shift_click(&mut app, PLANET_90210);
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 4);

    // Page 4: Next three times to Stalwart Defender #5, shift-click Alexander.
    for _ in 0..3 {
        assert!(app.select_adjacent_fleet(1));
    }
    assert_eq!(selected_fleet_id(&app), Some(4));
    shift_click(&mut app, ALEXANDER);
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 5);

    // Page 5: n twice round to Armed Probe #1, Weapons, Done.
    assert!(app.select_adjacent_fleet(1));
    assert!(app.select_adjacent_fleet(1));
    assert_eq!(selected_fleet_id(&app), Some(0));
    app.open_research();
    app.research_dialog.as_mut().expect("the dialog").field = 1;
    app.research_ok();
    assert!(app.advance_tutor(), "Weapons chosen");
    assert_eq!(page(&app), 6, "the year's last page turned, ready for F9");

    // F9. Page 6 belongs to 2401, and the tutorial waits on it.
    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("a game").turn, 1);
    assert!(!app.advance_tutor());
    assert_eq!(page(&app), 6);

    // --- 2401 -------------------------------------------------------------
    // Page 6: Stove Top, Change, Factory, shift-Add twice, OK.
    app.select_object(ScanObject::Planet(STOVE_TOP));
    app.open_production();
    queue(
        &mut app,
        item::FACTORY,
        App::production_step(false, true),
        2,
        None,
    );
    app.production_ok();
    assert_eq!(
        app.game
            .as_ref()
            .expect("a game")
            .planets
            .iter()
            .find(|p| p.id == STOVE_TOP)
            .map(|p| (p.queue[0].item, p.queue[0].count)),
        Some((item::FACTORY, 20))
    );
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 7);
    app.generate_turn();

    // --- 2402 -------------------------------------------------------------
    // Page 7: Armed Probe #1 has reached Prune; five more stops.
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[0].orbiting,
        Some(u16::try_from(PRUNE).expect("a small id")),
        "two years to Prune, as the page said"
    );
    assert!(app.goto_fleet(0));
    for planet in [HIHO, NO_VACANCY, SLIME, WALLABY, OXYGEN] {
        shift_click(&mut app, planet);
    }
    assert!(!app.advance_tutor());
    assert_eq!(bold(&app), Some(7), "now Goto Long Range Scout #2");
    assert!(app.goto_fleet(1));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 8);

    // Page 8: four stops for the scout, then Goto Stalwart Defender #5.
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[1].orbiting,
        Some(u16::try_from(PLANET_90210).expect("a small id")),
        "the scout got to 90210 in two years"
    );
    for planet in [DWARTE, MOBIUS, CASTLE, MOHOLDI] {
        shift_click(&mut app, planet);
    }
    assert!(app.goto_fleet(4));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 9);

    // Page 9: five for the destroyer, then Prune in the Summary pane.
    for planet in [SHAGGY_DOG, SEA_SQUARED, RED_STORM, BLOOP, KALAMAZOO] {
        shift_click(&mut app, planet);
    }
    assert!(!app.advance_tutor());
    // "Read your next two messages" is a hint, not a gate, and it stays lit
    // here because this engine sends no arrival messages yet; the page is
    // done all the same once Prune is in the Summary pane.
    assert_eq!(bold(&app), Some(5));
    app.select_object(ScanObject::Planet(PRUNE));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 10);

    // Page 10: Cotton Picker #6 to Prune, Remote Mining.
    assert!(app.goto_fleet(5));
    shift_click(&mut app, PRUNE);
    assert!(app.set_waypoint_task(task::REMOTE_MINING));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 11);

    // Page 11: Alexander, then 90210, in the Summary pane.
    app.select_object(ScanObject::Planet(ALEXANDER));
    app.select_object(ScanObject::Planet(PLANET_90210));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 12);

    // Page 12: Santa Maria #3, Xfer 25kT of colonists, 90210, Colonize.
    assert!(app.goto_fleet(2));
    let santa_maria = app.selection.fleet.expect("selected");
    assert_eq!(app.transfer_cargo(santa_maria, 3, 250), 25, "a full hold");
    shift_click(&mut app, PLANET_90210);
    assert!(app.set_waypoint_task(task::COLONIZE));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 13, "the year is done");
    {
        let fleet = &app.game.as_ref().expect("a game").fleets[santa_maria];
        let warp = i32::from(fleet.waypoints[1].warp);
        assert!(
            warp * warp >= 58,
            "a colony ship spends fuel on speed: warp {warp} reaches 90210 in a year"
        );
    }
    app.generate_turn();

    // --- 2403 -------------------------------------------------------------
    // The year's news, in the order the original's own turn-3 file has it:
    // factories built at Stove Top and its queue emptied, the colony ship
    // broken up on landing, and 90210 taken. Fleet #3 is gone, its number
    // free for the next ship built.
    {
        use stars_core::message::id;
        let ids: Vec<u16> = app.messages().iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            vec![
                id::BUILT_FACTORIES,
                id::QUEUE_EMPTY,
                id::FLEET_DISMANTLED,
                id::COLONISTS_CONTROL
            ],
            "{ids:?}"
        );
        assert!(
            !app.game
                .as_ref()
                .expect("a game")
                .fleets
                .iter()
                .any(|f| f.owner == 0 && f.id == 2),
            "the colony ship was dismantled"
        );
    }

    // Page 13: filter the factory message, then thirty auto-build factories
    // at the top of Stove Top's queue.
    assert!(app.filter_message(stars_core::message::id::BUILT_FACTORIES, true));
    assert!(!app.advance_tutor());
    assert_eq!(bold(&app), Some(2), "now Goto Stove Top");
    app.select_object(ScanObject::Planet(STOVE_TOP));
    app.open_production();
    queue(
        &mut app,
        item::AUTO_FACTORY,
        App::production_step(false, true),
        3,
        None,
    );
    app.production_ok();
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 14);

    // Page 14: 90210 is ours now. Three factories, three mines, leftover
    // only; then fill Teamster #4 with colonists.
    app.select_object(ScanObject::Planet(PLANET_90210));
    assert_eq!(
        app.selected_planet().and_then(|p| p.owner),
        Some(0),
        "the colony ship arrived and 90210 is the player's"
    );
    app.open_production();
    assert!(
        app.production.is_some(),
        "the queue opens on a planet of ours"
    );
    queue(&mut app, item::FACTORY, 1, 3, None);
    queue(&mut app, item::MINE, 1, 3, Some(0));
    app.production.as_mut().expect("open").no_research = true;
    app.production_ok();
    assert!(!app.advance_tutor());
    assert_eq!(bold(&app), Some(5), "now Teamster #4");
    assert!(app.goto_fleet(3));
    let teamster = app.selection.fleet.expect("selected");
    assert_eq!(
        app.transfer_cargo(teamster, 3, 10_000),
        210,
        "a full hold is 210kT"
    );
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 15);

    // Page 15: 90210, Transport, QuikDrop; then Hiho in the Summary pane and
    // Armed Probe #1 in hand.
    shift_click(&mut app, PLANET_90210);
    assert!(app.set_waypoint_task(task::TRANSPORT));
    assert!(app.zip_quik(false));
    assert!(!app.advance_tutor());
    assert_eq!(bold(&app), Some(3));
    app.select_object(ScanObject::Planet(HIHO));
    assert!(app.goto_fleet(0));
    assert!(app.advance_tutor());
    // Page 16 asks for the Hiho waypoint to be deleted, and is skipped when
    // the probe has already reached Hiho and dropped it on its own.
    assert!(page(&app) == 16 || page(&app) == 17, "page {}", page(&app));
    if page(&app) == 16 {
        app.selection.waypoint = Some(1);
        assert!(app.delete_waypoint(1));
        assert!(app.advance_tutor());
    }
    assert_eq!(page(&app), 17, "2403 is done; page 17 waits for 2404");
    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("a game").turn, 4);
    assert!(!app.advance_tutor());
    assert_eq!(page(&app), 17);

    // --- 2404 -------------------------------------------------------------
    // Page 17: Shaggy Dog in the Summary pane, then Stalwart Defender #5's
    // waypoint there deleted, Dwarte, and Stove Top double-clicked.
    app.select_object(ScanObject::Planet(SHAGGY_DOG));
    assert!(app.goto_fleet(4));
    // Its first stop is Shaggy Dog, waypoint 1 of the six it was given.
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[app.selection.fleet.expect("selected")]
            .waypoints
            .len(),
        6
    );
    app.selection.waypoint = Some(1);
    assert!(app.delete_waypoint(1));
    assert!(!app.advance_tutor());
    assert_eq!(bold(&app), Some(5), "now Goto Dwarte");
    app.select_object(ScanObject::Planet(DWARTE));
    app.select_object(ScanObject::Planet(STOVE_TOP));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 18);

    // Page 18: a Santa Maria into Stove Top's queue. The queue holds the
    // factory the auto-build order left part-built and the order itself,
    // and the dialog opens on the factory — the last real entry — so the
    // colony ship goes in between: three entries, the ship second, which
    // is exactly what the page checks for.
    app.open_production();
    {
        let dialog = app.production.as_ref().expect("open");
        assert_eq!(dialog.queue.len(), 2, "{:?}", dialog.queue);
        assert_eq!(dialog.queue[0].item, item::FACTORY);
        assert!(dialog.queue[0].completion > 0, "part-built");
        assert_eq!(dialog.queue[1].item, item::AUTO_FACTORY);
        assert_eq!(dialog.queue_index, Some(0), "opens on the last real entry");
    }
    let santa_maria_design = app
        .production_inventory()
        .iter()
        .position(|row| row.ship && row.item == 2)
        .expect("the colony ship design is on offer");
    app.production.as_mut().expect("open").inventory_index = santa_maria_design;
    app.production_add(1);
    app.production_ok();
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 19, "2404 is done");
    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("a game").turn, 5);
    assert!(!app.advance_tutor());

    // --- 2405 -------------------------------------------------------------
    // Page 19: the new Santa Maria is fleet #3 again, filled, then the value
    // view and a shift-click on Shaggy Dog.
    assert!(app.goto_fleet(2), "the new colony ship took the old number");
    let santa_maria = app.selection.fleet.expect("selected");
    assert_eq!(app.transfer_cargo(santa_maria, 3, 250), 25);
    assert!(!app.advance_tutor());
    app.scan_view = stars_ui::ScanView::PlanetValue;
    shift_click(&mut app, SHAGGY_DOG);
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 20);

    // Page 20: Colonize; normal view; Teamster #4 home; 90210; and Armed
    // Probe #1's waypoint at No Vacancy deleted.
    assert!(app.set_waypoint_task(task::COLONIZE));
    app.scan_view = stars_ui::ScanView::Normal;
    assert!(app.goto_fleet(3));
    shift_click(&mut app, STOVE_TOP);
    {
        // The freighter kept its fuel: QuikDrop's fifth column, fuel, moves
        // nothing at a planet with no starbase. With the tank still full
        // and a dock waiting at home, the leg back is flown fast enough to
        // be there next year, which is what page 21 assumes.
        let game = app.game.as_ref().expect("a game");
        let f = &game.fleets[app.selection.fleet.expect("selected")];
        assert!(f.cargo.fuel > 300, "fuel {}", f.cargo.fuel);
        let warp = i32::from(f.waypoints[1].warp);
        assert!(warp * warp >= 58, "warp {warp} gets home in a year");
    }
    app.select_object(ScanObject::Planet(PLANET_90210));
    // The last rung is Armed Probe #1 having other than five orders, which
    // a probe that has already reached No Vacancy satisfies on its own.
    if !app.advance_tutor() {
        assert!(app.goto_fleet(0));
        app.selection.waypoint = Some(1);
        assert!(app.delete_waypoint(1));
        assert!(app.advance_tutor());
    }
    assert_eq!(page(&app), 21, "2405 is done");
    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("a game").turn, 6);
    assert!(!app.advance_tutor());

    // --- 2406 -------------------------------------------------------------
    // Page 21: Teamster #4 to Prune, Transport, QuikLoad, then Stove Top.
    assert!(app.goto_fleet(3));
    shift_click(&mut app, PRUNE);
    assert!(app.set_waypoint_task(task::TRANSPORT));
    assert!(app.zip_quik(true));
    assert!(!app.advance_tutor());
    shift_click(&mut app, STOVE_TOP);
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 22);

    // Page 22: QuikDrop at Stove Top, Repeat Orders, then another Santa
    // Maria in Stove Top's queue.
    assert!(app.zip_quik(false));
    let teamster = app.selection.fleet.expect("selected");
    assert!(app.set_repeat_orders(teamster, true));
    assert!(!app.advance_tutor());
    app.select_object(ScanObject::Planet(STOVE_TOP));
    app.open_production();
    app.production.as_mut().expect("open").inventory_index = santa_maria_design;
    app.production_add(1);
    app.production_ok();
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 23, "2406 is done");
    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("a game").turn, 7);
    assert!(!app.advance_tutor());

    // --- 2407 -------------------------------------------------------------
    // Page 23: the new Santa Maria #7, filled; the value view; Colonize at
    // Red Storm; three more Santa Marias; the normal view.
    assert!(app.goto_fleet(6), "Santa Maria #7 is fleet 6");
    let santa_maria = app.selection.fleet.expect("selected");
    assert_eq!(app.transfer_cargo(santa_maria, 3, 250), 25);
    app.scan_view = stars_ui::ScanView::PlanetValue;
    shift_click(&mut app, RED_STORM);
    assert!(app.set_waypoint_task(task::COLONIZE));
    assert!(!app.advance_tutor());
    app.select_object(ScanObject::Planet(STOVE_TOP));
    app.open_production();
    app.production.as_mut().expect("open").inventory_index = santa_maria_design;
    app.production_add(3);
    app.production_ok();
    app.scan_view = stars_ui::ScanView::Normal;
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 24);

    // Page 24: the mine-building message filtered; 90210's queue — empty
    // now its first factories and mines are up — gets ten auto-build
    // factories and ten auto-build mines; then the rest of the messages.
    assert!(app.filter_message(stars_core::message::id::BUILT_MINES, true));
    app.select_object(ScanObject::Planet(PLANET_90210));
    {
        let planet = app.selected_planet().expect("90210");
        assert_eq!(planet.factories, 3, "the three factories are built");
        assert_eq!(planet.mines, 3, "and the three mines");
        assert!(planet.queue.is_empty(), "{:?}", planet.queue);
    }
    app.open_production();
    queue(&mut app, item::AUTO_FACTORY, 10, 1, None);
    queue(&mut app, item::AUTO_MINE, 10, 1, Some(0));
    app.production_ok();
    assert!(!app.advance_tutor());
    while app.message_next(false).is_some() {
        app.show_next_message();
    }
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 25);

    // Page 25: the enemy scout in the Summary pane is a hint; two Armed
    // Probes at the foot of Stove Top's queue is the task.
    app.select_object(ScanObject::Planet(STOVE_TOP));
    app.open_production();
    let armed_probe_design = app
        .production_inventory()
        .iter()
        .position(|row| row.ship && row.item == 0)
        .expect("the scout design is on offer");
    // The dialog opens on the last real entry, the colony ships, so the
    // scouts go in behind them and ahead of the auto-build order.
    app.production.as_mut().expect("open").inventory_index = armed_probe_design;
    app.production_add(2);
    app.production_ok();
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 26, "2407 is done");
    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("a game").turn, 8);
    assert!(!app.advance_tutor());

    // --- 2408 -------------------------------------------------------------
    // Nine fleets: the five the game began with (the Santa Maria of 2406
    // has settled Shaggy Dog and been dismantled), the colony ship built in
    // 2407, and the three colony ships and two scouts built this year.
    {
        let game = app.game.as_ref().expect("a game");
        let mut ids: Vec<u16> = game
            .fleets
            .iter()
            .filter(|f| f.owner == 0)
            .map(|f| f.id)
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }
    // Page 26: Goto the three new colony ships, Split one off into Fleet
    // #10, load the two that remain, Colonize at Slime, then Split All.
    assert!(app.goto_fleet(7));
    let colony_ships = app.selection.fleet.expect("selected");
    assert!(
        app.split_fleet(colony_ships, 2, 1),
        "one Santa Maria into a fleet of its own"
    );
    assert_eq!(app.own_fleets().len(), 10);
    assert_eq!(app.transfer_cargo(colony_ships, 3, 250), 50, "two holds");
    shift_click(&mut app, SLIME);
    assert!(app.set_waypoint_task(task::COLONIZE));
    assert!(!app.advance_tutor());
    assert_eq!(
        app.split_all(colony_ships),
        1,
        "the other one goes to a fleet of its own"
    );
    assert_eq!(app.own_fleets().len(), 11);
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 27);

    // Page 27: the waypoint at Slime dragged to Sea Squared — the colonize
    // order goes with it — then the new scouts sent to Hiho.
    let sea_squared = at_planet(&app, SEA_SQUARED);
    assert!(app.move_waypoint(1, sea_squared.x, sea_squared.y, 20.0));
    assert!(!app.advance_tutor());
    assert!(app.goto_fleet(8), "Armed Probe #9");
    shift_click(&mut app, HIHO);
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 28);

    // Page 28: the unloading message filtered, Wallaby in the Summary pane,
    // then F5.
    assert!(app.filter_message(stars_core::message::id::HAS_UNLOADED, true));
    app.select_object(ScanObject::Planet(WALLABY));
    app.open_research();
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 29);

    // Page 29: research up to 30% and Done.
    app.research_dialog.as_mut().expect("the dialog").percent = 30;
    app.research_ok();
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 30, "2408 is done");
}

fn at_planet(app: &App, planet: i16) -> stars_core::movement::Point {
    let game = app.game.as_ref().expect("a game");
    game.planets
        .iter()
        .chain(game.known_planets.iter())
        .find(|p| p.id == planet)
        .and_then(|p| p.position)
        .expect("a placed planet")
}
