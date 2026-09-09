//! Laying a course on the map: shift-click adds a waypoint, and a waypoint
//! near something lands on it.
//!
//! `ScannerWndProc` (`1058:0ae1`) sends a left click to `FAddWayPoint` when a
//! fleet is selected and either shift is held or Add Way Points mode is on.
//! `FAddWayPoint` (`1058:7504`) then measures the click against the nearest
//! object and keeps the *object's* position when the two are within
//! `ScanToPt(0x14)`.

use stars_core::fleet::{grobj, Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "waypoints".to_string(),
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

fn a_fleet(id: u16, owner: i16, at: Point) -> Fleet {
    Fleet {
        id,
        owner,
        position: at,
        orbiting: None,
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

/// A point with nothing within `clear` light years of it, so a test can put
/// one object there and know it is the nearest thing to any click nearby.
fn empty_spot(app: &App, clear: f64) -> Point {
    let game = app.game.as_ref().expect("a game");
    let (min_x, min_y, max_x, max_y) = app.extent().expect("an extent");
    let occupied: Vec<Point> = game
        .planets
        .iter()
        .chain(game.known_planets.iter())
        .filter_map(|p| p.position)
        .chain(game.fleets.iter().map(|f| f.position))
        .collect();
    #[allow(clippy::cast_possible_truncation)]
    let (min_x, min_y, max_x, max_y) = (min_x as i16, min_y as i16, max_x as i16, max_y as i16);
    for y in (min_y..=max_y).step_by(7) {
        for x in (min_x..=max_x).step_by(7) {
            let at = Point::new(x, y);
            if occupied
                .iter()
                .all(|p| stars_core::movement::distance(*p, at) > clear)
            {
                return at;
            }
        }
    }
    panic!("nowhere empty in this galaxy");
}

/// Put one of our fleets on the map and select it.
fn our_fleet_at(app: &mut App, at: Point) -> usize {
    let game = app.game.as_mut().expect("a game");
    let id = u16::try_from(game.fleets.len() + 1).expect("a small galaxy");
    game.fleets.push(a_fleet(id, 0, at));
    let index = game.fleets.len() - 1;
    app.selection.fleet = Some(index);
    app.selection.on_fleet = true;
    index
}

/// The last waypoint of a fleet.
fn last_leg(app: &App, fleet: usize) -> stars_core::fleet::Waypoint {
    app.game.as_ref().expect("a game").fleets[fleet]
        .waypoints
        .last()
        .expect("a waypoint")
        .clone()
}

/// A click a few light years off a planet lands **on** the planet, and the
/// waypoint records it.
#[test]
fn a_waypoint_near_a_planet_snaps_to_it() {
    let mut app = a_game();
    let (id, at) = {
        let game = app.game.as_ref().expect("a game");
        let planet = game
            .planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld");
        (planet.id, planet.position.expect("a position"))
    };
    // The fleet starts well away, so it is not itself the nearest object.
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);

    assert!(app.add_waypoint(at.x + 3, at.y - 2, 20.0));
    let leg = last_leg(&app, fleet);
    assert_eq!(leg.position, at, "it lands on the planet, not beside it");
    assert_eq!(leg.target, u16::try_from(id).ok());
    assert_eq!(leg.target_class, grobj::PLANET);
}

/// The same click with nothing in reach stays exactly where it was put, and
/// names nothing.
#[test]
fn a_waypoint_out_of_reach_stays_in_deep_space() {
    let mut app = a_game();
    let at = {
        let game = app.game.as_ref().expect("a game");
        game.planets
            .iter()
            .find(|p| p.owner == Some(0))
            .expect("a homeworld")
            .position
            .expect("a position")
    };
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);

    let asked = Point::new(at.x + 3, at.y - 2);
    assert!(app.add_waypoint(asked.x, asked.y, 0.0));
    let leg = last_leg(&app, fleet);
    assert_eq!(leg.position, asked);
    assert_eq!(leg.target, None);
    assert_eq!(leg.target_class, grobj::POSITION);
}

/// Fleets are snapped to as well as planets, and the class says which — a
/// bare id cannot tell planet 7 from fleet 7.
#[test]
fn a_waypoint_near_a_fleet_snaps_to_the_fleet() {
    let mut app = a_game();
    let target_at = empty_spot(&app, 60.0);
    let target_id = {
        let game = app.game.as_mut().expect("a game");
        let id = 41;
        game.fleets.push(a_fleet(id, 1, target_at));
        id
    };
    let start = empty_spot(&app, 60.0);
    // `empty_spot` is deterministic, so the mover would land on top of the
    // target; move it well clear first.
    let start = Point::new(start.x, start.y + 90);
    let fleet = our_fleet_at(&mut app, start);

    assert!(app.add_waypoint(target_at.x + 2, target_at.y + 2, 20.0));
    let leg = last_leg(&app, fleet);
    assert_eq!(leg.position, target_at);
    assert_eq!(leg.target, Some(target_id));
    assert_eq!(leg.target_class, grobj::FLEET);
}

/// Snapping onto the point the fleet is already at adds nothing: the leg
/// would go nowhere.
#[test]
fn a_leg_that_goes_nowhere_is_refused() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    assert!(!app.add_waypoint(start.x + 1, start.y, 20.0));
    assert!(app.game.as_ref().expect("a game").fleets[fleet]
        .waypoints
        .is_empty());
}

/// A fleet cannot hold more than `WAYPOINT_MAX` orders, the origin among
/// them; the original beeps and puts up an alert instead.
#[test]
fn the_order_list_has_a_ceiling() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);

    let mut added = 0;
    for step in 1..200_i16 {
        if app.add_waypoint(start.x + step * 2, start.y + 1, 0.0) {
            added += 1;
        } else {
            break;
        }
    }
    let held = app.game.as_ref().expect("a game").fleets[fleet]
        .waypoints
        .len();
    assert_eq!(held, App::WAYPOINT_MAX);
    assert_eq!(added, App::WAYPOINT_MAX);
    assert_eq!(App::WAYPOINT_MAX, 0x57);
    // And it stays refused.
    assert!(!app.add_waypoint(start.x + 500, start.y + 1, 0.0));
}

/// Another player's fleet takes no orders from you.
#[test]
fn only_your_own_fleets_take_a_waypoint() {
    let mut app = a_game();
    let at = empty_spot(&app, 60.0);
    let theirs = {
        let game = app.game.as_mut().expect("a game");
        game.fleets.push(a_fleet(9, 1, at));
        game.fleets.len() - 1
    };
    app.selection.fleet = Some(theirs);
    assert!(!app.add_waypoint(at.x + 40, at.y, 0.0));
}

/// Shift-clicking the map lays a leg for the selected fleet without leaving
/// select mode, which is the whole point of the shift.
#[test]
fn shift_clicking_the_map_adds_a_waypoint() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    assert!(!app.add_waypoints, "not in Add Way Points mode");

    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mut draw = |events: Vec<egui::Event>, modifiers: egui::Modifiers| {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            modifiers,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::galaxy::view(&mut app, ui);
            });
        });
    };

    // One frame to lay the map out, so the click has something to land on.
    draw(Vec::new(), egui::Modifiers::default());

    let shift = egui::Modifiers::SHIFT;
    let at = screen.center();
    draw(
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: shift,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: shift,
            },
        ],
        shift,
    );

    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        1,
        "the shift-click laid a leg"
    );
}

/// Without the shift it selects instead, and the fleet keeps its orders.
#[test]
fn a_plain_click_does_not_add_one() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);

    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mut draw = |events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::galaxy::view(&mut app, ui);
            });
        });
    };

    draw(Vec::new());
    let at = screen.center();
    draw(vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        },
    ]);

    assert!(app.game.as_ref().expect("a game").fleets[fleet]
        .waypoints
        .is_empty());
}
