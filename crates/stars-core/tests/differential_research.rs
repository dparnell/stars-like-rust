//! Differential verification of the research formulas against real save files.
//!
//! A `.mN` file records everything the research step needs: the six tech
//! levels, the resources banked toward the next level in each field, which
//! field is being researched, and how many resources research received from
//! the most recent turn. Replaying a year and comparing against what the
//! original engine wrote is therefore exact.
//!
//! Fixtures are optional: the test skips when the sample game is absent.

use std::path::{Path, PathBuf};

use stars_core::race::{Race, RaceStat};
use stars_core::research::{tech_level_cost, NextField, Research, TECH_FIELDS};
use stars_formats::{player_records, ResearchState, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// The research state and race of the file's own player.
fn load(path: &Path, player: u8) -> Option<(Race, ResearchState)> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    let players = player_records(&file).ok()?;
    let record = players.iter().find(|p| p.player_number == player)?;
    let race = record.race.as_ref()?;
    Some((to_core_race(race), record.research?))
}

fn to_core_race(r: &stars_formats::RaceRecord) -> Race {
    let mut race = Race::humanoid();
    // Only the research settings matter here; the six per-field costs sit at
    // rgAttr[rsTechBonus1..], in field order.
    for (field, cost) in r.research_cost.iter().enumerate() {
        race.attrs[RaceStat::TechBonus1 as usize + field] = i16::from(*cost);
    }
    race.lrt_bits = u32::from(r.lrt_bits);
    race
}

fn to_core_research(s: &ResearchState) -> Research {
    Research {
        levels: s.levels,
        points: s.points.map(|p| i32::try_from(p).unwrap_or(i32::MAX)),
        current_field: usize::from(s.current_field).min(TECH_FIELDS - 1),
        next_field: NextField::from_raw(s.next_field),
    }
}

/// Replay every consecutive-year pair of the Exodus game and check the
/// research arithmetic.
///
/// Two things are checked, and they are deliberately separated because they
/// fail for different reasons:
///
/// * **Accumulation** — with no breakthrough, the current field's banked
///   points must grow by exactly the year's research allocation. Nothing else
///   touches them.
/// * **Cost** — where exactly one field advanced by exactly one level and it
///   was the field being researched, the resources consumed must equal
///   `tech_level_cost` for that level.
///
/// Level-ups in fields nobody was researching, or several fields advancing at
/// once, are technology *gifts* (Mystery Traders, captured or scrapped ships)
/// rather than research, and this crate does not model those yet.
#[test]
fn exodus_research_matches_the_original_engine() {
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

    let mut accumulation_checked = 0;
    let mut cost_checked = 0;
    let mut gift_years = 0;
    let mut switch_years = 0;
    let mut failures = Vec::new();

    for w in years.windows(2) {
        if w[1] != w[0] + 1 {
            continue; // only a single year is a single research step
        }
        let before = games.join(w[0].to_string()).join("exodus.m6");
        let after = games.join(w[1].to_string()).join("exodus.m6");
        let (Some((race, s0)), Some((_, s1))) = (load(&before, 5), load(&after, 5)) else {
            continue;
        };

        let r0 = to_core_research(&s0);
        let field = r0.current_field;
        let added = i32::try_from(s1.last_year_resources).unwrap_or(0);
        let advanced: Vec<usize> = (0..TECH_FIELDS)
            .filter(|f| s1.levels[*f] > s0.levels[*f])
            .collect();

        // When research changes field the leftover resources move with it, so
        // the banked points of neither field can be read as a simple balance.
        // Those years are counted separately rather than checked here.
        if s1.current_field != s0.current_field {
            switch_years += 1;
            continue;
        }

        if advanced.is_empty() {
            // No breakthrough: the banked points simply grow by the allocation.
            let expected = r0.points[field] + added;
            let actual = i32::try_from(s1.points[field]).unwrap_or(0);
            if expected == actual {
                accumulation_checked += 1;
            } else {
                failures.push(format!(
                    "{}->{}: field {field} banked {} + {added} = {expected}, engine wrote {actual}",
                    w[0], w[1], r0.points[field]
                ));
            }
        } else if advanced == [field] && s1.levels[field] == s0.levels[field] + 1 {
            // A single level in the field being researched: check the price.
            let spent = r0.points[field] + added - i32::try_from(s1.points[field]).unwrap_or(0);
            let expected = tech_level_cost(field, s0.levels[field] + 1, &r0, &race, false);
            let available = r0.points[field] + added;

            if spent <= 0 || available < expected {
                // The player could not have bought this level: banked plus
                // allocated resources do not cover it, or nothing was spent at
                // all. Something other than research granted it — a Mystery
                // Trader, or technology from a captured or scrapped ship.
                //
                // This rule could in principle hide a cost formula that is
                // systematically too high, which is why the priced
                // breakthroughs below must match *exactly* and are required to
                // outnumber these.
                gift_years += 1;
            } else if spent == expected {
                cost_checked += 1;
            } else {
                failures.push(format!(
                    "{}->{}: field {field} level {} cost {spent}, formula says {expected}",
                    w[0],
                    w[1],
                    s0.levels[field] + 1
                ));
            }
        } else {
            gift_years += 1;
        }
    }

    eprintln!(
        "exodus research: {accumulation_checked} accumulation years, {cost_checked} priced \
         breakthroughs, {gift_years} gifted/multi-field years, {switch_years} field-switch \
         years, {} failures",
        failures.len()
    );
    for f in failures.iter().take(10) {
        eprintln!("  {f}");
    }

    assert!(
        accumulation_checked >= 10,
        "expected a meaningful sample of accumulation years, got {accumulation_checked}"
    );
    assert!(
        cost_checked >= 3,
        "expected several priced breakthroughs, got {cost_checked}"
    );
    assert!(
        failures.is_empty(),
        "research arithmetic diverged from the original engine: {failures:?}"
    );
    assert!(
        cost_checked > gift_years,
        "more advances were written off as gifts ({gift_years}) than were priced \
         ({cost_checked}); the cost formula may be too high"
    );
}
