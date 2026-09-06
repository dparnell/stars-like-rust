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
            let Some(earned) = resources_at_planet(planet, &race, 0) else {
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
        planetary_item_cost(item::TERRAFORM, &humanoid, false)
            .unwrap()
            .resources,
        100
    );
    let mut tt = humanoid.clone();
    tt.lrt_bits |= 1 << lrt::TT;
    assert_eq!(
        planetary_item_cost(item::TERRAFORM, &tt, false)
            .unwrap()
            .resources,
        70
    );
    let mut ca = humanoid.clone();
    ca.attrs[RaceStat::MajorAdv as usize] = Prt::Ca as i16;
    assert_eq!(
        planetary_item_cost(item::TERRAFORM, &ca, false)
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
    use stars_core::production::item;
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
    planet.queue = vec![stars_core::production::QueueItem {
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

/// A ship in the queue costs its design and joins a fleet in orbit.
///
/// Before this, a ship entry was skipped entirely: its minerals were never
/// spent, so a planet building warships looked as rich as one building
/// nothing.
#[test]
fn a_queued_ship_is_paid_for_and_joins_the_fleet() {
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_core::fleet::Fleet;
    use stars_core::rng::Rng;
    use stars_core::Point;
    use stars_core::{generate_turn, GameState};

    let race = Race::humanoid();
    let mut planet = Planet::unowned(7);
    planet.owner = Some(0);
    planet.pop = 30_000;
    planet.factories = 50;
    planet.surface_min = [5_000, 5_000, 5_000];

    // A scout: the smallest hull with an engine in it.
    let design = ShipDesign {
        name: String::new(),
        picture: 0,
        stored_armor: 0,
        hull_id: 0,
        slots: vec![DesignSlot {
            category: stars_core::components::slot::ENGINE,
            item: 1,
            count: 1,
        }],
    };
    let unit = design.cost().expect("the design costs something");
    assert!(unit.resources > 0, "a ship should cost resources");

    planet.queue = vec![stars_core::production::QueueItem {
        count: 3,
        item: 0,
        ship: true,
        completion: 0,
    }];

    let mut state = GameState::new(1);
    state.players = vec![stars_core::Player::new(race)];
    state.designs = vec![vec![design]];
    state.planets = vec![planet];
    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        id: 1,
        owner: 0,
        position: Point { x: 0, y: 0 },
        orbiting: Some(7),
        stacks: Vec::new(),
        cargo: stars_core::fleet::Cargo::default(),
        battle_plan: 0,
        warp: None,
        waypoints: Vec::new(),
    }];

    let before = state.planets[0].surface_min;
    let mut rng = Rng::randomize(1);
    let report = generate_turn(&mut state, &mut rng);

    let built: i32 = report.ships_built.iter().map(|(_, _, n)| *n).sum();
    assert!(
        built > 0,
        "expected some ships, got {:?}",
        report.ships_built
    );

    // The minerals actually left the planet.
    let after = state.planets[0].surface_min;
    assert!(
        (0..3).any(|i| after[i] < before[i] + 10_000),
        "ship minerals should have been spent: {before:?} -> {after:?}"
    );

    // And the ships are in the fleet that was in orbit.
    let ships: i32 = state.fleets[0].stacks.iter().map(|s| s.count).sum();
    assert_eq!(ships, built, "every ship built should join the fleet");
}

/// The auto-build items are ids 0..=6, not 7 and up.
///
/// The disassembly settles it — `FillProdSrcLB` (`10d0:3b00`) appends
/// " (Auto Build)" exactly when the id is below 7 — and every queue in the
/// fixtures corroborates it: ids 0, 1 and 2 carry a count of **100** and
/// nothing else, which is an "up to 100" order, while the plain ids carry
/// ordinary varying counts. See `docs/formats/production.md`.
#[test]
fn the_auto_build_items_are_the_low_ids() {
    for id in 0..=item::AUTO_PACKET {
        assert!(item::is_auto(id), "{id} should be auto-build");
        assert!(item::auto_builds(id).is_some());
    }
    for id in [
        item::FACTORY,
        item::MINE,
        item::DEFENSE,
        item::ALCHEMY,
        item::TERRAFORM,
        item::GENESIS,
        item::PACKET_MIXED,
        item::PLANETARY_SCANNER,
    ] {
        assert!(!item::is_auto(id), "{id} should not be auto-build");
        assert_eq!(item::auto_builds(id), None);
    }

    assert_eq!(item::auto_builds(item::AUTO_MINE), Some(item::MINE));
    assert_eq!(item::auto_builds(item::AUTO_FACTORY), Some(item::FACTORY));
    assert_eq!(item::auto_builds(item::AUTO_DEFENSE), Some(item::DEFENSE));
    assert_eq!(item::auto_builds(item::AUTO_ALCHEMY), Some(item::ALCHEMY));
    // Both terraform variants build the same thing; they differ in how far
    // they go, not in what they make.
    assert_eq!(
        item::auto_builds(item::AUTO_MIN_TERRAFORM),
        Some(item::TERRAFORM)
    );
    assert_eq!(
        item::auto_builds(item::AUTO_MAX_TERRAFORM),
        Some(item::TERRAFORM)
    );

    // The names are the giveaway: plural for the auto items, singular for the
    // one-offs.
    use stars_core::production::item_name;
    assert_eq!(item_name(item::AUTO_MINE), "Mines");
    assert_eq!(item_name(item::MINE), "Mine");
    assert_eq!(item_name(item::AUTO_FACTORY), "Factories");
    assert_eq!(item_name(item::FACTORY), "Factory");
    assert_eq!(item_name(item::AUTO_MAX_TERRAFORM), "Max Terraform");
    assert_eq!(item_name(item::TERRAFORM), "Terraform Environment");
    assert_eq!(item_name(item::GENESIS), "Genesis Device");
    assert_eq!(item_name(item::PACKET_MIXED), "Mixed Mineral Packet");
    assert_eq!(item_name(item::PLANETARY_SCANNER_FIRST), "Viewer 50");
    assert_eq!(item_name(item::PLANETARY_SCANNER), "Planetary Scanner");
}

/// The fixture half of the same claim, run over every save this checkout has.
#[test]
fn every_auto_build_entry_in_the_fixtures_is_an_up_to_order() {
    use stars_formats::production::{ProductionQueueRecord, QueueClass};

    let root = workspace_root().join("fixtures");
    if !root.is_dir() {
        eprintln!("skipping: no fixtures");
        return;
    }

    let mut seen = 0usize;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let ext = ext.to_ascii_lowercase();
            if !(ext.starts_with('m') || ext == "hst") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(file) = StarsFile::decode(&bytes) else {
                continue;
            };
            for block in &file.blocks {
                if block.type_id != 28 {
                    continue;
                }
                for queued in ProductionQueueRecord::decode(&block.data).items {
                    if matches!(queued.class, QueueClass::Fleet) {
                        continue;
                    }
                    if queued.item <= item::AUTO_DEFENSE {
                        assert_eq!(
                            queued.count,
                            100,
                            "auto-build {} in {} should be an up-to order",
                            queued.item,
                            path.display()
                        );
                        seen += 1;
                    }
                }
            }
        }
    }
    assert!(
        seen > 1000,
        "expected the fixtures to hold many, saw {seen}"
    );
}

/// The production inventory: what a planet may add to its queue.
#[test]
fn the_inventory_offers_what_the_planet_can_build() {
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_core::parts::Builder;
    use stars_core::production::{inventory, item_cost, mass_driver_warp, QueueItem, UNLIMITED};
    use stars_core::startup::FIRST_STARBASE_SLOT;

    let race = Race::humanoid();
    let player = stars_core::Player::new(race.clone());
    let who = Builder::player(&player);

    let mut planet = Planet::unowned(1);
    planet.owner = Some(0);
    planet.pop = 5_000;
    planet.env = [50, 50, 50];
    planet.min_conc = [50, 50, 50];

    // A bare colony with no starbase: no ships, no starbases in orbit to
    // exclude, no packets.
    let rows = inventory(&planet, &who, &[], &[]);
    let ids: Vec<u16> = rows.iter().filter(|r| !r.ship).map(|r| r.item).collect();
    assert!(!rows.iter().any(|r| r.ship), "nothing to build ships with");
    assert!(ids.contains(&item::FACTORY));
    assert!(ids.contains(&item::MINE));
    assert!(ids.contains(&item::ALCHEMY));
    assert!(ids.contains(&item::PLANETARY_SCANNER));
    assert!(!ids.contains(&item::PACKET_MIXED), "no mass driver");
    // The auto-build items are all there, and marked as such.
    for id in item::AUTO_MINE..=item::AUTO_PACKET {
        let row = rows
            .iter()
            .find(|r| !r.ship && r.item == id)
            .unwrap_or_else(|| panic!("auto item {id} should be offered"));
        assert!(row.auto);
        assert!(row.unlimited());
    }
    // Alchemy is unlimited; the installations are capped by what the planet
    // can run.
    let alchemy = rows.iter().find(|r| r.item == item::ALCHEMY).unwrap();
    assert_eq!(alchemy.count, UNLIMITED);
    // A grown planet can hold more factories than the ten-bit count field
    // will express, so this one reads as unlimited; a small colony does not.
    let factories = rows.iter().find(|r| r.item == item::FACTORY).unwrap();
    assert_eq!(factories.count, UNLIMITED);
    // The cap is the planet's own maximum, so it only bites once most of the
    // room is used up.
    let cap = i32::from(stars_core::resources::max_factories(&planet, &race));
    let mut nearly_full = planet.clone();
    nearly_full.factories = i16::try_from(cap - 5).expect("a sane cap");
    let rows_full = inventory(&nearly_full, &who, &[], &[]);
    let capped = rows_full
        .iter()
        .find(|r| r.item == item::FACTORY)
        .expect("five more will fit");
    assert_eq!(capped.count, 5);

    // A scanner is offered exactly once: once the planet has one it is gone.
    planet.scanner = Some(0);
    let rows = inventory(&planet, &who, &[], &[]);
    assert!(!rows.iter().any(|r| r.item == item::PLANETARY_SCANNER));
    planet.scanner = None;

    // Queueing a unique item takes it out of the list, and queueing some of a
    // limited one takes them off its count.
    let queued = vec![
        QueueItem {
            count: 1,
            item: item::PLANETARY_SCANNER,
            ship: false,
            completion: 0,
        },
        QueueItem {
            count: 3,
            item: item::FACTORY,
            ship: false,
            completion: 0,
        },
    ];
    let rows = inventory(&planet, &who, &[], &queued);
    assert!(!rows.iter().any(|r| r.item == item::PLANETARY_SCANNER));
    // Three factories queued come off a *limited* count; an unlimited one
    // stays unlimited.
    let rows_full = inventory(&nearly_full, &who, &[], &queued);
    assert_eq!(
        rows_full
            .iter()
            .find(|r| r.item == item::FACTORY)
            .unwrap()
            .count,
        capped.count - 3
    );

    // A starbase with a dock offers ships — but only ones it is big enough
    // for. The Space Dock's is 200kT.
    let scout = ShipDesign {
        hull_id: 4,
        slots: Vec::new(),
        name: "Scout".into(),
        picture: 0,
        stored_armor: 0,
    };
    // A Dreadnought's bare hull is 250kT, more than a Space Dock's 200.
    let dreadnought = ShipDesign {
        hull_id: 10,
        slots: Vec::new(),
        name: "Dreadnought".into(),
        picture: 0,
        stored_armor: 0,
    };
    let mut designs = vec![scout.clone(), dreadnought.clone()];
    designs.resize(
        usize::from(FIRST_STARBASE_SLOT) + 2,
        ShipDesign {
            hull_id: -1,
            slots: Vec::new(),
            name: String::new(),
            picture: 0,
            stored_armor: 0,
        },
    );
    // Slot 16: a Space Dock with a mass driver in its orbital slot.
    designs[usize::from(FIRST_STARBASE_SLOT)] = ShipDesign {
        hull_id: 33,
        slots: vec![DesignSlot {
            category: stars_core::components::slot::SPECIAL_SB,
            item: 7, // Mass Driver 5
            count: 1,
        }],
        name: "Dock".into(),
        picture: 0,
        stored_armor: 1000,
    };
    // Slot 17: an Orbital Fort, so there is a second starbase to offer.
    designs[usize::from(FIRST_STARBASE_SLOT) + 1] = ShipDesign {
        hull_id: 32,
        slots: Vec::new(),
        name: "Fort".into(),
        picture: 0,
        stored_armor: 1000,
    };
    planet.starbase = true;
    planet.starbase_design = Some(0);

    assert_eq!(mass_driver_warp(&planet, &designs), 5);
    let rows = inventory(&planet, &who, &designs, &[]);
    let ships: Vec<u16> = rows.iter().filter(|r| r.ship).map(|r| r.item).collect();
    assert!(ships.contains(&0), "the Scout fits the dock: {ships:?}");
    assert!(
        !ships.contains(&1),
        "a Dreadnought is heavier than a Space Dock will build: {ships:?}"
    );
    // The starbase already in orbit is not on offer; the other one is.
    assert!(!ships.contains(&u16::from(FIRST_STARBASE_SLOT)));
    assert!(ships.contains(&(u16::from(FIRST_STARBASE_SLOT) + 1)));
    // And with a driver, packets appear.
    for id in item::PACKET_IRONIUM..=item::PACKET_MIXED {
        assert!(rows.iter().any(|r| !r.ship && r.item == id), "packet {id}");
    }

    // Packet costs come from the primary trait.
    let mixed = item_cost(item::PACKET_MIXED, &who, false).expect("a mixed packet costs something");
    assert_eq!(mixed.minerals, [44, 44, 44]);
    assert_eq!(mixed.resources, 10);
    let iron = item_cost(item::PACKET_IRONIUM, &who, false).expect("an ironium packet");
    assert_eq!(iron.minerals, [110, 0, 0]);
}
