//! The combat formulas checked against `docs/vectors/combat.json`.

use serde_json::Value;

use stars_core::battle::{distance, movement_this_round, start_square, torpedo_accuracy, Square};

fn vectors() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/vectors/combat.json"
    );
    let text = std::fs::read_to_string(path).expect("vectors are versioned with the specs");
    serde_json::from_str(&text).expect("vectors parse")
}

fn i(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("case is missing {key}: {v}"))
}

#[test]
fn movement_matches_the_manuals_table() {
    let v = vectors();
    let cases = v["movement_per_round"]["cases"].as_array().unwrap().clone();
    assert_eq!(cases.len(), 9, "the manual lists nine speeds");

    for case in cases {
        let speed = u8::try_from(i(&case, "speed")).unwrap();
        let label = case["speed_label"].as_str().unwrap();
        let expected: Vec<u8> = case["rounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| u8::try_from(r.as_i64().unwrap()).unwrap())
            .collect();

        let got: Vec<u8> = (0..8).map(|r| movement_this_round(speed, r)).collect();
        assert_eq!(got, expected, "speed {label} (stored {speed})");

        // Over eight rounds the average must be the nominal speed, which is
        // what the fractional schedule exists to achieve.
        let total: u32 = got.iter().map(|d| u32::from(*d)).sum();
        let nominal = (u32::from(speed) + 2) * 8;
        assert!(
            total * 4 == nominal,
            "speed {label}: eight rounds cover {total} squares, expected {}",
            nominal / 4
        );
    }
}

#[test]
fn torpedo_accuracy_matches_the_manuals_examples() {
    let v = vectors();
    for case in v["torpedo_accuracy"]["cases"].as_array().unwrap() {
        let got = torpedo_accuracy(
            i32::try_from(i(case, "base")).unwrap(),
            i32::try_from(i(case, "jam")).unwrap(),
            i32::try_from(i(case, "computer")).unwrap(),
        );
        assert_eq!(
            i64::from(got),
            i(case, "expect"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

#[test]
fn starting_squares_match_the_table() {
    let v = vectors();
    for case in v["start_squares"]["cases"].as_array().unwrap() {
        let players = u8::try_from(i(case, "players")).unwrap();
        for (side, square) in case["squares"].as_array().unwrap().iter().enumerate() {
            let coords = square.as_array().unwrap();
            let expected = Square::new(
                u8::try_from(coords[0].as_i64().unwrap()).unwrap(),
                u8::try_from(coords[1].as_i64().unwrap()).unwrap(),
            );
            assert_eq!(
                start_square(players, u8::try_from(side).unwrap()),
                Some(expected),
                "{players} players, side {side}"
            );
        }
        // One past the last side has no square.
        assert_eq!(start_square(players, players), None);
    }
}

#[test]
fn board_distance_is_chebyshev() {
    let v = vectors();
    for case in v["board_distance"]["cases"].as_array().unwrap() {
        let from = case["from"].as_array().unwrap();
        let to = case["to"].as_array().unwrap();
        let a = Square::new(
            u8::try_from(from[0].as_i64().unwrap()).unwrap(),
            u8::try_from(from[1].as_i64().unwrap()).unwrap(),
        );
        let b = Square::new(
            u8::try_from(to[0].as_i64().unwrap()).unwrap(),
            u8::try_from(to[1].as_i64().unwrap()).unwrap(),
        );
        assert_eq!(i64::from(distance(a, b)), i(case, "expect"));
        assert_eq!(distance(a, b), distance(b, a), "distance is symmetric");
    }
}

#[test]
fn squares_round_trip_through_their_packed_byte() {
    for y in 0..16u8 {
        for x in 0..16u8 {
            let s = Square::new(x, y);
            assert_eq!(Square::from_brc(s.to_brc()), s);
        }
    }
}

#[test]
fn design_armour_matches_the_vectors() {
    use stars_core::components::slot;
    use stars_core::design::{DesignSlot, ShipDesign};

    let v = vectors();
    for case in v["design_armour"]["cases"].as_array().unwrap() {
        let hull_id = i16::try_from(i(case, "hull")).unwrap();
        let count = u8::try_from(i(case, "armor_count")).unwrap();
        let regenerating = case["regenerating_shields"].as_bool().unwrap();

        let mut slots = Vec::new();
        if let Some(item) = case["armor_item"].as_i64() {
            slots.push(DesignSlot {
                category: slot::ARMOR,
                item: u8::try_from(item).unwrap(),
                count,
            });
        }

        let design = ShipDesign { hull_id, slots };
        assert_eq!(
            i64::from(design.armor(regenerating).expect("known hull")),
            i(case, "expect"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

/// The manual's worked damage examples (`MANUAL.PDF` p. 23-2).
///
/// These exercise the two rules that make stacking ships worthwhile: shields
/// pool across a whole token, and leftover beam damage is spread thinly over
/// the survivors instead of finishing another ship.
#[test]
fn damage_matches_the_manuals_worked_examples() {
    use stars_core::battle::{apply_damage, Damage, TokenState};

    // "If the ships had 100 dp armor and 50 dp shields each, then stacked
    // together the shields would have absorbed all 500dp and no ships would
    // have been lost."
    let stacked = TokenState {
        ships: 10,
        shields: 50,
        armor: 100,
        damage: Damage::default(),
    };
    let result = apply_damage(stacked, 500, false);
    assert_eq!(result.ships_killed, 0, "the pooled shields absorb it all");
    assert_eq!(result.shield_damage, 500);
    assert_eq!(result.after.shields, 0, "and are stripped doing so");

    // "If those 10 tokens had been a single token of 10 ships they would have
    // still lost three ships, but each of the remaining seven ships would have
    // taken less than 5% damage." — ten ships of 150 armour, no shields.
    let unshielded = TokenState {
        ships: 10,
        shields: 0,
        armor: 150,
        damage: Damage::default(),
    };
    let result = apply_damage(unshielded, 500, false);
    assert_eq!(result.ships_killed, 3, "450 of the 500 kills three ships");
    assert_eq!(result.after.ships, 7);
    assert_eq!(
        result.after.damage.pct_ships, 100,
        "the rest share the damage"
    );
    // pct_damage is in 500ths of the design's armour.
    let pct = f64::from(result.after.damage.pct_damage) / 5.0;
    assert!(
        pct < 5.0,
        "each survivor should carry under 5% damage, got {pct:.1}%"
    );

    // A single ship of 150 armour takes 500: it dies, and 350 spills over to
    // whatever else shares its square.
    let lone = TokenState {
        ships: 1,
        shields: 0,
        armor: 150,
        damage: Damage::default(),
    };
    let result = apply_damage(lone, 500, false);
    assert_eq!(result.ships_killed, 1);
    assert_eq!(result.overflow, 350, "the excess spills to other tokens");
}

#[test]
fn a_sapper_strips_shields_and_stops() {
    use stars_core::battle::{apply_damage, Damage, TokenState};

    let token = TokenState {
        ships: 5,
        shields: 20,
        armor: 100,
        damage: Damage::default(),
    };
    let result = apply_damage(token, 500, true);
    assert_eq!(result.shield_damage, 100, "5 ships x 20 shield points");
    assert_eq!(result.ships_killed, 0, "a sapper never touches armour");
    assert_eq!(result.overflow, 0);
}

#[test]
fn damaged_ships_are_finished_off_first() {
    use stars_core::battle::{apply_damage, Damage, TokenState};

    // Ten ships of 100 armour, half of them already at 80% damage: those cost
    // only 20 apiece to destroy, so 100 damage kills five of them rather than
    // one fresh ship.
    let token = TokenState {
        ships: 10,
        shields: 0,
        armor: 100,
        damage: Damage {
            pct_ships: 50,
            pct_damage: 400, // 400/500 of 100 armour = 80 already taken
        },
    };
    let result = apply_damage(token, 100, false);
    assert_eq!(
        result.ships_killed, 5,
        "five hulks at 20 apiece, not one fresh ship"
    );
    assert_eq!(result.after.ships, 5);
}

#[test]
fn beam_damage_falls_off_with_range() {
    use stars_core::battle::{beam_damage, Weapon};

    // A range-3 beam doing 100 damage, one launcher, one ship.
    let weapon = Weapon {
        torpedo: false,
        dp: 100,
        count: 1,
        range: 3,
        initiative: 5,
        accuracy: 100,
        abilities: 0,
    };

    assert_eq!(
        beam_damage(weapon, 1, 0, 0, 100),
        100,
        "point blank is full"
    );
    // A tenth of the damage is lost across the full range band.
    assert_eq!(
        beam_damage(weapon, 1, 3, 0, 100),
        90,
        "10% lost at max range"
    );
    assert_eq!(beam_damage(weapon, 1, 1, 0, 100), 97);
    assert_eq!(beam_damage(weapon, 1, 4, 0, 100), 0, "out of range");

    // Damage scales with the number of ships firing.
    assert_eq!(beam_damage(weapon, 8, 0, 0, 100), 800);
    // A capacitor scales it up; beam deflection scales it down.
    assert_eq!(beam_damage(weapon, 1, 0, 120, 100), 120);
    assert_eq!(beam_damage(weapon, 1, 0, 0, 90), 90);
}
