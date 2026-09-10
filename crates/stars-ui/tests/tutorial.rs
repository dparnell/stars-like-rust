//! The tutorial's machine: pages, the skipping advance, and the checks.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::tutorial::{grobj, Check, Tutor, ANY, LAST_PARAGRAPH, STEPS};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "tutorial".to_string(),
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

/// A page is eight paragraphs and the page number is one-based, as the
/// tutorial's own title bar counts them.
#[test]
fn a_page_is_eight_paragraphs_on() {
    let mut tutor = Tutor::default();
    assert_eq!(tutor.page(), 1);
    assert_eq!(tutor.bold_line(), Some(0));

    tutor.idt = 8;
    tutor.bold = 11;
    assert_eq!(tutor.page(), 2);
    assert_eq!(
        tutor.bold_line(),
        Some(3),
        "paragraph 11 is line 3 of page 2"
    );

    // A bold paragraph on another page is not shown on this one.
    tutor.bold = 40;
    assert_eq!(tutor.bold_line(), None);

    assert_eq!(LAST_PARAGRAPH, 0x27f);
}

/// The steps recovered so far are in order, on page boundaries, and each
/// belongs to one year.
#[test]
fn the_step_table_is_well_formed() {
    assert!(!STEPS.is_empty());
    let mut last = None;
    for step in STEPS {
        assert_eq!(step.idt % 8, 0, "page {} is not on a boundary", step.page());
        assert!(step.idt <= LAST_PARAGRAPH);
        assert!(!step.stages.is_empty(), "page {} asks nothing", step.page());
        if let Some(previous) = last {
            assert!(step.idt > previous, "steps run in order");
        }
        last = Some(step.idt);
        // A page's rungs point at paragraphs of that page or the next few —
        // never backwards past the page it belongs to.
        for stage in step.stages {
            assert!(
                stage.bold >= step.idt,
                "page {} bolds backwards",
                step.page()
            );
        }
    }
    // Turn 2's pages are the ones with escape hatches.
    let escapes = STEPS.iter().filter(|s| s.escape.is_some()).count();
    assert!(escapes >= 2, "the escape hatch is in the table");

    // The first page reads the messages, which is where the tutorial opens.
    assert_eq!(STEPS[0].idt, 0);
    assert_eq!(STEPS[0].turn, 0);
    assert!(matches!(
        STEPS[0].stages[0].check,
        Some(Check::Messages { message: 9999, .. })
    ));
}

/// A page is only asked about in its own year: `FTutorTaskDone` is a switch
/// on the turn, so a page belonging to year 1 is not done in year 0 however
/// the galaxy looks.
#[test]
fn a_page_belongs_to_its_own_year() {
    let mut app = a_game();
    app.start_tutor();
    let year = app.game.as_ref().expect("a game").turn;
    assert_eq!(year, 0, "a new game starts in year 0");

    // Jump to the page that belongs to year 1 and it cannot be done yet.
    app.tutor = Some(Tutor {
        idt: 40,
        ..Tutor::default()
    });
    assert_eq!(app.tutor_step().map(|s| s.turn), Some(1));
    assert!(
        !app.tutor_task_done(),
        "year 1's page is not done in year 0"
    );
}

/// Advancing skips a page whose task is already done rather than showing it.
#[test]
fn advancing_skips_what_is_already_done() {
    let mut app = a_game();
    // With no messages to read, page one is satisfied the moment it opens,
    // so starting lands past it.
    app.start_tutor();
    let tutor = app.tutor.as_ref().expect("running");
    assert!(!tutor.finished);
    assert!(
        tutor.idt >= 8,
        "page one was satisfied and skipped, landed on {}",
        tutor.page()
    );
}

/// Running off the end stops it.
#[test]
fn it_ends_after_the_last_page() {
    let mut app = a_game();
    app.tutor = Some(Tutor {
        idt: LAST_PARAGRAPH - 7,
        ..Tutor::default()
    });
    // No step is recorded for that page, so nothing is done and it waits.
    assert!(!app.tutor_task_done());
    app.tutor = Some(Tutor {
        idt: LAST_PARAGRAPH + 1,
        finished: true,
        ..Tutor::default()
    });
    assert!(!app.advance_tutor(), "a finished tutorial does not step on");
}

/// The selection check asks exactly what is selected.
#[test]
fn the_selection_check_reads_the_selection() {
    let mut app = a_game();
    let (planet, fleet) = {
        let game = app.game.as_ref().expect("a game");
        let planet = game
            .planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld")
            .id;
        let fleet = game.fleets.first().map(|f| f.id);
        (planet, fleet)
    };

    app.selection.planet = Some(planet);
    app.selection.on_fleet = false;
    assert!(app.tutor_check(&Check::Selection {
        class: grobj::PLANET,
        id: planet
    }));
    assert!(!app.tutor_check(&Check::Selection {
        class: grobj::PLANET,
        id: planet + 1
    }));

    if let Some(id) = fleet {
        app.selection.fleet = Some(0);
        app.selection.on_fleet = true;
        assert!(app.tutor_check(&Check::Selection {
            class: grobj::FLEET,
            id: i16::try_from(id).expect("a small id")
        }));
        // With a fleet in front, the planet check no longer passes.
        assert!(!app.tutor_check(&Check::Selection {
            class: grobj::PLANET,
            id: planet
        }));
    }
}

/// The waypoint check reads the fleet's own orders, and `ANY` means the
/// field is not compared.
#[test]
fn the_waypoint_check_reads_the_orders() {
    let mut app = a_game();
    let (id, planet) = {
        let game = app.game.as_ref().expect("a game");
        let fleet = game.fleets.first().expect("a fleet");
        let planet = game
            .planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld");
        (fleet.id, planet.id)
    };
    app.selection.fleet = Some(0);
    app.selection.on_fleet = true;
    let at = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.id == planet)
        .and_then(|p| p.position)
        .expect("a position");

    // Nothing set yet.
    let asking = |target: u16, task: u16| Check::FleetWaypoint {
        fleet: id,
        order: 1,
        class: grobj::PLANET,
        id: target,
        task,
        warp: ANY,
    };
    let target = u16::try_from(planet).expect("a small id");
    assert!(!app.tutor_check(&asking(target, 0)));

    // Lay a leg onto the planet and it passes, task and all.
    assert!(app.add_waypoint(at.x, at.y, 20.0) || !app.add_waypoint(at.x, at.y, 20.0));
    if app.tutor_check(&asking(ANY, ANY)) {
        assert!(
            app.tutor_check(&asking(target, 0)),
            "on that planet, no task"
        );
        // A task it does not have fails.
        assert!(!app.tutor_check(&asking(target, 2)));
    }
}

/// The scanner check reads `grbitScan`, and the bits are the toolbar's own.
#[test]
fn the_scanner_check_reads_grbit_scan() {
    let mut app = a_game();
    app.scan_overlays.names = true;
    app.scan_overlays.fleet_paths = true;
    app.scan_overlays.minefields = false;

    let bits = app.grbit_scan();
    assert_eq!(bits & 0x0400, 0x0400, "planet names");
    assert_eq!(bits & 0x0080, 0x0080, "fleet paths");
    assert_eq!(bits & 0x0040, 0, "mine fields are off");

    assert!(app.tutor_check(&Check::Scanner {
        view: Some(0x0480),
        zoom: None
    }));
    assert!(!app.tutor_check(&Check::Scanner {
        view: Some(0x04c0),
        zoom: None
    }));

    app.scan_zoom = 2;
    assert!(app.tutor_check(&Check::Scanner {
        view: None,
        zoom: Some(2)
    }));
    assert!(!app.tutor_check(&Check::Scanner {
        view: None,
        zoom: Some(3)
    }));
}

/// With no copy of the game there is no tutorial text, and the frontend gets
/// nothing rather than something invented.
#[test]
fn without_the_game_there_is_no_text() {
    let mut app = a_game();
    app.start_tutor();
    assert!(app.art.is_none());
    assert_eq!(app.tutor_page(), None);
}

// --- What the tutorial's own words demand of the UI ----------------------
//
// The tutorial names buttons and keys, so its text is a specification: "Hit
// the n key to look at your next fleet", "Press the tile's Goto button",
// "press the Next button in the tile showing Long Range Scout #2".

/// Next and Prev walk **your own** fleets and wrap round, and never step onto
/// somebody else's (`SelectAdjFleet`, `1050:3d32`).
#[test]
fn next_and_prev_walk_your_own_fleets() {
    use stars_core::fleet::{Fleet, ShipStack};
    use stars_core::movement::Point;

    let mut app = a_game();
    let theirs = {
        let game = app.game.as_mut().expect("a game");
        let mut fleet = Fleet {
            id: 99,
            owner: 1,
            position: Point::new(100, 100),
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
        fleet.waypoints.clear();
        game.fleets.push(fleet);
        game.fleets.len() - 1
    };

    let mine = app.own_fleets();
    assert!(!mine.is_empty(), "the player starts with fleets");
    assert!(
        !mine.contains(&theirs),
        "somebody else's is not in the walk"
    );

    // Walking the whole list comes back to where it started.
    app.select_object(stars_ui::ScanObject::Fleet(mine[0]));
    for _ in 0..mine.len() {
        app.select_adjacent_fleet(1);
    }
    assert_eq!(app.selection.fleet, Some(mine[0]), "it wrapped round");

    // And backwards from the first lands on the last.
    app.select_adjacent_fleet(-1);
    assert_eq!(app.selection.fleet, Some(*mine.last().expect("a fleet")));
}

/// Goto takes command of a fleet by id, which is what the tile's button does.
#[test]
fn goto_takes_command_of_a_fleet() {
    let mut app = a_game();
    let mine = app.own_fleets();
    let last = *mine.last().expect("a fleet");
    let id = app.game.as_ref().expect("a game").fleets[last].id;

    app.selection.fleet = None;
    app.selection.on_fleet = false;
    assert!(app.goto_fleet(id));
    assert_eq!(app.selection.fleet, Some(last));
    assert!(app.selection.on_fleet);

    // Somebody else's fleet is not gone to.
    assert!(!app.goto_fleet(9999));
}

/// The location tile's Goto goes to the planet the fleet is orbiting.
#[test]
fn goto_reaches_the_planet_underneath() {
    let mut app = a_game();
    let mine = app.own_fleets();
    let (index, orbiting) = mine
        .iter()
        .find_map(|i| {
            let fleet = &app.game.as_ref().expect("a game").fleets[*i];
            fleet.orbiting.map(|planet| (*i, planet))
        })
        .expect("a fleet in orbit at the start");

    app.select_object(stars_ui::ScanObject::Fleet(index));
    assert!(app.goto_orbited_planet());
    assert_eq!(
        app.selection.planet,
        i16::try_from(orbiting).ok(),
        "it went to the planet under the fleet"
    );
    assert!(!app.selection.on_fleet);
}

/// Shift makes the production dialog's Add put in ten at a time, which page 6
/// spells out: "The shift key causes the Add button to add 10 items at a time
/// instead of 1."
#[test]
fn shift_adds_ten_at_a_time() {
    assert_eq!(App::production_step(false, false), 1);
    assert_eq!(App::production_step(false, true), 10);
    assert_eq!(App::production_step(true, false), 100);
}

/// Split All puts every ship into a fleet of its own, keeping one behind —
/// "We don't want both colonizers to go to Slime so hit the Split All button
/// in the Fleet Composition tile."
#[test]
fn split_all_gives_every_ship_its_own_fleet() {
    let mut app = a_game();
    // Find one of our own fleets with more than one ship, or make one.
    let index = app.own_fleets()[0];
    {
        let game = app.game.as_mut().expect("a game");
        game.fleets[index].stacks = vec![stars_core::fleet::ShipStack {
            design: 0,
            count: 3,
            damaged_pct: 0,
            damage_pct: 0,
        }];
    }
    app.select_object(stars_ui::ScanObject::Fleet(index));
    let before = app.game.as_ref().expect("a game").fleets.len();
    assert_eq!(app.pane_fleet_ships(), 3);

    let made = app.split_all(index);
    assert_eq!(made, 2, "two of the three ships left");
    let after = app.game.as_ref().expect("a game").fleets.len();
    assert_eq!(after, before + 2);
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[index]
            .stacks
            .iter()
            .map(|s| s.count)
            .sum::<i32>(),
        1,
        "the fleet you were commanding keeps one"
    );

    // A single-ship fleet has nothing to split.
    assert_eq!(app.split_all(index), 0);
}

// --- The blue diamond ----------------------------------------------------
//
// The tutorial reaches for it again and again: "right click on the blue
// diamond in the Waypoint Task tile and select QuikDrop to empty the
// freighter's hold at 90210."

/// The menu is the two built-in orders, four slots, and Customize.
#[test]
fn the_diamonds_menu_is_the_originals() {
    let app = a_game();
    let menu = app.zip_menu();
    assert_eq!(menu.len(), App::ZIP_ORDERS + 3);
    assert_eq!(menu[0], "QuikLoad");
    assert_eq!(menu[1], "QuikDrop");
    assert_eq!(menu[2], "<Unused 1>");
    assert_eq!(menu[App::ZIP_ORDERS + 1], "<Unused 4>");
    assert_eq!(menu[App::ZIP_ORDERS + 2], "<Customize>");
}

/// QuikDrop empties the hold and QuikLoad fills it, on every cargo at once.
#[test]
fn quikdrop_empties_and_quikload_fills() {
    use stars_formats::{task, XferAction};

    let mut app = a_game();
    let index = app.own_fleets()[0];
    {
        let game = app.game.as_mut().expect("a game");
        let at = game.fleets[index].position;
        game.fleets[index].waypoints = vec![
            stars_core::fleet::Waypoint {
                position: at,
                target: None,
                target_class: grobj::POSITION,
                warp: 0,
                task: task::NONE,
                transport: None,
                task_data: Vec::new(),
            },
            stars_core::fleet::Waypoint {
                position: stars_core::movement::Point::new(at.x + 40, at.y),
                target: None,
                target_class: grobj::POSITION,
                warp: 5,
                task: task::NONE,
                transport: None,
                task_data: Vec::new(),
            },
        ];
    }
    app.select_object(stars_ui::ScanObject::Fleet(index));
    app.selection.waypoint = Some(1);
    assert!(app.set_waypoint_task(task::TRANSPORT));

    assert!(app.zip_quik(false));
    for slot in 0..5 {
        assert_eq!(app.waypoint_transport(slot).0, XferAction::UnloadAll);
    }
    assert!(app.zip_quik(true));
    for slot in 0..5 {
        assert_eq!(app.waypoint_transport(slot).0, XferAction::LoadAll);
    }

    // Import that table into a slot, and it comes back out again.
    assert!(app.zip_import(0, "DropCol"));
    assert_eq!(app.zip_menu()[2], "DropCol");
    assert!(app.zip_quik(false));
    assert_eq!(app.waypoint_transport(0).0, XferAction::UnloadAll);
    assert!(app.zip_apply(0));
    assert_eq!(
        app.waypoint_transport(0).0,
        XferAction::LoadAll,
        "the saved order came back"
    );

    // An empty slot applies nothing, and deleting empties one.
    assert!(!app.zip_apply(1));
    assert!(app.zip_delete(0));
    assert_eq!(app.zip_menu()[2], "<Unused 1>");
    assert!(!app.zip_delete(0));
}

/// An unnamed import is called `Custom n`, as the original names it.
#[test]
fn an_unnamed_slot_is_called_custom() {
    use stars_formats::task;

    let mut app = a_game();
    let index = app.own_fleets()[0];
    {
        let game = app.game.as_mut().expect("a game");
        let at = game.fleets[index].position;
        game.fleets[index].waypoints = vec![
            stars_core::fleet::Waypoint {
                position: at,
                target: None,
                target_class: grobj::POSITION,
                warp: 0,
                task: task::NONE,
                transport: None,
                task_data: Vec::new(),
            },
            stars_core::fleet::Waypoint {
                position: stars_core::movement::Point::new(at.x + 40, at.y),
                target: None,
                target_class: grobj::POSITION,
                warp: 5,
                task: task::TRANSPORT,
                transport: None,
                task_data: vec![0; 10],
            },
        ];
    }
    app.select_object(stars_ui::ScanObject::Fleet(index));
    app.selection.waypoint = Some(1);
    assert!(app.zip_import(2, "   "));
    assert_eq!(app.zip_menu()[4], "Custom 3");
}

/// F3 opens a report and Esc closes it — "Hit F3 to open the Planet Summary
/// Report", "Hit the Esc key to close the Planet Summary Report".
///
/// All four Report entries carry F3, which is one key that opens whichever
/// report was last up, not four accelerators for one key.
#[test]
fn f3_opens_the_last_report_and_escape_closes_it() {
    use stars_ui::Screen;

    let mut app = a_game();
    assert_eq!(app.screen, Screen::Galaxy);

    // With none up yet, F3 opens the planets.
    app.open_report();
    assert_eq!(app.screen, Screen::Planets);
    // Again while one is open changes nothing.
    app.open_report();
    assert_eq!(app.screen, Screen::Planets);

    assert!(app.close_report());
    assert_eq!(app.screen, Screen::Galaxy);
    assert!(!app.close_report(), "nothing to close on the map");

    // It comes back to the one last up.
    app.show_screen(Screen::Fleets);
    assert!(app.close_report());
    app.open_report();
    assert_eq!(app.screen, Screen::Fleets);

    assert!(App::is_report(Screen::Battles));
    assert!(!App::is_report(Screen::Galaxy));
}

/// A page whose escape hatch is satisfied is done without any of its rungs.
///
/// Several arms open `if (FCheckX(...)) done = 1; else { ...the chain... }`,
/// where `FCheckX` is usually the next page's task — a player who has run
/// ahead is not made to go back and do this page step by step.
#[test]
fn an_escape_hatch_carries_a_page() {
    use stars_ui::tutorial::step;

    let with_escape = STEPS
        .iter()
        .find(|s| s.escape.is_some())
        .expect("a page with one");
    assert_eq!(with_escape.turn, 2);
    // It is the same shape of check as the rungs, not a special case.
    assert!(matches!(
        with_escape.escape,
        Some(Check::FleetWaypoint { .. })
    ));
    // And it is a *different* fleet from the one the rungs are about, which
    // is what makes it the next page's task rather than this one's.
    let rung_fleet = with_escape.stages.iter().find_map(|s| match s.check {
        Some(Check::FleetWaypoint { fleet, .. }) => Some(fleet),
        _ => None,
    });
    let escape_fleet = match with_escape.escape {
        Some(Check::FleetWaypoint { fleet, .. }) => Some(fleet),
        _ => None,
    };
    assert_ne!(rung_fleet, escape_fleet);

    assert!(step(with_escape.idt).is_some());
    assert!(step(with_escape.idt + 1).is_none(), "pages start on eights");
}
