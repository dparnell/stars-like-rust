//! The pop-up summary a press in the scanner's status bar raises.
//!
//! `ScannerWndProc` (`1058:043a`) → `Popup` (`10c0:0c7c`). See
//! `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::popup::{damage_text, Popup};
use stars_ui::{App, ScanObject, ScanThing, Selection};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "popup".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.selection = Selection::default();
    app
}

fn stack(design: u8, count: i32) -> ShipStack {
    ShipStack {
        design,
        count,
        damaged_pct: 0,
        damage_pct: 0,
    }
}

fn a_fleet(at: Point, stacks: Vec<ShipStack>) -> Fleet {
    Fleet {
        id: 0,
        owner: 0,
        position: at,
        orbiting: None,
        stacks,
        cargo: stars_core::fleet::Cargo::default(),
        battle_plan: 0,
        warp: None,
        waypoints: Vec::new(),
        name: None,
        repeat_orders: false,
        direction: None,
    }
}

/// A planet gets the four-row summary, with the ID one-based and the
/// coordinates from the scan.
#[test]
fn a_planet_gets_its_name_id_and_coordinates() {
    let mut app = a_game();
    let (home, at) = {
        let game = app.game.as_ref().expect("a game");
        let home = game
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(0))
            .expect("a home world");
        (home.id, home.position.expect("a position"))
    };
    app.select_object(ScanObject::Planet(home));
    let Some(Popup::Planet(summary)) = app.status_bar_popup() else {
        panic!("a planet summary");
    };
    assert_eq!(
        summary.values,
        [
            app.planet_name(home),
            (home + 1).to_string(),
            at.x.to_string(),
            at.y.to_string(),
        ]
    );
}

/// A fleet **in orbit** is the planet, exactly as the status bar shows it:
/// `ChangeScanSel` turns the scan into the planet whenever the point has one,
/// so the ship list is reached only out in open space.
#[test]
fn a_fleet_in_orbit_gets_the_planets_summary() {
    let mut app = a_game();
    let at = {
        let game = app.game.as_ref().expect("a game");
        game.planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(0))
            .expect("a home world")
            .position
            .expect("a position")
    };
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![a_fleet(at, vec![stack(0, 3)])];
    }
    app.select_object(ScanObject::Fleet(0));
    assert!(matches!(app.status_bar_popup(), Some(Popup::Planet(_))));

    // Move it away and the same fleet gets its ship list instead.
    if let Some(game) = app.game.as_mut() {
        game.fleets[0].position = Point::new(at.x + 77, at.y + 77);
    }
    app.selection = Selection::default();
    app.select_object(ScanObject::Fleet(0));
    assert!(matches!(app.status_bar_popup(), Some(Popup::Fleet(_))));
}

/// The ship list is one row per design the fleet holds any of, in design-slot
/// order, and the counts are the stack counts.
#[test]
fn the_ship_list_is_one_row_per_design() {
    let mut app = a_game();
    let names: Vec<String> = app
        .game
        .as_ref()
        .expect("a game")
        .designs
        .first()
        .expect("this player's designs")
        .iter()
        .map(|design| design.name.clone())
        .collect();
    if let Some(game) = app.game.as_mut() {
        // Out of slot order on purpose: the pop-up sorts them back.
        game.fleets = vec![a_fleet(
            Point::new(300, 300),
            vec![stack(2, 5), stack(0, 12), stack(1, 0)],
        )];
    }
    app.select_object(ScanObject::Fleet(0));
    let Some(Popup::Fleet(summary)) = app.status_bar_popup() else {
        panic!("a ship list");
    };
    assert_eq!(summary.rows.len(), 2, "the empty stack is not a row");
    assert_eq!(summary.rows[0].name, names[0]);
    assert_eq!(summary.rows[0].count, "12");
    assert_eq!(summary.rows[1].name, names[2]);
    assert_eq!(summary.rows[1].count, "5");
    // The scanner leaves `fRedDamage` alone, so no damage column.
    assert!(!summary.show_damage);
    assert!(!summary.damage_column());
}

/// A fleet with nothing left in it has no rows, which is what makes the
/// pop-up say `None`.
#[test]
fn an_empty_fleet_has_no_rows() {
    let mut app = a_game();
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![a_fleet(Point::new(300, 300), vec![stack(0, 0)])];
    }
    app.select_object(ScanObject::Fleet(0));
    let Some(Popup::Fleet(summary)) = app.status_bar_popup() else {
        panic!("a ship list");
    };
    assert!(summary.rows.is_empty());
}

/// A space object raises nothing: the press is guarded on
/// `sel.scan.grobjFull` having the planet or the fleet bit.
#[test]
fn a_space_object_raises_nothing() {
    let mut app = a_game();
    if let Some(game) = app.game.as_mut() {
        game.wormholes = vec![stars_core::wormhole::Wormhole {
            id: 1,
            position: Point::new(120, 120),
            stability: 2,
            years_still: 0,
            dest_known: false,
            include: true,
            partner: 0,
            detected_by: 0,
            traversed_by: 0,
            turn: 0,
        }];
    }
    app.select_object(ScanObject::Thing(ScanThing::Wormhole(0)));
    assert!(app.status_bar_popup().is_none());
}

/// `"%d@%d%%"`: how many ships are damaged, and how badly.
///
/// Both halves are floored at one, and the percentage is the whole packed
/// damage word divided by 640 — the original's shortcut for `pctDp / 5`,
/// since `pctDp` is in 500ths.
#[test]
fn damage_reads_as_ships_at_a_percentage() {
    // Undamaged: no damage cell at all.
    assert_eq!(damage_text(10, 0, 0), None);
    // Half of ten ships, at 250/500ths — fifty per cent.
    assert_eq!(damage_text(10, 50, 250), Some("5@50%".to_string()));
    // One ship in a hundred, barely scratched: both halves floor at one.
    assert_eq!(damage_text(100, 1, 1), Some("1@1%".to_string()));
    // The packed word is `pctSh | (pctDp << 7)`, and 640 divides it back.
    assert_eq!(damage_text(4, 100, 500), Some("4@100%".to_string()));
}
