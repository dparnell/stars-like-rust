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
