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

// --- The other views ----------------------------------------------------

/// The Planet Value view: two discs, sized and coloured by what the planet is
/// worth — and by what it would be worth terraformed when that is negative.
#[test]
fn planet_value_draws_two_discs() {
    let app = a_game();
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("a homeworld")
        .clone();

    let [outer, inner] = app.planet_value_discs(&home).expect("a value");
    // A homeworld is at its race's ideal, so it is green and white and big.
    assert_eq!(outer.1, [0x00, 0x80, 0x00], "green: habitable");
    assert_eq!(inner.1, [0xff, 0xff, 0xff]);
    assert!(outer.0 > inner.0, "the core is the smaller of the two");
    assert!(outer.0 <= 10.0, "ten is as large as it goes");
    assert!(inner.0 >= 1.0);

    // A planet nobody has looked at has no value to show.
    let mut unknown = home.clone();
    unknown.detail = stars_core::planet::Detail::Minimal;
    assert!(app.planet_value_discs(&unknown).is_none());
}

/// A hostile planet is grey and red, and grows with how hostile it is.
#[test]
fn a_hostile_planet_is_red_and_grows_with_the_harm() {
    let app = a_game();
    let mut planet = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("a homeworld")
        .clone();
    // Push every variable to an extreme the race cannot reach.
    planet.env = [0, 0, 100];
    planet.owner = None;
    planet.detail = stars_core::planet::Detail::Scanned;

    let [outer, inner] = app.planet_value_discs(&planet).expect("a value");
    let hostile = outer.1 == [0x60, 0x70, 0x80] && inner.1 == [0xff, 0x00, 0x00];
    let terraformable = outer.1 == [0x80, 0x80, 0x00] && inner.1 == [0xff, 0xff, 0x00];
    assert!(
        hostile || terraformable,
        "an unlivable planet is red, or yellow if terraforming would fix it: {outer:?} {inner:?}"
    );
}

/// The flag says who lives there, and an empty planet has none.
#[test]
fn the_value_view_plants_a_flag_on_an_inhabited_planet() {
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
    assert_eq!(
        app.planet_value_flag(&planet(None)),
        None,
        "nobody lives there"
    );
    assert_eq!(
        app.planet_value_flag(&planet(Some(i16::try_from(me).expect("a player")))),
        Some([0x40, 0x80, 0xff]),
        "yours is blue"
    );
    // A neutral and an enemy are told apart here, where the manual lumps them.
    assert_eq!(
        app.planet_value_flag(&planet(Some(1))),
        Some([0x80, 0x90, 0xa0])
    );
    if let Some(game) = app.game.as_mut() {
        game.players[me].relations = vec![0, 2, 1];
    }
    assert_eq!(
        app.planet_value_flag(&planet(Some(1))),
        Some([0xff, 0x00, 0x00])
    );
    assert_eq!(
        app.planet_value_flag(&planet(Some(2))),
        Some([0xff, 0xd0, 0x40])
    );
}

/// The mineral views: three bars, scaled two different ways, halved when the
/// map is zoomed out.
#[test]
fn the_mineral_views_draw_three_bars() {
    let mut app = a_game();
    let mut planet = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("a homeworld")
        .clone();
    planet.surface_min = [0, 2500, 100_000];
    planet.min_conc = [0, 50, 200];

    // Surface: (amount + max/40) / (max/20), capped at 20.
    let bars = app.planet_mineral_bars(&planet, false);
    assert_eq!(bars.len(), 3);
    // (amount + 5000/40) / (5000/20), which is (amount + 125) / 250.
    assert_eq!(bars[0].height, 0);
    assert_eq!(bars[1].height, (2500 + 125) / 250);
    assert_eq!(bars[2].height, 20, "a rich planet tops the scale out");

    // Concentration: a fifth, capped the same way.
    let bars = app.planet_mineral_bars(&planet, true);
    assert_eq!(bars[0].height, 0);
    assert_eq!(bars[1].height, 10);
    assert_eq!(bars[2].height, 20);

    // Zoomed out, everything is half as tall and the small layout is used.
    assert_eq!(app.mineral_bar_layout(), [7, 12, 19, 4, 6]);
    app.scan_zoom = -1;
    assert_eq!(app.mineral_bar_layout(), [3, 10, 11, 2, 3]);
    assert_eq!(app.planet_mineral_bars(&planet, true)[1].height, 5);

    // A planet this player has only scanned has concentrations but no surface
    // reading.
    planet.detail = stars_core::planet::Detail::Scanned;
    assert!(app.planet_mineral_bars(&planet, false).is_empty());
    assert_eq!(app.planet_mineral_bars(&planet, true).len(), 3);
}

/// The population circle steps up a nineteen-entry ladder.
#[test]
fn the_population_circle_steps_up_the_ladder() {
    let app = a_game();
    let mut planet = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("a homeworld")
        .clone();

    // The ladder is in hundreds of colonists, and the circle is the step it
    // does not reach plus two.
    assert_eq!(stars_ui::POPULATION_STEPS[0], 25);
    assert_eq!(stars_ui::POPULATION_STEPS[18], 25_000);

    planet.pop = 25;
    assert_eq!(app.planet_population_disc(&planet).expect("a disc").0, 2.0);
    planet.pop = 26;
    assert_eq!(app.planet_population_disc(&planet).expect("a disc").0, 3.0);
    planet.pop = 25_000;
    assert_eq!(app.planet_population_disc(&planet).expect("a disc").0, 20.0);
    planet.pop = 99_999;
    assert_eq!(
        app.planet_population_disc(&planet).expect("a disc").0,
        21.0,
        "past the end of the ladder it stops growing"
    );

    // Green for one's own; nothing at all for a planet nobody lives on.
    assert_eq!(
        app.planet_population_disc(&planet).expect("a disc").1,
        [0x00, 0xc0, 0x00]
    );
    planet.owner = None;
    assert!(app.planet_population_disc(&planet).is_none());
}
