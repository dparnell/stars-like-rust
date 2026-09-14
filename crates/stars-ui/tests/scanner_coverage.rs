//! The scanner-coverage discs — `DrawScanner`'s `grbitScan & 0x20` block.
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "coverage".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.scan_overlays.scanner_coverage = true;
    app
}

fn fleet(owner: i16, at: Point, design: u8) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: at,
        orbiting: None,
        stacks: vec![ShipStack {
            design,
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

/// Nothing at all with the overlay off.
#[test]
fn the_overlay_can_be_turned_off() {
    let mut app = a_game();
    assert!(!app.scanner_coverage().is_empty(), "a home planet scans");
    app.scan_overlays.scanner_coverage = false;
    assert!(app.scanner_coverage().is_empty());
}

/// Only this player's things contribute.
#[test]
fn only_your_own_planets_and_fleets_are_drawn() {
    let mut app = a_game();
    let mine: Vec<_> = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .filter(|p| p.owner == Some(0))
        .map(|p| p.position)
        .collect();
    assert!(!mine.is_empty(), "the player has a home world");

    let discs = app.scanner_coverage();
    for disc in &discs {
        assert!(
            mine.contains(&Some(disc.position)) || fleet_at(&app, disc.position),
            "a disc somewhere this player has nothing: {disc:?}"
        );
    }

    // An enemy fleet parked next door adds nothing.
    let there = Point::new(400, 400);
    if let Some(game) = app.game.as_mut() {
        game.fleets.push(fleet(1, there, 0));
    }
    let after = app.scanner_coverage();
    assert!(!after.iter().any(|d| d.position == there));
}

fn fleet_at(app: &App, at: Point) -> bool {
    app.game
        .as_ref()
        .is_some_and(|game| game.fleets.iter().any(|fleet| fleet.position == at))
}

/// The percentage in the toolbar narrows every disc, rounding to nearest the
/// way `MulDiv` does.
#[test]
fn the_toolbar_percentage_scales_every_radius() {
    let mut app = a_game();
    assert_eq!(
        app.coverage_scaled(300),
        300,
        "a hundred per cent is a no-op"
    );

    app.set_scan_coverage("50");
    assert_eq!(app.coverage_scaled(300), 150);
    assert_eq!(
        app.coverage_scaled(75),
        38,
        "MulDiv rounds rather than cuts"
    );

    let full = {
        app.set_scan_coverage("100");
        app.scanner_coverage()
    };
    app.set_scan_coverage("50");
    let half = app.scanner_coverage();
    assert_eq!(full.len(), half.len(), "the same discs, smaller");
    for (a, b) in full.iter().zip(half.iter()) {
        assert!(b.radius < a.radius || a.radius == 0, "{a:?} against {b:?}");
    }
}

/// A fleet takes the **largest** range among its designs, and a fleet with no
/// scanner at all gets no disc.
#[test]
fn a_fleet_scans_at_its_best_design() {
    let app = a_game();
    let here = Point::new(100, 100);
    let designs = app
        .game
        .as_ref()
        .expect("a game")
        .designs
        .first()
        .expect("this player's designs")
        .len();
    assert!(designs > 0, "a new game starts with designs");

    let ranges: Vec<_> = (0..designs)
        .map(|slot| {
            let f = fleet(0, here, u8::try_from(slot).expect("a slot"));
            app.fleet_scan_range(&f).normal
        })
        .collect();
    let best = ranges.iter().copied().max().unwrap_or(0);
    assert!(best > 0, "a starting design carries a scanner: {ranges:?}");

    // One fleet holding every design scans at the best of them.
    let mut all = fleet(0, here, 0);
    all.stacks = (0..designs)
        .map(|slot| ShipStack {
            design: u8::try_from(slot).expect("a slot"),
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        })
        .collect();
    assert_eq!(app.fleet_scan_range(&all).normal, best);

    // And an empty fleet scans not at all.
    let mut empty = fleet(0, here, 0);
    empty.stacks.clear();
    assert_eq!(app.fleet_scan_range(&empty).normal, 0);
}

/// The penetrating pass comes after every normal disc, so it paints on top.
#[test]
fn the_penetrating_discs_come_last() {
    let app = a_game();
    let discs = app.scanner_coverage();
    let first_deep = discs.iter().position(|d| d.penetrating);
    if let Some(at) = first_deep {
        assert!(
            discs[at..].iter().all(|d| d.penetrating),
            "the two passes are not interleaved"
        );
    }
}

/// The two kinds of coverage: a normal range in dark red and a penetrating
/// one in dark yellow over it. The tutorial's Armed Probe — a Rhino on a
/// Jack of All Trades' Scout hull, whose built-in scanner penetrates twenty
/// — draws both; the home world's Scoper 150 draws the red disc alone.
#[test]
fn a_penetrating_scanner_draws_its_own_disc() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    app.scan_overlays.scanner_coverage = true;
    let discs = app.scanner_coverage();
    let game = app.game.as_ref().expect("a game");
    let probe = game
        .fleets
        .iter()
        .find(|f| f.owner == 0 && f.id == 0)
        .expect("Armed Probe #1");
    let at_probe: Vec<_> = discs
        .iter()
        .filter(|d| d.position == probe.position)
        .collect();
    assert!(
        at_probe.iter().any(|d| !d.penetrating && d.radius == 54),
        "a normal disc of fifty-four: {at_probe:?}"
    );
    assert!(
        at_probe.iter().any(|d| d.penetrating && d.radius == 20),
        "and a penetrating one of twenty: {at_probe:?}"
    );
    // The penetrating discs are painted after every normal one, so they lie
    // on top, in their own colour.
    let first_deep = discs
        .iter()
        .position(|d| d.penetrating)
        .expect("a deep disc");
    assert!(discs[..first_deep].iter().all(|d| !d.penetrating));
    assert!(discs[first_deep..].iter().all(|d| d.penetrating));
    assert_ne!(stars_ui::COVERAGE_NORMAL, stars_ui::COVERAGE_PENETRATING);
}
