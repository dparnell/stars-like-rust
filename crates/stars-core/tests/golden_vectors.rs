//! The Step 3 formulas checked against the golden vectors in
//! `docs/vectors/planetary-economy.json`.
//!
//! Every case in that file is a rule stated in the original game manual, so
//! these tests check the implementation against the *documented* behaviour of
//! Stars! rather than against itself. Where the manual and the binary could
//! disagree the binary wins — see `docs/formulas/` — but on all of these they
//! agree, which is why they make good vectors.

use serde_json::Value;

use stars_core::hab::{calc_planet_max_pop, max_pop_for_hab};
use stars_core::mining::minerals_mined;
use stars_core::movement::travel_per_year;
use stars_core::planet::Planet;
use stars_core::population::chg_pop_from_planet;
use stars_core::race::{Prt, Race, RaceStat};
use stars_core::resources::resources_at_planet;
use stars_core::scanning::combine_ranges;

fn vectors() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/vectors/planetary-economy.json"
    );
    let text = std::fs::read_to_string(path).expect("golden vectors are versioned with the specs");
    serde_json::from_str(&text).expect("golden vectors parse")
}

fn i(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("case is missing {key}: {v}"))
}

fn prt_of(name: &str) -> Prt {
    match name {
        "He" => Prt::He,
        "Is" => Prt::Is,
        "Ar" => Prt::Ar,
        _ => Prt::Joat,
    }
}

/// A race whose habitable range is a single point, so that a planet sitting on
/// that point has exactly the habitability we want to test with.
fn race_with(prt: Prt, obrm: bool) -> Race {
    let mut race = Race::humanoid();
    race.attrs[RaceStat::MajorAdv as usize] = prt as i16;
    if obrm {
        race.lrt_bits |= 1 << stars_core::race::lrt::OBRM;
    }
    race
}

/// A planet sitting exactly on the race's ideal, i.e. 100% habitable.
fn optimal_planet(race: &Race) -> Planet {
    let mut planet = Planet::unowned(0);
    planet.owner = Some(0);
    planet.env = race.env_center;
    assert_eq!(
        stars_core::pct_planet_desirability(&planet, race),
        100,
        "a planet on the race's ideal is 100% habitable"
    );
    planet
}

/// A planet `clicks` outside the race's habitable range on one variable, which
/// gives it a habitability of exactly `-clicks` (the penalty is the distance to
/// the nearest bound, capped at 15).
fn hostile_planet(race: &Race, clicks: i8) -> Planet {
    assert!((1..=15).contains(&clicks));
    let mut planet = Planet::unowned(0);
    planet.owner = Some(0);
    planet.env = [
        race.env_min[0] - clicks,
        race.env_center[1],
        race.env_center[2],
    ];
    assert_eq!(
        stars_core::pct_planet_desirability(&planet, race),
        i16::from(-clicks),
        "a planet {clicks} clicks outside the range is -{clicks}% habitable"
    );
    planet
}

#[test]
fn maximum_population_matches_the_manual() {
    let v = vectors();
    let cases = v["max_population"]["cases"].as_array().unwrap().clone();
    assert!(!cases.is_empty());
    for case in cases {
        let prt = prt_of(case["prt"].as_str().unwrap());
        let obrm = case["obrm"].as_bool().unwrap();
        let race = race_with(prt, obrm);
        let hab = i16::try_from(i(&case, "hab")).unwrap();

        let got = max_pop_for_hab(hab, &race);
        assert_eq!(
            i64::from(got),
            i(&case, "expect"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

#[test]
fn population_death_rate_matches_the_manual() {
    let v = vectors();
    let race = race_with(Prt::Is, false);
    for case in v["population_change"]["cases"].as_array().unwrap() {
        // Only the hostile-planet cases are expressible without a max_pop.
        if case.get("max_pop").is_some() {
            continue;
        }
        let hab = i16::try_from(i(case, "hab")).unwrap();
        let mut planet = hostile_planet(&race, i8::try_from(-hab).unwrap());
        planet.pop = i32::try_from(i(case, "pop")).unwrap();

        let change = chg_pop_from_planet(&planet, &race).expect("owned and populated");
        assert_eq!(
            i64::from(change.delta),
            i(case, "expect_delta"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

#[test]
fn overcrowding_deaths_reach_twelve_percent_at_four_times_capacity() {
    // MANUAL.PDF p. 6-3. A 100% planet for a race with no population modifier
    // holds 10000; four times that is 40000, and the annual loss is 12%.
    let race = race_with(Prt::Is, false);
    let mut planet = optimal_planet(&race);
    planet.pop = 40_000;

    let max_pop = calc_planet_max_pop(&planet, &race).unwrap();
    assert_eq!(max_pop, 10_000, "an optimal planet holds 1,000,000");

    let change = chg_pop_from_planet(&planet, &race).expect("owned and populated");
    assert_eq!(change.delta, -4_800, "12% of 40000");
}

#[test]
fn resource_output_matches_the_manual() {
    let v = vectors();
    let race = race_with(Prt::Is, false);
    for case in v["resources"]["cases"].as_array().unwrap() {
        let mut planet = optimal_planet(&race);
        planet.pop = i32::try_from(i(case, "pop")).unwrap();
        planet.factories = i16::try_from(i(case, "factories")).unwrap();

        if let Some(expected_cap) = case.get("max_pop").and_then(Value::as_i64) {
            assert_eq!(
                i64::from(calc_planet_max_pop(&planet, &race).unwrap()),
                expected_cap,
                "case assumes a capacity of {expected_cap}"
            );
        }

        let got = resources_at_planet(&planet, &race, 0).expect("not an AR race");
        assert_eq!(
            i64::from(got),
            i(case, "expect"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

#[test]
fn mining_output_matches_the_manual() {
    let v = vectors();
    let race = race_with(Prt::Is, false);
    for case in v["mining"]["cases"].as_array().unwrap() {
        let mut planet = optimal_planet(&race);
        // Enough population to staff every mine, and not a home world (whose
        // concentration floor of 30 would change the answer).
        planet.pop = 10_000;
        planet.mines = i16::try_from(i(case, "mines")).unwrap();
        let conc = u8::try_from(i(case, "concentration")).unwrap();
        planet.min_conc = [conc, conc, conc];

        let mined = minerals_mined(&planet, &race, None, None);
        assert_eq!(
            i64::from(mined[0]),
            i(case, "expect_kt"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

#[test]
fn scanner_ranges_combine_as_the_fourth_root_of_fourth_powers() {
    let v = vectors();
    for case in v["scanning"]["cases"].as_array().unwrap() {
        let ranges: Vec<i32> = case["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| i32::try_from(r.as_i64().unwrap()).unwrap())
            .collect();
        assert_eq!(
            i64::from(combine_ranges(&ranges)),
            i(case, "expect"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}

#[test]
fn fleets_travel_warp_squared_light_years_a_year() {
    let v = vectors();
    for case in v["movement"]["cases"].as_array().unwrap() {
        let warp = i16::try_from(i(case, "warp")).unwrap();
        assert_eq!(
            i64::from(travel_per_year(warp)),
            i(case, "expect_ly"),
            "{}",
            case["why"].as_str().unwrap()
        );
    }
}
