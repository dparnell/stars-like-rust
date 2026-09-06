//! The scanner's orbit rings.
//!
//! See `docs/ui/scanner.md`.

use stars_core::design::ShipDesign;
use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, OrbitRing};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "rings".to_string(),
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

fn fleet(owner: i16, orbiting: u16, design: u8, count: i32) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: Point::new(0, 0),
        orbiting: Some(orbiting),
        stacks: vec![ShipStack {
            design,
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

/// Set the fleets of a game to exactly these.
fn with_fleets(app: &mut App, fleets: Vec<Fleet>) {
    app.game.as_mut().expect("a game").fleets = fleets;
}

/// One ring per planet, coloured by whose fleets are there.
#[test]
fn the_ring_says_whose_fleets_are_in_orbit() {
    let mut app = a_game();
    with_fleets(
        &mut app,
        vec![
            fleet(0, 10, 0, 3),
            fleet(1, 20, 0, 4),
            fleet(0, 30, 0, 1),
            fleet(1, 30, 0, 1),
        ],
    );
    let rings = app.orbit_rings();
    assert_eq!(rings.get(&10), Some(&OrbitRing::Yours));
    assert_eq!(rings.get(&20), Some(&OrbitRing::Theirs));
    assert_eq!(rings.get(&30), Some(&OrbitRing::Both), "some of each");
    assert_eq!(rings.get(&40), None, "nothing in orbit there");

    // Two fleets of the same side do not make it "both".
    with_fleets(&mut app, vec![fleet(0, 10, 0, 1), fleet(0, 10, 0, 1)]);
    assert_eq!(app.orbit_rings().get(&10), Some(&OrbitRing::Yours));
}

/// The three rings are three rows of the scanner's own sheet, and their
/// colours are the sprite's.
#[test]
fn the_three_rings_are_three_rows() {
    assert_eq!(OrbitRing::Yours.row(), 0);
    assert_eq!(OrbitRing::Theirs.row(), 1);
    assert_eq!(OrbitRing::Both.row(), 2);
    assert_eq!(OrbitRing::Yours.colour(), [0xc0, 0xc0, 0xc0]);
    assert_eq!(OrbitRing::Theirs.colour(), [0xff, 0x00, 0x00]);
    assert_eq!(OrbitRing::Both.colour(), [0xff, 0x00, 0xff]);
}

/// A fleet with nothing in it, or one going nowhere near a planet, earns no
/// ring.
#[test]
fn an_empty_or_wandering_fleet_earns_nothing() {
    let mut app = a_game();
    let mut adrift = fleet(0, 10, 0, 5);
    adrift.orbiting = None;
    with_fleets(&mut app, vec![adrift, fleet(0, 11, 0, 0)]);
    assert!(app.orbit_rings().is_empty());
}

/// **The ship filters narrow the rings**, which is what the manual means by
/// "only those planets orbited by the selected ships will have orbit rings".
#[test]
fn the_ship_filters_narrow_the_rings() {
    let mut app = a_game();
    with_fleets(
        &mut app,
        vec![fleet(0, 10, 0, 3), fleet(0, 11, 1, 3), fleet(1, 20, 0, 3)],
    );
    assert_eq!(app.orbit_rings().len(), 3, "all three to begin with");

    // Filter my own fleets down to design 1: planet 10 loses its ring and 11
    // keeps one, while the opponent's is untouched.
    app.scan_overlays.ship_design_filter = true;
    app.scan_design_filter = 1 << 1;
    let rings = app.orbit_rings();
    assert_eq!(rings.get(&10), None, "design 0 is filtered out");
    assert_eq!(rings.get(&11), Some(&OrbitRing::Yours));
    assert_eq!(rings.get(&20), Some(&OrbitRing::Theirs), "not my filter");

    // And a planet that had both loses the ring entirely when neither side's
    // ships pass.
    app.scan_design_filter = 0;
    with_fleets(&mut app, vec![fleet(0, 30, 0, 1), fleet(1, 30, 0, 1)]);
    assert_eq!(
        app.orbit_rings().get(&30),
        Some(&OrbitRing::Theirs),
        "mine are filtered out, theirs are not"
    );
}

/// The enemy class filter narrows the other side's rings the same way.
#[test]
fn the_class_filter_narrows_the_other_side() {
    let mut app = a_game();
    {
        let game = app.game.as_mut().expect("a game");
        let scout = stars_core::components::HULLS
            .iter()
            .position(|h| h.name == "Scout")
            .expect("a Scout");
        let designs = game.designs.get_mut(1).expect("their designs");
        designs.clear();
        designs.push(ShipDesign {
            hull_id: i16::try_from(scout).expect("a hull id"),
            slots: Vec::new(),
            name: "a scout".to_string(),
            picture: 0,
            stored_armor: 0,
        });
    }
    with_fleets(&mut app, vec![fleet(1, 20, 0, 3), fleet(0, 10, 0, 3)]);

    app.scan_overlays.enemy_class_filter = true;
    app.scan_class_filter = 1 << stars_core::design::ShipClass::Warship.index();
    let rings = app.orbit_rings();
    assert_eq!(rings.get(&20), None, "no warships there");
    assert_eq!(
        rings.get(&10),
        Some(&OrbitRing::Yours),
        "mine are untouched"
    );

    app.scan_class_filter = 1 << stars_core::design::ShipClass::Scout.index();
    assert_eq!(app.orbit_rings().get(&20), Some(&OrbitRing::Theirs));
}

/// The map draws with rings on it.
#[test]
fn the_map_draws_with_rings() {
    let mut app = a_game();
    with_fleets(
        &mut app,
        vec![fleet(0, 10, 0, 3), fleet(1, 20, 0, 4), fleet(0, 30, 0, 1)],
    );
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::galaxy::view(app_mut(&mut app), ui);
        });
    });
}

/// A borrow helper, so the closure above reads clearly.
fn app_mut(app: &mut App) -> &mut App {
    app
}
