//! Differential verification of resource output against real save files.
//!
//! Resources are never written to disk directly, so they cannot be compared
//! field-for-field. But the research allocation *is* written
//! (`PLAYER.lResLastYear`), and `Produce` (`10b8:0000`) defines it exactly:
//! every planet skims `pctResearch` of its output for research, and whatever
//! the build queue does not spend falls through to research as well.
//!
//! That gives a bound on a player's total output which does not depend on
//! modelling the build queue:
//!
//! ```text
//! lResLastYear <= sum(planet resources)
//! ```
//!
//! It is one-sided but sharp in the direction that matters: research can only
//! ever receive what the planets produced, so a resource formula that produced
//! too *little* would break it immediately, in every year.
//!
//! There is deliberately no lower bound. It is tempting to assume every planet
//! contributes at least its `pctResearch` skim, but `Produce` (`10b8:0000`)
//! gives a planet holding an empty production-queue array no contribution at
//! all, and a planet's leftover depends on what it built — which needs the
//! components table. Measured on this fixture the engine allocates a mean 43%
//! of our computed total against a 30% research setting — above the skim
//! because unspent production falls through to research, and never above 100%,
//! which is the bound this test enforces.

use std::path::{Path, PathBuf};

use stars_core::planet::Planet;
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

#[test]
fn exodus_research_allocation_is_bounded_by_our_resource_output() {
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

    let mut checked = 0;
    let mut exact = 0;
    let mut ratio_sum = 0;
    let mut violations = Vec::new();

    for w in years.windows(2) {
        if w[1] != w[0] + 1 {
            continue;
        }
        let before = games.join(w[0].to_string()).join("exodus.m6");
        let after = games.join(w[1].to_string()).join("exodus.m6");

        let (Ok(b0), Ok(b1)) = (std::fs::read(&before), std::fs::read(&after)) else {
            continue;
        };
        let (Ok(f0), Ok(f1)) = (StarsFile::decode(&b0), StarsFile::decode(&b1)) else {
            continue;
        };
        let b0 = f0.segment_blocks(f0.latest_segment());
        let b1 = f1.segment_blocks(f1.latest_segment());
        let (Ok(p0), Ok(p1)) = (player_records_in(b0), player_records_in(b1)) else {
            continue;
        };
        let Some(rec0) = p0.iter().find(|p| p.player_number == 5) else {
            continue;
        };
        let Some(rec1) = p1.iter().find(|p| p.player_number == 5) else {
            continue;
        };
        let (Some(race_rec), Some(res1)) = (rec0.race.as_ref(), rec1.research) else {
            continue;
        };
        let race = to_core_race(race_rec);
        let research_pct = i32::from(race_rec.research_percentage);

        // Sum this player's own planets.
        let mut total = 0i32;
        let mut skim = 0i32;
        let mut owned = 0;
        for record in planet_records_in(b0) {
            if record.owner != Some(5) {
                continue;
            }
            let Some(planet) = to_core_planet(&record) else {
                continue;
            };
            let Some(res) = resources_at_planet(&planet, &race) else {
                continue;
            };
            owned += 1;
            let res = i32::from(res);
            total += res;
            skim += res * research_pct / 100;
        }
        if owned == 0 {
            continue;
        }

        let allocated = i32::try_from(res1.last_year_resources).unwrap_or(0);
        checked += 1;
        if allocated == total {
            exact += 1;
        }
        ratio_sum += if total > 0 {
            allocated * 100 / total
        } else {
            0
        };
        if allocated > total {
            violations.push(format!(
                "{}->{}: engine allocated {allocated} to research but our {owned} planets \
                 only make {total} (skim would be {skim})",
                w[0], w[1]
            ));
        }
    }

    let mean_ratio = if checked > 0 { ratio_sum / checked } else { 0 };
    eprintln!(
        "exodus resources: {checked} years bounded, research took a mean {mean_ratio}% of our \
         computed output ({exact} years took all of it), {} violations",
        violations.len()
    );
    for v in violations.iter().take(8) {
        eprintln!("  {v}");
    }

    assert!(checked >= 10, "expected a meaningful sample, got {checked}");
    // The player's research setting is 30%, so a mean far below that would
    // mean our planets produce more than the engine's did.
    assert!(
        (15..=100).contains(&mean_ratio),
        "research took a mean {mean_ratio}% of our computed output, which does not \
         look like a 30% research setting"
    );
    assert!(
        violations.is_empty(),
        "resource output is inconsistent with the engine's research allocation: {violations:?}"
    );
}
