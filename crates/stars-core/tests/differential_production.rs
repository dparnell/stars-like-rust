//! Production costs checked against what the original engine actually built.
//!
//! A planet's mines and factories are recorded every year, and so is its
//! production queue. That gives a real bound: what a planet built between two
//! years cannot have cost more than the resources it had.
//!
//! Fixtures are optional; the test skips when the sample game is absent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stars_core::planet::Planet;
use stars_core::production::{item, planetary_item_cost};
use stars_core::race::{Prt as CorePrt, Race, RaceStat};
use stars_core::resources::resources_at_planet;
use stars_formats::{planet_records_in, player_records_in, PlanetRecord, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

fn to_core_race(r: &stars_formats::RaceRecord) -> Race {
    let mut attrs = [0i16; 16];
    attrs[RaceStat::ResGen as usize] = i16::from(r.economy.resource_per_colonist);
    attrs[RaceStat::FactProd as usize] = i16::from(r.economy.produce_per_factory);
    attrs[RaceStat::FactBuild as usize] = i16::from(r.economy.factory_build_cost);
    attrs[RaceStat::FactOperate as usize] = i16::from(r.economy.factories_operated);
    attrs[RaceStat::MineProd as usize] = i16::from(r.economy.produce_per_mine);
    attrs[RaceStat::MineBuild as usize] = i16::from(r.economy.mine_build_cost);
    attrs[RaceStat::MineOperate as usize] = i16::from(r.economy.mines_operated);
    attrs[RaceStat::MajorAdv as usize] = match r.prt.abbrev() {
        "HE" => CorePrt::He,
        "SS" => CorePrt::Ss,
        "WM" => CorePrt::Wm,
        "CA" => CorePrt::Ca,
        "IS" => CorePrt::Is,
        "SD" => CorePrt::Sd,
        "PP" => CorePrt::Pp,
        "IT" => CorePrt::It,
        "AR" => CorePrt::Ar,
        _ => CorePrt::Joat,
    } as i16;
    let axis = |h: stars_formats::HabRange| match (h.center, h.low, h.high) {
        (Some(c), Some(l), Some(x)) => (c as i8, l as i8, x as i8),
        _ => (0, 0, -1),
    };
    let (gc, gl, gh) = axis(r.gravity);
    let (tc, tl, th) = axis(r.temperature);
    let (rc, rl, rh) = axis(r.radiation);
    Race {
        attrs,
        lrt_bits: u32::from(r.lrt_bits),
        env_center: [gc, tc, rc],
        env_min: [gl, tl, rl],
        env_max: [gh, th, rh],
        pct_ideal_growth: r.growth_rate as i8,
    }
}

fn to_core_planet(r: &PlanetRecord) -> Option<Planet> {
    let owner = r.owner?;
    let env = r.environment?;
    let conc = r.concentration?;
    let pop = r.population?;
    let imp = r.installations?;
    Some(Planet {
        id: i16::try_from(r.id).ok()?,
        owner: Some(i16::from(owner)),
        env: [
            env.gravity as i8,
            env.temperature as i8,
            env.radiation as i8,
        ],
        min_conc: [conc.ironium, conc.boranium, conc.germanium],
        min_level: [0, 0, 0],
        surface_min: [0, 0, 0],
        pop: i32::try_from(pop / 100).ok()?,
        delta_pop: imp.delta_pop,
        mines: i16::try_from(imp.mines).ok()?,
        factories: i16::try_from(imp.factories).ok()?,
        homeworld: r.homeworld,
        starbase: r.has_starbase,
    })
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
            .map(to_core_race)?;
        let planets = planet_records_in(blocks)
            .into_iter()
            .filter(|r| r.owner == Some(owner))
            .filter_map(|r| to_core_planet(&r).map(|c| (r.id, c)))
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
