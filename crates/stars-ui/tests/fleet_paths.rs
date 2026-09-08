//! The Ship Paths overlay — `DrawScanner`'s `grbitScan & 0x80` pass.
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack, Waypoint};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, ScanView};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "paths".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.scan_overlays.fleet_paths = true;
    app.selection.fleet = None;
    app
}

fn waypoint(at: Point) -> Waypoint {
    Waypoint {
        position: at,
        target: None,
        target_class: 4,
        warp: 6,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    }
}

fn fleet(owner: i16, legs: &[Point]) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: legs.first().copied().unwrap_or(Point::new(0, 0)),
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Default::default(),
        battle_plan: 0,
        warp: Some(6),
        waypoints: legs.iter().copied().map(waypoint).collect(),
        name: None,
        repeat_orders: false,
        direction: None,
    }
}

/// The line runs through every waypoint, starting at the first — which is
/// where the fleet is — rather than from wherever its mark was drawn.
#[test]
fn a_path_is_its_waypoints_joined_up() {
    let mut app = a_game();
    let legs = [Point::new(10, 10), Point::new(40, 20), Point::new(70, 90)];
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, &legs)];
    }
    assert_eq!(app.fleet_paths(), vec![legs.to_vec()]);
}

/// One waypoint is not a path.
#[test]
fn a_fleet_going_nowhere_has_no_path() {
    let mut app = a_game();
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, &[Point::new(10, 10)])];
    }
    assert!(app.fleet_paths().is_empty());
}

/// The overlay's own switch, and the view that draws nothing.
#[test]
fn the_overlay_and_the_last_view_both_hide_it() {
    let mut app = a_game();
    let legs = [Point::new(10, 10), Point::new(40, 20)];
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, &legs)];
    }
    assert_eq!(app.fleet_paths().len(), 1);

    app.scan_overlays.fleet_paths = false;
    assert!(app.fleet_paths().is_empty());
    app.scan_overlays.fleet_paths = true;

    for view in [
        ScanView::Normal,
        ScanView::SurfaceMineral,
        ScanView::MineralConcentration,
        ScanView::PlanetValue,
        ScanView::Population,
    ] {
        app.scan_view = view;
        assert_eq!(app.fleet_paths().len(), 1, "{view:?}");
    }
    app.scan_view = ScanView::NoPlayerInfo;
    assert!(
        app.fleet_paths().is_empty(),
        "No Player Information draws no paths"
    );
}

/// Only a fleet described in full — one of this player's — has orders to
/// draw. Somebody else's is skipped whatever waypoints the engine holds.
#[test]
fn nobody_elses_path_is_drawn() {
    let mut app = a_game();
    let legs = [Point::new(10, 10), Point::new(40, 20)];
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(1, &legs), fleet(0, &legs)];
    }
    assert_eq!(app.fleet_paths().len(), 1, "only this player's");
}

/// The ship filters narrow the paths exactly as they narrow the arrows.
#[test]
fn the_ship_filters_hide_a_path_too() {
    let mut app = a_game();
    let legs = [Point::new(10, 10), Point::new(40, 20)];
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, &legs)];
    }
    assert_eq!(app.fleet_paths().len(), 1);

    app.scan_overlays.ship_design_filter = true;
    app.scan_design_filter = 0;
    assert!(app.fleet_paths().is_empty(), "nothing of it is counted");

    // Unless it is the fleet the pane is showing.
    app.selection.fleet = Some(0);
    assert_eq!(app.fleet_paths().len(), 1);
}
