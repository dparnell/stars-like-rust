//! The Cargo Transfer dialog — `TransferDlg`, the **Xfer** button — worked
//! through `App`. See `docs/ui/cargo-transfer.md`.

use stars_core::orders::{COLONISTS, FUEL};
use stars_ui::{App, ScanObject};

const STOVE_TOP: i16 = 0x0d;

/// The tutorial's world, with Santa Maria #3 (fleet 2, a colony ship with
/// a 25 kT hold) in hand at Stove Top.
fn colony_ship_in_hand() -> App {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    app.select_object(ScanObject::Planet(STOVE_TOP));
    assert!(app.goto_fleet(2), "Santa Maria #3");
    app
}

/// Xfer opens on the fleet in hand and the planet it orbits, with what each
/// side has; fuel can only pass where there is a starbase.
#[test]
fn xfer_opens_on_the_fleet_and_its_planet() {
    let mut app = colony_ship_in_hand();
    assert!(app.open_xfer());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(dialog.planet, STOVE_TOP);
    assert_eq!(dialog.cargo_capacity, 25);
    assert_eq!(dialog.aboard[..4], [0, 0, 0, 0]);
    assert!(dialog.fuel_here, "the home world has a starbase");
    let home = app.selected_planet().expect("Stove Top");
    assert_eq!(dialog.stock[..3], home.surface_min);
    assert_eq!(dialog.stock[COLONISTS], home.pop);
}

/// The arrows move what the giver has and the taker has room for, and
/// nothing reaches the game until OK.
#[test]
fn moves_are_clamped_and_kept_until_ok() {
    let mut app = colony_ship_in_hand();
    assert!(app.open_xfer());
    assert_eq!(app.xfer_move(COLONISTS, 10), 10);
    assert_eq!(app.xfer_move(COLONISTS, 100), 15, "the hold is 25");
    assert_eq!(app.xfer_move(0, 1), 0, "no room left for ironium");
    assert_eq!(app.xfer_move(COLONISTS, -30), -25, "only what is aboard");
    assert_eq!(app.xfer_move(0, 5), 5);
    let pop_before = app.selected_planet().expect("Stove Top").pop;
    {
        let game = app.game.as_ref().expect("a game");
        assert_eq!(
            game.fleets[2].cargo.minerals,
            [0, 0, 0],
            "nothing moved yet"
        );
    }
    assert_eq!(app.orders.len(), 0);

    app.xfer_ok();
    assert!(app.xfer.is_none());
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[2].cargo.minerals, [5, 0, 0]);
    assert_eq!(game.fleets[2].cargo.colonists, 0);
    assert_eq!(
        app.selected_planet().expect("Stove Top").pop,
        pop_before,
        "the colonists came back off"
    );
    assert_eq!(app.orders.len(), 1, "one transfer order for the lot");
}

/// A gauge press sets the hold to a figure; the far end is the whole hold.
#[test]
fn a_gauge_sets_the_hold() {
    let mut app = colony_ship_in_hand();
    assert!(app.open_xfer());
    assert_eq!(app.xfer_set(COLONISTS, 25), 25);
    assert_eq!(app.xfer_set(COLONISTS, 10), -15);
    assert_eq!(app.xfer.as_ref().expect("open").aboard[COLONISTS], 10);
    app.xfer_cancel();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[2].cargo.colonists, 0, "Cancel drops it all");
    assert!(app.orders.is_empty());
}

/// Fuel moves only at a planet with a starbase, and never counts against the
/// hold.
#[test]
fn fuel_takes_the_tank_not_the_hold() {
    let mut app = colony_ship_in_hand();
    assert!(app.open_xfer());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(dialog.aboard[FUEL], dialog.fuel_capacity, "it starts full");
    assert_eq!(app.xfer_move(FUEL, -50), -50);
    assert_eq!(app.xfer_move(COLONISTS, 25), 25, "the hold is untouched");
    assert_eq!(app.xfer_move(FUEL, 1000), 50, "back up to the brim");
}

/// No fleet in hand, or one in deep space, and there is nothing to open.
#[test]
fn nothing_opens_in_deep_space() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    assert!(app.goto_fleet(2));
    if let Some(game) = app.game.as_mut() {
        game.fleets[2].orbiting = None;
    }
    assert!(!app.open_xfer());
    assert!(app.xfer.is_none());
}
