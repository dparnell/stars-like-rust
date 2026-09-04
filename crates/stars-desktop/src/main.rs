//! Native shell for the Stars! reimplementation.
//!
//! The egui application arrives in delivery Step 5. Until then this is a small
//! command-line tool over the same engine, which is genuinely useful for
//! reverse-engineering work: it opens a real save file, reports what the
//! simulation makes of it, and can generate a turn.
//!
//! ```text
//! stars                             open the graphical shell
//! stars <file>                      ... with a game already loaded
//! stars <file> --summary            print a summary instead
//! stars <file> --turn               ... and generate one turn
//! stars <file> --vcr [id]           play back a recorded battle as text
//! stars --new <name> [options]      create a universe and write its .xy
//! ```
//!
//! `--new` takes `--size tiny|small|medium|large|huge`,
//! `--density sparse|normal|dense|packed`, `--distance close|moderate|distant`,
//! `--players N` (the rest are Turindrones, Standard), `--clumping` and
//! `--id 0x...`. It writes `<name>.xy` beside the working directory and prints
//! the starting position, which is how the generator is exercised without a
//! window.
//!
//! `--vcr` is the first screen of the frontend proper, rendered here as text
//! while the egui shell is still to come. The view logic lives in
//! `stars_ui::vcr` and is frontend-agnostic; this only draws it.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use stars_core::rng::Rng;
use stars_core::{generate_turn, GameState};
use stars_formats::StarsFile;

mod app;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.first().is_some_and(|a| a == "--new") {
        return create_game(&all[1..]);
    }
    let Some(path) = args.next() else {
        // No arguments: the graphical shell, with nothing open.
        return match app::run(None) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("cannot start the window: {e}");
                eprintln!("run `stars <file> --summary` for the text tools instead");
                ExitCode::FAILURE
            }
        };
    };
    let rest: Vec<String> = args.collect();
    let advance = rest.iter().any(|a| a == "--turn");
    let text_only =
        advance || rest.iter().any(|a| a == "--summary") || rest.iter().any(|a| a == "--vcr");
    let vcr = rest.iter().position(|a| a == "--vcr").map(|i| {
        rest.get(i + 1)
            .and_then(|s| u16::from_str_radix(s.trim_start_matches("0x"), 16).ok())
    });

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

    if let Some(wanted) = vcr {
        return play_battles(&file, wanted);
    }

    if !text_only {
        // A file and no other request: open it in the window.
        return match app::run(Some(std::path::PathBuf::from(&path))) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("cannot start the window: {e}");
                ExitCode::FAILURE
            }
        };
    }

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

/// Play the battle recordings a file carries, as text.
///
/// The VCR plays the recording rather than re-simulating it — see
/// `stars_ui::vcr` for why that distinction matters.
fn play_battles(file: &StarsFile, wanted: Option<u16>) -> ExitCode {
    use stars_formats::{battle_records_in_with, ActionLayout};
    use stars_ui::vcr::{Event, Vcr};

    let header = &file.latest_segment().header;
    let layout = ActionLayout::for_version(header.version_major, header.version_minor);
    let battles = battle_records_in_with(file.segment_blocks(file.latest_segment()), layout);

    if battles.is_empty() {
        eprintln!("no battle recordings in this file");
        return ExitCode::FAILURE;
    }
    let chosen: Vec<_> = match wanted {
        Some(id) => battles.iter().filter(|b| b.id == id).collect(),
        None => battles.iter().collect(),
    };
    if chosen.is_empty() {
        eprintln!("no battle with that id; this file has:");
        for b in &battles {
            eprintln!("  {:#06x}", b.id);
        }
        return ExitCode::FAILURE;
    }

    for battle in chosen {
        let mut vcr = Vcr::new(battle);
        println!(
            "battle {:#06x} at planet {}, players {:?}, {} tokens, {} frames",
            vcr.id,
            vcr.planet
                .map_or_else(|| "deep space".into(), |p| p.to_string()),
            vcr.players,
            vcr.tokens().len(),
            vcr.len()
        );
        for (i, token) in vcr.tokens().iter().enumerate() {
            println!(
                "  token {i}: player {} — {} ships, {} shields, {}",
                token.player,
                token.ships,
                token.shields,
                if token.armed { "armed" } else { "unarmed" }
            );
        }

        let mut round = u8::MAX;
        while vcr.step() {
            let Some(frame) = vcr.frame() else { break };
            if frame.round != round {
                round = frame.round;
                println!("\n-- round {round}");
                draw(&vcr);
            }
            match &frame.event {
                Event::Move { token, from, to } => {
                    println!(
                        "  token {token} moves ({},{}) -> ({},{})",
                        from.0, from.1, to.0, to.1
                    );
                }
                Event::Fire {
                    attacker,
                    target,
                    range,
                    ships_killed,
                } => {
                    print!("  token {attacker} fires on {target} at range {range}");
                    if *ships_killed > 0 {
                        print!(" — {ships_killed} ships destroyed");
                    }
                    println!();
                }
                Event::Disengage { token } => println!("  token {token} leaves the battle"),
            }
        }

        println!("\n-- end");
        draw(&vcr);
        for (player, lost) in vcr.losses() {
            println!("  player {player} lost {lost} ships");
        }
        println!();
    }
    ExitCode::SUCCESS
}

/// Draw the board: each square shows the tokens standing on it.
fn draw(vcr: &stars_ui::vcr::Vcr) {
    let grid = vcr.board();
    for row in &grid {
        let cells: Vec<String> = row
            .iter()
            .map(|tokens| match tokens.len() {
                0 => " . ".to_string(),
                1 => format!(" {} ", tokens[0]),
                n => format!("*{n} "),
            })
            .collect();
        println!("    {}", cells.join(""));
    }
}

/// `stars --new <name> [options]`: generate a universe and write its `.xy`.
fn create_game(args: &[String]) -> ExitCode {
    use stars_core::newgame::{generate, Density, NewGame, NewPlayer, Size, StartDistance};
    use stars_core::opponents;

    let value = |name: &str| -> Option<&str> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };

    let name = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "New Game".to_string());

    let size = match value("--size").unwrap_or("small") {
        "tiny" => Size::Tiny,
        "small" => Size::Small,
        "medium" => Size::Medium,
        "large" => Size::Large,
        "huge" => Size::Huge,
        other => {
            eprintln!("unknown universe size {other}");
            return ExitCode::FAILURE;
        }
    };
    let density = match value("--density").unwrap_or("normal") {
        "sparse" => Density::Sparse,
        "normal" => Density::Normal,
        "dense" => Density::Dense,
        "packed" => Density::Packed,
        other => {
            eprintln!("unknown density {other}");
            return ExitCode::FAILURE;
        }
    };
    let start_distance = match value("--distance").unwrap_or("moderate") {
        "close" => StartDistance::Close,
        "moderate" => StartDistance::Moderate,
        "distant" => StartDistance::Distant,
        other => {
            eprintln!("unknown starting distance {other}");
            return ExitCode::FAILURE;
        }
    };
    let players: usize = value("--players")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
        .clamp(1, stars_core::newgame::MAX_PLAYERS);
    let id = value("--id")
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x2a03_1dd8);

    let mut config = NewGame {
        name: name.clone(),
        id,
        size,
        density,
        start_distance,
        clumping: args.iter().any(|a| a == "--clumping"),
        players: vec![NewPlayer::human(stars_core::Race::humanoid())],
        ..NewGame::default()
    };
    while config.players.len() < players {
        match opponents::opponent(config.players.len() % 6, 1) {
            Some(opponent) => config.players.push(opponent.as_player()),
            None => break,
        }
    }

    let mut rng = Rng::randomize(config.id);
    let made = match generate(&config, &mut rng) {
        Ok(made) => made,
        Err(e) => {
            eprintln!("cannot create the game: {e}");
            return ExitCode::FAILURE;
        }
    };

    let file: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let file = format!("{}.xy", file.trim_matches('-'));
    let bytes = match made.universe.encode() {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("cannot encode the universe: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = std::fs::write(&file, bytes) {
        eprintln!("cannot write {file}: {e}");
        return ExitCode::FAILURE;
    }

    println!("{name}");
    println!(
        "  {} universe, {} density, players {}, {} planets -> {file}",
        size.name(),
        density.name(),
        config.players.len(),
        made.state.planets.len()
    );
    for (i, player) in made.state.players.iter().enumerate() {
        let home = made
            .state
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == i16::try_from(i).ok());
        let ships = made
            .state
            .fleets
            .iter()
            .filter(|f| f.owner == i16::try_from(i).unwrap_or(-1))
            .count();
        match home {
            Some(home) => println!(
                "  player {i} ({:?}) on {} at {:?}: env {:?}, minerals {:?}, {ships} ships",
                player.race.prt().map(|p| p.abbrev()),
                home.name.unwrap_or("?"),
                home.position.map(|p| (p.x, p.y)),
                home.env,
                home.surface_min
            ),
            None => println!("  player {i}: no homeworld"),
        }
    }
    println!(
        "  note: only the .xy is written. A generated game has no .hst yet — \
         this project has no writers for planet, player, fleet or design blocks."
    );
    ExitCode::SUCCESS
}
