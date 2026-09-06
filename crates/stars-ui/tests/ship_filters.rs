//! The scanner's two ship filters.
//!
//! See `docs/ui/toolbar.md`.

use stars_core::design::{ShipClass, ShipDesign};
use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, FilterCommand};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "filters".to_string(),
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

fn fleet(owner: i16, stacks: &[(u8, i32)]) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: Point::new(0, 0),
        orbiting: None,
        stacks: stacks
            .iter()
            .map(|(design, count)| ShipStack {
                design: *design,
                count: *count,
                damaged_pct: 0,
                damage_pct: 0,
            })
            .collect(),
        cargo: Default::default(),
        battle_plan: 0,
        warp: None,
        waypoints: Vec::new(),
        name: None,
        repeat_orders: false,
        direction: None,
    }
}

/// The eight classes are the hull categories, and a starbase is none of them.
#[test]
fn the_eight_classes_are_the_hull_categories() {
    assert_eq!(ShipClass::ALL.len(), 8);
    assert_eq!(
        ShipClass::ALL.map(|c| c.name()),
        [
            "Colony",
            "Freighter",
            "Scout",
            "Warship",
            "Utility",
            "Bomber",
            "Miner",
            "Fuel Transport"
        ]
    );
    for (index, class) in ShipClass::ALL.iter().enumerate() {
        assert_eq!(usize::from(class.index()), index);
        assert_eq!(ShipClass::from_category(class.index()), Some(*class));
    }
    assert_eq!(ShipClass::from_category(8), None);

    // Every ship hull lands in a class, and the ones whose names say what they
    // are land where they should.
    for (name, want) in [
        ("Colony Ship", ShipClass::Colony),
        ("Large Freighter", ShipClass::Freighter),
        ("Destroyer", ShipClass::Scout),
        ("Dreadnought", ShipClass::Warship),
        ("Super Mine Layer", ShipClass::Utility),
        ("B-52 Bomber", ShipClass::Bomber),
        ("Maxi-Miner", ShipClass::Miner),
        ("Super-Fuel Xport", ShipClass::FuelTransport),
    ] {
        let hull = stars_core::components::HULLS
            .iter()
            .find(|h| h.name == name)
            .unwrap_or_else(|| panic!("{name}"));
        assert_eq!(
            ShipClass::from_category(hull.category),
            Some(want),
            "{name}"
        );
    }

    // A starbase stores category 0, which would read as a colony ship, so it
    // is refused rather than misfiled.
    let starbase = ShipDesign {
        hull_id: 32,
        slots: Vec::new(),
        name: "Orbital Fort".to_string(),
        picture: 128,
        stored_armor: 1000,
    };
    assert_eq!(starbase.ship_class(), None);
    let empty = ShipDesign {
        hull_id: -1,
        ..starbase.clone()
    };
    assert_eq!(empty.ship_class(), None);
}

/// The two filters apply to **different fleets**: the design filter to this
/// player's own, the class filter to everybody else's.
#[test]
fn each_filter_applies_to_its_own_side() {
    let mut app = a_game();
    let mine = fleet(0, &[(0, 3), (1, 4)]);
    let theirs = fleet(1, &[(0, 5), (1, 6)]);

    // With neither on, every fleet is counted whole.
    assert_eq!(app.filtered_ship_count(&mine), 7);
    assert_eq!(app.filtered_ship_count(&theirs), 11);

    // The design filter narrows mine and leaves theirs alone.
    app.scan_overlays.ship_design_filter = true;
    app.scan_design_filter = 1 << 1;
    assert_eq!(app.filtered_ship_count(&mine), 4, "only design 1");
    assert_eq!(app.filtered_ship_count(&theirs), 11, "not their fleets");

    // With nothing ticked it counts nothing of mine, and still all of theirs.
    app.scan_design_filter = 0;
    assert_eq!(app.filtered_ship_count(&mine), 0);
    assert_eq!(app.filtered_ship_count(&theirs), 11);
}

/// The class filter counts an opponent's ships by their hull's class.
#[test]
fn the_class_filter_counts_by_hull_class() {
    let mut app = a_game();
    // Give the opponent two designs of known classes.
    {
        let game = app.game.as_mut().expect("a game");
        let scout = stars_core::components::HULLS
            .iter()
            .position(|h| h.name == "Scout")
            .expect("a Scout");
        let colony = stars_core::components::HULLS
            .iter()
            .position(|h| h.name == "Colony Ship")
            .expect("a Colony Ship");
        let designs = game.designs.get_mut(1).expect("their designs");
        designs.clear();
        for (hull, name) in [(scout, "a scout"), (colony, "a colony ship")] {
            designs.push(ShipDesign {
                hull_id: i16::try_from(hull).expect("a hull id"),
                slots: Vec::new(),
                name: name.to_string(),
                picture: 0,
                stored_armor: 0,
            });
        }
    }
    let theirs = fleet(1, &[(0, 3), (1, 9)]);

    app.scan_overlays.enemy_class_filter = true;
    app.scan_class_filter = 1 << ShipClass::Scout.index();
    assert_eq!(app.filtered_ship_count(&theirs), 3, "the scouts only");

    app.scan_class_filter = 1 << ShipClass::Colony.index();
    assert_eq!(app.filtered_ship_count(&theirs), 9, "the colony ships only");

    app.scan_class_filter = (1 << ShipClass::Scout.index()) | (1 << ShipClass::Colony.index());
    assert_eq!(app.filtered_ship_count(&theirs), 12, "both");

    app.scan_class_filter = 1 << ShipClass::Bomber.index();
    assert_eq!(app.filtered_ship_count(&theirs), 0, "no bombers there");

    // And my own fleet is untouched by it.
    assert_eq!(app.filtered_ship_count(&fleet(0, &[(0, 2)])), 2);
}

/// Ticking something while the overlay is off turns the overlay on — there is
/// no point choosing a design and seeing nothing change. Unticking does not
/// turn it off again.
#[test]
fn ticking_something_turns_the_overlay_on() {
    let mut app = a_game();
    assert!(!app.scan_overlays.ship_design_filter);
    app.toggle_design_filter(2);
    assert!(
        app.scan_overlays.ship_design_filter,
        "it switched itself on"
    );
    assert_eq!(app.scan_design_filter, 1 << 2);

    // Unticking the last one leaves the overlay on, as the original leaves it.
    app.toggle_design_filter(2);
    assert_eq!(app.scan_design_filter, 0);
    assert!(app.scan_overlays.ship_design_filter);

    // The same for the enemy filter.
    let mut app = a_game();
    assert!(!app.scan_overlays.enemy_class_filter);
    app.toggle_class_filter(ShipClass::Warship.index());
    assert!(app.scan_overlays.enemy_class_filter);

    // "None" leaves the mask empty, so it does not switch anything on.
    let mut app = a_game();
    app.design_filter_command(FilterCommand::None);
    assert!(!app.scan_overlays.ship_design_filter);
}

/// All, invert and none, on both masks.
#[test]
fn the_three_commands_work_on_both_masks() {
    let mut app = a_game();
    app.design_filter_command(FilterCommand::All);
    assert_eq!(app.scan_design_filter, u16::MAX);
    app.design_filter_command(FilterCommand::Invert);
    assert_eq!(app.scan_design_filter, 0);
    app.toggle_design_filter(3);
    app.design_filter_command(FilterCommand::Invert);
    assert_eq!(app.scan_design_filter, u16::MAX ^ (1 << 3));
    app.design_filter_command(FilterCommand::None);
    assert_eq!(app.scan_design_filter, 0);

    app.class_filter_command(FilterCommand::All);
    assert_eq!(app.scan_class_filter, u8::MAX);
    app.class_filter_command(FilterCommand::Invert);
    assert_eq!(app.scan_class_filter, 0);
}

/// The design menu lists the designs that exist, and each entry's bit is its
/// **slot** rather than its place in the menu.
#[test]
fn the_design_menu_lists_real_designs_by_slot() {
    let mut app = a_game();
    // Blank out the first design, which should drop out of the menu without
    // moving anybody else's bit.
    let (first, second) = {
        let game = app.game.as_mut().expect("a game");
        let designs = game.designs.get_mut(0).expect("my designs");
        assert!(designs.len() >= 2, "a new game starts with designs");
        let second = designs[1].name.clone();
        designs[0].hull_id = -1;
        (designs[0].name.clone(), second)
    };

    let entries = app.design_filter_entries();
    assert!(
        !entries.iter().any(|e| e.name == first),
        "the emptied slot is gone"
    );
    let kept = entries
        .iter()
        .find(|e| e.name == second)
        .expect("the second design is still listed");
    assert_eq!(kept.bit, 1, "and still on its own slot, not renumbered");
    assert!(!kept.on);

    app.toggle_design_filter(kept.bit);
    assert!(
        app.design_filter_entries()
            .iter()
            .find(|e| e.name == second)
            .expect("still there")
            .on
    );

    // The class menu always has all eight.
    assert_eq!(app.class_filter_entries().len(), 8);
}
