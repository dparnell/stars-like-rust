//! How a planet is drawn on the map — `DrawScanner`'s two loops.
//!
//! See `docs/ui/scanner.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "marks".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
            opponents::opponent(2, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

/// A planet of one's own is the green cell; a stranger's the red one; a
/// friend's the yellow one — and the selected one is the 11x11 blob over the
/// shared mask.
#[test]
fn a_planet_is_drawn_by_whose_it_is() {
    let mut app = a_game();
    let me = app.local_player();
    let base = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .first()
        .expect("a planet")
        .clone();
    let planet = |owner: Option<i16>| {
        let mut planet = base.clone();
        planet.owner = owner;
        planet
    };

    let mine = planet(Some(i16::try_from(me).expect("a player")));
    let mark = app.planet_mark(&mine, false);
    assert_eq!(mark.cell, (0xb, 0), "the 5x5 at the top of its column");
    assert_eq!(mark.side, 5);
    assert!(mark.mask.is_none(), "only the blobs have a mask");

    let theirs = planet(Some(1));
    assert_eq!(app.planet_mark(&theirs, false).cell, (0xb, 5));

    // A friend is set apart from a stranger; a neutral is not.
    if let Some(game) = app.game.as_mut() {
        game.players[me].relations = vec![0, 1, 0];
    }
    assert_eq!(app.planet_mark(&planet(Some(1)), false).cell, (0xb, 10));
    assert_eq!(
        app.planet_mark(&planet(Some(2)), false).cell,
        (0xb, 5),
        "a neutral is drawn like an enemy"
    );

    // Selected: the 11x11 blob, over the one mask they all share.
    for (owner, cell) in [(Some(0), (0, 0)), (Some(1), (0, 0x16)), (Some(2), (0, 0xb))] {
        let mark = app.planet_mark(&planet(owner), true);
        assert_eq!(mark.cell, cell);
        assert_eq!(mark.side, 11);
        assert_eq!(mark.mask, Some((0, 0x45)));
    }
}

/// A planet nobody owns is a 3x3 dot, and the grey starburst when selected.
#[test]
fn an_unowned_planet_is_a_dot() {
    let app = a_game();
    let mut planet = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .first()
        .expect("a planet")
        .clone();
    planet.owner = None;

    let mark = app.planet_mark(&planet, false);
    assert_eq!(mark.cell, (0xb, 0x12));
    assert_eq!(mark.side, 3);

    let mark = app.planet_mark(&planet, true);
    assert_eq!(mark.cell, (0, 0x21), "the starburst");
    assert_eq!(mark.side, 11);
    assert_eq!(mark.mask, Some((0, 0x45)));
}

/// Every position on the map gets a dot, whether it has been explored or not.
#[test]
fn an_unexplored_position_still_gets_a_dot() {
    assert_eq!(stars_ui::PLANET_UNEXPLORED.cell, (0xb, 0xf));
    assert_eq!(stars_ui::PLANET_UNEXPLORED.side, 3);
    assert!(stars_ui::PLANET_UNEXPLORED.mask.is_none());
}

/// The starbase flag: blue for a starbase, yellow for an orbital fort, and
/// nothing at all without one.
#[test]
fn the_starbase_flag_tells_a_fort_from_a_starbase() {
    let mut app = a_game();
    let mut planet = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .first()
        .expect("a planet")
        .clone();
    planet.starbase = false;
    assert_eq!(app.planet_starbase_mark(&planet), None);

    planet.starbase = true;
    planet.owner = Some(0);
    // Whatever the homeworld starts with, both answers are one of the two.
    let mark = app.planet_starbase_mark(&planet).expect("a mark");
    assert!(mark == [0x40, 0x80, 0xff] || mark == [0xff, 0xd0, 0x40]);

    // A design on hull 32 — the Orbital Fort — is the yellow one.
    if let Some(game) = app.game.as_mut() {
        if let Some(design) = game.designs[0].first_mut() {
            design.hull_id = 32;
        }
    }
    planet.starbase_design = Some(0);
    assert_eq!(
        app.planet_starbase_mark(&planet),
        Some([0xff, 0xd0, 0x40]),
        "a fort is yellow"
    );
    if let Some(game) = app.game.as_mut() {
        if let Some(design) = game.designs[0].first_mut() {
            design.hull_id = 33;
        }
    }
    assert_eq!(
        app.planet_starbase_mark(&planet),
        Some([0x40, 0x80, 0xff]),
        "anything more is blue"
    );
}
