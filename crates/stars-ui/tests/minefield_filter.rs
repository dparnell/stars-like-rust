//! The scanner's minefield menu.
//!
//! See `docs/ui/toolbar.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::relations::{Party, Relation};
use stars_core::{opponents, Race};
use stars_ui::toolbar::Button;
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    let mut players = vec![NewPlayer::human(Race::humanoid())];
    for index in 1..4 {
        players.push(
            opponents::opponent(index, 1)
                .expect("an opponent")
                .as_player(),
        );
    }
    app.new_game(&NewGame {
        name: "mines".to_string(),
        size: Size::Small,
        players,
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

/// The four groups, and who falls in which.
#[test]
fn the_four_groups_follow_the_relations_table() {
    assert_eq!(
        Party::ALL.map(|p| p.name()),
        ["Yours", "Friends", "Neutrals", "Enemies"]
    );
    for (index, party) in Party::ALL.iter().enumerate() {
        assert_eq!(usize::from(party.index()), index);
    }

    let mut app = a_game();
    app.set_regard(1, Relation::Friend);
    app.set_regard(2, Relation::Enemy);
    // Player 3 is left neutral.
    let game = app.game.as_ref().expect("a game");
    assert_eq!(stars_core::relations::party(game, 0, 0), Party::Yours);
    assert_eq!(stars_core::relations::party(game, 0, 1), Party::Friends);
    assert_eq!(stars_core::relations::party(game, 0, 2), Party::Enemies);
    assert_eq!(stars_core::relations::party(game, 0, 3), Party::Neutrals);
}

/// The game starts with the overlay on and all four groups shown, which is
/// what `stars.ini` defaults to when it says nothing.
#[test]
fn it_starts_with_everything_shown() {
    let app = a_game();
    assert_eq!(app.scan_minefield_filter, 0xf);
    assert!(app.scan_overlays.minefields);
    assert!(app.toolbar_down(Button::MineFields));
    // And the other two defaults from the same line of `stars.ini`.
    assert!(app.scan_overlays.scanner_coverage);
    assert!(app.scan_overlays.fleet_paths);
    assert_eq!(app.scan_coverage_pct, 100);
    // The two ship filters start empty, as they do there too.
    assert_eq!(app.scan_design_filter, 0);
    assert_eq!(app.scan_class_filter, 0);
}

/// The button shows pressed only when **all four** are shown; narrowing the
/// menu leaves it up.
#[test]
fn a_partial_choice_leaves_the_button_up() {
    let mut app = a_game();
    assert!(app.toolbar_down(Button::MineFields));
    app.toggle_minefield_filter(Party::Enemies.index());
    assert!(app.scan_overlays.minefields, "still drawing three of them");
    assert!(
        !app.toolbar_down(Button::MineFields),
        "but no longer pressed"
    );
    app.toggle_minefield_filter(Party::Enemies.index());
    assert!(app.toolbar_down(Button::MineFields));
}

/// The overlay follows the filter exactly — which is the opposite of the two
/// ship filters, where unticking the last one leaves the overlay on.
#[test]
fn unticking_the_last_group_turns_the_overlay_off() {
    let mut app = a_game();
    for party in Party::ALL {
        app.toggle_minefield_filter(party.index());
    }
    assert_eq!(app.scan_minefield_filter, 0);
    assert!(!app.scan_overlays.minefields, "off, not merely empty");

    // And ticking one brings it back.
    app.toggle_minefield_filter(Party::Yours.index());
    assert!(app.scan_overlays.minefields);

    // The ship filters do not behave this way, which is the contrast worth
    // pinning.
    let mut app = a_game();
    app.toggle_design_filter(0);
    app.toggle_design_filter(0);
    assert_eq!(app.scan_design_filter, 0);
    assert!(
        app.scan_overlays.ship_design_filter,
        "the ship filter stays on when emptied"
    );
}

/// Opening the menu with the overlay off empties the filter first.
#[test]
fn opening_it_while_off_empties_the_filter() {
    let mut app = a_game();
    assert_eq!(app.scan_minefield_filter, 0xf);
    // Turn the overlay off the way the menu does.
    app.minefield_filter_command(false);
    assert!(!app.scan_overlays.minefields);
    // Something else turns it back on without touching the filter.
    app.scan_overlays.minefields = true;
    app.scan_minefield_filter = 0xf;
    app.scan_overlays.minefields = false;

    app.open_minefield_menu();
    assert_eq!(app.scan_minefield_filter, 0, "emptied on the way in");

    // Opening it while the overlay is on leaves the choice alone.
    app.minefield_filter_command(true);
    assert!(app.scan_overlays.minefields);
    app.toggle_minefield_filter(Party::Neutrals.index());
    let narrowed = app.scan_minefield_filter;
    app.open_minefield_menu();
    assert_eq!(app.scan_minefield_filter, narrowed);
}

/// Only the chosen groups' fields are drawn.
#[test]
fn only_the_chosen_groups_are_drawn() {
    let mut app = a_game();
    app.set_regard(1, Relation::Friend);
    app.set_regard(2, Relation::Enemy);

    assert!(app.shows_minefield(0));
    assert!(app.shows_minefield(1));
    assert!(app.shows_minefield(2));
    assert!(app.shows_minefield(3));

    // Just my own.
    app.minefield_filter_command(false);
    app.toggle_minefield_filter(Party::Yours.index());
    assert!(app.shows_minefield(0));
    assert!(!app.shows_minefield(1), "not a friend's");
    assert!(!app.shows_minefield(2), "not an enemy's");
    assert!(!app.shows_minefield(3), "not a neutral's");

    // Enemies only, which is the pairing a player at war would want.
    app.minefield_filter_command(false);
    app.toggle_minefield_filter(Party::Enemies.index());
    assert!(!app.shows_minefield(0));
    assert!(app.shows_minefield(2));

    // With the overlay off nothing is drawn whatever is ticked.
    app.scan_overlays.minefields = false;
    assert!(!app.shows_minefield(2));
}

/// The two commands: all four, or none. There is no invert here, unlike the
/// ship filters' menus.
#[test]
fn the_menu_has_two_commands_not_three() {
    let mut app = a_game();
    app.minefield_filter_command(false);
    assert_eq!(app.scan_minefield_filter, 0);
    assert!(!app.scan_overlays.minefields);
    app.minefield_filter_command(true);
    assert_eq!(app.scan_minefield_filter, 0xf);
    assert!(app.scan_overlays.minefields);

    assert_eq!(app.minefield_filter_entries().len(), 4);
    assert_eq!(
        app.minefield_filter_entries()
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>(),
        [
            "Mine Fields of Yours",
            "Mine Fields of Friends",
            "Mine Fields of Neutrals",
            "Mine Fields of Enemies",
        ]
    );
}

// --- How a field is drawn -----------------------------------------------
//
// `DrawScanner` walks the fields in three groups, each with its own colour,
// and fills each field with one of three pattern brushes by kind.

/// Yours blue, a friend's yellow, anybody else's red — the grouping the manual
/// gives on p. 5-14.
#[test]
fn a_field_is_coloured_by_whose_it_is() {
    let mut app = a_game();
    let me = app.local_player();
    let field = |owner: i16, detonating: bool| stars_core::minefield::Minefield {
        id: 1,
        owner,
        position: stars_core::movement::Point::new(10, 10),
        mines: 100,
        kind: 0,
        detonating,
        detected_by: 0xFFFF,
        visible_to: 0xFFFF,
        turn: 0,
    };
    let mine = app.minefield_colour(&field(i16::try_from(me).expect("a player"), false));
    let theirs = app.minefield_colour(&field(1, false));
    assert_ne!(mine, theirs);
    assert!(mine[2] > mine[0], "ours is blue: {mine:?}");
    assert_eq!(theirs, [0xff, 0x40, 0x40], "a stranger's is red");

    // A friend's is yellow.
    if let Some(game) = app.game.as_mut() {
        game.players[me].relations = vec![0, 1];
    }
    let friend = app.minefield_colour(&field(1, false));
    assert!(
        friend[0] > 0x80 && friend[1] > 0x80 && friend[2] < 0x80,
        "a friend's is yellow: {friend:?}"
    );

    // And one armed to detonate is red whoever owns it.
    assert_eq!(
        app.minefield_colour(&field(i16::try_from(me).expect("a player"), true)),
        [0xff, 0x00, 0x00]
    );
}

/// One pattern brush per kind: 460 standard, 461 heavy, 462 speed bump.
#[test]
fn the_hatch_says_which_kind_of_field_it_is() {
    let field = |kind: u8| stars_core::minefield::Minefield {
        id: 1,
        owner: 0,
        position: stars_core::movement::Point::new(10, 10),
        mines: 100,
        kind,
        detonating: false,
        detected_by: 0xFFFF,
        visible_to: 0xFFFF,
        turn: 0,
    };
    assert_eq!(App::minefield_pattern(&field(0)), 460);
    assert_eq!(App::minefield_pattern(&field(1)), 461);
    assert_eq!(App::minefield_pattern(&field(2)), 462);
    // Nothing else is a kind, and nothing else has a brush.
    assert_eq!(App::minefield_pattern(&field(9)), 462);
}

/// A field centred on a planet gets no centre mark: the planet's own dot is
/// already there.
#[test]
fn only_a_field_away_from_a_planet_marks_its_centre() {
    let app = a_game();
    let at = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find_map(|p| p.position)
        .expect("a planet");
    let field = |position| stars_core::minefield::Minefield {
        id: 1,
        owner: 0,
        position,
        mines: 100,
        kind: 0,
        detonating: false,
        detected_by: 0xFFFF,
        visible_to: 0xFFFF,
        turn: 0,
    };
    assert!(!app.minefield_centre_marked(&field(at)));
    assert!(
        app.minefield_centre_marked(&field(stars_core::movement::Point::new(at.x + 3, at.y + 3)))
    );
}
