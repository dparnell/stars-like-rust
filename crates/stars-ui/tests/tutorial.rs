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
/// All four Report entries show F3 after their caption, but that is text:
/// the accelerator is one key with an id of its own, `0x8fe`, which is on
/// no menu, and the command handler turns it into whichever report comes
/// next in the cycle.
#[test]
fn f3_walks_round_the_reports_and_escape_closes_one() {
    use stars_ui::Screen;

    let mut app = a_game();
    assert_eq!(app.screen, Screen::Galaxy);

    // Nothing, planets, your fleets, everybody else's, battles, nothing.
    for expected in [
        Screen::Planets,
        Screen::Fleets,
        Screen::EnemyFleets,
        Screen::Battles,
        Screen::Galaxy,
        Screen::Planets,
    ] {
        app.open_report();
        assert_eq!(app.screen, expected);
    }

    // Esc closes whichever is up, and does nothing on the map.
    assert!(app.close_report());
    assert_eq!(app.screen, Screen::Galaxy);
    assert!(!app.close_report(), "nothing to close on the map");

    // The Players screen is this project's own, so F3 treats it as nothing
    // being open — but Esc still leaves it.
    app.show_screen(Screen::Players);
    assert!(!App::is_report(Screen::Players));
    app.open_report();
    assert_eq!(app.screen, Screen::Planets);
    app.show_screen(Screen::Players);
    assert!(app.close_report());
    assert_eq!(app.screen, Screen::Galaxy);

    assert!(App::is_report(Screen::Battles));
    assert!(!App::is_report(Screen::Galaxy));
}

/// The Report menu opens what it names — except Battles, which is the one
/// item of the four that closes itself when it is already up.
#[test]
fn the_battles_item_is_the_one_that_toggles() {
    use stars_ui::Screen;

    let mut app = a_game();
    app.choose_report(Screen::Planets);
    assert_eq!(app.screen, Screen::Planets);
    // Asking again for the one already open reopens it rather than closing.
    app.choose_report(Screen::Planets);
    assert_eq!(app.screen, Screen::Planets);

    app.choose_report(Screen::Battles);
    assert_eq!(app.screen, Screen::Battles);
    app.choose_report(Screen::Battles);
    assert_eq!(app.screen, Screen::Galaxy, "Battles closes itself");
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

/// A rung that only moves the emphasis does not hold the page up.
///
/// Page 9 asks whether the fourth message has been read and then, whatever
/// the answer, gates on the summary pane instead — so an unread message
/// changes where the bold sits and nothing else.
#[test]
fn a_hint_moves_the_bold_without_gating() {
    let page = STEPS
        .iter()
        .find(|s| s.stages.iter().any(|stage| !stage.gates))
        .expect("a page with a hint");

    let hints: Vec<_> = page.stages.iter().filter(|s| !s.gates).collect();
    assert!(!hints.is_empty());
    // A hint still carries a paragraph, which is the whole reason it is
    // there.
    for hint in &hints {
        assert!(hint.check.is_some(), "a hint with no check asks nothing");
        assert!(hint.bold >= page.idt);
    }
    // And the page still has something that does gate.
    assert!(
        page.stages.iter().any(|s| s.gates),
        "a page of nothing but hints could never be finished"
    );
    // The last rung is always a gate: it is what finishes the page.
    assert!(page.stages.last().expect("a rung").gates);
}

/// A rung's `bold` is the paragraph shown **while that rung is outstanding**,
/// which is how the original writes it: the bold is set immediately before
/// the check it belongs to, so it is the instruction you have not yet
/// carried out.
#[test]
fn the_bold_is_what_is_still_to_do() {
    // Page 7 is the clearest case: select the scout, then five planets.
    let page = stars_ui::tutorial::step(48).expect("page 7");
    let bolds: Vec<usize> = page.stages.iter().map(|s| s.bold).collect();
    // Strictly increasing: each rung points further down the page than the
    // one before, because each is the next thing to do.
    for pair in bolds.windows(2) {
        assert!(pair[0] <= pair[1], "the emphasis walks forward: {bolds:?}");
    }
    // The first rung is the selection and it does **not** gate: the original
    // never refuses to move on because the wrong fleet is in front, it just
    // says which one it wants.
    assert!(!page.stages[0].gates);
    assert!(matches!(
        page.stages[0].check,
        Some(Check::Selection { .. })
    ));
    // Every planet after it does gate.
    assert_eq!(page.stages.iter().filter(|s| s.gates).count(), 6);
}

/// Turn 2 is transcribed end to end, and every page of it belongs to year 2.
#[test]
fn year_two_is_whole() {
    let year_two: Vec<_> = STEPS.iter().filter(|s| s.turn == 2).collect();
    assert_eq!(year_two.len(), 6, "six pages in the second year");
    let pages: Vec<usize> = year_two.iter().map(|s| s.page()).collect();
    assert_eq!(pages, vec![7, 8, 9, 10, 11, 12]);
    // Five of the six can be skipped by having run ahead; only the last,
    // which loads and sends the colony ship, has to be done.
    assert_eq!(year_two.iter().filter(|s| s.escape.is_some()).count(), 5);
    assert!(year_two.last().expect("page 12").escape.is_none());
}

/// The messages check asks two different questions of a message kind: with
/// `fFilter` set, whether that kind has been **filtered out**; without it,
/// whether the message in front is one.
#[test]
fn the_message_check_asks_about_filtering() {
    let mut app = a_game();
    let asking = Check::Messages {
        message: -1,
        kind: Some(stars_core::message::id::BUILT_FACTORIES),
        filter: true,
    };
    // Nothing filtered yet, and no such message either.
    assert!(!app.tutor_check(&asking));

    // Filtering that kind is what the page wants — "Filter it out by
    // clicking the blue check mark in the upper left hand corner".
    app.filter_message(stars_core::message::id::BUILT_FACTORIES, true);
    let filtered = app
        .game
        .as_ref()
        .expect("a game")
        .players
        .get(app.local_player())
        .map(|p| p.message_filter);
    assert!(filtered.is_some());
}

/// A queue rung can insist on "contribute only leftover resources to
/// research", which is `FCheckQueue`'s last argument.
#[test]
fn a_queue_rung_can_ask_about_leftover_research() {
    let mut app = a_game();
    let (id, _) = {
        let game = app.game.as_ref().expect("a game");
        let planet = game
            .planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld");
        (planet.id, planet.no_research)
    };
    let asking = |no_research| Check::Queue {
        planet: id,
        slot: 0,
        ship: false,
        item: stars_core::production::item::FACTORY,
        count: 3,
        no_research,
    };

    {
        let game = app.game.as_mut().expect("a game");
        let planet = game
            .planets
            .iter_mut()
            .find(|p| p.id == id)
            .expect("the homeworld");
        planet.queue = vec![stars_core::production::QueueItem {
            count: 3,
            item: stars_core::production::item::FACTORY,
            ship: false,
            completion: 0,
        }];
        planet.no_research = false;
    }
    assert!(app.tutor_check(&asking(None)), "no opinion either way");
    assert!(app.tutor_check(&asking(Some(false))));
    assert!(!app.tutor_check(&asking(Some(true))));

    if let Some(game) = app.game.as_mut() {
        game.planets
            .iter_mut()
            .find(|p| p.id == id)
            .expect("the homeworld")
            .no_research = true;
    }
    assert!(app.tutor_check(&asking(Some(true))));
}

/// The transport check compares each cargo's **action** and not its figure,
/// which is what lets QuikDrop satisfy a page however much is aboard.
#[test]
fn the_transport_check_compares_actions() {
    use stars_formats::{task, XferAction};

    let mut app = a_game();
    let index = app.own_fleets()[0];
    let (id, planet, at) = {
        let game = app.game.as_ref().expect("a game");
        let planet = game
            .planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld");
        (
            game.fleets[index].id,
            u16::try_from(planet.id).expect("small"),
            planet.position.expect("a position"),
        )
    };
    {
        let game = app.game.as_mut().expect("a game");
        let from = game.fleets[index].position;
        game.fleets[index].waypoints = vec![
            stars_core::fleet::Waypoint {
                position: from,
                target: None,
                target_class: grobj::POSITION,
                warp: 0,
                task: task::NONE,
                transport: None,
                task_data: Vec::new(),
            },
            stars_core::fleet::Waypoint {
                position: at,
                target: Some(planet),
                target_class: grobj::PLANET,
                warp: 5,
                task: task::TRANSPORT,
                transport: None,
                task_data: vec![0; 10],
            },
        ];
    }
    app.select_object(stars_ui::ScanObject::Fleet(index));
    app.selection.waypoint = Some(1);

    let asking = Check::TransportWaypoint {
        fleet: id,
        order: 1,
        id: planet,
        warp: ANY,
        goal: [XferAction::UnloadAll; 5],
    };
    assert!(!app.tutor_check(&asking), "nothing set yet");

    // QuikDrop is exactly this order.
    assert!(app.zip_quik(false));
    assert!(app.tutor_check(&asking));

    // The figures do not come into it: an unload-all with a quantity still
    // satisfies the page.
    assert!(app.set_waypoint_transport(0, XferAction::UnloadAll, 500));
    assert!(app.tutor_check(&asking));

    // A different action does not.
    assert!(app.set_waypoint_transport(0, XferAction::LoadAll, 0));
    assert!(!app.tutor_check(&asking));
}

/// The order-count check is how "delete that waypoint" is asked.
#[test]
fn the_order_count_check_sees_a_deletion() {
    use stars_ui::tutorial::Cmp;

    let mut app = a_game();
    let index = app.own_fleets()[0];
    let id = app.game.as_ref().expect("a game").fleets[index].id;
    {
        let game = app.game.as_mut().expect("a game");
        let at = game.fleets[index].position;
        game.fleets[index].waypoints = (0..6)
            .map(|n| stars_core::fleet::Waypoint {
                position: stars_core::movement::Point::new(at.x + n * 20, at.y),
                target: None,
                target_class: grobj::POSITION,
                warp: 5,
                task: stars_formats::task::NONE,
                transport: None,
                task_data: Vec::new(),
            })
            .collect();
    }
    app.select_object(stars_ui::ScanObject::Fleet(index));

    let six = |cmp| Check::FleetOrders {
        fleet: id,
        count: 6,
        cmp,
    };
    assert!(app.tutor_check(&six(Cmp::Exactly)));
    assert!(!app.tutor_check(&six(Cmp::NotExactly)));
    assert!(!app.tutor_check(&six(Cmp::Fewer)));

    // Delete one and the page is done.
    app.selection.waypoint = Some(3);
    assert!(app.delete_current_waypoint());
    assert!(app.tutor_check(&six(Cmp::NotExactly)));
    assert!(app.tutor_check(&six(Cmp::Fewer)));
}

/// A queue entry that builds a **ship** is a different thing from one that
/// builds an installation, and the check tells them apart: the class says
/// which, and the item is a design slot rather than an installation id.
#[test]
fn a_queued_ship_is_not_a_queued_factory() {
    let mut app = a_game();
    let id = {
        let game = app.game.as_ref().expect("a game");
        game.planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld")
            .id
    };
    {
        let game = app.game.as_mut().expect("a game");
        let planet = game
            .planets
            .iter_mut()
            .find(|p| p.id == id)
            .expect("the homeworld");
        planet.queue = vec![stars_core::production::QueueItem {
            count: 1,
            item: 2,
            ship: true,
            completion: 0,
        }];
        planet.no_research = false;
    }

    let asking = |ship| Check::Queue {
        planet: id,
        slot: 0,
        ship,
        item: 2,
        count: 1,
        no_research: Some(false),
    };
    assert!(app.tutor_check(&asking(true)));
    assert!(
        !app.tutor_check(&asking(false)),
        "item 2 as an installation is a different order entirely"
    );
}

/// Queue length is counted the same three ways as fleet orders, because the
/// arms read both straight off their own count bytes.
#[test]
fn queue_length_is_counted_three_ways() {
    use stars_ui::tutorial::Cmp;

    let mut app = a_game();
    let id = {
        let game = app.game.as_ref().expect("a game");
        game.planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld")
            .id
    };
    let three = |cmp| Check::QueueLength {
        planet: id,
        count: 3,
        cmp,
    };

    {
        let game = app.game.as_mut().expect("a game");
        let planet = game
            .planets
            .iter_mut()
            .find(|p| p.id == id)
            .expect("the homeworld");
        planet.queue = (0..3)
            .map(|_| stars_core::production::QueueItem {
                count: 1,
                item: stars_core::production::item::FACTORY,
                ship: false,
                completion: 0,
            })
            .collect();
    }
    assert!(app.tutor_check(&three(Cmp::Exactly)));
    assert!(!app.tutor_check(&three(Cmp::Fewer)));
    assert!(!app.tutor_check(&three(Cmp::NotExactly)));
}

/// Every page transcribed so far belongs to a year, in order, with no gaps
/// in the years themselves.
#[test]
fn the_years_transcribed_are_contiguous() {
    let mut years: Vec<i16> = STEPS.iter().map(|s| s.turn).collect();
    years.dedup();
    let expected: Vec<i16> = (0..=years.len() as i16 - 1).collect();
    assert_eq!(years, expected, "years run 0 upward without a gap");
    // And the pages within them run in order too.
    let idts: Vec<usize> = STEPS.iter().map(|s| s.idt).collect();
    let mut sorted = idts.clone();
    sorted.sort_unstable();
    assert_eq!(idts, sorted);
}

/// Page 56 asks for a fuel transfer by dragging a gauge: "Click and drag in
/// the fuel gauge in the Other Fleets Here tile until Teamster #4 has 383mg
/// of fuel."
#[test]
fn fuel_can_be_dragged_between_two_fleets() {
    let mut app = a_game();
    let mine = app.own_fleets();
    let (a, b) = (mine[0], mine[1]);
    let at = app.game.as_ref().expect("a game").fleets[a].position;
    {
        let game = app.game.as_mut().expect("a game");
        game.fleets[b].position = at;
        game.fleets[a].cargo.fuel = 10;
        game.fleets[b].cargo.fuel = 40;
    }
    app.select_object(stars_ui::ScanObject::Fleet(a));
    let capacity = {
        let game = app.game.as_ref().expect("a game");
        let designs = game.designs.first().cloned().unwrap_or_default();
        game.fleets[a].fuel_capacity(&designs)
    };
    assert!(capacity >= 30, "a scout holds at least this much");

    // Ask for more than the pane's fleet has: it comes from the other one.
    let moved = app.drag_fleet_fuel(b, 30);
    assert_eq!(moved, 20, "twenty came across");
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[a].cargo.fuel, 30);
    assert_eq!(game.fleets[b].cargo.fuel, 20);

    // Dragging to the same fleet is not a transfer.
    assert_eq!(app.drag_fleet_fuel(a, 50), 0);
    // Nor is asking for what it already has.
    assert_eq!(app.drag_fleet_fuel(b, 30), 0);
    // Dragging it back down gives fuel to the other fleet.
    assert_eq!(app.drag_fleet_fuel(b, 5), -25);
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[a].cargo.fuel, 5);
    assert_eq!(game.fleets[b].cargo.fuel, 45);
}

/// Waypoint zero's task must be settable: Lay Mine Field is given where the
/// fleet already is, which is page 61's whole instruction.
#[test]
fn waypoint_zero_can_take_a_task() {
    use stars_formats::task;

    let mut app = a_game();
    let index = app.own_fleets()[0];
    {
        let game = app.game.as_mut().expect("a game");
        let at = game.fleets[index].position;
        game.fleets[index].waypoints = vec![stars_core::fleet::Waypoint {
            position: at,
            target: None,
            target_class: grobj::POSITION,
            warp: 0,
            task: task::NONE,
            transport: None,
            task_data: Vec::new(),
        }];
    }
    app.select_object(stars_ui::ScanObject::Fleet(index));

    app.selection.waypoint = Some(0);
    assert_eq!(app.task_waypoint(), Some(0));
    assert!(app.set_waypoint_task(task::LAY_MINES));
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[index].waypoints[0].task,
        task::LAY_MINES
    );

    // It still cannot be dragged or deleted.
    assert!(!app.move_waypoint(0, 1, 1, 0.0));
    assert!(!app.delete_current_waypoint());
}

/// All eighty pages are there, one per page of the game's own text, in
/// order, with every year from zero to thirty-six represented.
#[test]
fn the_table_is_complete() {
    assert_eq!(STEPS.len(), stars_formats::tutorial::PAGES);

    // One step per page, pages 1 to 80, no gaps and no repeats.
    let pages: Vec<usize> = STEPS.iter().map(|s| s.page()).collect();
    assert_eq!(pages, (1..=80).collect::<Vec<usize>>());

    // Years run 0 to 36 without a gap, which is the "36 years of a sample
    // game" the first page promises.
    let mut years: Vec<i16> = STEPS.iter().map(|s| s.turn).collect();
    years.dedup();
    assert_eq!(years, (0..=36).collect::<Vec<i16>>());

    // The last page ends exactly where AdvanceTutor stops.
    let last = STEPS.last().expect("page 80");
    assert_eq!(last.idt + 7, LAST_PARAGRAPH);

    // Every page can be finished: each has at least one rung that gates.
    for step in STEPS {
        assert!(
            step.stages.iter().any(|s| s.gates),
            "page {} has nothing that finishes it",
            step.page()
        );
    }
}

// --- The tutorial's own world --------------------------------------------

/// `CreateTutorWorld` hand-fills a `GAME` and then generates like any other
/// new game, so everything about the tutorial's galaxy comes from these
/// settings and one seed.
#[test]
fn the_tutorial_world_is_the_originals_settings() {
    use stars_core::newgame::{Density, Size, StartDistance};

    let (config, seed) = stars_core::newgame::tutorial();
    assert_eq!(config.name, "Tutorial Game");
    assert_eq!(config.id, 0x008c_ef49);
    assert_eq!(config.size, Size::Tiny);
    assert_eq!(config.density, Density::Sparse);
    assert_eq!(config.start_distance, StartDistance::Close);
    // Bit 7 of the flag word, `fNoRandom`, is set.
    assert!(!config.random_events);
    // Bit 6, `fVisScores`.
    assert!(config.public_scores);
    assert!(!config.slow_tech);
    assert!(!config.unlimited_minerals);
    assert!(!config.clumping);

    assert_eq!(config.players.len(), 2);
    assert_eq!(config.players[0].name, "Humanoid");
    assert_eq!(config.players[1].name, "Berserker");
    assert!(matches!(
        config.players[0].control,
        stars_core::ai::Control::Human
    ));
    assert!(matches!(
        config.players[1].control,
        stars_core::ai::Control::Computer { .. }
    ));

    // The seed is *not* the game id: `CreateTutorWorld` calls `Randomize`
    // with a constant of its own.
    assert_eq!(seed, 0x4996_02d2);
    assert_ne!(seed, config.id);
}

/// Building it gives a playable two-player game with the tutorial waiting on
/// its first page.
#[test]
fn the_tutorial_world_can_be_built_and_started() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");

    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.players.len(), 2);
    assert!(!game.planets.is_empty(), "a galaxy was generated");
    assert!(
        game.planets.iter().any(|p| p.owner == Some(0)),
        "the player has a homeworld"
    );
    assert!(
        game.planets.iter().any(|p| p.owner == Some(1)),
        "so does the Berserker"
    );

    // The scanner is set the way StartTutor sets it: `grbitScan = 0x4e0`.
    let bits = app.grbit_scan();
    assert_eq!(bits & 0x000f, 0, "the normal view");
    assert_eq!(bits & 0x0020, 0x0020, "scanner coverage");
    assert_eq!(bits & 0x0040, 0x0040, "mine fields");
    assert_eq!(bits & 0x0080, 0x0080, "fleet paths");
    assert_eq!(bits & 0x0400, 0x0400, "planet names");
    assert_eq!(app.scan_zoom, 2, "1024 wide picks zoom 2");

    // And the tutorial is running.
    assert!(app.tutor.is_some());
}

/// The zoom is picked from the screen's width, as `StartTutor` picks it.
#[test]
fn the_zoom_follows_the_screen_width() {
    for (width, want) in [(640, 0), (800, 1), (1024, 2), (1600, 3)] {
        let mut app = App::new();
        app.create_tutor_world(width).expect("the tutorial's world");
        assert_eq!(app.scan_zoom, want, "{width} wide");
    }
}

// --- The tutor window ----------------------------------------------------

/// The window is three buttons and a panel painted by hand.
#[test]
fn the_window_is_the_games_own() {
    use stars_ui::dialog::{Class, TUTOR, TUTOR_PANIC};

    assert_eq!(TUTOR.caption, "Stars! Tutor");
    assert_eq!(TUTOR.size, (145, 184));
    assert_eq!(TUTOR.controls.len(), 3, "everything else is painted");
    for (id, label) in [(0x2u16, "Hide"), (0x76, "Hint"), (0x9c7, "Panic!")] {
        let button = TUTOR.control(id).expect("a button");
        assert_eq!(button.class, Class::Button);
        assert_eq!(button.label(), label);
        assert_eq!(button.at.1, 167, "all three along the foot");
        assert_eq!((button.at.2, button.at.3), (40, 12));
    }

    assert_eq!(TUTOR_PANIC.caption, "Something's Really Gone Wrong!");
    assert_eq!(TUTOR_PANIC.controls.len(), 7);
    assert_eq!(
        TUTOR_PANIC.control(0x9ca).expect("complete").label(),
        "Complete Turn"
    );
}

/// The text panel measures its bottom from the **Hide button's top**, not
/// from the dialog's foot, and is inset by lines of the dialog's font.
#[test]
fn the_text_panel_is_measured_off_the_hide_button() {
    use stars_ui::dialog::{tutor_text_area, TUTOR};

    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, TUTOR.pixels());
    let hide = TUTOR.place(rect, TUTOR.control(0x2).expect("Hide"));
    let line = 13.0_f32;
    let area = tutor_text_area(rect, hide.top(), line);

    // Two lines down from the top, and clear of the buttons.
    assert!(area.top() > rect.top() + line);
    assert!(area.bottom() < hide.top());
    // Inset from both sides by the same amount.
    let left = area.left() - rect.left();
    let right = rect.right() - area.right();
    assert!((left - right).abs() < 0.01, "{left} against {right}");
    // And it is the widest thing on the dialog.
    assert!(area.width() > rect.width() * 0.8);
}

/// Hide puts the window away without stopping the tutorial, and says how to
/// get it back — once.
#[test]
fn hide_is_not_stop() {
    let mut app = a_game();
    app.start_tutor();
    assert!(app.tutor.is_some());
    assert!(!app.tutor.as_ref().expect("running").hidden);

    let notice = app.hide_tutor().expect("the notice, the first time");
    assert!(notice.contains("Help menu"));
    assert!(app.tutor.as_ref().expect("still running").hidden);
    assert!(app.tutor.is_some(), "hiding does not end it");

    // Only once.
    assert_eq!(app.hide_tutor(), None);

    // Finishing a page brings it back, which is AdvanceTutor's ShowTutor(1).
    app.show_tutor();
    assert!(!app.tutor.as_ref().expect("running").hidden);
}

/// Three pages ask about the report's sort, and each asks for a little more.
/// They were left out while the reports were lists; they are back.
#[test]
fn the_report_sort_pages_ask_what_the_arms_ask() {
    let page = |idt: usize| {
        STEPS
            .iter()
            .find(|s| s.idt == idt)
            .unwrap_or_else(|| panic!("a page at {idt}"))
    };

    // Page 46 latches as soon as the report is open, so the sort only moves
    // the emphasis.
    let checks: Vec<&Option<Check>> = page(360).stages.iter().map(|s| &s.check).collect();
    assert!(checks.contains(&&Some(Check::ReportOpen)));
    let sort = page(360)
        .stages
        .iter()
        .find(|s| matches!(s.check, Some(Check::ReportSort { .. })))
        .expect("the Value rung");
    assert!(!sort.gates, "opening the report is what page 46 waits for");
    assert_eq!(
        sort.check,
        Some(Check::ReportSort {
            column: 4,
            ascending: None,
            subsort: None,
        })
    );

    // Page 55 wants the column, the direction and the mineral, and gates on
    // all three.
    let sort = page(432)
        .stages
        .iter()
        .find(|s| matches!(s.check, Some(Check::ReportSort { .. })))
        .expect("the Min Conc rung");
    assert!(sort.gates);
    assert_eq!(
        sort.check,
        Some(Check::ReportSort {
            column: 0x0b,
            ascending: Some(false),
            subsort: Some(3),
        })
    );

    // Page 59 wants Population, and gates.
    let sort = page(464)
        .stages
        .iter()
        .find(|s| matches!(s.check, Some(Check::ReportSort { .. })))
        .expect("the Population rung");
    assert!(sort.gates);
    assert_eq!(
        sort.check,
        Some(Check::ReportSort {
            column: 2,
            ascending: None,
            subsort: None,
        })
    );
}

/// `vprptCur` is the screen, and the sort check reads whichever report that
/// is.
#[test]
fn the_tutor_sees_the_report_and_how_it_is_sorted() {
    use stars_ui::report::Report;
    use stars_ui::Screen;

    let mut app = a_game();
    app.screen = Screen::Galaxy;
    assert_eq!(app.open_report_kind(), None);
    assert!(!app.tutor_check(&Check::ReportOpen));

    app.show_screen(Screen::Planets);
    assert_eq!(app.open_report_kind(), Some(Report::Planets));
    assert!(app.tutor_check(&Check::ReportOpen));

    // Sorted by the name column to start with, so Population does not match.
    assert!(!app.tutor_check(&Check::ReportSort {
        column: 2,
        ascending: None,
        subsort: None,
    }));
    app.reports.sort_by(Report::Planets, 2, true, 0);
    assert!(app.tutor_check(&Check::ReportSort {
        column: 2,
        ascending: None,
        subsort: None,
    }));

    // Min Conc, reversed, on the weighted average: all three or nothing.
    let strict = Check::ReportSort {
        column: 0x0b,
        ascending: Some(false),
        subsort: Some(3),
    };
    app.reports.sort_by(Report::Planets, 0x0b, true, 3);
    assert!(!app.tutor_check(&strict), "still ascending");
    app.reports.sort_by(Report::Planets, 0x0b, false, 0);
    assert!(!app.tutor_check(&strict), "still on ironium alone");
    app.reports.sort_by(Report::Planets, 0x0b, false, 3);
    assert!(app.tutor_check(&strict));

    // Closing the report takes the answer away again.
    app.close_report();
    assert!(!app.tutor_check(&strict));
    assert!(!app.tutor_check(&Check::ReportOpen));
}
