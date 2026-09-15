//! The Cargo Transfer dialog — `TransferDlg`, the **Xfer** button — worked
//! through `App`. See `docs/ui/cargo-transfer.md`.

use stars_core::orders::{COLONISTS, FUEL};
use stars_ui::{App, ScanObject, XferObject};

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
/// side has; the planet has no tank, so fuel does not move.
#[test]
fn xfer_opens_on_the_fleet_and_its_planet() {
    let mut app = colony_ship_in_hand();
    assert!(app.open_xfer());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(
        dialog.objects,
        [XferObject::Fleet(2), XferObject::Planet(STOVE_TOP)]
    );
    assert_eq!(dialog.cargo_capacity[0], 25);
    assert_eq!(dialog.aboard()[..4], [0, 0, 0, 0]);
    assert_eq!(dialog.own, [true, true]);
    assert!(!dialog.fuel_moves(), "a planet has no tank");
    let home = app.selected_planet().expect("Stove Top");
    assert_eq!(dialog.has[1][..3], home.surface_min);
    assert_eq!(dialog.has[1][COLONISTS], home.pop);
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
    assert_eq!(app.xfer_set(0, COLONISTS, 25), 25);
    assert_eq!(app.xfer_set(0, COLONISTS, 10), -15);
    assert_eq!(app.xfer.as_ref().expect("open").aboard()[COLONISTS], 10);
    assert_eq!(app.xfer_set(1, COLONISTS, 0), 0, "a planet has no gauge");
    app.xfer_cancel();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[2].cargo.colonists, 0, "Cancel drops it all");
    assert!(app.orders.is_empty());
}

/// Fuel never crosses to or from a planet, which has no tank: `ChgCargo`
/// moves none on one, and `UpdateXferBtns` keeps the fuel pair dead.
#[test]
fn fuel_stays_put_at_a_planet() {
    let mut app = colony_ship_in_hand();
    assert!(app.open_xfer());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(
        dialog.aboard()[FUEL],
        dialog.fuel_capacity[0],
        "it starts full"
    );
    assert_eq!(app.xfer_move(FUEL, -50), 0);
    assert_eq!(app.xfer_move(FUEL, 50), 0);
    assert_eq!(app.xfer_move(COLONISTS, 25), 25, "the hold is untouched");
}

/// No fleet in hand and there is nothing to open; in deep space the
/// dialog is the jettison's, with no planet on the right.
#[test]
fn nothing_opens_without_a_fleet_and_deep_space_jettisons() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    assert!(!app.open_xfer());
    assert!(app.goto_fleet(2));
    if let Some(game) = app.game.as_mut() {
        game.fleets[2].orbiting = None;
    }
    assert!(app.open_xfer());
    assert_eq!(
        app.xfer.as_ref().expect("open").objects[1],
        XferObject::Space
    );
}

/// Two fleets of the player's standing together, the second put beside
/// the first with some cargo aboard.
fn two_fleets_together() -> (App, usize, usize) {
    let mut app = colony_ship_in_hand();
    let at = app.game.as_ref().expect("a game").fleets[2].position;
    let other = app
        .own_fleets()
        .into_iter()
        .find(|index| {
            let game = app.game.as_ref().expect("a game");
            let designs = game.designs.first().cloned().unwrap_or_default();
            *index != 2 && game.fleets[*index].cargo_capacity(&designs) >= 25
        })
        .expect("another fleet with a hold");
    {
        let game = app.game.as_mut().expect("a game");
        game.fleets[other].position = at;
        game.fleets[other].orbiting = Some(u16::try_from(STOVE_TOP).unwrap());
        game.fleets[other].cargo.minerals = [12, 0, 0];
        game.fleets[other].cargo.fuel = 30;
    }
    (app, 2, other)
}

/// The fleets-here tile's Cargo: the fleet in hand on the left and the
/// fleet the tile shows on the right, gauges both sides, fuel movable, and
/// OK logs one fleet-to-fleet order naming the left fleet first.
#[test]
fn cargo_moves_between_two_fleets() {
    let (mut app, mine, other) = two_fleets_together();
    app.choose_pane_fleet({
        let game = app.game.as_ref().expect("a game");
        (game.fleets[other].owner, game.fleets[other].id)
    });
    assert!(app.open_xfer_with_fleet_here());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(
        dialog.objects,
        [XferObject::Fleet(mine), XferObject::Fleet(other)]
    );
    assert_eq!(dialog.own, [true, true]);
    assert!(dialog.fuel_moves());
    assert_eq!(dialog.has[1][0], 12);
    assert_eq!(dialog.has[1][FUEL], 30);

    // Ironium comes across as far as the other has; fuel goes the other
    // way as far as its tank holds.
    assert_eq!(app.xfer_move(0, 100), 12);
    let tank = dialog.fuel_capacity[1];
    assert_eq!(
        app.xfer_move(FUEL, -1000),
        -(tank - 30).min(dialog.aboard()[FUEL])
    );
    // The right gauge sets the right side.
    assert_eq!(app.xfer_set(1, 0, 2), 2);
    assert_eq!(app.xfer.as_ref().expect("open").has[0][0], 10);

    app.xfer_ok();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[mine].cargo.minerals[0], 10);
    assert_eq!(game.fleets[other].cargo.minerals[0], 2);
    assert_eq!(
        game.fleets[other].cargo.fuel,
        tank.min(30 + dialog.aboard()[FUEL])
    );
    let order = app
        .orders
        .last()
        .expect("an order")
        .as_cargo_transfer()
        .expect("a cargo transfer");
    let word = |index: usize| {
        let f = &game.fleets[index];
        (u16::try_from(f.owner).unwrap() << 9) | (f.id & 0x1ff)
    };
    assert_eq!((order.id1, order.id2), (word(mine), word(other)));
    assert_eq!((order.grobj1, order.grobj2), (2, 2));
    assert_eq!(order.items_mask, (1 << 0) | (1 << FUEL));
    assert_eq!(order.quantities[0], 10, "the left fleet's gain");
    assert!(order.quantities[1] < 0, "and its loss of fuel");
}

/// From the planet pane the planet is on the left and the fleet in orbit
/// on the right: the left arrow unloads onto the planet.
#[test]
fn the_planet_pane_puts_the_planet_on_the_left() {
    let (mut app, _, other) = two_fleets_together();
    app.select_object(ScanObject::Planet(STOVE_TOP));
    app.choose_pane_fleet({
        let game = app.game.as_ref().expect("a game");
        (game.fleets[other].owner, game.fleets[other].id)
    });
    assert!(app.open_xfer_with_fleet_here());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(
        dialog.objects,
        [XferObject::Planet(STOVE_TOP), XferObject::Fleet(other)]
    );
    assert!(!dialog.fuel_moves());
    let ironium = dialog.has[0][0];
    assert_eq!(app.xfer_move(0, 5), 5, "five kT onto the planet");
    assert_eq!(app.xfer_move(FUEL, -10), 0, "the planet gives no fuel");
    app.xfer_ok();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[other].cargo.minerals[0], 7);
    assert_eq!(
        app.selected_planet().expect("Stove Top").surface_min[0],
        ironium + 5
    );
    let order = app
        .orders
        .last()
        .expect("an order")
        .as_cargo_transfer()
        .expect("a cargo transfer");
    assert_eq!((order.grobj1, order.grobj2), (1, 2));
    assert_eq!(order.id1, u16::try_from(STOVE_TOP).unwrap());
    assert_eq!(order.quantities, vec![5]);
}

/// Another player's fleet shows figures, gives no colonists, and can be
/// given minerals.
#[test]
fn another_players_fleet_takes_minerals_but_no_colonists() {
    let (mut app, mine, other) = two_fleets_together();
    {
        let game = app.game.as_mut().expect("a game");
        game.fleets[other].owner = 1;
        // Its holds are sized by its owner's designs.
        game.designs[1] = game.designs[0].clone();
        game.fleets[mine].cargo.minerals = [5, 0, 0];
        game.fleets[mine].cargo.colonists = 3;
    }
    assert!(app.open_xfer_between([XferObject::Fleet(mine), XferObject::Fleet(other)]));
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(dialog.own, [true, false]);
    assert_eq!(
        app.xfer_move(COLONISTS, -3),
        0,
        "no colonists to a stranger"
    );
    assert_eq!(app.xfer_move(0, -5), -5, "minerals, yes");
    assert_eq!(
        app.xfer_set(1, 0, 0),
        0,
        "and no gauge to drag on their side"
    );
}

/// The tile's Merge: the Ship Transfer dialog over the two fleets, with a
/// row for every design either has; OK moves the ships and a fleet left
/// with none is gone.
#[test]
fn merge_moves_ships_between_two_fleets() {
    let (mut app, mine, other) = two_fleets_together();
    let key = {
        let game = app.game.as_ref().expect("a game");
        (game.fleets[other].owner, game.fleets[other].id)
    };
    app.choose_pane_fleet(key);
    assert!(app.open_merge_with_fleet_here());
    let dialog = app.split.clone().expect("open");
    assert_eq!(dialog.target, Some(other));
    let (mine_design, other_design) = {
        let game = app.game.as_ref().expect("a game");
        (
            game.fleets[mine].stacks[0].design,
            game.fleets[other].stacks[0].design,
        )
    };
    let row_of = |design: u8| {
        dialog
            .designs
            .iter()
            .position(|(d, _)| *d == design)
            .expect("a row for the design")
    };
    assert_eq!(dialog.left[row_of(mine_design)], 1);
    assert!(dialog.right[row_of(other_design)] >= 1);

    // The colony ship crosses to the other fleet.
    assert_eq!(app.split_move(row_of(mine_design), 1), 1);
    let fleets_before = app.game.as_ref().expect("a game").fleets.len();
    app.split_ok();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(
        game.fleets.len(),
        fleets_before - 1,
        "the emptied fleet is gone"
    );
    let merged = game
        .fleets
        .iter()
        .find(|f| f.owner == key.0 && f.id == key.1)
        .expect("the other fleet");
    assert!(merged.stacks.iter().any(|s| s.design == mine_design));
    let order = app
        .orders
        .last()
        .expect("an order")
        .as_cargo_transfer()
        .expect("a ships record");
    assert_eq!(order.items_mask, 1 << mine_design);
    assert_eq!(
        order.quantities,
        vec![-1],
        "one ship left the fleet named first"
    );
}

/// A salvage packet at the spot, for the tile to list.
fn salvage_at(app: &mut App, at: stars_core::movement::Point) -> u16 {
    let game = app.game.as_mut().expect("a game");
    let id = game.packets.iter().map(|p| p.id).max().map_or(0, |m| m + 1);
    game.packets.push(stars_core::packet::Packet {
        id,
        owner: -1,
        position: at,
        target: 0x3ff,
        warp: 0,
        minerals: [40, 5, 0],
        decay_rate: 0,
        moved: false,
        include: true,
        turn: 0,
    });
    id
}

/// The tile lists the packets at the spot after the fleets, and Cargo on
/// one raises the dialog with the packet on the right: minerals come off it
/// into the hold, colonists never cross, and OK logs a record naming the
/// thing by its `idFull`.
#[test]
fn minerals_are_taken_off_a_packet_in_the_tile() {
    let mut app = colony_ship_in_hand();
    let at = app.game.as_ref().expect("a game").fleets[2].position;
    let id = salvage_at(&mut app, at);
    let list = app.pane_fleet_list();
    let entry = list
        .iter()
        .find(|entry| entry.packet.is_some())
        .expect("the salvage is listed");
    assert_eq!(entry.name, "Salvage");
    assert_eq!(entry.label(), "Salvage (45kT)");
    assert_eq!(entry.key, (i16::MIN, id));
    app.choose_pane_fleet(entry.key);
    assert_eq!(app.pane_fleet_choice(), None, "a packet is not a fleet");
    assert_eq!(app.pane_packet_gauge(), Some(([40, 5, 0], 50)));

    assert!(app.open_xfer_with_fleet_here());
    let dialog = app.xfer.clone().expect("open");
    assert!(matches!(dialog.objects[1], XferObject::Packet(_)));
    assert_eq!(dialog.has[1][..3], [40, 5, 0]);
    assert_eq!(
        dialog.cargo_capacity[1], 50,
        "the shell: the mass over ten, up"
    );
    assert!(!dialog.fuel_moves());
    assert!(!dialog.colonists_move());
    assert_eq!(app.xfer_move(0, 100), 25, "the hold is 25");
    assert_eq!(app.xfer_move(COLONISTS, -1), 0);
    // The packet's own gauge can be dragged: back up to its shell.
    assert_eq!(app.xfer_set(1, 0, 100), 25, "no more than the shell holds");
    assert_eq!(app.xfer_set(1, 0, 20), -20);
    app.xfer_ok();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[2].cargo.minerals, [20, 0, 0]);
    let packet = game
        .packets
        .iter()
        .find(|p| p.id == id)
        .expect("still there");
    assert_eq!(packet.minerals, [20, 5, 0]);
    let order = app
        .orders
        .last()
        .expect("an order")
        .as_cargo_transfer()
        .expect("a cargo transfer");
    assert_eq!((order.grobj1, order.grobj2), (2, 8));
    assert_eq!(order.id2, stars_core::orders::packet_word(packet));
    assert_eq!(order.quantities, vec![20]);
}

/// In deep space the location tile's button is Jettison: the dialog opens
/// with Deep Space on the right, what goes over the side is logged against
/// no object, and it can be picked up again while the turn lasts. Salvage
/// at the spot keeps the button dead.
#[test]
fn cargo_is_jettisoned_in_deep_space_and_can_be_taken_back() {
    let mut app = colony_ship_in_hand();
    let at = {
        let game = app.game.as_mut().expect("a game");
        game.fleets[2].orbiting = None;
        game.fleets[2].position = stars_core::movement::Point::new(1000, 1000);
        game.fleets[2].cargo.minerals = [10, 0, 0];
        game.fleets[2].cargo.colonists = 5;
        game.fleets[2].position
    };
    app.select_object(ScanObject::Fleet(2));
    assert!(app.can_jettison());
    assert!(app.open_xfer());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(dialog.objects, [XferObject::Fleet(2), XferObject::Space]);
    assert!(
        dialog.is_planet(1),
        "space is drawn and clamped as a planet"
    );
    assert!(!dialog.fuel_moves());
    assert_eq!(app.xfer_move(0, -6), -6);
    assert_eq!(app.xfer_move(COLONISTS, -5), -5, "people can go overboard");
    app.xfer_ok();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[2].cargo.minerals, [4, 0, 0]);
    assert_eq!(game.fleets[2].cargo.colonists, 0);
    let order = app
        .orders
        .last()
        .expect("an order")
        .as_cargo_transfer()
        .expect("a cargo transfer");
    assert_eq!((order.grobj1, order.grobj2), (2, 4));
    assert_eq!(order.id2, 0xffff);
    assert_eq!(order.quantities, vec![-6, -5]);

    // Open again: what went over the side is still there to pick up.
    assert!(app.open_xfer());
    let dialog = app.xfer.clone().expect("open");
    assert_eq!(dialog.has[1][0], 6);
    assert_eq!(dialog.has[1][COLONISTS], 5);
    assert_eq!(app.xfer_move(0, 100), 6);
    app.xfer_ok();
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.fleets[2].cargo.minerals, [10, 0, 0]);
    assert!(app.open_xfer());
    assert_eq!(app.xfer.as_ref().expect("open").has[1][0], 0);
    app.xfer_cancel();

    // Salvage at the spot, and there is no jettisoning.
    salvage_at(&mut app, at);
    assert!(!app.can_jettison());
    assert!(!app.open_xfer());
}
