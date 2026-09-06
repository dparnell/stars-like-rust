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
