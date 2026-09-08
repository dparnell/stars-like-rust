//! How a fleet is drawn on the map — `DrawScanner`'s fleet loop.
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, ScanView, SCAN_FRIEND, SCAN_OTHER, SCAN_YOURS};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "fleets".to_string(),
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

fn fleet(owner: i16, orbiting: Option<u16>) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: Point::new(10, 10),
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
        direction: None,
    }
}

/// Blue for this player, yellow for a friend, red for anybody else — the same
/// three the planets and the minefields use, not a colour per player.
#[test]
fn an_arrow_is_coloured_by_whose_fleet_it_is() {
    let mut app = a_game();
    assert_eq!(app.fleet_arrow_colour(&fleet(0, None)), SCAN_YOURS);
    assert_eq!(app.fleet_arrow_colour(&fleet(1, None)), SCAN_OTHER);

    if let Some(game) = app.game.as_mut() {
        game.players[0].relations = vec![0, 1, 2];
    }
    assert_eq!(
        app.fleet_arrow_colour(&fleet(1, None)),
        SCAN_FRIEND,
        "a friend's is yellow"
    );
    assert_eq!(
        app.fleet_arrow_colour(&fleet(2, None)),
        SCAN_OTHER,
        "an enemy's is red"
    );
}

/// A fleet in orbit has no arrow: its planet's ring is its mark.
#[test]
fn only_a_fleet_in_deep_space_gets_an_arrow() {
    assert!(App::fleet_draws_arrow(&fleet(0, None)));
    assert!(!App::fleet_draws_arrow(&fleet(0, Some(3))));
}

/// A fleet on the selected point is an 11x11 glyph rather than an arrow, and
/// there are two of them: one for this player and one for everybody else.
#[test]
fn a_fleet_on_the_selected_point_gets_its_own_glyph() {
    let app = a_game();
    assert_eq!(app.fleet_selected_cell(&fleet(0, None)), (0xb, 0x24));
    assert_eq!(app.fleet_selected_cell(&fleet(1, None)), (0xb, 0x2f));
}

/// The rings are drawn in the first three views only: the other three redraw
/// the planets themselves and the ring arm is guarded by `uVar8 < 3`.
#[test]
fn the_orbit_rings_belong_to_the_first_three_views() {
    let mut app = a_game();
    for view in [
        ScanView::Normal,
        ScanView::SurfaceMineral,
        ScanView::MineralConcentration,
    ] {
        app.scan_view = view;
        assert!(app.orbit_rings_visible(), "{view:?}");
    }
    for view in [
        ScanView::PlanetValue,
        ScanView::Population,
        ScanView::NoPlayerInfo,
    ] {
        app.scan_view = view;
        assert!(!app.orbit_rings_visible(), "{view:?}");
    }
}

/// A ring says whose fleets are there, and the two kinds add up to a third.
#[test]
fn a_ring_gathers_whose_fleets_are_in_orbit() {
    let mut app = a_game();
    let planet = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .first()
        .expect("a planet")
        .id;
    let at = u16::try_from(planet).expect("a planet id");

    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, Some(at))];
    }
    assert_eq!(
        app.orbit_rings().get(&at),
        Some(&stars_ui::OrbitRing::Yours)
    );

    if let Some(game) = app.game.as_mut() {
        game.fleets.push(fleet(1, Some(at)));
    }
    assert_eq!(app.orbit_rings().get(&at), Some(&stars_ui::OrbitRing::Both));

    if let Some(game) = app.game.as_mut() {
        game.fleets.remove(0);
    }
    assert_eq!(
        app.orbit_rings().get(&at),
        Some(&stars_ui::OrbitRing::Theirs)
    );
}

/// The ship filters hide arrows the same way they hide orbit rings — except
/// over the fleet the pane is showing, which is always drawn.
#[test]
fn the_ship_filters_hide_a_fleet_that_is_all_filtered_out() {
    let mut app = a_game();
    // A new game starts with the home fleet selected; the filters cannot hide
    // the selected one, so look at another index.
    app.selection.fleet = None;
    let mine = fleet(0, None);
    assert!(app.fleet_scan_visible(0, &mine), "nothing is filtered yet");

    app.scan_overlays.ship_design_filter = true;
    app.scan_design_filter = 0;
    assert!(
        !app.fleet_scan_visible(0, &mine),
        "no design is ticked, so nothing of this player's is counted"
    );

    app.selection.fleet = Some(0);
    assert!(
        app.fleet_scan_visible(0, &mine),
        "the selected fleet is drawn whatever the filters say"
    );

    app.scan_design_filter = 1;
    app.selection.fleet = None;
    assert!(
        app.fleet_scan_visible(0, &mine),
        "its design is ticked again"
    );
}
