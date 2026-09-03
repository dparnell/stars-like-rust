//! The research formulas checked against `docs/vectors/research.json`.

use serde_json::Value;

use stars_core::race::{lrt, Race, RaceStat};
use stars_core::research::{add_research, tech_level_cost, NextField, Research, TECH_FIELDS};

fn vectors() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/vectors/research.json"
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
fn tech_level_costs_match_the_vectors() {
    let v = vectors();
    let cases = v["tech_level_cost"]["cases"].as_array().unwrap().clone();
    assert!(!cases.is_empty());

    for case in cases {
        let mut research = Research::default();
        for (field, level) in case["levels"].as_array().unwrap().iter().enumerate() {
            research.levels[field] = u8::try_from(level.as_i64().unwrap()).unwrap();
        }

        let field = usize::try_from(i(&case, "field")).unwrap();
        let level = u8::try_from(i(&case, "level")).unwrap();
        let setting = i16::try_from(i(&case, "setting")).unwrap();
        let slow_tech = case["slow_tech"].as_bool().unwrap();

        let mut race = Race::humanoid();
        for f in 0..TECH_FIELDS {
            race.attrs[RaceStat::TechBonus1 as usize + f] = 1;
        }
        race.attrs[RaceStat::TechBonus1 as usize + field] = setting;

        let got = tech_level_cost(field, level, &research, &race, slow_tech);
        assert_eq!(
            i64::from(got),
            i(&case, "expect"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

#[test]
fn generalized_research_splits_the_budget() {
    let v = vectors();
    let mut race = Race::humanoid();
    for f in 0..TECH_FIELDS {
        race.attrs[RaceStat::TechBonus1 as usize + f] = 1;
    }
    race.lrt_bits |= 1 << lrt::GENERALIZED_RESEARCH;

    for case in v["generalized_research"]["cases"].as_array().unwrap() {
        let budget = i32::try_from(i(case, "budget")).unwrap();

        // Levels high enough that nothing can be afforded, so the banked
        // points are left where the split put them.
        let mut research = Research {
            levels: [26; TECH_FIELDS],
            points: [0; TECH_FIELDS],
            current_field: 2,
            next_field: NextField::Same,
        };
        let gained = add_research(&mut research, &race, budget, false);
        assert!(
            gained.is_empty(),
            "nothing should be affordable at level 26"
        );

        if let Some(expected) = case.get("expect_primary").and_then(Value::as_i64) {
            assert_eq!(
                i64::from(research.points[2]),
                expected,
                "primary field: {}",
                case["why"].as_str().unwrap()
            );
        }
        if let Some(expected) = case.get("expect_other").and_then(Value::as_i64) {
            for field in 0..TECH_FIELDS {
                if field == 2 {
                    continue;
                }
                assert_eq!(
                    i64::from(research.points[field]),
                    expected,
                    "field {field}: {}",
                    case["why"].as_str().unwrap()
                );
            }
        }
    }
}

#[test]
fn research_switches_field_and_carries_the_leftover() {
    let mut race = Race::humanoid();
    for f in 0..TECH_FIELDS {
        race.attrs[RaceStat::TechBonus1 as usize + f] = 1;
    }

    // Energy at level 0 costs 50; give it 200 and ask to move to Weapons next.
    let mut research = Research {
        levels: [0; TECH_FIELDS],
        points: [0; TECH_FIELDS],
        current_field: 0,
        next_field: NextField::Field(1),
    };
    let gained = add_research(&mut research, &race, 200, false);

    assert_eq!(gained.first().map(|b| (b.field, b.level)), Some((0, 1)));
    assert_eq!(research.current_field, 1, "research moved to Weapons");
    assert_eq!(research.points[0], 0, "the old field keeps nothing");
    assert!(
        research.points[1] > 0 || research.levels[1] > 0,
        "the leftover followed the player to the new field"
    );
}
