//! The model behind the Ship and Starbase Designer: which hulls and components
//! a player may build, where they sit on the schematic, and what they cost.
//!
//! See `docs/ui/ship-design.md` for the screen itself.

use serde_json::Value;

use stars_core::components::{slot, Hull, HULLS, STARBASE_HULLS};
use stars_core::design::{copied_name, true_part_cost, DesignSlot, ShipDesign};
use stars_core::parts::{availability, filtered, part, Availability, Builder};
use stars_core::race::{lrt, Prt, Race};

/// A plain race with no lesser traits, at the given primary trait.
fn race(prt: Prt) -> Race {
    let mut race = Race::humanoid();
    race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = prt as i16;
    race.lrt_bits = 0;
    race
}

fn builder(race: &Race, levels: [u8; 6]) -> Builder<'_> {
    Builder {
        race,
        levels,
        researching: 0,
        trader_parts: 0,
        starbase: false,
    }
}

// --- the schematic -------------------------------------------------------

#[test]
fn every_hull_schematic_matches_the_binary() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/vectors/hull-schematics.json"
    );
    let text = std::fs::read_to_string(path).expect("vectors are versioned with the specs");
    let v: Value = serde_json::from_str(&text).expect("vectors parse");
    let cases = v["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), HULLS.len() + STARBASE_HULLS.len());

    for case in cases {
        let index = usize::try_from(case["index"].as_u64().unwrap()).unwrap();
        let hull: &Hull = match case["table"].as_str().unwrap() {
            "HULLS" => &HULLS[index],
            "STARBASE_HULLS" => &STARBASE_HULLS[index],
            other => panic!("unknown table {other}"),
        };
        let name = case["name"].as_str().unwrap();
        assert_eq!(hull.name, name);
        assert_eq!(u64::from(hull.slot_count), case["chs"].as_u64().unwrap());

        let cargo = u16::from_str_radix(case["wrcCargo"].as_str().unwrap(), 16).unwrap();
        assert_eq!(hull.cargo_pos, cargo, "{name} cargo rectangle");

        let brc = case["rgbrc"].as_str().unwrap();
        let bytes: Vec<u8> = (0..brc.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&brc[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(hull.slot_pos.to_vec(), bytes, "{name} slot positions");
    }
}

/// Every slot has somewhere to go, no two slots share a cell, and a hull with a
/// hold has somewhere to draw it — the checks that would catch a transcription
/// that had drifted by a byte.
#[test]
fn no_two_slots_sit_in_the_same_place() {
    for hull in HULLS.iter().chain(STARBASE_HULLS.iter()) {
        let mut seen = Vec::new();
        for i in 0..usize::from(hull.slot_count) {
            let cell = hull.slot_cell(i).expect("a real slot has a place");
            assert!(
                !seen.contains(&cell),
                "{} has two slots at {cell:?}",
                hull.name
            );
            seen.push(cell);
        }
        assert_eq!(hull.slot_cell(usize::from(hull.slot_count)), None);

        if hull.cargo_max != 0 {
            let ((left, top), (right, bottom)) =
                hull.cargo_cells().expect("a hold is drawn somewhere");
            assert!(left < right && top < bottom, "{} cargo box", hull.name);
        } else {
            assert_eq!(hull.cargo_cells(), None, "{} has no hold", hull.name);
        }
    }
}

/// The Small Freighter is the one worked example: three slots in a row with the
/// hold between the engine and the armour.
#[test]
fn the_small_freighter_reads_the_way_it_looks() {
    let hull = &HULLS[0];
    assert_eq!(hull.name, "Small Freighter");
    assert_eq!(hull.slot_cell(0), Some((1, 3))); // engine, at the left
    assert_eq!(hull.slot_cell(1), Some((7, 3))); // scanner or special, at the right
    assert_eq!(hull.slot_cell(2), Some((5, 3))); // shield or armour
    assert_eq!(hull.cargo_cells(), Some(((3, 3), (5, 5))));
}

/// Only the three biggest starbases have an unlimited dock; the Space Dock's is
/// 200kT and the Orbital Fort has none at all.
#[test]
fn a_starbase_dock_is_a_cargo_hold() {
    assert_eq!(STARBASE_HULLS[0].name, "Orbital Fort");
    assert_eq!(STARBASE_HULLS[0].cargo_cells(), None);
    assert_eq!(STARBASE_HULLS[1].name, "Space Dock");
    assert_eq!(STARBASE_HULLS[1].cargo_max, 200);
    assert!(!STARBASE_HULLS[1].unlimited_cargo());
    for hull in &STARBASE_HULLS[2..] {
        assert!(hull.unlimited_cargo(), "{} docks anything", hull.name);
    }
}

// --- who may build what --------------------------------------------------

/// The components a primary trait keeps for itself, each checked from both
/// sides: the race that has it can build it and a race that does not, cannot.
#[test]
fn a_primary_trait_reserves_its_own_components() {
    let levels = [26; 6];
    // (category, item, the trait that owns it)
    let owned = [
        (slot::ENGINE, 0, Prt::He),     // Settler's Delight
        (slot::HULL, 14, Prt::He),      // Mini-Colony Ship
        (slot::HULL, 31, Prt::He),      // Meta Morph
        (slot::SCANNER, 6, Prt::Ss),    // Chameleon
        (slot::SHIELD, 4, Prt::Ss),     // Shadow Shield
        (slot::ARMOR, 7, Prt::Ss),      // Depleted Neutronium
        (slot::HULL, 12, Prt::Ss),      // Rogue
        (slot::HULL, 8, Prt::Wm),       // Battle Cruiser
        (slot::HULL, 10, Prt::Wm),      // Dreadnought
        (slot::BOMB, 9, Prt::Ca),       // Retro Bomb
        (slot::MINING, 7, Prt::Ca),     // Orbital Adjuster
        (slot::SHIELD, 3, Prt::Is),     // Croby Sharmor
        (slot::ARMOR, 6, Prt::Is),      // Fielded Kelarium
        (slot::HULL, 3, Prt::Is),       // Super Freighter
        (slot::HULL, 25, Prt::Is),      // Fuel Transport
        (slot::HULL, 27, Prt::Sd),      // Mini Mine Layer
        (slot::SPECIAL_E, 14, Prt::Sd), // Energy Dampener
        (slot::SPECIAL_SB, 8, Prt::Pp), // a mass driver
        (slot::SPECIAL_E, 16, Prt::It), // Anti-matter Generator
        (slot::SB_HULL, 4, Prt::Ar),    // Death Star
        (slot::SPECIAL_M, 1, Prt::Ar),  // Orbital Construction Module
    ];

    for (category, item, owner) in owned {
        let owner_race = race(owner);
        assert_eq!(
            availability(&builder(&owner_race, levels), category, item),
            Availability::Available,
            "{owner:?} should own {category:#x}/{item}"
        );
        for other in Prt::ALL {
            if other == owner {
                continue;
            }
            let other_race = race(other);
            assert_eq!(
                availability(&builder(&other_race, levels), category, item),
                Availability::Forbidden,
                "{other:?} should not have {category:#x}/{item}"
            );
        }
    }
}

/// Two rules the reconstruction had backwards, so both are pinned here.
///
/// Hyper Expansion cannot build a stargate at all — the manual says so on
/// p. 6-10 — and the Mine Dispenser 50 is the one mine layer that is *denied*
/// to War Monger rather than reserved for it.
#[test]
fn hyper_expansion_has_no_stargates_and_war_monger_no_dispenser_50() {
    let levels = [26; 6];
    for item in 0..7 {
        let he = race(Prt::He);
        assert_eq!(
            availability(&builder(&he, levels), slot::SPECIAL_SB, item),
            Availability::Forbidden,
            "Hyper Expansion should have no stargate {item}"
        );
    }

    // Everybody else gets the first four bar the second; only Interstellar
    // Traveler gets the rest.
    for prt in [Prt::Joat, Prt::Wm, Prt::Is] {
        let r = race(prt);
        let who = builder(&r, levels);
        for item in 0..7 {
            let want = if item == 0 || (2..=3).contains(&item) {
                Availability::Available
            } else {
                Availability::Forbidden
            };
            assert_eq!(
                availability(&who, slot::SPECIAL_SB, item),
                want,
                "{prt:?} stargate {item}"
            );
        }
    }
    let it = race(Prt::It);
    for item in 0..7 {
        assert_eq!(
            availability(&builder(&it, levels), slot::SPECIAL_SB, item),
            Availability::Available
        );
    }

    let wm = race(Prt::Wm);
    assert_eq!(
        availability(&builder(&wm, levels), slot::MINES, 1),
        Availability::Forbidden
    );
    let joat = race(Prt::Joat);
    assert_eq!(
        availability(&builder(&joat, levels), slot::MINES, 1),
        Availability::Available
    );
}

/// A lesser trait can forbid as firmly as a primary one, and one of them —
/// No Ramscoop Engines — gives something back.
#[test]
fn lesser_traits_open_and_close_the_list() {
    let levels = [26; 6];

    let mut plain = race(Prt::Joat);
    plain.lrt_bits = 0;
    let mut scoopless = race(Prt::Joat);
    scoopless.lrt_bits = 1 << lrt::NO_RAMSCOOPS;

    // The ramscoops go, and the Interspace-10 arrives in their place.
    for item in 10..=15 {
        assert_eq!(
            availability(&builder(&scoopless, levels), slot::ENGINE, item),
            Availability::Forbidden
        );
    }
    assert_eq!(
        availability(&builder(&scoopless, levels), slot::ENGINE, 7),
        Availability::Available
    );
    assert_eq!(
        availability(&builder(&plain, levels), slot::ENGINE, 7),
        Availability::Forbidden
    );

    // Improved Starbases is what unlocks the Space Dock and the Ultra Station.
    let mut isb = race(Prt::Joat);
    isb.lrt_bits = 1 << lrt::ISB;
    for item in [1, 3] {
        assert_eq!(
            availability(&builder(&plain, levels), slot::SB_HULL, item),
            Availability::Forbidden
        );
        assert_eq!(
            availability(&builder(&isb, levels), slot::SB_HULL, item),
            Availability::Available
        );
    }
}

/// The Mystery Trader's twelve components are not in the list until it has
/// handed them over.
#[test]
fn a_traders_part_stays_hidden_until_it_is_given() {
    use stars_core::wormhole::part as gift;
    let r = race(Prt::Joat);
    let levels = [26; 6];

    let gifts = [
        (slot::ENGINE, 8, gift::ENGINE),
        (slot::SHIELD, 6, gift::SHIELD),
        (slot::ARMOR, 9, gift::ARMOR),
        (slot::BEAM, 18, gift::BEAM),
        (slot::TORPEDO, 7, gift::TORP),
        (slot::BOMB, 8, gift::BOMB),
        (slot::MINING, 6, gift::MINER),
        (slot::SPECIAL_E, 4, gift::SPECIAL),
        (slot::SPECIAL_M, 4, gift::CARGO),
        (slot::SPECIAL_M, 9, gift::JUMPGATE),
        (slot::HULL, 30, gift::HULL),
        (slot::PLANETARY, 14, gift::GENESIS),
    ];

    for (category, item, bit) in gifts {
        let mut without = builder(&r, levels);
        without.trader_parts = gift::ALL & !bit;
        assert_eq!(
            availability(&without, category, item),
            Availability::Forbidden,
            "{category:#x}/{item} should need the trader"
        );

        let mut with = builder(&r, levels);
        with.trader_parts = bit;
        assert_eq!(
            availability(&with, category, item),
            Availability::Available,
            "{category:#x}/{item} should arrive with the trader"
        );
    }
}

/// Being one level short of the field you are already researching is its own
/// answer, because the component turns up on its own.
#[test]
fn the_tech_gate_counts_how_far_away_a_part_is() {
    let r = race(Prt::Joat);
    // The Long Hump 6 needs 3 propulsion (field 2) and nothing else.
    let engine = part(slot::ENGINE, 3).expect("Long Hump 6");
    assert_eq!(engine.name, "Long Hump 6");
    assert_eq!(engine.tech, [0, 0, 3, 0, 0, 0]);

    let mut who = builder(&r, [0, 0, 3, 0, 0, 0]);
    assert_eq!(availability(&who, slot::ENGINE, 3), Availability::Available);

    who.levels = [0, 0, 2, 0, 0, 0];
    who.researching = 2;
    assert_eq!(availability(&who, slot::ENGINE, 3), Availability::Nearly);

    // One short, but researching something else: the game reports the gap plus
    // one, which is what its own countdown shows.
    who.researching = 0;
    assert_eq!(availability(&who, slot::ENGINE, 3), Availability::Levels(2));

    who.levels = [0; 6];
    assert_eq!(availability(&who, slot::ENGINE, 3), Availability::Levels(4));

    // Two fields short is simply "far".
    let torp = part(slot::TORPEDO, 5).expect("a mid torpedo");
    assert!(torp.tech.iter().filter(|t| **t > 0).count() >= 2);
    assert_eq!(
        availability(&who, slot::TORPEDO, 5),
        Availability::Far,
        "{} needs {:?}",
        torp.name,
        torp.tech
    );
}

/// The parts list is built category by category from the lowest bit up, so a
/// filter's contents come out in a fixed order.
#[test]
fn the_parts_list_runs_in_category_order() {
    let r = race(Prt::Joat);
    let who = builder(&r, [26; 6]);

    let weapons = filtered(&who, slot::BEAM | slot::TORPEDO);
    let split = weapons
        .iter()
        .position(|p| p.category == slot::TORPEDO)
        .expect("torpedoes are in there");
    assert!(weapons[..split].iter().all(|p| p.category == slot::BEAM));
    assert!(weapons[split..].iter().all(|p| p.category == slot::TORPEDO));

    // "All" for a ship is every category but the starbase specials.
    let all = filtered(&who, 0x19ff);
    assert!(all.iter().any(|p| p.category == slot::ENGINE));
    assert!(all.iter().all(|p| p.category != slot::SPECIAL_SB));
    assert!(all.len() > weapons.len());

    // A starbase's list has the orbital specials and no engines.
    let base = Builder {
        starbase: true,
        ..who
    };
    let sb = filtered(&base, 0x0a3c);
    assert!(sb.iter().any(|p| p.category == slot::SPECIAL_SB));
    assert!(sb.iter().all(|p| p.category != slot::ENGINE));
    // and neither of the two electrical specials the designer drops.
    assert!(!sb
        .iter()
        .any(|p| p.category == slot::SPECIAL_E && (p.item == 15 || p.item == 16)));
}

// --- what it costs -------------------------------------------------------

/// Miniaturisation: 4% a level over the requirement, to a floor of a quarter of
/// the list price.
#[test]
fn technology_you_do_not_need_makes_a_part_cheaper() {
    let r = race(Prt::Joat);
    let engine = part(slot::ENGINE, 3).expect("Long Hump 6"); // 3 propulsion
    let list = engine.resource_cost;

    // Exactly at the requirement: full price.
    let at = builder(&r, [0, 0, 3, 0, 0, 0]);
    assert_eq!(true_part_cost(&engine, &at).resources, list);

    // Five levels over: 20% off.
    let over = builder(&r, [0, 0, 8, 0, 0, 0]);
    let cut = (list * 20 + 50) / 100;
    assert_eq!(true_part_cost(&engine, &over).resources, list - cut);

    // Far over: the discount stops at 75%.
    let far = builder(&r, [0, 0, 26, 0, 0, 0]);
    let floor = list - (list * 75 + 50) / 100;
    assert_eq!(true_part_cost(&engine, &far).resources, floor);
    let further = builder(&r, [0, 0, 60, 0, 0, 0]);
    assert_eq!(true_part_cost(&engine, &further).resources, floor);
}

/// Bleeding Edge Technology: a steeper curve, paid for up front.
#[test]
fn bleeding_edge_technology_doubles_then_undercuts() {
    let mut bet = race(Prt::Joat);
    bet.lrt_bits = 1 << lrt::BLEEDING_EDGE_TECH;
    let plain = race(Prt::Joat);

    let engine = part(slot::ENGINE, 3).expect("Long Hump 6");
    let list = engine.resource_cost;

    // Only just able to build it: twice the price.
    let just = builder(&bet, [0, 0, 3, 0, 0, 0]);
    assert_eq!(true_part_cost(&engine, &just).resources, list * 2);
    let normal = builder(&plain, [0, 0, 3, 0, 0, 0]);
    assert_eq!(true_part_cost(&engine, &normal).resources, list);

    // One level past every requirement and the surcharge is gone, replaced by
    // 5% a level rather than 4%.
    let past = builder(&bet, [0, 0, 4, 0, 0, 0]);
    assert_eq!(
        true_part_cost(&engine, &past).resources,
        list - (list * 5 + 50) / 100
    );

    // And it bottoms out lower: 80% off rather than 75%.
    let far = builder(&bet, [0, 0, 40, 0, 0, 0]);
    assert_eq!(
        true_part_cost(&engine, &far).resources,
        list - (list * 80 + 50) / 100
    );
}

/// A starbase's stored cost is double what it is built for, and Improved
/// Starbases takes a fifth off before the halving.
#[test]
fn a_starbase_is_built_for_half_what_the_table_says() {
    let plain = race(Prt::Joat);
    let mut isb = race(Prt::Joat);
    isb.lrt_bits = 1 << lrt::ISB;

    // An Orbital Fort with nothing fitted, for a race with no technology at
    // all, so miniaturisation cannot muddy the figures.
    let fort = ShipDesign {
        hull_id: 32,
        slots: Vec::new(),
        name: "Orbital Fort".into(),
        picture: 0,
        stored_armor: 1000,
    };
    assert_eq!(STARBASE_HULLS[0].resource_cost, 80);
    assert_eq!(STARBASE_HULLS[0].ore_cost, [24, 0, 34]);

    let cost = fort
        .true_cost(&builder(&plain, [0; 6]))
        .expect("a hull costs something");
    assert_eq!(cost.resources, 40);
    assert_eq!(cost.minerals, [12, 0, 17]);

    let cheap = fort
        .true_cost(&builder(&isb, [0; 6]))
        .expect("a hull costs something");
    assert_eq!(cheap.resources, 32); // 80 - 16, halved
    assert_eq!(cheap.minerals, [10, 0, 14]); // 24→20→10, 34→28→14

    // A ship is not halved.
    let scout = ShipDesign {
        hull_id: 4,
        slots: Vec::new(),
        name: "Scout".into(),
        picture: 0,
        stored_armor: 0,
    };
    let raw = scout.cost().expect("a hull costs something");
    let true_cost = scout
        .true_cost(&builder(&plain, [0; 6]))
        .expect("a hull costs something");
    assert_eq!(raw, true_cost);
}

/// The whole design is miniaturised, hull and fittings alike.
#[test]
fn a_designs_true_cost_adds_up_its_miniaturised_parts() {
    let r = race(Prt::Joat);
    let levels = [10; 6];
    let who = builder(&r, levels);

    let scout = ShipDesign {
        hull_id: 4,
        slots: vec![
            DesignSlot {
                category: slot::ENGINE,
                item: 3,
                count: 1,
            },
            DesignSlot {
                category: slot::SCANNER,
                item: 0,
                count: 1,
            },
        ],
        name: "Long Range Scout".into(),
        picture: 0,
        stored_armor: 0,
    };

    let hull = part(slot::HULL, 4).expect("Scout");
    let engine = part(slot::ENGINE, 3).expect("Long Hump 6");
    let scanner = part(slot::SCANNER, 0).expect("a scanner");
    let want = true_part_cost(&hull, &who).resources
        + true_part_cost(&engine, &who).resources
        + true_part_cost(&scanner, &who).resources;

    assert_eq!(scout.true_cost(&who).expect("a cost").resources, want);
    // and it really is cheaper than the list price.
    assert!(want < scout.cost().expect("a list price").resources);
}

// --- copying a design ----------------------------------------------------

#[test]
fn copying_a_design_numbers_the_copy() {
    assert_eq!(copied_name("Long Range Scout"), "Long Range Scout (2)");
    assert_eq!(copied_name("Long Range Scout (2)"), "Long Range Scout (3)");
    // The one digit wraps rather than carrying.
    assert_eq!(copied_name("Scout (9)"), "Scout (0)");
    // Twenty-eight characters is as far as the name will stretch, so a long one
    // is copied unchanged — two designs can share a name.
    let long = "A twenty eight char name!!!!!";
    assert!(long.len() >= 28);
    assert_eq!(copied_name(long), long);
}

// --- design changes on the wire ------------------------------------------

/// A design change names its slot **once**, in the order's header word.
///
/// `SHDEF.det` packs the slot as a five-bit `ishdef` and stores a starbase
/// design at 16..=25, so its top bit is set — and that same bit is what the
/// embedded design record calls its "starbase" flag, with `design_number`
/// carrying only the low four. Adding the starbase offset to the header value
/// would count it twice and put every edited starbase design in slot 32 and up.
#[test]
fn a_starbase_design_change_lands_in_its_own_slot() {
    use stars_core::startup::FIRST_STARBASE_SLOT;
    use stars_core::{replay, GameState, TurnOrders};
    use stars_formats::{DesignRecord, OrderLog, ShipDesignChange};

    let slot = usize::from(FIRST_STARBASE_SLOT) + 2;
    let record = DesignRecord {
        full_design: true,
        transferred: false,
        starbase: true,
        // The record carries only the low four bits of the slot.
        design_number: u8::try_from(slot & 0x0f).unwrap(),
        hull_id: 32,
        pic: 0,
        armor: Some(1000),
        mass: None,
        turn_designed: Some(0),
        total_built: Some(0),
        total_remaining: Some(0),
        slots: Vec::new(),
        name: "Fort".into(),
        flags0: 0,
        flags1: 0,
        trailing: Vec::new(),
    };
    let change = ShipDesignChange {
        mode: 1,
        player: 0,
        // The header carries the whole thing.
        design_index: u8::try_from(slot).unwrap(),
        header_high: 0,
        design: Some(record),
    };

    let mut state = GameState::new(1);
    state
        .players
        .push(stars_core::Player::new(Race::humanoid()));
    let log = OrderLog {
        header: None,
        records: vec![stars_formats::LogRecord::ship_design(&change).expect("encodes")],
    };
    let mut orders = TurnOrders::default();
    let report = replay::replay(&mut state, 0, &log, &mut orders);
    assert_eq!(report.designs, 1);
    assert_eq!(state.designs[0][slot].name, "Fort");
    assert_eq!(state.designs[0][slot].hull_id, 32);
    assert!(
        state.designs[0].len() <= slot + 1,
        "and nothing was created past it"
    );

    // A bare header deletes the same slot; there is no record to read the
    // starbase bit from at all.
    let delete = ShipDesignChange {
        design: None,
        mode: 0,
        ..change
    };
    let log = OrderLog {
        header: None,
        records: vec![stars_formats::LogRecord::ship_design(&delete).expect("encodes")],
    };
    let report = replay::replay(&mut state, 0, &log, &mut orders);
    assert_eq!(report.designs, 1);
    assert_eq!(state.designs[0][slot].hull_id, -1);
}
