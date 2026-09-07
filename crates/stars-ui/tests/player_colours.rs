//! Player Colors, and the two things it colours.
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    let mut players = vec![NewPlayer::human(Race::humanoid())];
    for index in 1..3 {
        players.push(
            opponents::opponent(index, 1)
                .expect("an opponent")
                .as_player(),
        );
    }
    app.new_game(&NewGame {
        name: "colours".to_string(),
        size: Size::Small,
        players,
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

fn fleet(owner: i16, at: Point, count: i32) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: at,
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count,
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
    }
}

/// It starts off — `grbitScan` defaults to `0xe0`, which does not include it —
/// and it is the View menu's rather than the toolbar's.
#[test]
fn it_starts_off() {
    let app = a_game();
    assert!(!app.scan_overlays.player_colours);
    // No toolbar button owns this bit.
    for button in stars_ui::toolbar::Button::ALL {
        let before = app.scan_overlays.player_colours;
        let mut app = a_game();
        app.toolbar_click(button);
        assert_eq!(
            app.scan_overlays.player_colours,
            before,
            "{} does not touch Player Colors",
            button.name()
        );
    }
}

/// A planet's name: the owner's colour, white for your own, and **the
/// ordinary colour for an unowned one**, which the original leaves alone.
#[test]
fn planet_names_take_their_owner_s_colour() {
    let mut app = a_game();
    // Off, nothing is coloured.
    assert_eq!(app.planet_name_colour(Some(0)), None);
    assert_eq!(app.planet_name_colour(Some(1)), None);

    app.scan_overlays.player_colours = true;
    assert_eq!(app.planet_name_colour(Some(0)), Some(None), "yours: white");
    assert_eq!(
        app.planet_name_colour(Some(1)),
        Some(Some(1)),
        "theirs: their colour"
    );
    assert_eq!(
        app.planet_name_colour(None),
        None,
        "unowned keeps the ordinary colour"
    );
}

/// Names are hidden when the map is zoomed too far out to place them.
#[test]
fn names_go_when_the_map_is_small() {
    let mut app = a_game();
    app.scan_overlays.names = true;
    for zoom in [-1, 0, 4] {
        app.scan_zoom = zoom;
        assert!(app.planet_names_visible(), "zoom {zoom}");
    }
    for zoom in [-2, -3, -4] {
        app.scan_zoom = zoom;
        assert!(!app.planet_names_visible(), "zoom {zoom}");
    }
    // And not at all with the overlay off.
    app.scan_zoom = 0;
    app.scan_overlays.names = false;
    assert!(!app.planet_names_visible());
}

/// One count per **location**, not per fleet, capped at 999 — and a location
/// totalling nothing gets no number.
#[test]
fn counts_are_gathered_per_location() {
    let mut app = a_game();
    let here = Point::new(50, 50);
    let there = Point::new(80, 80);
    app.game.as_mut().expect("a game").fleets = vec![
        fleet(0, here, 3),
        fleet(0, here, 4),
        fleet(1, there, 5),
        fleet(0, Point::new(10, 10), 0),
    ];
    let counts = app.ship_counts();
    assert_eq!(counts.len(), 2, "two places with ships: {counts:?}");
    let at_here = counts
        .iter()
        .find(|c| c.position == here)
        .expect("the first spot");
    assert_eq!(at_here.ships, 7, "both fleets, added");
    assert_eq!(at_here.owner, Some(0));

    // Capped at 999.
    app.game.as_mut().expect("a game").fleets = vec![fleet(0, here, 5000)];
    assert_eq!(app.ship_counts()[0].ships, 999);
}

/// The count's colour: the owner's, but only when Player Colors is on, the
/// spot has a single owner, and that owner is not this player.
#[test]
fn a_count_is_coloured_only_when_one_player_owns_the_spot() {
    let mut app = a_game();
    let mine = Point::new(50, 50);
    let theirs = Point::new(60, 60);
    let mixed = Point::new(70, 70);
    app.game.as_mut().expect("a game").fleets = vec![
        fleet(0, mine, 2),
        fleet(1, theirs, 2),
        fleet(1, mixed, 2),
        fleet(2, mixed, 2),
    ];

    // Off: everything is white.
    for count in app.ship_counts() {
        assert_eq!(app.ship_count_colour(&count), None);
    }

    app.scan_overlays.player_colours = true;
    let counts = app.ship_counts();
    let at = |p: Point| counts.iter().find(|c| c.position == p).expect("a count");
    assert_eq!(app.ship_count_colour(at(mine)), None, "yours stays white");
    assert_eq!(app.ship_count_colour(at(theirs)), Some(1), "their colour");
    assert_eq!(
        app.ship_count_colour(at(mixed)),
        None,
        "two players there, so white"
    );
    assert_eq!(at(mixed).ships, 4, "but still counted together");
    assert_eq!(at(mixed).owner, None);
}

/// The map draws with it on and off.
#[test]
fn the_map_draws_either_way() {
    let mut app = a_game();
    app.scan_overlays.names = true;
    app.scan_overlays.ship_counts = true;
    app.game.as_mut().expect("a game").fleets = vec![
        fleet(0, Point::new(50, 50), 3),
        fleet(1, Point::new(50, 50), 4),
    ];
    for on in [false, true] {
        app.scan_overlays.player_colours = on;
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::galaxy::view(&mut app, ui);
            });
        });
    }
}
