//! Clicking the same spot again to cycle through what is on it.
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, ScanObject, Screen, SurveySubject};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "cycle".to_string(),
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

fn fleet(owner: i16, at: Point, orbiting: Option<u16>) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: at,
        orbiting,
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
    }
}

/// Start with nothing selected.
///
/// A new game already has the player's homeworld selected, so without this the
/// first click on it is a no-op — correctly, since it is already the thing
/// selected — and every sequence below would run a step ahead.
fn nothing_selected(app: &mut App) {
    app.selection.planet = None;
    app.selection.fleet = None;
    app.selection.on_fleet = false;
}

/// The player's own homeworld, and where it is.
fn homeworld(app: &App) -> (i16, Point) {
    let game = app.game.as_ref().expect("a game");
    let planet = game
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("a homeworld");
    (planet.id, planet.position.expect("a position"))
}

/// Clicking the same spot walks the planet and then each fleet, and comes back
/// round to the planet.
#[test]
fn clicking_again_walks_round_the_spot() {
    let mut app = a_game();
    let (planet, at) = homeworld(&app);
    app.game.as_mut().expect("a game").fleets = vec![fleet(0, at, Some(0)), fleet(0, at, Some(0))];
    nothing_selected(&mut app);

    // The planet comes first.
    assert!(app.scan_click(at.x, at.y));
    assert_eq!(app.selected_object(), Some(ScanObject::Planet(planet)));

    // Then each fleet in turn.
    assert!(app.scan_click(at.x, at.y));
    assert_eq!(app.selected_object(), Some(ScanObject::Fleet(0)));
    assert!(app.scan_click(at.x, at.y));
    assert_eq!(app.selected_object(), Some(ScanObject::Fleet(1)));

    // And round to the planet again.
    assert!(app.scan_click(at.x, at.y));
    assert_eq!(app.selected_object(), Some(ScanObject::Planet(planet)));
}

/// Only the player's own things are in the cycle: `FGetNextObjHere` is called
/// with `fOnlyOurs`, and it returns to the planet only when the planet is the
/// player's.
#[test]
fn only_your_own_things_take_part() {
    let mut app = a_game();
    let (planet, at) = homeworld(&app);
    app.game.as_mut().expect("a game").fleets = vec![
        fleet(1, at, Some(0)), // somebody else's, at the same spot
        fleet(0, at, Some(0)),
    ];
    let here = app.objects_at(at.x, at.y);
    assert_eq!(
        here,
        vec![ScanObject::Planet(planet), ScanObject::Fleet(1)],
        "their fleet is not in the cycle"
    );

    // A planet that is not the player's is not in it either, so a spot can
    // hold plenty and cycle through nothing.
    let elsewhere = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner != Some(0) && p.position.is_some())
        .map(|p| p.position.expect("a position"));
    if let Some(other) = elsewhere {
        app.game.as_mut().expect("a game").fleets = vec![fleet(1, other, None)];
        assert!(app.objects_at(other.x, other.y).is_empty());
        assert!(!app.scan_click(other.x, other.y), "nothing of mine there");
    }
}

/// A spot holding one thing does not change when clicked again.
#[test]
fn one_thing_stays_put() {
    let mut app = a_game();
    let (planet, at) = homeworld(&app);
    app.game.as_mut().expect("a game").fleets = Vec::new();
    nothing_selected(&mut app);
    assert!(app.scan_click(at.x, at.y));
    assert_eq!(app.selected_object(), Some(ScanObject::Planet(planet)));
    assert!(!app.scan_click(at.x, at.y), "nowhere else to go");
    assert_eq!(app.selected_object(), Some(ScanObject::Planet(planet)));
}

/// A different spot selects what is there rather than cycling.
#[test]
fn a_different_spot_selects_rather_than_cycles() {
    let mut app = a_game();
    let (planet, at) = homeworld(&app);
    let away = Point::new(at.x + 40, at.y + 40);
    app.game.as_mut().expect("a game").fleets = vec![fleet(0, at, Some(0)), fleet(0, away, None)];
    nothing_selected(&mut app);

    app.scan_click(at.x, at.y);
    app.scan_click(at.x, at.y);
    assert_eq!(app.selected_object(), Some(ScanObject::Fleet(0)));

    // Clicking the far fleet takes it, rather than stepping the old spot.
    assert!(app.scan_click(away.x, away.y));
    assert_eq!(app.selected_object(), Some(ScanObject::Fleet(1)));
    // And going back starts that spot at its planet again.
    assert!(app.scan_click(at.x, at.y));
    assert_eq!(app.selected_object(), Some(ScanObject::Planet(planet)));
}

/// Selecting a fleet keeps the planet it orbits, because the status bar names
/// the planet whichever is in front — and the pane follows the selection.
#[test]
fn the_pane_follows_the_cycle() {
    let mut app = a_game();
    let (planet, at) = homeworld(&app);
    app.game.as_mut().expect("a game").fleets = vec![fleet(0, at, Some(0))];
    app.screen = Screen::Galaxy;
    nothing_selected(&mut app);

    app.scan_click(at.x, at.y);
    assert_eq!(app.survey_subject(), SurveySubject::Planet(planet));

    app.scan_click(at.x, at.y);
    assert_eq!(
        app.survey_subject(),
        SurveySubject::Fleet(0),
        "the fleet pane"
    );
    assert_eq!(
        app.selection.planet,
        Some(0),
        "and the planet it orbits is still known"
    );

    // Back to the planet, and the pane goes with it.
    app.scan_click(at.x, at.y);
    assert_eq!(app.survey_subject(), SurveySubject::Planet(planet));
}

/// The map draws and takes clicks.
#[test]
fn the_map_draws_after_a_cycle() {
    let mut app = a_game();
    let (_, at) = homeworld(&app);
    app.game.as_mut().expect("a game").fleets = vec![fleet(0, at, Some(0))];
    nothing_selected(&mut app);
    for _ in 0..3 {
        app.scan_click(at.x, at.y);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::galaxy::view(&mut app, ui);
            });
        });
    }
}
