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
///
/// Waypoint 0 is where the fleet *is* — every fleet the loader builds has
/// one, and the indices of everything after it depend on that.
fn our_fleet_at(app: &mut App, at: Point) -> usize {
    let index = add_our_fleet(app, at);
    app.select_object(stars_ui::ScanObject::Fleet(index));
    index
}

/// The same, without selecting it.
fn add_our_fleet(app: &mut App, at: Point) -> usize {
    let game = app.game.as_mut().expect("a game");
    let id = u16::try_from(game.fleets.len() + 1).expect("a small galaxy");
    let mut fleet = a_fleet(id, 0, at);
    fleet.waypoints.push(stars_core::fleet::Waypoint {
        position: at,
        target: None,
        target_class: grobj::POSITION,
        warp: 0,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    });
    game.fleets.push(fleet);
    game.fleets.len() - 1
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
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        1,
        "only waypoint 0, where it already is"
    );
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
    // Waypoint 0 was already there, so the legs laid are one fewer.
    assert_eq!(added, App::WAYPOINT_MAX - 1);
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
        2,
        "waypoint 0 and the leg the shift-click laid"
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

    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        1,
        "only waypoint 0"
    );
}

// --- Dragging one that is already there ----------------------------------
//
// `ScannerWndProc` reaches this on any left press that is not the add path:
// `FNearAWayPoint` (`1058:8074`) asks whether one of the selected fleet's own
// waypoints is within reach — with the same mask, so the same twenty pixels —
// and hands the press to `FHandleWayPointDrag` (`1058:8176`).

/// Dropping a waypoint near a planet lands it on the planet, exactly as
/// laying a new one does.
#[test]
fn a_dragged_waypoint_snaps_too() {
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
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    // One leg out into nowhere, then drag its end near the planet.
    assert!(app.add_waypoint(start.x + 40, start.y + 40, 0.0));

    assert!(app.move_waypoint(1, at.x - 4, at.y + 1, 20.0));
    let leg = last_leg(&app, fleet);
    assert_eq!(leg.position, at);
    assert_eq!(leg.target, u16::try_from(id).ok());
    assert_eq!(leg.target_class, grobj::PLANET);

    // And with the snap suppressed — which is what holding shift during a
    // drag does — it stays where it was put.
    assert!(app.move_waypoint(1, at.x - 4, at.y + 1, 0.0));
    let leg = last_leg(&app, fleet);
    assert_eq!(leg.position, Point::new(at.x - 4, at.y + 1));
    assert_eq!(leg.target, None);
    assert_eq!(leg.target_class, grobj::POSITION);
}

/// The grab picks the **nearest** waypoint, not the first one in reach.
#[test]
fn the_grab_takes_the_nearest_waypoint() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 30, start.y, 0.0));
    assert!(app.add_waypoint(start.x + 36, start.y, 0.0));

    // Within reach of both; the second is closer.
    assert_eq!(app.waypoint_at(start.x + 35, start.y, 10.0), Some(2));
    assert_eq!(app.waypoint_at(start.x + 31, start.y, 10.0), Some(1));
    assert_eq!(app.waypoint_at(start.x + 30, start.y, 1.0), Some(1));
    // Waypoint 0 is where the fleet is and is never grabbed.
    assert_eq!(app.waypoint_at(start.x, start.y, 10.0), None);
}

/// A waypoint dropped onto its neighbour is a request to delete it: the drag
/// puts it back and the map asks.
#[test]
fn dropping_one_on_its_neighbour_offers_to_delete_it() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 30, start.y, 0.0));
    assert!(app.add_waypoint(start.x + 60, start.y, 0.0));
    let middle = Point::new(start.x + 30, start.y);

    // Drag the middle waypoint onto the last one.
    assert!(app.move_waypoint(1, start.x + 60, start.y, 0.0));
    assert!(app.waypoint_meets_neighbour(1));

    // The view puts it back and raises the question.
    assert!(app.revert_waypoint(1, middle));
    assert!(!app.waypoint_meets_neighbour(1));
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet].waypoints[1].position,
        middle
    );

    // Saying yes drops it.
    assert!(app.delete_waypoint(1));
    let held = &app.game.as_ref().expect("a game").fleets[fleet].waypoints;
    assert_eq!(held.len(), 2);
    assert_eq!(held[1].position, Point::new(start.x + 60, start.y));
}

/// Deleting a waypoint whose neighbours then coincide collapses the pair,
/// rather than leaving a leg of no length behind.
#[test]
fn a_delete_collapses_a_doubled_pair() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    let there = Point::new(start.x + 30, start.y);
    assert!(app.add_waypoint(there.x, there.y, 0.0));
    assert!(app.add_waypoint(start.x + 60, start.y, 0.0));
    assert!(app.add_waypoint(there.x, there.y, 0.0));
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        4
    );

    // Waypoints 1 and 3 are on the same point, so dropping 2 leaves them
    // adjacent and identical.
    assert!(app.delete_waypoint(2));
    let held = &app.game.as_ref().expect("a game").fleets[fleet].waypoints;
    assert_eq!(held.len(), 2, "the doubled pair collapsed as well");
    assert_eq!(held[1].position, there);
}

/// Dragging works with Add Way Points mode **off**, which is the whole of
/// this change: `ScannerWndProc` reaches the drag on any press that is not
/// the add path.
#[test]
fn dragging_needs_neither_the_mode_nor_the_shift() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 40, start.y, 0.0));
    assert!(!app.add_waypoints);

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

    // Press somewhere on the map and drag: whether the press lands on the
    // waypoint depends on where the map put it, so this checks the wiring
    // rather than the arithmetic — `dragging_waypoint` is only ever set by
    // the branch this test is about.
    let from = screen.center();
    draw(vec![
        egui::Event::PointerMoved(from),
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
    ]);
    draw(vec![egui::Event::PointerMoved(
        from + egui::vec2(40.0, 0.0),
    )]);
    draw(vec![egui::Event::PointerButton {
        pos: from + egui::vec2(40.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    }]);

    assert_eq!(app.dragging_waypoint, None, "the drag was let go of");
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        2,
        "and the drag added nothing"
    );
}

/// A drag moves the waypoint every frame it is held, and the log keeps one
/// record with the final position rather than one per frame.
#[test]
fn a_drag_leaves_one_order_not_a_trail_of_them() {
    use stars_formats::LogRecordType;

    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 40, start.y, 0.0));
    let before = app.orders.len();

    for step in 1..25_i16 {
        assert!(app.move_waypoint(1, start.x + 40, start.y + step, 0.0));
    }

    let updates = app.orders[before..]
        .iter()
        .filter(|r| r.record_type == LogRecordType::FleetOrderUpdate)
        .count();
    assert_eq!(updates, 1, "one record, however long the drag");
    let last = app
        .orders
        .last()
        .expect("an order")
        .as_waypoint()
        .expect("a waypoint order");
    assert_eq!(last.y, start.y + 24, "and it holds where the drag ended");
    assert_eq!(last.waypoint_index, 1);
}

/// The logged `grobj` is the waypoint's own class. Before waypoints could
/// land on anything but a planet, "has a target" and "is a planet" were the
/// same test; they are not any more.
#[test]
fn the_log_records_what_the_waypoint_landed_on() {
    let mut app = a_game();
    let target_at = empty_spot(&app, 60.0);
    {
        let game = app.game.as_mut().expect("a game");
        game.fleets.push(a_fleet(41, 1, target_at));
    }
    let start = Point::new(target_at.x, target_at.y + 90);
    our_fleet_at(&mut app, start);

    assert!(app.add_waypoint(target_at.x + 2, target_at.y, 20.0));
    let order = app
        .orders
        .last()
        .expect("an order")
        .as_waypoint()
        .expect("a waypoint order");
    assert_eq!(order.grobj, grobj::FLEET, "a fleet, not a planet");
    assert_eq!(order.target_id, 41);
}

// --- Taking one back off -------------------------------------------------
//
// `FHandleKey` (`1018:165a`) sends Backspace and Delete straight to
// `DeleteCurWayPoint` (`1050:9b08`) whenever the selection is a fleet — no
// mode, no modifier, and no question first. It acts on `sel.iwpAct`, the
// waypoint the map has in hand.

/// Laying a waypoint leaves it in hand, and Delete takes it straight back
/// off — no mode, no modifier, no question.
#[test]
fn delete_removes_the_waypoint_in_hand() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 30, start.y, 0.0));
    assert!(app.add_waypoint(start.x + 60, start.y, 0.0));
    assert_eq!(app.selection.waypoint, Some(2), "the new one is in hand");

    assert!(app.delete_current_waypoint());
    let held = &app.game.as_ref().expect("a game").fleets[fleet].waypoints;
    assert_eq!(held.len(), 2);
    assert_eq!(held[1].position, Point::new(start.x + 30, start.y));
    // `fBackup` is 8, so it falls back rather than stepping on.
    assert_eq!(app.selection.waypoint, Some(1));

    assert!(app.delete_current_waypoint());
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        1
    );
    assert_eq!(app.selection.waypoint, Some(0));
}

/// Waypoint 0 is where the fleet is, not an order, and cannot be deleted —
/// `DeleteCurWayPoint` beeps and returns.
#[test]
fn waypoint_zero_is_never_deleted() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 30, start.y, 0.0));

    assert!(app.delete_current_waypoint());
    assert_eq!(app.selection.waypoint, Some(0));
    // Now only waypoint 0 is left, and it stays.
    assert!(!app.delete_current_waypoint());
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        1
    );
}

/// With nothing in hand there is nothing to delete.
#[test]
fn delete_does_nothing_with_no_waypoint_in_hand() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let fleet = our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 30, start.y, 0.0));
    app.selection.waypoint = None;

    assert!(!app.delete_current_waypoint());
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[fleet]
            .waypoints
            .len(),
        2
    );
}

/// Selecting a different fleet lets go of the waypoint, so Delete cannot
/// reach into the orders of a fleet that is no longer in front.
#[test]
fn changing_fleet_lets_go_of_the_waypoint() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    let second = add_our_fleet(&mut app, Point::new(start.x, start.y + 90));
    let first = our_fleet_at(&mut app, start);
    assert_ne!(first, second);
    assert!(app.add_waypoint(start.x + 30, start.y, 0.0));
    assert_eq!(app.selection.waypoint, Some(1));

    app.select_object(stars_ui::ScanObject::Fleet(second));
    assert_eq!(app.selection.waypoint, None);
    assert!(!app.delete_current_waypoint());
    // And coming back to the first leaves its orders untouched.
    app.select_object(stars_ui::ScanObject::Fleet(first));
    assert_eq!(
        app.game.as_ref().expect("a game").fleets[first]
            .waypoints
            .len(),
        2
    );
}

/// The status bar names the waypoint in hand, which is the only thing that
/// says what Delete will take.
#[test]
fn the_bar_names_the_waypoint_in_hand() {
    let mut app = a_game();
    let start = empty_spot(&app, 60.0);
    our_fleet_at(&mut app, start);
    assert!(app.add_waypoint(start.x + 30, start.y, 0.0));

    let bar = app.status_bar();
    assert_eq!(bar.id, "WP #1");
    assert_eq!(bar.name, "Deep Space Waypoint");
}
