//! Production costs checked against what the original engine actually built.
//!
//! A planet's mines and factories are recorded every year, and so is its
//! production queue. That gives a real bound: what a planet built between two
//! years cannot have cost more than the resources it had.
//!
//! Fixtures are optional; the test skips when the sample game is absent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stars_core::load::{planet_from_record, race_from_record};
use stars_core::planet::Planet;
use stars_core::production::{item, planetary_item_cost};
use stars_core::race::Race;
use stars_core::resources::resources_at_planet;
use stars_formats::{planet_records_in, player_records_in, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// What a planet built between two years cannot have cost more than it earned.
///
/// This is one-sided, like the resource bound, but it is the sharp direction:
/// if mine or factory costs were too *low* the engine would appear to build
/// more than it could afford, which is exactly what a wrong race attribute
/// would look like.
#[test]
fn planets_never_build_more_than_they_could_afford() {
    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let load = |path: &Path| -> Option<(Race, BTreeMap<u16, Planet>)> {
        let bytes = std::fs::read(path).ok()?;
        let file = StarsFile::decode(&bytes).ok()?;
        let blocks = file.segment_blocks(file.latest_segment());
        let owner = file.latest_segment().header.player;
        let players = player_records_in(blocks).ok()?;
        let race = players
            .iter()
            .find(|p| p.player_number == owner)
            .and_then(|p| p.race.as_ref())
            .map(race_from_record)?;
        let planets = planet_records_in(blocks)
            .into_iter()
            .filter(|r| r.owner == Some(owner))
            .filter_map(|r| planet_from_record(&r).map(|c| (r.id, c)))
            .collect();
        Some((race, planets))
    };

    let mut checked = 0;
    let mut built_any = 0;
    let mut violations = Vec::new();

    for w in years.windows(2) {
        if w[1] != w[0] + 1 {
            continue;
        }
        let (Some((race, before)), Some((_, after))) = (
            load(&games.join(w[0].to_string()).join("exodus.m6")),
            load(&games.join(w[1].to_string()).join("exodus.m6")),
        ) else {
            continue;
        };

        let mine = planetary_item_cost(item::MINE, &race, false).expect("mine has a cost");
        let factory = planetary_item_cost(item::FACTORY, &race, false).expect("factory has a cost");

        for (id, planet) in &before {
            let Some(next) = after.get(id) else { continue };
            let new_mines = i32::from(next.mines - planet.mines).max(0);
            let new_factories = i32::from(next.factories - planet.factories).max(0);
            if new_mines == 0 && new_factories == 0 {
                continue;
            }
            built_any += 1;

            let spent = new_mines * mine.resources + new_factories * factory.resources;
            let Some(earned) = resources_at_planet(planet, &race) else {
                continue;
            };
            checked += 1;

            // A queue item carries partial progress between years in its
            // `completion` field, so one item can have been paid for over
            // several years — a colony making 2 resources a year does finish a
            // 9-resource factory eventually. Only what was built *beyond* a
            // single carried item has to come out of this year's income.
            let carried = mine.resources.max(factory.resources);
            let must_afford = (spent - carried).max(0);

            // Resources can also arrive from elsewhere, so allow headroom; the
            // point is to catch costs that are wildly wrong, not to model the
            // whole economy.
            if must_afford > i32::from(earned) * 4 {
                violations.push(format!(
                    "{}->{}: planet {id} built {new_mines} mines and {new_factories} \
                     factories for {spent} resources ({must_afford} beyond a carried \
                     item), but makes only {earned}",
                    w[0], w[1]
                ));
            }
        }
    }

    eprintln!(
        "production: {built_any} planet-years built something; {checked} checked against \
         what the planet earns, {} implausible",
        violations.len()
    );
    for v in violations.iter().take(6) {
        eprintln!("  {v}");
    }
    assert!(checked >= 10, "expected a meaningful sample, got {checked}");
    assert!(
        violations.is_empty(),
        "{} planets built more than they could plausibly afford: {violations:?}",
        violations.len()
    );
}

/// The documented costs of the things a planet builds directly.
#[test]
fn planetary_item_costs_match_the_manual() {
    use stars_core::race::{lrt, Prt, RaceStat};

    let mut humanoid = Race::humanoid();
    humanoid.attrs[RaceStat::MineBuild as usize] = 5;
    humanoid.attrs[RaceStat::FactBuild as usize] = 10;

    // A mine costs resources only.
    let mine = planetary_item_cost(item::MINE, &humanoid, false).unwrap();
    assert_eq!(mine.resources, 5);
    assert_eq!(mine.minerals, [0, 0, 0]);

    // A factory costs resources and four germanium.
    let factory = planetary_item_cost(item::FACTORY, &humanoid, false).unwrap();
    assert_eq!(factory.resources, 10);
    assert_eq!(factory.minerals, [0, 0, 4]);

    // "Factories cost one less germanium to build" does exactly that.
    let mut cheap = humanoid.clone();
    cheap.lrt_bits |= 1 << lrt::CHEAP_FACT;
    assert_eq!(
        planetary_item_cost(item::FACTORY, &cheap, false)
            .unwrap()
            .minerals,
        [0, 0, 3]
    );

    // Mineral alchemy: 100 resources, or 25 with the trait
    // (MANUAL.PDF p. 20-13).
    assert_eq!(
        planetary_item_cost(item::ALCHEMY, &humanoid, false)
            .unwrap()
            .resources,
        100
    );
    let mut alchemist = humanoid.clone();
    alchemist.lrt_bits |= 1 << lrt::MINERAL_ALCHEMY;
    assert_eq!(
        planetary_item_cost(item::ALCHEMY, &alchemist, false)
            .unwrap()
            .resources,
        25
    );

    // Terraforming: 100, 70 with Total Terraforming, halved for Claim Adjuster.
    assert_eq!(
        planetary_item_cost(item::MIN_TERRAFORM, &humanoid, false)
            .unwrap()
            .resources,
        100
    );
    let mut tt = humanoid.clone();
    tt.lrt_bits |= 1 << lrt::TT;
    assert_eq!(
        planetary_item_cost(item::MIN_TERRAFORM, &tt, false)
            .unwrap()
            .resources,
        70
    );
    let mut ca = humanoid.clone();
    ca.attrs[RaceStat::MajorAdv as usize] = Prt::Ca as i16;
    assert_eq!(
        planetary_item_cost(item::MIN_TERRAFORM, &ca, false)
            .unwrap()
            .resources,
        50
    );

    // Inner Strength pays three fifths for defences.
    let normal = planetary_item_cost(item::DEFENSE, &humanoid, false).unwrap();
    let mut inner = humanoid.clone();
    inner.attrs[RaceStat::MajorAdv as usize] = Prt::Is as i16;
    let cheaper = planetary_item_cost(item::DEFENSE, &inner, false).unwrap();
    assert_eq!(cheaper.resources, normal.resources * 3 / 5);

    // Ship designs are costed elsewhere.
    assert!(planetary_item_cost(999, &humanoid, false).is_none());
}

/// The build loop's behaviour, from `CBuildProdItem`.
#[test]
fn building_completes_whole_units_then_banks_the_rest() {
    use stars_core::production::{build_item, ItemCost};

    // A factory: 10 resources and 4 germanium.
    let factory = ItemCost {
        minerals: [0, 0, 4],
        resources: 10,
    };

    // Plenty of everything: build all three outright.
    let mut have = [0, 0, 100, 100];
    let out = build_item(factory, 3, 0, &mut have, false);
    assert_eq!(out.built, 3);
    assert_eq!(out.remaining, 0);
    assert_eq!(
        have,
        [0, 0, 88, 70],
        "three factories cost 12 germanium, 30 resources"
    );

    // Enough for one and part of another: the remainder is banked.
    let mut have = [0, 0, 100, 15];
    let out = build_item(factory, 3, 0, &mut have, false);
    assert_eq!(out.built, 1);
    assert_eq!(out.remaining, 2);
    assert!(
        out.completion_pct > 0 && out.completion_pct < 100,
        "part of the next factory should be paid for, got {}%",
        out.completion_pct
    );

    // Carrying that progress forward finishes it more cheaply.
    let mut have = [0, 0, 100, 6];
    let out = build_item(factory, 1, 50, &mut have, false);
    assert_eq!(out.built, 1, "half-paid, so 5 more resources finishes it");

    // An auto-build item blocked on minerals banks nothing.
    let mut have = [0, 0, 1, 100];
    let out = build_item(factory, 2, 0, &mut have, true);
    assert_eq!(out.built, 0);
    assert!(out.mineral_blocked);
    assert_eq!(out.completion_pct, 0, "auto-build does not part-pay");
    assert_eq!(have, [0, 0, 1, 100], "and spends nothing");

    // The same shortage on a manual item does part-pay.
    let mut have = [0, 0, 1, 100];
    let out = build_item(factory, 2, 0, &mut have, false);
    assert!(out.completion_pct > 0, "a manual item banks what it can");
    assert!(have[2] < 1 || have[3] < 100, "and spends something");
}

/// Auto-build keeps pace with what the population can operate.
#[test]
fn auto_build_is_capped_by_what_can_be_operated() {
    use stars_core::production::{auto_build_cap, item};

    let race = Race::humanoid();
    let mut planet = Planet::unowned(0);
    planet.owner = Some(0);
    planet.env = race.env_center;
    planet.pop = 1000; // 100,000 colonists
    planet.factories = 0;

    // Ten factories per 10,000 colonists, so 100 for this population.
    let cap = auto_build_cap(&planet, &race, item::FACTORY);
    assert!(cap > 0, "a populated planet can add factories");

    // Already at the cap: nothing more to add.
    planet.factories = i16::try_from(cap).unwrap();
    assert_eq!(auto_build_cap(&planet, &race, item::FACTORY), 0);

    // The auto-build id and the plain id cost and cap the same.
    planet.factories = 0;
    assert_eq!(
        auto_build_cap(&planet, &race, item::FACTORY),
        auto_build_cap(&planet, &race, item::AUTO_FACTORY)
    );
}

/// A whole turn's production, end to end.
#[test]
fn a_generated_turn_builds_from_the_queue() {
    use stars_core::production::{item, QueueItem};
    use stars_core::rng::Rng;
    use stars_core::{generate_turn, GameState, Player, SkippedStep};

    let race = Race::humanoid();
    let mut planet = Planet::unowned(0);
    planet.owner = Some(0);
    planet.env = race.env_center;
    planet.pop = 2500; // 250,000 colonists: plenty of resources
    planet.factories = 10;
    planet.mines = 10;
    planet.surface_min = [500, 500, 500];
    planet.queue = vec![QueueItem {
        count: 5,
        item: item::FACTORY,
        ship: false,
        completion: 0,
    }];

    let mut state = GameState::new(1);
    state.planets = vec![planet];
    state.players = vec![Player::new(race)];

    let mut rng = Rng::randomize(7);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(report.year, 2401);
    let built: i32 = report
        .built
        .iter()
        .flat_map(|(_, items)| items.iter())
        .filter(|(id, _)| *id == item::FACTORY)
        .map(|(_, n)| *n)
        .sum();
    assert!(built > 0, "the planet should have built some factories");
    assert_eq!(
        state.planets[0].factories,
        10 + i16::try_from(built).unwrap(),
        "and they should be on the planet"
    );
    assert!(
        state.planets[0].surface_min[2] < 500,
        "germanium should have been spent"
    );

    // The build queue is no longer listed as skipped.
    assert!(
        !report.skipped.contains(&SkippedStep::BuildQueue),
        "the queue now runs"
    );
    // Research still receives whatever production did not spend.
    assert!(report.research_spending[0] > 0);
}
