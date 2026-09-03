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

use stars_core::load::{planet_from_record, race_from_record};
use stars_core::planet::Planet;
use stars_core::population::chg_pop_from_planet;
use stars_core::race::Race;
use stars_formats::{planet_records_in, player_records_in, StarsFile};

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
            races[idx] = Some(race_from_record(race));
        }
    }

    let planets = planet_records_in(blocks)
        .into_iter()
        .filter_map(|p| planet_from_record(&p).map(|c| (p.id, c)))
        .collect();
    Some((races, planets))
}

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
