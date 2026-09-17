//! New-game generation, checked against a real turn-0 game.
//!
//! `fixtures/incoming/turn0/` is a three-player game as `GenerateWorld` left
//! it, before anybody took a turn. It pins down almost every stage: the planet
//! count, the coordinate band, the homeworlds' installations and environments,
//! how the leftover advantage points were spent, which built-in computer race
//! the two opponents are, and all six of player 0's ship designs.
//!
//! Tests skip rather than fail when the fixtures are absent, so a checkout
//! without sample games still builds green.

use stars_core::newgame::{generate, planet_count, Density, NewGame, NewPlayer, Size};
use stars_core::opponents;
use stars_core::race::{Prt, Race};
use stars_core::startup::{ship, upgrade_slots, SHIPS};
use stars_core::{advantage_points, GameState, Planet, Rng};
use stars_formats::{StarsFile, Universe};

use serde_json::Value;

/// The golden vectors that accompany `docs/formulas/new-game.md`.
fn vectors() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/vectors/new-game.json"
    );
    let text = std::fs::read_to_string(path).expect("vectors are versioned with the specs");
    serde_json::from_str(&text).expect("vectors parse")
}

/// The cases of one vector section.
fn cases(section: &str) -> Vec<Value> {
    vectors()[section]["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{section} has no cases"))
        .clone()
}

fn number(case: &Value, key: &str) -> i64 {
    case.get(key)
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("case is missing {key}: {case}"))
}

fn numbers(case: &Value, key: &str) -> Vec<i64> {
    case.get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("case is missing {key}: {case}"))
        .iter()
        .map(|v| v.as_i64().expect("a number"))
        .collect()
}

const TURN0: &str = "../../fixtures/incoming/turn0";

fn read(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

fn turn0_state() -> Option<GameState> {
    let bytes = read(&format!("{TURN0}/Game.hst"))?;
    let file = StarsFile::decode(&bytes).ok()?;
    let (mut state, _) = GameState::from_file(&file);
    if let Some(universe) =
        read(&format!("{TURN0}/Game.xy")).and_then(|b| Universe::decode(&b).ok())
    {
        state.apply_universe(&universe);
    }
    Some(state)
}

/// Walk `fixtures/` for files with one extension.
fn fixtures(extension: &str) -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, extension: &str, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, extension, out);
            } else if path.extension().is_some_and(|e| e == extension) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(std::path::Path::new("../../fixtures"), extension, &mut out);
    out.sort();
    out
}

/// The planet-count vectors from `docs/vectors/new-game.json`.
#[test]
fn planet_counts_match_the_vectors() {
    for case in cases("planet_count") {
        let size = Size::ALL
            .iter()
            .find(|s| i64::from(**s as i16) == number(&case, "size"))
            .copied()
            .expect("a known size");
        let density = Density::ALL
            .iter()
            .find(|d| i64::from(**d as i16) == number(&case, "density"))
            .copied()
            .expect("a known density");
        assert_eq!(
            i64::from(planet_count(size, density)),
            number(&case, "expect"),
            "{case}"
        );
    }
}

/// The coordinate band each universe size generates into.
#[test]
fn coordinate_bands_match_the_vectors() {
    for case in cases("coordinate_band") {
        let size = Size::ALL
            .iter()
            .find(|s| i64::from(**s as i16) == number(&case, "size"))
            .copied()
            .expect("a known size");
        assert_eq!(i64::from(1010), number(&case, "low"));
        assert_eq!(
            i64::from(1010 + size.span() - 20),
            number(&case, "high"),
            "{case}"
        );
    }
}

/// Starting technology, against the vectors.
#[test]
fn starting_technology_matches_the_vectors() {
    let want = |name: &str| -> Option<[u8; 6]> {
        let race = match name {
            "Jack of All Trades" => stars_core::newgame::stock_race(Prt::Joat),
            "Super Stealth with Improved Fuel Efficiency" => {
                let mut race = stars_core::newgame::stock_race(Prt::Ss);
                race.lrt_bits |= 1 << stars_core::race::lrt::IFE;
                race
            }
            "War Monger" => stars_core::newgame::stock_race(Prt::Wm),
            "Claim Adjuster" => stars_core::newgame::stock_race(Prt::Ca),
            "Inner Tech" => stars_core::newgame::stock_race(Prt::It),
            "Inner Strength" => stars_core::newgame::stock_race(Prt::Is),
            _ => return None,
        };
        Some(stars_core::newgame::starting_tech(&race))
    };
    for case in cases("starting_technology") {
        let name = case["race"].as_str().expect("a race name");
        let Some(got) = want(name) else {
            panic!("the vectors name a race this test does not build: {name}");
        };
        let expect: Vec<i64> = numbers(&case, "expect");
        assert_eq!(
            got.iter().map(|v| i64::from(*v)).collect::<Vec<_>>(),
            expect,
            "{name}"
        );
    }
}

/// The planet count formula, against every real universe we have.
#[test]
fn planet_counts_match_every_real_universe() {
    let mut checked = 0;
    for path in fixtures("xy") {
        let Some(bytes) = read(&path.to_string_lossy()) else {
            continue;
        };
        let Ok(universe) = Universe::decode(&bytes) else {
            continue;
        };
        let Ok(game) = universe.game() else { continue };
        let (Some(size), Some(density)) = (
            Size::ALL.iter().find(|s| **s as i16 == game.size).copied(),
            Density::ALL
                .iter()
                .find(|d| **d as i16 == game.density)
                .copied(),
        ) else {
            continue;
        };
        assert_eq!(
            planet_count(size, density),
            game.planets,
            "{} ({} / {})",
            path.display(),
            size.name(),
            density.name()
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("no .xy fixtures; skipping");
    }
}

/// Planets are generated inside `[1010, 1010 + span - 20]` on both axes.
#[test]
fn generated_planets_sit_in_the_same_band_as_real_ones() {
    for size in Size::ALL {
        let config = NewGame {
            size,
            players: vec![NewPlayer::human(Race::humanoid())],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(0x1234_5678);
        let made = generate(&config, &mut rng).expect("generates");
        let low = 1010;
        let high = 1010 + size.span() - 20;
        for planet in &made.state.planets {
            let point = planet.position.expect("every planet is placed");
            assert!(
                (low..=high).contains(&point.x) && (low..=high).contains(&point.y),
                "{size:?}: {point:?} outside {low}..={high}"
            );
        }
        assert_eq!(
            made.state.planets.len(),
            planet_count(size, config.density) as usize
        );
    }
}

/// No two generated planets sit within twelve light years of each other.
#[test]
fn generated_planets_keep_their_distance() {
    let config = NewGame {
        size: Size::Small,
        density: Density::Packed,
        players: vec![NewPlayer::human(Race::humanoid())],
        ..NewGame::default()
    };
    let mut rng = Rng::randomize(0xfeed_face);
    let made = generate(&config, &mut rng).expect("generates");
    let points: Vec<_> = made
        .state
        .planets
        .iter()
        .filter_map(|p| p.position)
        .collect();
    for (i, a) in points.iter().enumerate() {
        for b in &points[i + 1..] {
            let dx = i32::from(a.x - b.x);
            let dy = i32::from(a.y - b.y);
            assert!(dx * dx + dy * dy > 12 * 12, "{a:?} and {b:?} are too close");
        }
    }
}

/// A new game opens with five messages for each player: the four playing
/// tips, with no object, and then one about the home planet, whose id is
/// both the object and the parameter.
///
/// `GenerateWorld` (`1078:0136`) sends them in that order, and the turn-0
/// fixture's `Game.m1` holds exactly those five records.
#[test]
fn a_new_game_opens_with_five_messages() {
    use stars_core::message::id;

    let config = NewGame {
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    };
    let mut rng = Rng::randomize(0xfeed_face);
    let made = generate(&config, &mut rng).expect("generates");
    let state = &made.state;

    for player in 0..2 {
        let mine: Vec<_> = state
            .messages
            .iter()
            .filter(|m| m.player == player)
            .collect();
        let ids: Vec<u16> = mine.iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            vec![
                id::TIP_FILTERING,
                id::TIP_WAYPOINTS,
                id::TIP_DESIGNER,
                id::TIP_POPUPS,
                id::HOME_PLANET
            ],
            "player {player}"
        );
        assert_eq!(
            ids,
            vec![127, 128, 129, 130, 169],
            "as the file numbers them"
        );
        for tip in &mine[..4] {
            assert_eq!(tip.object, -1, "a tip is about nothing in particular");
            assert!(tip.params.is_empty());
        }
        let home = state
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(i16::try_from(player).expect("a player")))
            .expect("a home world");
        assert_eq!(mine[4].object, home.id);
        assert_eq!(mine[4].params, vec![home.id]);
        // And each has words of its own, not a bare number.
        for message in &mine {
            assert!(
                !message.summary().starts_with("Message "),
                "{} has no wording",
                message.id
            );
        }
    }

    // The fixture agrees: player 0's turn-0 file carries these five.
    if let Some(bytes) = read("../../fixtures/incoming/turn0/Game.m1") {
        let file = StarsFile::decode(&bytes).expect("decodes");
        let records = stars_formats::message::message_records(&file);
        let ids: Vec<u16> = records.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![127, 128, 129, 130, 169]);
        assert_eq!(records[4].object, records[4].params[0]);
    }
}

/// The turn-0 game's homeworlds, field by field.
#[test]
fn turn0_homeworlds_are_what_generation_would_produce() {
    let Some(state) = turn0_state() else {
        eprintln!("no turn-0 fixture; skipping");
        return;
    };
    let homeworlds: Vec<&Planet> = state.planets.iter().filter(|p| p.homeworld).collect();
    assert_eq!(homeworlds.len(), 3);

    for home in &homeworlds {
        assert_eq!(home.mines, 10);
        assert_eq!(home.factories, 10);
        assert_eq!(home.defenses, 10);
        assert!(home.starbase);
        assert_eq!(home.pop, 250, "no race here has Low Starting Population");
        assert!(!home.artifact, "a homeworld never hides an artifact");

        // The environment is the exact middle of the owner's habitable band.
        let race = &state.owner(home).expect("owned").race;
        for axis in 0..3 {
            let want = i16::from(race.env_min[axis])
                + (i16::from(race.env_max[axis]) - i16::from(race.env_min[axis])) / 2;
            assert_eq!(i16::from(home.env[axis]), want, "axis {axis}");
        }
    }

    // Every homeworld is stocked from planet 0, with concentrations floored at
    // 30. Planet 0 of this game reads 86/8/67, and every homeworld 86/30/67.
    let planet0 = state
        .known_planets
        .iter()
        .chain(state.planets.iter())
        .find(|p| p.id == 0)
        .expect("planet 0");
    for home in &homeworlds {
        for axis in 0..3 {
            assert_eq!(
                home.min_conc[axis],
                planet0.min_conc[axis].max(30),
                "concentration {axis}"
            );
        }
    }
}

/// The advantage points the turn-0 homeworlds were stocked with.
///
/// The three homeworlds hold 399/399/432 (player 0) and 462/462/556 (players 1
/// and 2). Both come from the same planet-0 stock by the same rule, and the
/// only pair of point values that produces both is 25 and 50 (the cap).
///
/// The Jack of All Trades comes out at exactly 25. The two computer players do
/// **not** come out at 50 or more: our transcription prices that race at 13,
/// and the difference is not the Cheap Factories deduction alone. This test
/// pins the case that works and records the one that does not, rather than
/// asserting a number we know to be wrong — see the open question in
/// `docs/formulas/new-game.md`.
#[test]
fn turn0_advantage_points_are_reproduced() {
    let Some(state) = turn0_state() else {
        eprintln!("no turn-0 fixture; skipping");
        return;
    };
    assert_eq!(
        advantage_points(&state.players[0].race),
        25,
        "the human player is a stock Humanoid and prices exactly"
    );
    let computer = advantage_points(&state.players[1].race);
    assert_eq!(
        computer,
        advantage_points(&state.players[2].race),
        "the two computer players are the same race"
    );
    assert!(
        computer < 50,
        "known discrepancy: our transcription prices Turindrones, Standard at \
         {computer}, while the homeworlds it was given imply at least 50"
    );
}

/// The two computer players are `Turindrones, Standard`, byte for byte.
#[test]
fn turn0_computer_players_come_from_the_built_in_table() {
    let Some(state) = turn0_state() else {
        eprintln!("no turn-0 fixture; skipping");
        return;
    };
    let standard = opponents::opponent(1, 1).expect("Turindrones, Standard");
    assert_eq!(standard.name(), "Turindrones, Standard");
    assert_eq!(state.players[1].race, standard.race);
    assert_eq!(state.players[2].race, standard.race);
}

/// Every one of player 0's six starting designs, exactly as the file holds it.
///
/// Player 0 is a Jack of All Trades, which starts at technology 3 in all six
/// fields, so its templates are upgraded: Quick Jump 5 becomes a Long Hump 6,
/// the Bat Scanner a Rhino, Tritanium becomes Crobmnium and the Laser an X-Ray
/// — but the Alpha Torpedo and the Robo Mini-Miner stay, because nothing
/// better is in reach.
#[test]
fn turn0_starting_designs_are_reproduced() {
    /// A template index, the hull it should end up on, and its fitted slots.
    type Expected = (usize, i16, &'static [(u16, u8, u8)]);

    let joat = [3u8; 6];
    let want: [Expected; 3] = [
        (ship::TEAMSTER, 1, &[(1, 3, 1), (2, 1, 1), (8, 1, 1)]),
        (
            ship::STALWART_DEFENDER,
            6,
            &[
                (1, 3, 1),
                (16, 1, 1),
                (32, 0, 1),
                (2, 1, 1),
                (8, 1, 2),
                (4096, 5, 1),
                (2048, 5, 1),
            ],
        ),
        (
            ship::COTTON_PICKER,
            21,
            &[(1, 3, 1), (2, 1, 1), (128, 1, 1), (128, 1, 1)],
        ),
    ];
    for (template, hull, slots) in want {
        let mut design = SHIPS[template].design();
        upgrade_slots(&mut design, &joat);
        assert_eq!(design.hull_id, hull, "{}", SHIPS[template].name);
        let got: Vec<(u16, u8, u8)> = design
            .slots
            .iter()
            .map(|s| (s.category, s.item, s.count))
            .collect();
        assert_eq!(got, slots, "{}", SHIPS[template].name);
    }

    // The two computer players start at Electronics 5 and Propulsion 1, so
    // they get a Possum Scanner but keep the Quick Jump 5.
    let turindrone = [0u8, 0, 1, 0, 5, 0];
    let mut scout = SHIPS[ship::SMAUGARIAN_PEEPING_TOM].design();
    upgrade_slots(&mut scout, &turindrone);
    let got: Vec<(u16, u8, u8)> = scout
        .slots
        .iter()
        .map(|s| (s.category, s.item, s.count))
        .collect();
    assert_eq!(got, vec![(1, 1, 1), (2, 4, 1), (4096, 5, 1)]);
}

/// The leftover-points rule, against the fixture's own numbers.
///
/// Planet 0's surface stock is not in the file — generation zeroes it once it
/// finds the planet unowned — but it is determined by the three homeworlds:
/// only `[337, 337, 306]` produces 399/399/432 at 25 points and 462/462/556 at
/// 50 by the quarter-shares rule. 50 is the cap, so the second case says only
/// that the race priced at 50 or more.
#[test]
fn leftover_points_stock_the_homeworld() {
    let section = vectors()["advantage_points"].clone();
    let stock: Vec<i64> = numbers(&section, "planet_0_surface");
    let stock = [stock[0] as i32, stock[1] as i32, stock[2] as i32];

    for case in section["cases"].as_array().expect("cases") {
        let name = case["race"].as_str().expect("a race name");
        let race = if name.starts_with("stock Humanoid") {
            Race::humanoid()
        } else {
            opponents::opponent(1, 1).expect("Turindrones").race.clone()
        };
        // Where the vectors say what `CAdvantagePoints` must return, check it.
        if let Some(expect) = case.get("expect_points").and_then(Value::as_i64) {
            assert_eq!(i64::from(advantage_points(&race)), expect, "{name}");
        }
        // The spending rule is checked on its own, against the point value the
        // file implies rather than the one we compute — for one of these two
        // races those differ, and the rule is not what is in doubt.
        let points = i16::try_from(number(case, "points")).expect("a point value");
        let mut home = Planet::unowned(0);
        home.surface_min = stock;
        stars_core::newgame::spend_points(&mut home, &race, points, false);
        let expect: Vec<i64> = numbers(case, "expect_surface");
        assert_eq!(
            home.surface_min
                .iter()
                .map(|v| i64::from(*v))
                .collect::<Vec<_>>(),
            expect,
            "{name}"
        );
    }
}

/// Generated environments and mineral concentrations have the same shape as
/// real ones.
///
/// Gravity and temperature are the sum of two draws and radiation is a single
/// one, so the extremes of the first two are about half as common as the
/// third's — 4.8%/3.8% against 9.2% in the real games. And about two thirds of
/// planets have at least one mineral below 31 because of the impoverishment
/// pass. Both are reproduced to within the ~2% sampling error of the 600
/// unowned planets the fixtures describe.
#[test]
fn generated_planets_have_the_same_shape_as_real_ones() {
    let mut env = Vec::new();
    let mut conc = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for path in fixtures("hst") {
        let Some(bytes) = read(&path.to_string_lossy()) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let id = file.latest_segment().header.game_id;
        let (state, _) = GameState::from_file(&file);
        for planet in state.planets.iter().chain(state.known_planets.iter()) {
            if planet.owner.is_some() || !seen.insert((id, planet.id)) {
                continue;
            }
            env.push(planet.env);
            conc.push(planet.min_conc);
        }
    }
    if env.len() < 200 {
        eprintln!("only {} real planets; skipping", env.len());
        return;
    }

    let extreme = |env: &[[i8; 3]], axis: usize| -> f64 {
        env.iter().filter(|e| e[axis] <= 9).count() as f64 / env.len() as f64 * 100.0
    };
    let poor = |conc: &[[u8; 3]]| -> f64 {
        conc.iter().filter(|c| c.iter().any(|v| *v < 31)).count() as f64 / conc.len() as f64 * 100.0
    };

    let (real_grav, real_rad, real_poor) = (extreme(&env, 0), extreme(&env, 2), poor(&conc));

    let mut env = Vec::new();
    let mut conc = Vec::new();
    for seed in 0..40u32 {
        let config = NewGame {
            size: Size::Medium,
            id: seed.wrapping_mul(2_654_435_761),
            players: vec![NewPlayer::human(Race::humanoid())],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(config.id);
        let made = generate(&config, &mut rng).expect("generates");
        for planet in made.state.planets.iter().filter(|p| p.owner.is_none()) {
            env.push(planet.env);
            conc.push(planet.min_conc);
        }
    }
    let (made_grav, made_rad, made_poor) = (extreme(&env, 0), extreme(&env, 2), poor(&conc));

    assert!(
        (real_grav - made_grav).abs() < 3.0,
        "low gravity {real_grav:.1}% real vs {made_grav:.1}% generated"
    );
    assert!(
        (real_rad - made_rad).abs() < 3.0,
        "low radiation {real_rad:.1}% real vs {made_rad:.1}% generated"
    );
    assert!(
        made_rad > made_grav + 2.0,
        "radiation should reach its extremes about twice as often as gravity"
    );
    assert!(
        (real_poor - made_poor).abs() < 5.0,
        "impoverished planets {real_poor:.1}% real vs {made_poor:.1}% generated"
    );
}

/// A new game plays: it generates turns and its population grows.
#[test]
fn a_new_game_generates_turns() {
    let config = NewGame {
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("Turindrones").as_player(),
        ],
        ..NewGame::default()
    };
    let mut rng = Rng::randomize(config.id);
    let mut made = generate(&config, &mut rng).expect("generates");
    let before = made.state.planets.iter().map(|p| p.pop).sum::<i32>();
    for _ in 0..10 {
        stars_core::generate_turn(&mut made.state, &mut rng);
    }
    assert_eq!(made.state.year(), 2410);
    let after = made.state.planets.iter().map(|p| p.pop).sum::<i32>();
    assert!(after > before, "population {before} -> {after}");
}

/// Every primary racial trait produces a playable start.
#[test]
fn every_primary_trait_starts_a_game() {
    for prt in Prt::ALL {
        let race = stars_core::newgame::stock_race(prt);
        let config = NewGame {
            size: Size::Small,
            players: vec![NewPlayer::human(race)],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(0x2a03_1dd8);
        let made = generate(&config, &mut rng).unwrap_or_else(|e| panic!("{prt:?}: {e}"));
        let owned = made
            .state
            .planets
            .iter()
            .filter(|p| p.owner == Some(0))
            .count();
        let wanted = if matches!(prt, Prt::Pp | Prt::It) {
            2
        } else {
            1
        };
        assert_eq!(owned, wanted, "{prt:?} should hold {wanted} planets");
        assert!(
            !made.state.fleets.is_empty(),
            "{prt:?} starts with no ships"
        );
        for fleet in &made.state.fleets {
            assert!(fleet.cargo.fuel > 0, "{prt:?} starts with an empty tank");
        }
    }
}

/// The universe a new game produces is a real `.xy`: it re-reads as itself.
#[test]
fn the_generated_universe_round_trips() {
    let config = NewGame {
        name: "Round Trip".into(),
        size: Size::Medium,
        density: Density::Dense,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(3, 2).expect("Rototills").as_player(),
        ],
        ..NewGame::default()
    };
    let mut rng = Rng::randomize(config.id);
    let made = generate(&config, &mut rng).expect("generates");

    let bytes = made.universe.encode().expect("encodes");
    let back = Universe::decode(&bytes).expect("decodes");
    assert_eq!(back.encode().expect("re-encodes"), bytes);

    let game = back.game().expect("game info");
    assert_eq!(game.name, "Round Trip");
    assert_eq!(game.players, 3);
    assert_eq!(back.player_count(), 3);
    assert_eq!(
        game.planets as usize,
        planet_count(config.size, config.density) as usize
    );
    assert_eq!(back.planet_count(), made.state.planets.len());

    // Every planet keeps its position and its name through the packing.
    for (planet, resolved) in made.state.planets.iter().zip(back.planets_resolved()) {
        let point = planet.position.expect("placed");
        assert_eq!(u32::from(point.x as u16), resolved.x);
        assert_eq!(point.y as u16, resolved.y);
        assert_eq!(planet.name, resolved.name);
    }
}

/// A Jack of All Trades' six fleets start with the fuel the fixture records.
#[test]
fn starting_fuel_matches_the_vectors() {
    let section = vectors()["starting_fuel"].clone();
    let expect: Vec<i64> = numbers(&section, "expect_fuel_mg");

    let config = NewGame {
        size: Size::Small,
        players: vec![NewPlayer::human(Race::humanoid())],
        ..NewGame::default()
    };
    let mut rng = Rng::randomize(config.id);
    let made = generate(&config, &mut rng).expect("generates");
    let got: Vec<i64> = made
        .state
        .fleets
        .iter()
        .map(|f| i64::from(f.cargo.fuel))
        .collect();
    assert_eq!(got, expect, "a Jack of All Trades' starting fleets");
}

/// Who gets how many leftover advantage points, checked against every turn-0
/// homeworld in the fixtures.
///
/// A person spends `min(50, CAdvantagePoints)`; a computer player spends the
/// full fifty whatever its race costs. From Tough upward it also gets the
/// mineral-concentration bonus even when its race would have taken surface
/// minerals, and from Expert upward a tenth more colonists.
///
/// The two sixteen-player games make this checkable without knowing anything
/// the file does not say. Every homeworld in a game is stocked from planet 0,
/// so a race that spends its leftovers on **concentrations** leaves the surface
/// minerals untouched and shows the stock directly; a player below Tough leaves
/// the concentrations untouched and shows those. From those two the other
/// twenty-nine homeworlds are reconstructed exactly.
#[test]
fn turn0_leftover_points_follow_the_rule() {
    use stars_core::ai::Control;
    use stars_core::newgame::{spend_points, CONCENTRATION_BONUS_LEVEL, POPULATION_BONUS_LEVEL};
    use stars_core::race::{lrt, RaceStat};

    let mut checked = 0usize;
    for game in [
        "../../fixtures/incoming/turn0/Game.hst",
        "../../fixtures/games/no-random-events/2400/Game.hst",
        "../../fixtures/games/all-computer-players/2400/Game.hst",
    ] {
        let Some(bytes) = read(game) else {
            eprintln!("no fixture {game}; skipping");
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);

        let level = |control: Control| match control {
            Control::Human => 0,
            Control::Computer { skill_bits, .. } => skill_bits,
        };
        let points = |player: &stars_core::Player| -> i16 {
            match player.control {
                Control::Human => advantage_points(&player.race).min(50),
                Control::Computer { .. } => 50,
            }
        };

        // Every homeworld, with its owner.
        let homes: Vec<(&stars_core::Player, &Planet)> = state
            .planets
            .iter()
            .filter(|p| p.homeworld)
            .filter_map(|p| {
                let owner = usize::try_from(p.owner?).ok()?;
                Some((state.players.get(owner)?, p))
            })
            .collect();
        assert!(!homes.is_empty(), "{game} has no homeworlds");

        // Population: the starting figure, a tenth more from Expert upward,
        // then four fifths of it for the two traits that start with a second
        // planet.
        for (player, home) in &homes {
            let mut want = if player.race.has_lrt(lrt::LOW_STARTING_POP) {
                175
            } else {
                250
            };
            if level(player.control) >= POPULATION_BONUS_LEVEL {
                want += want / 10;
            }
            if matches!(player.race.prt(), Some(Prt::Pp) | Some(Prt::It)) {
                want = want * 4 / 5;
            }
            assert_eq!(home.pop, want, "{game}: player {:?} population", home.owner);
            checked += 1;
        }

        // The concentrations of a player below Tough whose race does not spend
        // on them are planet 0's, floored at 30.
        let base_conc = homes
            .iter()
            .find(|(player, _)| {
                player.race.stat(RaceStat::UseLeftover) != 1
                    && level(player.control) < CONCENTRATION_BONUS_LEVEL
            })
            .map(|(_, home)| home.min_conc);
        // The surface minerals of a race that spends on concentrations are
        // planet 0's, untouched.
        let base_surface = homes
            .iter()
            .find(|(player, _)| player.race.stat(RaceStat::UseLeftover) != 0)
            .map(|(_, home)| home.surface_min);

        for (player, home) in &homes {
            let mut rebuilt = Planet::unowned(home.id);
            let Some(conc) = base_conc else { continue };
            rebuilt.min_conc = conc;
            if let Some(surface) = base_surface {
                rebuilt.surface_min = surface;
            } else {
                rebuilt.surface_min = home.surface_min;
            }
            spend_points(
                &mut rebuilt,
                &player.race,
                points(player),
                level(player.control) >= CONCENTRATION_BONUS_LEVEL,
            );
            assert_eq!(
                rebuilt.min_conc, home.min_conc,
                "{game}: player {:?} concentrations",
                home.owner
            );
            if base_surface.is_some() {
                assert_eq!(
                    rebuilt.surface_min, home.surface_min,
                    "{game}: player {:?} surface minerals",
                    home.owner
                );
                checked += 1;
            }
        }
    }
    if checked == 0 {
        eprintln!("no turn-0 fixtures; skipping");
    } else {
        eprintln!("{checked} homeworld figures reproduced");
    }
}

/// A generated game hands its computer players the same advantages.
#[test]
fn generated_computer_players_get_their_bonuses() {
    let human = NewPlayer::human(Race::humanoid());
    let easy = opponents::opponent(0, 0)
        .expect("Robotoids, Easy")
        .as_player();
    let tough = opponents::opponent(0, 2)
        .expect("Robotoids, Tough")
        .as_player();
    let expert = opponents::opponent(0, 3)
        .expect("Robotoids, Expert")
        .as_player();

    let config = NewGame {
        size: Size::Small,
        players: vec![human, easy, tough, expert],
        ..NewGame::default()
    };
    let mut rng = Rng::randomize(config.id);
    let made = generate(&config, &mut rng).expect("generates");
    let home = |player: i16| {
        made.state
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(player))
            .unwrap_or_else(|| panic!("player {player} has no homeworld"))
    };

    // All four races put their leftovers into surface minerals, and every
    // computer player gets the full fifty however much its race costs — the
    // Robotoids at Tough and Expert price well below zero.
    assert!(advantage_points(&made.state.players[2].race) < 0);
    assert_eq!(home(1).surface_min, home(2).surface_min);
    assert_eq!(home(1).surface_min, home(3).surface_min);
    assert_ne!(
        home(0).surface_min,
        home(1).surface_min,
        "the person spent 25, not 50"
    );

    // From Tough upward the concentrations are raised too, and from Expert
    // upward the homeworld starts with a tenth more colonists.
    assert_eq!(home(0).min_conc, home(1).min_conc, "below Tough: no bonus");
    assert_ne!(home(1).min_conc, home(2).min_conc, "Tough: concentrations");
    assert_eq!(home(2).min_conc, home(3).min_conc);
    assert_eq!(home(0).pop, 250);
    assert_eq!(home(1).pop, 250);
    assert_eq!(home(2).pop, 250);
    assert_eq!(home(3).pop, 275, "Expert: a tenth more colonists");
}

/// A new universe starts with wormholes, in pairs that name each other,
/// as many as the size's table allows, each inside the galaxy and on top
/// of nothing — and none at all without random events.
#[test]
fn a_new_universe_has_its_wormholes() {
    use stars_core::newgame::{WORMHOLES_MIN, WORMHOLES_VAR};
    for size in Size::ALL {
        for seed in [1u32, 77, 4000] {
            let config = NewGame {
                size,
                players: vec![NewPlayer::human(Race::humanoid())],
                ..NewGame::default()
            };
            let mut rng = Rng::randomize(seed);
            let made = generate(&config, &mut rng).expect("generates");
            let holes = &made.state.wormholes;
            let pairs = i16::try_from(holes.len() / 2).expect("fits");
            assert_eq!(holes.len() % 2, 0, "{size:?}: {} ends", holes.len());
            let least = WORMHOLES_MIN[size as usize];
            let most = least + WORMHOLES_VAR[size as usize] - 1;
            assert!(
                (least..=most).contains(&pairs),
                "{size:?} seed {seed}: {pairs} pairs, expected {least}..={most}"
            );
            let planets: Vec<_> = made
                .state
                .planets
                .iter()
                .filter_map(|p| p.position)
                .collect();
            for (n, hole) in holes.iter().enumerate() {
                let other = &holes[n ^ 1];
                assert_eq!(
                    hole.partner,
                    (2 << 13) | other.id,
                    "{size:?}: partner of {n}"
                );
                assert!(hole.stability <= 2);
                let p = hole.position;
                assert!(
                    p.x >= 1000
                        && p.y >= 1000
                        && p.x <= 1000 + size.span()
                        && p.y <= 1000 + size.span(),
                    "{size:?}: {p:?} outside the galaxy"
                );
                assert!(!planets.contains(&p), "{size:?}: a wormhole on a planet");
            }
        }
    }
    let config = NewGame {
        random_events: false,
        players: vec![NewPlayer::human(Race::humanoid())],
        ..NewGame::default()
    };
    let made = generate(&config, &mut Rng::randomize(1)).expect("generates");
    assert!(made.state.wormholes.is_empty());
}

/// A player whose race is the wizard's "Random" is rolled a real one at
/// generation (`CreateRandomRace`): its advantage points end inside
/// `0..=50`, its habitability is well formed, it is named from the list
/// given, and it hardly ever has to fall back on the stock Humanoid.
#[test]
fn a_random_race_is_rolled_into_a_balanced_one() {
    use stars_core::race::lrt;
    let template = stars_core::presets::ALL
        .iter()
        .find(|p| p.name == "Random")
        .expect("the Random preset");
    let names: Vec<String> = (0..24).map(|n| format!("Name{n}")).collect();
    let mut seen_names = std::collections::BTreeSet::new();
    let mut stock = 0;
    for seed in 0..60u32 {
        let config = NewGame {
            players: vec![
                NewPlayer::human(Race::humanoid()),
                NewPlayer {
                    race: template.race.clone(),
                    control: stars_core::ai::Control::Human,
                    name: "Random".to_string(),
                    plural_name: "Randoms".to_string(),
                },
            ],
            random_names: names.clone(),
            ..NewGame::default()
        };
        let made = generate(&config, &mut Rng::randomize(seed * 977 + 5)).expect("generates");
        let player = &made.state.players[1];
        let race = &player.race;
        assert!(
            race != &template.race,
            "seed {seed}: the template was handed out unchanged"
        );
        let points = advantage_points(race);
        assert!((0..=50).contains(&points), "seed {seed}: {points} points");
        for axis in 0..3 {
            let (low, high, mid) = (
                race.env_min[axis],
                race.env_max[axis],
                race.env_center[axis],
            );
            if high < 0 {
                assert_eq!((low, mid), (-1, -1), "seed {seed}: immune axis {axis}");
            } else {
                assert!((0..=100).contains(&low) && (0..=100).contains(&high) && low < high);
                assert!(
                    mid >= low && mid <= high,
                    "seed {seed}: {low}..{high} mid {mid}"
                );
            }
        }
        assert!((1..=20).contains(&race.pct_ideal_growth), "seed {seed}");
        assert!(
            names.contains(&player.name),
            "seed {seed}: named {:?}",
            player.name
        );
        seen_names.insert(player.name.clone());
        // The template's mark stays on the race; a race the balancing gave
        // up on is the predefined Humanoid, mark and all gone.
        if race.has_lrt(lrt::AI_PLAYER) {
            assert_ne!(race, &stars_core::presets::ALL[0].race);
        } else {
            assert_eq!(race, &stars_core::presets::ALL[0].race, "seed {seed}");
            stock += 1;
        }
        // The human keeps the race they chose.
        assert_eq!(made.state.players[0].race, Race::humanoid());
    }
    assert!(seen_names.len() > 5, "only {seen_names:?}");
    assert!(stock < 10, "{stock} of 60 fell back to the stock race");
}
