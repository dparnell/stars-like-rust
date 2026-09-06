//! The transcribed component tables checked against bytes read out of
//! `stars.2.7j.exe`.
//!
//! `docs/vectors/components.json` records, for a sample of entries, the
//! address they live at and the raw bytes found there. This test decodes those
//! bytes with the layout the NB09 structs describe and asserts our table says
//! the same thing — so a transcription slip cannot pass unnoticed.

use serde_json::Value;

use stars_core::components::{ARMORS, BEAMS, ENGINES, PLANETARY};

fn vectors() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/vectors/components.json"
    );
    let text = std::fs::read_to_string(path).expect("vectors are versioned with the specs");
    serde_json::from_str(&text).expect("vectors parse")
}

fn bytes_of(case: &Value) -> Vec<u8> {
    let hex = case["hex"].as_str().expect("case has hex bytes");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn word(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

/// Decode the header every component shares.
///
/// It ends at `ibmp`, the picture index, at `+0x32` — the same offset in every
/// one of the sixteen part tables and in both hull tables.
fn header(b: &[u8]) -> (i16, [i8; 6], String, i16, u16, [i16; 3], u16) {
    let id = word(b, 0);
    let mut tech = [0i8; 6];
    for (i, t) in tech.iter_mut().enumerate() {
        *t = b[2 + i] as i8;
    }
    let name_end = b[8..40].iter().position(|c| *c == 0).unwrap_or(32) + 8;
    let name = String::from_utf8_lossy(&b[8..name_end]).into_owned();
    let mass = word(b, 0x28);
    let resource_cost = u16::from_le_bytes([b[0x2a], b[0x2b]]);
    let ore = [word(b, 0x2c), word(b, 0x2e), word(b, 0x30)];
    let picture = word(b, 0x32);
    assert!(picture >= 0, "a picture index is never negative");
    (
        id,
        tech,
        name,
        mass,
        resource_cost,
        ore,
        u16::try_from(picture).expect("checked just above"),
    )
}

#[test]
fn tables_match_the_bytes_in_the_binary() {
    let v = vectors();
    let cases = v["cases"].as_array().expect("cases array").clone();
    assert!(!cases.is_empty());

    for case in cases {
        let b = bytes_of(&case);
        let index = usize::try_from(case["index"].as_u64().unwrap()).unwrap();
        let table = case["table"].as_str().unwrap();
        let expect = &case["expect"];
        let (id, tech, name, mass, resource_cost, ore, picture) = header(&b);
        let where_ = format!("{table}[{index}] at {}", case["address"].as_str().unwrap());

        // The vector's own decoded fields must agree with the raw bytes, so a
        // typo in the JSON cannot silently define away the check.
        assert_eq!(
            i64::from(id),
            expect["id"].as_i64().unwrap(),
            "{where_}: id"
        );
        assert_eq!(name, expect["name"].as_str().unwrap(), "{where_}: name");
        assert_eq!(
            i64::from(picture),
            expect["picture"].as_i64().unwrap(),
            "{where_}: picture"
        );

        match table {
            "ENGINES" => {
                let e = &ENGINES[index];
                assert_eq!(e.id, id, "{where_}");
                assert_eq!(e.name, name, "{where_}");
                assert_eq!(e.tech, tech, "{where_}");
                assert_eq!(e.mass, mass, "{where_}");
                assert_eq!(e.resource_cost, resource_cost, "{where_}");
                assert_eq!(e.picture, picture, "{where_}: picture");
                assert_eq!(e.ore_cost, ore, "{where_}");
                assert_eq!(e.abilities, word(&b, 0x34), "{where_}");
                for (i, fuel) in e.fuel_used.iter().enumerate() {
                    assert_eq!(*fuel, word(&b, 0x36 + i * 2), "{where_}: fuel at warp {i}");
                }
            }
            "ARMORS" => {
                let a = &ARMORS[index];
                assert_eq!(
                    (a.id, a.name, a.tech, a.mass),
                    (id, name.as_str(), tech, mass)
                );
                assert_eq!(a.resource_cost, resource_cost, "{where_}");
                assert_eq!(a.picture, picture, "{where_}: picture");
                assert_eq!(a.ore_cost, ore, "{where_}");
                assert_eq!(a.dp, word(&b, 0x34), "{where_}: dp");
            }
            "BEAMS" => {
                let w = &BEAMS[index];
                assert_eq!(
                    (w.id, w.name, w.tech, w.mass),
                    (id, name.as_str(), tech, mass)
                );
                assert_eq!(w.resource_cost, resource_cost, "{where_}");
                assert_eq!(w.picture, picture, "{where_}: picture");
                assert_eq!(w.ore_cost, ore, "{where_}");
                assert_eq!(w.range_max, word(&b, 0x34), "{where_}: range");
                assert_eq!(w.dp, word(&b, 0x36), "{where_}: damage");
                assert_eq!(w.initiative, word(&b, 0x38), "{where_}: initiative");
                assert_eq!(w.abilities, word(&b, 0x3a), "{where_}: abilities");
            }
            "PLANETARY" => {
                let p = &PLANETARY[index];
                assert_eq!(
                    (p.id, p.name, p.tech, p.mass),
                    (id, name.as_str(), tech, mass)
                );
                assert_eq!(p.resource_cost, resource_cost, "{where_}");
                assert_eq!(p.picture, picture, "{where_}: picture");
                assert_eq!(p.ore_cost, ore, "{where_}");
                assert_eq!(p.ability, word(&b, 0x34), "{where_}: ability");
            }
            other => panic!("vector names an unknown table {other}"),
        }
    }
}

#[test]
fn table_sizes_match_the_binary_declarations() {
    // Sizes come from the global table declarations in
    // docs/ghidra/stars-globals.csv.
    assert_eq!(ENGINES.len(), 16);
    assert_eq!(ARMORS.len(), 12);
    assert_eq!(BEAMS.len(), 24);
    assert_eq!(PLANETARY.len(), 15);
    assert_eq!(stars_core::components::SCANNERS.len(), 16);
    assert_eq!(stars_core::components::SHIELDS.len(), 10);
    assert_eq!(stars_core::components::TORPEDOES.len(), 12);
}

#[test]
fn penetrating_scanners_are_stored_as_negative_ranges() {
    // The Snooper series are the planet-penetrating planetary scanners; every
    // one of them stores a negative range.
    let snoopers: Vec<_> = PLANETARY.iter().filter(|p| p.ability < 0).collect();
    assert!(
        snoopers.iter().all(|p| p.name.starts_with("Snooper")),
        "only Snoopers should store a negative range"
    );
    assert_eq!(snoopers.len(), 4, "there are four Snoopers");
}

#[test]
fn engine_fuel_lookup_matches_the_table() {
    use stars_core::movement::engine_fuel_use;

    // Settler's Delight (id 1) is free up to warp 6 and costs 140 at warp 7.
    assert_eq!(engine_fuel_use(1, 6), Some(0));
    assert_eq!(engine_fuel_use(1, 7), Some(140));
    assert_eq!(engine_fuel_use(1, 10), Some(576));
    // Warp 11 is past the table's useful range for this engine.
    assert_eq!(engine_fuel_use(1, 11), Some(0));
    assert_eq!(engine_fuel_use(999, 7), None, "unknown engine id");
}

#[test]
fn the_best_planetary_scanner_advances_with_electronics() {
    use stars_core::components::best_planetary_scanner;

    // With no technology at all, only the Viewer 50 is buildable.
    let none = best_planetary_scanner(&[0, 0, 0, 0, 0, 0]).expect("Viewer 50 needs nothing");
    assert_eq!(none.name, "Viewer 50");
    assert_eq!(none.ability, 50);

    // Electronics 3 reaches the Scoper 150.
    let scoper = best_planetary_scanner(&[0, 0, 0, 0, 3, 0]).expect("Scoper 150");
    assert_eq!(scoper.name, "Scoper 150");

    // The Snooper 320X additionally needs Energy 3 and Biotech 3, and is
    // penetrating, so its stored range is negative.
    let snooper = best_planetary_scanner(&[3, 0, 0, 0, 10, 3]).expect("Snooper 320X");
    assert_eq!(snooper.name, "Snooper 320X");
    assert_eq!(snooper.ability, -320);
}

/// Every component's picture lands on a cell that is really in the sheet it
/// names — and a hull's names a different sheet altogether.
///
/// This is what says the sheet geometry in `stars_formats::resources::art` and
/// the `ibmp` values here agree. Two separate index spaces meet in the same
/// field, so the test is in two halves.
#[test]
fn every_component_picture_is_a_real_cell() {
    use stars_core::components::slot;
    use stars_core::parts::part;
    use stars_formats::resources::art;

    // Only the last component sheet is narrow: four cells across, not eight.
    let columns = |resource: u16| if resource == 506 { 4 } else { 8 };

    let mut checked = 0;
    let mut on_the_narrow_sheet = 0;
    for category in [
        slot::ENGINE,
        slot::SCANNER,
        slot::SHIELD,
        slot::ARMOR,
        slot::BEAM,
        slot::TORPEDO,
        slot::BOMB,
        slot::MINING,
        slot::MINES,
        slot::SPECIAL_SB,
        slot::SPECIAL_E,
        slot::SPECIAL_M,
        slot::TERRA,
        slot::PLANETARY,
    ] {
        for item in 0..64 {
            let Some(p) = part(category, item) else { break };
            let cell = art::component(p.picture)
                .unwrap_or_else(|| panic!("{} has no picture cell", p.name));
            assert_eq!((cell.width, cell.height), (64, 64), "{}", p.name);
            assert!(
                cell.x / 64 < columns(cell.resource),
                "{} is off the edge of sheet {}",
                p.name,
                cell.resource
            );
            assert!(cell.y / 64 < 4, "{}", p.name);
            if cell.resource == 506 {
                on_the_narrow_sheet += 1;
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 202, "every component that is not a hull");
    assert_eq!(
        on_the_narrow_sheet, 10,
        "and ten of them share the last sheet"
    );
}

/// A hull's picture is the base of its **own group of four** in the ship
/// sheets, and between them the thirty-seven hulls account for every one of
/// the 148 pictures there are.
///
/// That is the arithmetic `DrawFleetBitmap` relies on when it reduces an index
/// modulo 148, and it is what the designer's spin buttons walk: they change
/// the low two bits and nothing else, so a hull's four are its own.
#[test]
fn the_hulls_own_the_ship_pictures_four_at_a_time() {
    use stars_core::components::slot;
    use stars_core::parts::part;
    use stars_formats::resources::art;

    let mut bases = Vec::new();
    for category in [slot::HULL, slot::SB_HULL] {
        for item in 0..64 {
            let Some(p) = part(category, item) else { break };
            assert_eq!(
                p.picture % art::PICTURES_PER_HULL,
                0,
                "{} does not start a group of four",
                p.name
            );
            assert!(p.picture < art::SHIP_PICTURES, "{}", p.name);
            bases.push(p.picture);

            // All four of its pictures are real cells, and all four are in the
            // one column: that is what makes the designer's spin coherent.
            let column = art::ship(p.picture, art::ShipSize::Large);
            for variant in 0..4u8 {
                let index = art::hull_picture(p.picture, variant);
                assert_eq!(index, p.picture + u16::from(variant), "{}", p.name);
                let cell = art::ship(index, art::ShipSize::Large);
                assert_eq!(cell.resource, column.resource, "{}", p.name);
                assert_eq!(cell.x, column.x, "{} stays in its column", p.name);
                assert_eq!(cell.y, u32::from(variant) * 64, "{}", p.name);
                // And the half-size sheet holds the same picture again.
                let small = art::ship(index, art::ShipSize::Small);
                assert_eq!((small.width, small.height), (32, 32));
                assert_eq!((small.x, small.y), (cell.x / 2, cell.y / 2), "{}", p.name);
            }
            // Spinning past either end comes back round inside the four.
            assert_eq!(art::hull_picture(p.picture, 4), p.picture);
        }
    }

    assert_eq!(bases.len(), 37, "thirty-two ship hulls and five starbases");
    bases.sort_unstable();
    bases.dedup();
    assert_eq!(bases.len(), 37, "no two hulls share a group");
    let want: Vec<u16> = (0..37).map(|i| i * art::PICTURES_PER_HULL).collect();
    assert_eq!(bases, want, "and between them they cover all 148 pictures");
}

/// The terraforming modules are named with a "±", which says the module moves
/// a value either way rather than only up.
#[test]
fn the_terraforming_names_keep_their_sign() {
    use stars_core::components::TERRAFORMING;
    assert_eq!(TERRAFORMING[0].name, "Total Terraform ±3");
    assert_eq!(TERRAFORMING[8].name, "Gravity Terraform ±3");
    assert!(
        TERRAFORMING.iter().all(|t| t.name.contains('±')),
        "all twenty of them"
    );
}
