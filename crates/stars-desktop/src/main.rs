//! Native shell for the Stars! reimplementation.
//!
//! The egui application arrives in delivery Step 5. Until then this is a small
//! command-line tool over the same engine, which is genuinely useful for
//! reverse-engineering work: it opens a real save file, reports what the
//! simulation makes of it, and can generate a turn.
//!
//! ```text
//! stars <file.mN|file.hst>          summarise a saved game
//! stars <file> --turn               ... and generate one turn
//! ```

#![forbid(unsafe_code)]

use std::process::ExitCode;

use stars_core::rng::Rng;
use stars_core::{generate_turn, GameState};
use stars_formats::StarsFile;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: stars <game file> [--turn]");
        eprintln!("       a Stars! player file (.m1 …) or host file (.hst)");
        return ExitCode::from(2);
    };
    let advance = args.any(|a| a == "--turn");

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let file = match StarsFile::decode(&bytes) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("cannot decode {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (mut state, report) = GameState::from_file(&file);
    summarise(&path, &file, &state, &report);

    if advance {
        println!();
        let mut rng = Rng::randomize(state.seed);
        let turn = generate_turn(&mut state, &mut rng);
        describe_turn(&turn);
    }

    ExitCode::SUCCESS
}

fn summarise(path: &str, file: &StarsFile, state: &GameState, report: &stars_core::LoadReport) {
    let header = &file.latest_segment().header;
    println!("{path}");
    println!(
        "  {:?} file, game {:#010x}, year {}",
        header.file_type,
        header.game_id,
        state.year()
    );
    if file.segments().len() > 1 {
        println!(
            "  note: holds {} turns; reading the most recent",
            file.segments().len()
        );
    }
    println!(
        "  {} planets simulated, {} known only at a distance, {} designs, {} fleets",
        report.planets_loaded, report.planets_partial, report.designs_loaded, report.fleets_loaded
    );

    for (i, player) in state.players.iter().enumerate() {
        let owned = state
            .planets
            .iter()
            .filter(|p| p.owner == i16::try_from(i).ok())
            .count();
        if owned == 0 && player.research.levels.iter().all(|l| *l == 0) {
            continue;
        }
        let pop: i64 = state
            .planets
            .iter()
            .filter(|p| p.owner == i16::try_from(i).ok())
            .map(|p| p.colonists())
            .sum();
        let fleets = state
            .fleets
            .iter()
            .filter(|f| f.owner == i16::try_from(i).unwrap_or(-1))
            .count();
        let ships: i32 = state
            .fleets
            .iter()
            .filter(|f| f.owner == i16::try_from(i).unwrap_or(-1))
            .map(stars_core::Fleet::ships)
            .sum();
        println!(
            "  player {i}: {owned} planets, {pop} colonists, {fleets} fleets ({ships} ships), \
             tech {:?}, {}% to research",
            player.research.levels, player.research_pct
        );
    }
}

fn describe_turn(turn: &stars_core::TurnReport) {
    println!("generated year {}", turn.year);

    let mined: i32 = turn.mined.iter().map(|(_, m)| m.iter().sum::<i32>()).sum();
    let growth: i32 = turn.population.iter().map(|(_, d)| *d).sum();
    println!("  {mined} kT mined, population {growth:+} (in hundreds)");

    for (id, items) in &turn.built {
        let what: Vec<String> = items
            .iter()
            .map(|(item, n)| format!("{n} of item {item}"))
            .collect();
        println!("  planet {id} built {}", what.join(", "));
    }

    for (i, spent) in turn.research_spending.iter().enumerate() {
        if *spent == 0 {
            continue;
        }
        let gained = turn.breakthroughs.get(i).map_or(0, Vec::len);
        println!("  player {i} put {spent} into research, gaining {gained} levels");
    }

    if !turn.skipped.is_empty() {
        println!("  not simulated: {:?}", turn.skipped);
    }
}
