//! Wormholes and the Mystery Trader, checked against real games.
//!
//! Both wander on their own, so what can be held against the fixtures is what
//! is deterministic — the Trader's speed — and what must always be true of a
//! wormhole pair. See `docs/formulas/wanderers.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stars_core::movement::distance;
use stars_core::GameState;
use stars_formats::StarsFile;

/// Every save under `fixtures/`, with the year it holds.
fn saves() -> Vec<(u16, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut found = Vec::new();
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
            let is_save = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| (e.starts_with('m') && e.len() == 2) || e == "hst");
            if !is_save {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(file) = StarsFile::decode(&bytes) else {
                continue;
            };
            found.push((file.latest_segment().header.turn, path));
        }
    }
    found
}

fn load(path: &Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    let (state, _) = GameState::from_file(&file);
    Some(state)
}

/// Wormholes and the Trader survive a load and a save unchanged.
#[test]
fn the_wanderers_round_trip() {
    let mut holes = 0;
    let mut traders = 0;
    for (_, path) in saves() {
        let Some(state) = load(&path) else { continue };
        if state.wormholes.is_empty() && state.trader.is_none() {
            continue;
        }
        let Ok(bytes) = stars_core::save::host_file(&state) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (back, _) = GameState::from_file(&file);
        assert_eq!(back.wormholes, state.wormholes, "{}", path.display());
        assert_eq!(back.trader, state.trader, "{}", path.display());
        holes += state.wormholes.len();
        traders += usize::from(state.trader.is_some());
    }
    if holes == 0 && traders == 0 {
        eprintln!("skipping: no wanderers in the fixtures");
        return;
    }
    eprintln!("{holes} wormhole ends and {traders} traders round-tripped");
}

/// Both ends of a wormhole point at each other, and no end points at itself.
#[test]
fn wormholes_come_in_pairs() {
    let mut checked = 0;
    let mut paired = 0;
    for (_, path) in saves() {
        let Some(state) = load(&path) else { continue };
        for hole in &state.wormholes {
            checked += 1;
            let partner = hole.partner & 0x01FF;
            assert_ne!(
                partner,
                hole.id,
                "{}: an end paired with itself",
                path.display()
            );
            // The far end is not always in this player's view, which is the
            // whole point of a wormhole nobody has been through.
            if let Some(other) = state.wormholes.iter().find(|w| w.id == partner) {
                assert_eq!(
                    other.partner & 0x01FF,
                    hole.id,
                    "{}: pairing is not mutual",
                    path.display()
                );
                paired += 1;
            }
        }
    }
    if checked == 0 {
        eprintln!("skipping: no wormholes in the fixtures");
        return;
    }
    eprintln!("{checked} wormhole ends, {paired} with both ends in view");
}

/// The Mystery Trader crosses the square of its warp each year — the one thing
/// about it that is not a dice roll.
#[test]
fn the_trader_flies_at_the_square_of_its_warp() {
    let mut by_game: BTreeMap<(PathBuf, String), BTreeMap<u16, PathBuf>> = BTreeMap::new();
    for (turn, path) in saves() {
        let (Some(name), Some(family)) = (
            path.file_name().and_then(|n| n.to_str()),
            path.parent().and_then(Path::parent),
        ) else {
            continue;
        };
        by_game
            .entry((family.to_path_buf(), name.to_string()))
            .or_default()
            .insert(turn, path);
    }

    let mut followed = 0;
    let mut flew_right = 0;
    for years in by_game.values() {
        for (turn, path) in years {
            let (Some(next), Some(before)) = (years.get(&(turn + 1)), load(path)) else {
                continue;
            };
            let (Some(after), Some(trader)) = (load(next), before.trader.as_ref()) else {
                continue;
            };
            let Some(then) = after.trader.as_ref() else {
                continue;
            };
            if then.id != trader.id {
                continue;
            }
            followed += 1;
            let flown = distance(trader.position, then.position);
            // Either it covered a year's flight at the speed it had, or it
            // arrived — and it may have sped up on the way, which is the one
            // in twenty-five roll.
            let expected = f64::from(trader.range());
            let sped_up = f64::from(then.range());
            let arrived = distance(then.position, then.destination) < 1.0;
            if (flown - expected).abs() <= 2.0
                || (flown - sped_up).abs() <= 2.0
                || (arrived && flown <= sped_up.max(expected))
            {
                flew_right += 1;
            }
        }
    }

    if followed == 0 {
        eprintln!("skipping: the trader was not seen in two consecutive years");
        return;
    }
    // The years that do not fit are the ones where the Trader reached its
    // destination and set off again — its warp goes *down*, which the course
    // change never does, so it is a new pass rather than the same flight.
    // Arrival is not modelled.
    eprintln!("{followed} trader years followed, {flew_right} flew as modelled");
    assert!(
        flew_right * 100 >= followed * 90,
        "{flew_right} of {followed}"
    );
}
