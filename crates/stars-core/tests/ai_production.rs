//! The computer players' production decisions, checked against a recorded game.
//!
//! `fixtures/games/all-computer-players` is 101 turns (2400-2500) of a game
//! with sixteen computer players covering six of the seven personalities and
//! all four skill settings. That makes the AI's queue decisions checkable for
//! the first time.
//!
//! What these tests pin is what the corpus actually confirms, with the
//! measured accuracy of each piece. See `docs/formulas/ai.md` for the full
//! numbers and the controls behind them.

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

/// The mine and factory decision reproduces *whether* the AI builds, even
/// though it often gets the amount wrong.
///
/// The queue is not the observable here: the next turn's production builds
/// these entries and empties it, so mines or factories grow on thousands of
/// planet-year pairs that show no queue entry at all. This scores the change
/// in the planet's counts instead.
///
/// The thresholds are set below the measured values (99% recall, 72%
/// precision) so that a real regression trips them without normal drift doing
/// so. Exact counts are only right about a third of the time and are
/// deliberately not asserted.
#[test]
fn the_mine_and_factory_decision_predicts_when_the_ai_builds() {
    use stars_core::ai::production::{fill_prod_mines_and_factories, Context};

    let dir = workspace_root().join("fixtures/games/all-computer-players");
    if !dir.is_dir() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    // planet id -> (predicted total, mines before, factories before)
    let mut pending: BTreeMap<i16, (i32, i16, i16)> = BTreeMap::new();
    let (mut built, mut predicted_some, mut true_positive) = (0usize, 0usize, 0usize);

    for year in &years {
        let Ok(bytes) = std::fs::read(year.join("Game.hst")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);
        let turn = state.year() - 2400;

        for planet in &state.planets {
            let Some(&(predicted, was_mines, was_factories)) = pending.get(&planet.id) else {
                continue;
            };
            let grew =
                i32::from(planet.mines - was_mines) + i32::from(planet.factories - was_factories);
            if grew < 0 {
                continue; // changed hands or was bombed
            }
            if grew > 0 {
                built += 1;
            }
            if predicted > 0 {
                predicted_some += 1;
                if grew > 0 {
                    true_positive += 1;
                }
            }
        }

        pending.clear();
        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };
            let mut tech = [0u8; 6];
            for (slot, level) in tech.iter_mut().zip(player.research.levels.iter()) {
                *slot = *level;
            }
            let ctx = Context {
                personality,
                research_pct: player.research_pct,
                tech,
                turn,
                ..Context::default()
            };
            let d = fill_prod_mines_and_factories(planet, &player.race, &ctx);
            pending.insert(
                planet.id,
                (d.mines + d.factories, planet.mines, planet.factories),
            );
        }
    }

    assert!(built > 5000, "expected a large sample, got {built}");
    let recall = true_positive * 100 / built;
    let precision = true_positive * 100 / predicted_some.max(1);
    assert!(recall >= 95, "recall fell to {recall}% (was 99%)");
    assert!(precision >= 65, "precision fell to {precision}% (was 72%)");
}

/// What the corpus confirms about the AI's starbase orders.
///
/// `QueueAiStarbases` (`1090:8524`) and `FUpgradeAiStarbase` (`1090:882a`)
/// both append exactly one starbase, with a count of one, and both refuse
/// outright when the queue already holds a starbase order. The design index is
/// encoded as `slot - 0x10`.
#[test]
fn starbase_orders_follow_the_shape_the_routines_impose() {
    use stars_core::ai::ships::{is_starbase_slot, STARBASE_MIN_POP};

    let years = {
        let dir = workspace_root().join("fixtures/games/all-computer-players");
        if !dir.is_dir() {
            eprintln!("skipping: all-computer-players fixture absent");
            return;
        }
        let mut v: Vec<_> = std::fs::read_dir(&dir)
            .expect("game directory")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        v.sort();
        v
    };

    let mut orders = 0usize;
    let mut more_than_one = 0usize;
    let mut count_not_one = 0usize;
    let mut below_pop_gate = 0usize;
    let mut same_design_as_existing = 0usize;
    let mut with_existing = 0usize;

    for year in &years {
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
            if !player.control.is_computer() {
                continue;
            }
            let bases: Vec<_> = planet
                .queue
                .iter()
                .filter(|e| e.ship && is_starbase_slot(e.item))
                .collect();
            if bases.is_empty() {
                continue;
            }
            orders += 1;
            if bases.len() > 1 {
                more_than_one += 1;
            }
            if bases.iter().any(|e| e.count != 1) {
                count_not_one += 1;
            }
            if planet.pop <= STARBASE_MIN_POP {
                below_pop_gate += 1;
            }
            if let Some(have) = planet.starbase_design.filter(|_| planet.starbase) {
                with_existing += 1;
                if bases.iter().any(|e| e.item - 0x10 == u16::from(have)) {
                    same_design_as_existing += 1;
                }
            }
        }
    }

    assert!(orders > 2000, "expected a large sample, got {orders}");
    assert_eq!(more_than_one, 0, "two starbases queued at once");
    assert_eq!(
        count_not_one, 0,
        "a starbase order with a count other than 1"
    );
    assert!(
        same_design_as_existing == 0,
        "{same_design_as_existing} of {with_existing} orders re-queue the design \
         the planet already has"
    );
    // One planet in the corpus dips below the gate after its order was placed.
    assert!(
        below_pop_gate <= 1,
        "{below_pop_gate} starbase orders sit below the population gate"
    );
}

/// Every recorded Macinti starbase replacement is one of the three moves
/// `FUpgradeAiStarbase` can make.
///
/// The routine is gated on `Random(100)` throughout, so no single decision can
/// be checked without the RNG in the same state. Its *arithmetic* is
/// deterministic given the design being replaced, and that is what this pins:
/// from design `d` the only reachable results are `d + 1` (and only from the
/// four hull slots that have one above them), `d + 3`, and `d - 3` where
/// `d + 3` would pass 9. Designs below 4 walk the recycling table instead and
/// are excluded.
#[test]
fn macinti_starbase_replacements_use_only_the_moves_the_routine_has() {
    use stars_core::ai::ships::is_starbase_slot;

    let dir = workspace_root().join("fixtures/games/all-computer-players");
    if !dir.is_dir() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut checked = 0usize;
    let mut unexplained: BTreeMap<(u8, u8), usize> = BTreeMap::new();

    for year in &years {
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
            if personality != Some(AiPersonality::Macinti) || !planet.starbase {
                continue;
            }
            let Some(have) = planet.starbase_design else {
                continue;
            };
            if have < 4 {
                continue; // walks the recycling table, whose state is not saved
            }
            for e in planet
                .queue
                .iter()
                .filter(|e| e.ship && is_starbase_slot(e.item))
            {
                let want = u8::try_from(e.item - 0x10).unwrap_or(u8::MAX);
                checked += 1;

                let nudge = matches!(have, 4 | 5 | 7 | 8) && want == have + 1;
                let jump = if have + 3 > 9 {
                    want + 3 == have
                } else {
                    want == have + 3
                };
                if !nudge && !jump {
                    *unexplained.entry((have, want)).or_default() += 1;
                }
            }
        }
    }

    assert!(checked > 200, "expected a real sample, got {checked}");
    assert!(
        unexplained.is_empty(),
        "{} of {checked} Macinti replacements are moves the routine cannot make: {unexplained:?}",
        unexplained.values().sum::<usize>()
    );
}
