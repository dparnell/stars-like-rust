//! The computer players' production decisions, checked against a recorded game.
//!
//! `fixtures/games/all-computer-players` is 101 turns (2400-2500) of a game
//! with sixteen computer players covering six of the seven personalities and
//! all four skill settings. That makes the AI's queue decisions checkable for
//! the first time.
//!
//! What these tests pin is what the corpus actually confirms. The mine and
//! factory decision is deliberately **not** asserted: see
//! `docs/formulas/ai.md` for the measured result and why.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stars_core::ai::production::{queue_ai_terraforming, Context, MAX_TERRAFORM_QUEUED};
use stars_core::ai::{AiPersonality, Control};
use stars_core::production::item;
use stars_core::GameState;
use stars_formats::StarsFile;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// One AI-owned planet in one turn.
struct Sample {
    personality: Option<AiPersonality>,
    pop: i32,
    terraform: i32,
}

fn corpus() -> Vec<Sample> {
    let dir = workspace_root().join("fixtures/games/all-computer-players");
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut out = Vec::new();
    for year in years {
        let Ok(bytes) = std::fs::read(year.join("Game.hst")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);
        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };
            out.push(Sample {
                personality,
                pop: planet.pop,
                terraform: planet
                    .queue
                    .iter()
                    .filter(|e| !e.ship && e.item == item::AUTO_TERRAFORM)
                    .map(|e| e.count)
                    .sum(),
            });
        }
    }
    out
}

/// `FQueueAiTerraforming` clamps the catalogue's available count to 4, and
/// nothing else queues more than that either.
#[test]
fn auto_terraform_is_never_queued_more_than_four_at_a_time() {
    let samples = corpus();
    if samples.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }
    let queued: Vec<i32> = samples
        .iter()
        .map(|s| s.terraform)
        .filter(|t| *t > 0)
        .collect();
    assert!(
        queued.len() > 1000,
        "expected a large sample, got {}",
        queued.len()
    );
    let over = queued.iter().filter(|t| **t > MAX_TERRAFORM_QUEUED).count();
    assert_eq!(over, 0, "{over} of {} exceeded the cap of 4", queued.len());
}

/// Cyber is the one personality `FQueueAiTerraforming` refuses to serve
/// (`1090:8d3a` tests `mode >> 13 != 4`).
///
/// Its terraform entries therefore come from somewhere else, and they look
/// nothing like the others': every Cyber entry is exactly 1, where the
/// personalities that do reach the routine show the full 1-4 spread. This is
/// the corpus confirming both the gate and the personality numbering, since
/// nothing else in the data would single out this one value.
#[test]
fn cyber_never_shows_the_terraform_routines_signature() {
    let samples = corpus();
    if samples.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }
    let mut spread: BTreeMap<&'static str, BTreeMap<i32, usize>> = BTreeMap::new();
    for s in &samples {
        if s.terraform == 0 {
            continue;
        }
        let name = s.personality.map_or("none", AiPersonality::name);
        *spread
            .entry(name)
            .or_default()
            .entry(s.terraform)
            .or_default() += 1;
    }

    let cyber = spread.get("Cyber").expect("Cyber queued some terraforming");
    assert_eq!(
        cyber.keys().copied().collect::<Vec<_>>(),
        vec![1],
        "Cyber should only ever show a count of 1, got {cyber:?}"
    );

    // The personalities that do reach the routine use its whole range.
    for name in ["Turindrone", "Automitron", "Macinti"] {
        let counts = spread.get(name).unwrap_or_else(|| panic!("{name} absent"));
        assert_eq!(
            counts.keys().copied().collect::<Vec<_>>(),
            vec![1, 2, 3, 4],
            "{name} should use the full 1-4 range, got {counts:?}"
        );
    }
}

/// The routine's population gate holds for the personalities that reach it.
///
/// Macinti and Cyber are excluded: both have a second, separate source of
/// auto-terraform entries — Macinti the branch inside
/// `FFillProdMinesAndFactories` that only it takes, Cyber whatever queues its
/// always-1 entries — and those are not subject to this gate.
#[test]
fn the_terraform_population_gate_holds_where_the_routine_is_the_only_source() {
    let samples = corpus();
    if samples.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }
    for name in ["Turindrone", "Automitron"] {
        let mine: Vec<&Sample> = samples
            .iter()
            .filter(|s| s.personality.map_or("none", AiPersonality::name) == name)
            .filter(|s| s.terraform > 0)
            .collect();
        assert!(!mine.is_empty(), "{name} queued no terraforming");
        let below = mine.iter().filter(|s| s.pop <= 199).count();
        let pct = below * 100 / mine.len();
        assert!(
            pct <= 5,
            "{name}: {below} of {} terraform entries sit below the population \
             gate ({pct}%), which is too many for a gate that should hold",
            mine.len()
        );
    }
}

/// The implementation agrees with the gates the corpus confirms.
#[test]
fn the_transcription_honours_its_gates() {
    use stars_core::planet::Planet;
    use stars_core::race::Race;

    let race = Race::humanoid();
    let mut planet = Planet::unowned(0);
    planet.owner = Some(0);
    planet.pop = 5_000;
    planet.env = [10, 50, 50]; // off ideal in gravity

    let base = Context {
        terraform_steps: 9,
        ..Context::default()
    };

    // Clamped to four, never nine.
    assert_eq!(queue_ai_terraforming(&planet, &race, &base), 4);

    // Cyber is refused.
    let cyber = Context {
        personality: Some(AiPersonality::Cyber),
        ..base
    };
    assert_eq!(queue_ai_terraforming(&planet, &race, &cyber), 0);

    // So is a planet at or below the population gate.
    let mut small = planet.clone();
    small.pop = 199;
    assert_eq!(queue_ai_terraforming(&small, &race, &base), 0);

    // And a planet already at its race's ideal.
    let mut ideal = planet.clone();
    ideal.env = race.env_center;
    assert_eq!(queue_ai_terraforming(&ideal, &race, &base), 0);

    // And one whose queue already holds an auto-terraform entry.
    let mut queued = planet.clone();
    queued.queue.push(stars_core::production::QueueItem {
        count: 1,
        item: item::AUTO_TERRAFORM,
        ship: false,
        completion: 0,
    });
    assert_eq!(queue_ai_terraforming(&queued, &race, &base), 0);
}
