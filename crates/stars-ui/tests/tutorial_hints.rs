//! The tutor's **Hint** button — `tutor.idh`, the help topic each page's
//! checks leave behind as they fail, opened in the player's guide.
//! See `docs/ui/tutorial.md` and `docs/ui/help.md`.

use std::path::PathBuf;

use stars_ui::tutorial::topic;
use stars_ui::App;

const PRUNE: i16 = 0x0c;
const PLANET_90210: i16 = 0x10;

fn help(app: &App) -> u16 {
    app.tutor.as_ref().expect("running").help
}

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
    assert!(app.add_waypoint(at.x, at.y, 20.0));
}

/// The first pages, check by check: the topic follows the rung the page
/// is waiting on, and the arm's own topic where it sets one.
#[test]
fn the_hint_follows_the_rung_the_page_waits_on() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");

    // Page 1 waits on the messages: `FCheckMessages` leaves the Messages
    // Pane topic.
    assert_eq!(help(&app), topic::MESSAGES_PANE);
    for _ in 0..3 {
        app.show_next_message();
        app.advance_tutor();
    }
    assert_eq!(help(&app), topic::MESSAGES_PANE);
    app.show_next_message();
    app.advance_tutor();
    assert_eq!(app.tutor.as_ref().unwrap().page(), 2);

    // Page 2 waits on Armed Probe #1 being picked: `FCheckFleetWP` fails
    // for want of the fleet in hand, and `FCheckSelection` on the way out
    // says how to pick one — the Fleets in Orbit tile, since the planet
    // it orbits is what is in hand.
    assert_eq!(help(&app), topic::FLEETS_IN_ORBIT_TILE);
    assert!(app.goto_fleet(0));
    app.advance_tutor();
    // Picked, with no second waypoint yet: adding one.
    assert_eq!(help(&app), topic::ADDING_WAYPOINTS);
    shift_click(&mut app, PRUNE);
    app.advance_tutor();
    assert_eq!(app.tutor.as_ref().unwrap().page(), 3);

    // Page 3's arm sets Keyboard Shortcuts outright while Armed Probe #2
    // is not in hand (`10f8:1059`).
    assert_eq!(help(&app), topic::KEYBOARD_SHORTCUTS);
    assert!(app.select_adjacent_fleet(1));
    app.advance_tutor();
    assert_eq!(help(&app), topic::ADDING_WAYPOINTS);
    shift_click(&mut app, PLANET_90210);
    app.advance_tutor();
    assert_eq!(app.tutor.as_ref().unwrap().page(), 4);
}

/// A page waiting for the turn offers `0xdb6`, which the help file has
/// no page for — WinHelp says so, and so does the viewer.
#[test]
fn waiting_for_the_turn_offers_a_topic_the_file_lacks() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    // Play to the end of 2400: pages 1 to 5.
    for _ in 0..4 {
        app.show_next_message();
    }
    app.advance_tutor();
    assert!(app.goto_fleet(0));
    shift_click(&mut app, PRUNE);
    app.advance_tutor();
    assert!(app.select_adjacent_fleet(1));
    shift_click(&mut app, PLANET_90210);
    app.advance_tutor();
    for _ in 0..3 {
        assert!(app.select_adjacent_fleet(1));
    }
    shift_click(&mut app, 0x0f);
    app.advance_tutor();
    assert!(app.select_adjacent_fleet(1));
    assert!(app.select_adjacent_fleet(1));
    app.open_research();
    app.research_dialog.as_mut().expect("the dialog").field = 1;
    app.research_ok();
    app.advance_tutor();
    assert!(app.tutor_waiting(), "2400's work is done");
    assert_eq!(help(&app), topic::WAITING_FOR_THE_TURN);

    // Hint on that page puts up the notice, with the file present.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    let Ok(bytes) = std::fs::read(root.join("STARS!.HLP")) else {
        return;
    };
    app.load_help(bytes, "STARS!.HLP")
        .expect("the help file reads");
    app.help_context(u32::from(help(&app)));
    assert_eq!(
        app.help.notice,
        Some(stars_ui::help::Notice::NoTopic(u32::from(
            topic::WAITING_FOR_THE_TURN
        )))
    );
    // Where a page's topic is in the file, Hint opens it.
    app.help_context(u32::from(topic::MESSAGES_PANE));
    assert_eq!(app.help.title(), Some("Messages Pane"));
    app.help_context(u32::from(topic::KEYBOARD_SHORTCUTS));
    assert_eq!(app.help.title(), Some("Keyboard Shortcuts"));
}

/// Every topic the tutor can name, save the two the original names that
/// are not there, is a page of the guide.
#[test]
fn every_named_topic_is_in_the_guide() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    let Ok(bytes) = std::fs::read(root.join("STARS!.HLP")) else {
        return;
    };
    let help = stars_formats::HelpFile::read(bytes).expect("reads");
    let named = [
        (topic::KEYBOARD_SHORTCUTS, "Keyboard Shortcuts"),
        (topic::LOCATION_TILE, "Location Tile"),
        (topic::FLEETS_IN_ORBIT_TILE, "Fleets in Orbit Tile"),
        (topic::SELECTING_AN_OBJECT, "Selecting an Object to Command"),
        (topic::MESSAGES_PANE, "Messages Pane"),
        (topic::KEY_TO_THE_SCANNER, "Key to the Scanner"),
        (topic::SCANNER_VIEW, "Choosing Your View of the Universe"),
        (topic::ZOOMING, "Zooming"),
        (topic::FLEET_WAYPOINTS_TILE, "Fleet Waypoints Tile"),
        (topic::WAYPOINT_TASK_TILE, "Waypoint Task Tile"),
        (topic::ADDING_WAYPOINTS, "Adding Fleet Waypoints"),
        (topic::MOVING_WAYPOINTS, "Moving Fleet Waypoints"),
        (topic::COLONIZE, "Colonize"),
        (topic::TRANSPORT, "Transport"),
        (topic::PRODUCTION_TILE, "Production Tile"),
        (topic::PRODUCTION_DIALOG, "Production Dialog"),
        (topic::CARGO_TRANSFER, "Cargo Transfer Dialogs"),
        (topic::SHIP_DESIGNER, "Ship Designer"),
        (topic::EDITING_A_DESIGN, "Editing an Existing Ship Design"),
        (topic::DESIGNING_A_SHIP, "Designing a New Ship from Scratch"),
        (topic::ZIP_ORDERS, "Custom Zip Orders dialog"),
        (topic::PRODUCTION_TEMPLATES, "Production Templates"),
        (topic::RESEARCH_DIALOG, "Research Dialog"),
        (topic::OTHER_FLEETS_HERE_TILE, "Other Fleets Here Tile"),
        (topic::FLEET_COMPOSITION_TILE, "Fleet Composition Tile"),
        (topic::MERGE_FLEETS, "Merge Fleets dialog"),
    ];
    for (id, title) in named {
        let offset = help
            .topic_for_context(u32::from(id))
            .unwrap_or_else(|| panic!("{id:#x} {title}"));
        assert_eq!(help.title_of(offset), Some(title), "{id:#x}");
    }
    for id in [topic::DESIGNER_SHUT, topic::WAITING_FOR_THE_TURN] {
        assert_eq!(help.topic_for_context(u32::from(id)), None, "{id:#x}");
    }
}
