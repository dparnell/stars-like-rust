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

/// Every game's saves, keyed by the year they hold.
fn games() -> Vec<BTreeMap<u16, PathBuf>> {
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
    by_game.into_values().collect()
}

/// Wormholes and the Trader survive a load and a save unchanged.
#[test]
fn the_wanderers_round_trip() {
    let mut holes = 0;
    let mut traders = 0;
    for (_, path) in saves() {
        let Some(state) = load(&path) else { continue };
        if state.wormholes.is_empty() && state.traders.is_empty() {
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
        assert_eq!(back.traders, state.traders, "{}", path.display());
        holes += state.wormholes.len();
        traders += state.traders.len();
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
    let mut followed = 0;
    let mut flew_right = 0;
    for years in games() {
        for (turn, path) in &years {
            let (Some(next), Some(before)) = (years.get(&(turn + 1)), load(path)) else {
                continue;
            };
            let Some(after) = load(next) else { continue };
            for trader in &before.traders {
                let Some(then) = after.traders.iter().find(|t| t.id == trader.id) else {
                    continue;
                };
                followed += 1;
                let flown = distance(trader.position, then.position);
                // Either it covered a year's flight at the speed it had, or it
                // arrived — and it may have sped up on the way, which is the one
                // in twenty-five roll.
                let expected = f64::from(trader.range());
                let sped_up = f64::from(then.range());
                // Arriving means standing on the destination it *had*: on
                // another pass it already has a new one.
                let arrived = then.position == trader.destination;
                if (flown - expected).abs() <= 2.0
                    || (flown - sped_up).abs() <= 2.0
                    || (arrived && flown <= sped_up.max(expected))
                {
                    flew_right += 1;
                }
            }
        }
    }

    if followed == 0 {
        eprintln!("skipping: the trader was not seen in two consecutive years");
        return;
    }
    eprintln!("{followed} trader years followed, {flew_right} flew as modelled");
    assert_eq!(flew_right, followed, "{flew_right} of {followed}");
}

/// A Trader that reaches its destination turns round: same spot, new heading,
/// and a warp of `max(warp − 2, 6) + 1`.
///
/// This is what the twenty years that would not fit the flight model turned out
/// to be. Every one of them has the Trader standing exactly on the destination
/// it had, with a new destination and a warp one step slower — which is a pass
/// ending, not a flight continuing.
#[test]
fn the_trader_turns_round_at_its_destination() {
    let mut turned = 0;
    for years in games() {
        for (turn, path) in &years {
            let (Some(next), Some(before)) = (years.get(&(turn + 1)), load(path)) else {
                continue;
            };
            let Some(after) = load(next) else { continue };
            for trader in &before.traders {
                let Some(then) = after.traders.iter().find(|t| t.id == trader.id) else {
                    continue;
                };
                if then.warp >= trader.warp {
                    continue;
                }
                turned += 1;
                assert_eq!(
                    then.position,
                    trader.destination,
                    "{}: slowed down without arriving",
                    path.display()
                );
                assert_eq!(
                    then.warp,
                    trader.warp.saturating_sub(2).max(6) + 1,
                    "{}: not the another-pass warp",
                    path.display()
                );
                assert_ne!(
                    then.destination,
                    trader.destination,
                    "{}: turned round without a new heading",
                    path.display()
                );
            }
        }
    }
    if turned == 0 {
        eprintln!("skipping: no trader reached its destination in the fixtures");
        return;
    }
    eprintln!("{turned} trader-years ended a pass, all of them as modelled");
}

/// What the Trader carries is one technology, not a list of players.
///
/// The field sits beside a player mask in the record and was read as one here
/// until the fixtures said otherwise: across every Trader in them it holds a
/// single `GrbitTrader` bit or nothing at all, never a combination.
#[test]
fn the_trader_carries_one_thing() {
    let mut seen = 0;
    let mut carrying = 0;
    for (_, path) in saves() {
        let Some(state) = load(&path) else { continue };
        for trader in &state.traders {
            seen += 1;
            let part = trader.part;
            assert!(
                part & !stars_core::wormhole::part::ALL == 0,
                "{}: {part:#06x} is not a GrbitTrader mask",
                path.display()
            );
            assert!(
                part.count_ones() <= 1,
                "{}: carrying {part:#06x}, which is more than one thing",
                path.display()
            );
            carrying += usize::from(part != 0);
        }
    }
    if seen == 0 {
        eprintln!("skipping: no trader in the fixtures");
        return;
    }
    eprintln!("{seen} traders, {carrying} carrying something");
}

/// Going through a wormhole marks **both** ends for the traveller.
///
/// `MoveFleets` (`10b0:4d5b`) sets the travelled bit on the end the fleet
/// entered and on the one it came out of, together, so a player who has been
/// through one end has been through the other. Note that this is a different
/// mask from the one the scanners fill in: an end somebody travelled years ago
/// is routinely no longer in anybody's view, and a jump clears the view mask
/// and keeps the travel mask, so travelled is *not* a subset of seen.
#[test]
fn going_through_marks_both_ends() {
    let mut travelled = 0;
    let mut both_in_view = 0;
    let mut out_of_view = 0;
    for (_, path) in saves() {
        let Some(state) = load(&path) else { continue };
        for hole in &state.wormholes {
            if hole.traversed_by == 0 {
                continue;
            }
            travelled += 1;
            // Having been through an end says nothing about seeing it now.
            out_of_view += usize::from(hole.traversed_by & !hole.detected_by != 0);
            let partner = hole.partner & 0x01FF;
            let Some(other) = state.wormholes.iter().find(|w| w.id == partner) else {
                continue;
            };
            both_in_view += 1;
            assert_eq!(
                hole.traversed_by & !other.traversed_by,
                0,
                "{}: end {} travelled by someone who has not been through end {partner}",
                path.display(),
                hole.id
            );
        }
    }
    if travelled == 0 {
        eprintln!("skipping: nobody has been through a wormhole in the fixtures");
        return;
    }
    eprintln!(
        "{travelled} ends somebody has been through, {both_in_view} with both ends in view, \
         {out_of_view} travelled by somebody who cannot see them now"
    );
}
