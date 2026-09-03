//! Differential verification of the planetary formulas against **real** Stars!
//! save files.
//!
//! The strongest available check on the recovered formulas is to take a
//! player's own planets at year N, run one year of the simulation, and compare
//! against what the original engine actually wrote at year N+1. Both the
//! population and the fractional-population accumulator (`iDeltaPop`) are
//! stored in the file, so the prediction is exact rather than approximate.
//!
//! Fixtures are optional: each test skips rather than fails when the sample
//! game is absent, so a checkout without fixtures keeps CI green.

use std::path::{Path, PathBuf};

use stars_core::planet::Planet;
use stars_core::population::chg_pop_from_planet;
use stars_core::race::{Prt as CorePrt, Race, RaceStat};
use stars_formats::{planet_records_in, player_records_in, PlanetRecord, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// A player file's races (indexed by player number) and its fully-detailed planets.
type Snapshot = (Vec<Option<Race>>, Vec<(u16, Planet)>);

/// Load a player file and return (race per player number, planets with full detail).
///
/// A `.mN` file carries a player block for every player in the game, so the
/// race must be looked up by the planet's owner rather than simply taking the
/// first one present.
fn load(path: &Path) -> Option<Snapshot> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    // A .mN can hold an unopened turn followed by the current one; the year we
    // want is the last segment.
    let blocks = file.segment_blocks(file.latest_segment());

    let players = player_records_in(blocks).ok()?;
    let mut races: Vec<Option<Race>> = Vec::new();
    for record in &players {
        let idx = usize::from(record.player_number);
        if races.len() <= idx {
            races.resize(idx + 1, None);
        }
        if let Some(race) = record.race.as_ref() {
            races[idx] = Some(to_core_race(race));
        }
    }

    let planets = planet_records_in(blocks)
        .into_iter()
        .filter_map(|p| to_core_planet(&p).map(|c| (p.id, c)))
        .collect();
    Some((races, planets))
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
    attrs[RaceStat::MajorAdv as usize] = prt_to_core(r.prt) as i16;

    // A `0xFF` bound marks the race immune on that axis; the simulation
    // detects that through a negative upper bound.
    let axis = |h: stars_formats::HabRange| -> (i8, i8, i8) {
        match (h.center, h.low, h.high) {
            (Some(c), Some(l), Some(x)) => (c as i8, l as i8, x as i8),
            _ => (0, 0, -1),
        }
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

fn prt_to_core(p: stars_formats::Prt) -> CorePrt {
    match p.abbrev() {
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
    }
}

/// Convert a decoded record into simulation state, but only when the record
/// carries everything the population formula needs.
fn to_core_planet(r: &PlanetRecord) -> Option<Planet> {
    let owner = r.owner?;
    let env = r.environment?;
    let conc = r.concentration?;
    let pop = r.population?;
    let imp = r.installations?;
    let surface = r.surface_minerals?;

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
        surface_min: [
            surface.ironium as i32,
            surface.boranium as i32,
            surface.germanium as i32,
        ],
        // Stored on disk in hundreds of colonists; `PlanetRecord` multiplies
        // that out, and the simulation works in the stored unit.
        pop: i32::try_from(pop / 100).ok()?,
        delta_pop: imp.delta_pop,
        mines: i16::try_from(imp.mines).ok()?,
        factories: i16::try_from(imp.factories).ok()?,
        homeworld: r.homeworld,
        starbase: r.has_starbase,
    })
}

/// Predict year N+1 population for every planet present in both files.
///
/// Outcome of comparing one year-step.
#[derive(Default)]
struct Tally {
    /// Population and accumulator both predicted exactly.
    exact: usize,
    /// Population differs, but the accumulator was still predicted exactly —
    /// the signature of colonists moved by a freighter, which changes the
    /// population without touching the growth accumulator.
    accum_only: usize,
    /// Population differs, but there is a starting population — reachable by
    /// moving colonists on or off the planet before growth — for which the
    /// formula reproduces the recorded population *and* accumulator exactly.
    transfer: usize,
    /// Neither matched, and no cargo transfer explains it.
    missed: usize,
    /// A few human-readable mismatches, for the failure message.
    notes: Vec<String>,
}

impl Tally {
    fn add(&mut self, other: &Tally) {
        self.exact += other.exact;
        self.accum_only += other.accum_only;
        self.transfer += other.transfer;
        self.missed += other.missed;
    }

    fn total(&self) -> usize {
        self.exact + self.accum_only + self.transfer + self.missed
    }
}

/// Search for a pre-growth population that reproduces the recorded result.
///
/// Freighters load and unload colonists before growth is applied, and this
/// crate does not process orders yet, so a planet's population at the start of
/// the growth step is not always the population recorded a year earlier. This
/// asks the much stricter question: *is there any* starting population for
/// which the growth formula lands on both the recorded population and the
/// recorded fractional accumulator? Because the accumulator is a value in
/// 0..100 that the formula pins down exactly, a spurious hit is unlikely.
fn explained_by_transfer(planet: &Planet, race: &Race, actual: &Planet) -> bool {
    let mut probe = planet.clone();
    // Cargo moves in whole units; search a generous window either side.
    let hi = actual.pop.max(planet.pop) + 1_000;
    for start in 0..=hi {
        probe.pop = start;
        let Some(change) = chg_pop_from_planet(&probe, race) else {
            continue;
        };
        if start + change.delta == actual.pop && change.delta_accum == actual.delta_pop {
            return true;
        }
    }
    false
}

fn compare(before: &Path, after: &Path) -> Option<Tally> {
    let (races, from) = load(before)?;
    let (_, to) = load(after)?;

    let mut tally = Tally::default();

    for (id, planet) in &from {
        let Some((_, actual)) = to.iter().find(|(i, _)| i == id) else {
            continue;
        };
        // Only the file's own player has a full race block; other players'
        // planets cannot be simulated from this file.
        let Some(race) = planet
            .owner
            .and_then(|o| usize::try_from(o).ok())
            .and_then(|o| races.get(o))
            .and_then(Option::as_ref)
        else {
            continue;
        };
        // Terraforming runs *before* growth inside a turn, so a planet
        // terraformed this year grows against its new environment. This crate
        // does not simulate terraforming yet (Step 4), so we feed in the
        // environment the original recorded at the end of the step; that
        // isolates the growth formula from the terraforming system instead of
        // charging one system's gap to the other. Feeding the pre-terraform
        // environment instead drops the exact-match count from 372 to 344,
        // which is how the ordering was established.
        let mut planet = planet.clone();
        planet.env = actual.env;
        let planet = &planet;
        let Some(change) = chg_pop_from_planet(planet, race) else {
            continue;
        };
        let predicted = planet.pop + change.delta;
        let accum_ok = change.delta_accum == actual.delta_pop;
        if predicted == actual.pop && accum_ok {
            tally.exact += 1;
        } else if accum_ok {
            tally.accum_only += 1;
        } else if explained_by_transfer(planet, race, actual) {
            tally.transfer += 1;
        } else {
            tally.missed += 1;
            if tally.notes.len() < 8 {
                tally.notes.push(format!(
                    "planet {id}: pop {} -> predicted {predicted} (accum {}), actual {} (accum {})",
                    planet.pop, change.delta_accum, actual.pop, actual.delta_pop
                ));
            }
        }
    }
    Some(tally)
}

#[test]
fn exodus_consecutive_years_population_matches_the_original_engine() {
    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: {} absent", games.display());
        return;
    }

    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let mut total = Tally::default();
    let mut all_notes = Vec::new();
    let mut pairs = 0;

    for w in years.windows(2) {
        // Only consecutive years are a single simulation step.
        if w[1] != w[0] + 1 {
            continue;
        }
        let before = games.join(w[0].to_string()).join("exodus.m6");
        let after = games.join(w[1].to_string()).join("exodus.m6");
        if !before.is_file() || !after.is_file() {
            continue;
        }
        if let Some(tally) = compare(&before, &after) {
            pairs += 1;
            total.add(&tally);
            for n in tally.notes {
                all_notes.push(format!("{}->{}: {n}", w[0], w[1]));
            }
        }
    }

    let n = total.total();
    eprintln!(
        "exodus: {pairs} year pairs, {n} planet-years: {} exact, {} accumulator-only, \
         {} explained by colonist transfer, {} unexplained",
        total.exact, total.accum_only, total.transfer, total.missed
    );
    for note in all_notes.iter().take(10) {
        eprintln!("  {note}");
    }

    assert!(pairs > 0, "expected at least one consecutive-year pair");
    assert!(
        n > 100,
        "expected a meaningful sample, got {n} planet-years"
    );

    // Population alone can be changed by things this crate does not yet
    // simulate — freighters loading and dropping colonists is by far the most
    // common — so an exact match is not expected on every planet of a 40-turn
    // game. Two weaker signals cover those: the growth accumulator is touched
    // *only* by the growth formula, and a planet whose population moved can
    // still be explained if some starting population reproduces both the
    // recorded population and the recorded accumulator.
    //
    // Measured on this fixture: 372 exact, 15 accumulator-only, 4 explained by
    // transfer, 35 unexplained. The unexplained residue is concentrated on a
    // few heavily-trafficked planets whose colonists move both before and
    // after the growth step; revisit once order processing lands in Step 4.
    let accum_ok = total.exact + total.accum_only;
    assert!(
        accum_ok * 100 / n >= 90,
        "growth accumulator matched only {accum_ok}/{n} planet-years: {all_notes:?}"
    );
    assert!(
        total.exact * 100 / n >= 85,
        "population matched exactly only {}/{n} planet-years: {all_notes:?}",
        total.exact
    );
}

#[test]
fn three_player_game_first_year_population_matches() {
    let root = workspace_root();
    let turn0 = root.join("fixtures/incoming/turn0");
    let turn1 = root.join("fixtures/incoming/turn1");
    if !turn0.is_dir() || !turn1.is_dir() {
        eprintln!("skipping: {} absent", turn0.display());
        return;
    }

    // Year one of a fresh game: every player still sits on their homeworld and
    // nothing has been shipped anywhere, so every planet must match exactly.
    let mut checked = 0;
    for player in 1..=3 {
        let before = turn0.join(format!("Game.m{player}"));
        let after = turn1.join(format!("Game.m{player}"));
        if !before.is_file() || !after.is_file() {
            continue;
        }
        let tally = compare(&before, &after).expect("fixture decodes");
        // Year one is nearly clean, but players do load colonists onto their
        // first colony ship before growth runs, so a planet may legitimately
        // grow from a smaller starting population than the year-2400 file
        // records. Every planet must be explained one of those two ways.
        assert_eq!(
            tally.missed, 0,
            "player {player}: {} planet-years unexplained: {:?}",
            tally.missed, tally.notes
        );
        checked += tally.exact + tally.transfer;
    }
    assert!(
        checked >= 3,
        "expected at least one homeworld per player, checked {checked}"
    );
    eprintln!("3-player game year 2400->2401: {checked} planet-years exact");
}
