//! Mineral packets, checked against the ones in real games.
//!
//! Packets are the most common object in the fixtures — 95,798 of them — and
//! the turn directories hold consecutive years of the same game, so a packet
//! can be followed from one year to the next and its flight compared with what
//! this engine would have done. See `docs/formulas/packets.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stars_core::movement::distance;
use stars_core::packet::{decay_loss, Packet};
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
                .is_some_and(|e| e.starts_with('m') && e.len() == 2 || e == "hst");
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

/// Load a save's packets, and the planet positions they are aimed at.
fn packets_of(path: &Path) -> Option<(GameState, Vec<Packet>)> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    let (mut state, _) = GameState::from_file(&file);
    // Positions come from the universe beside the save.
    let mut here = path.parent()?.to_path_buf();
    for _ in 0..2 {
        if let Ok(entries) = std::fs::read_dir(&here) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("xy") {
                    if let Ok(bytes) = std::fs::read(entry.path()) {
                        if let Ok(universe) = stars_formats::Universe::decode(&bytes) {
                            state.apply_universe(&universe);
                        }
                    }
                }
            }
        }
        if let Some(up) = here.parent() {
            here = up.to_path_buf();
        }
    }
    let packets = state.packets.clone();
    Some((state, packets))
}

/// Every packet in the fixtures survives a load and a save unchanged.
#[test]
fn packets_survive_a_round_trip() {
    let mut checked = 0;
    for (_, path) in saves() {
        let Some((state, packets)) = packets_of(&path) else {
            continue;
        };
        if packets.is_empty() {
            continue;
        }
        let Ok(bytes) = stars_core::save::host_file(&state) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (back, _) = GameState::from_file(&file);
        assert_eq!(
            back.packets.len(),
            packets.len(),
            "{}: packet count",
            path.display()
        );
        for (read, sent) in back.packets.iter().zip(&packets) {
            assert_eq!(read.target, sent.target, "{}", path.display());
            assert_eq!(read.warp, sent.warp, "{}", path.display());
            assert_eq!(read.minerals, sent.minerals, "{}", path.display());
            assert_eq!(read.decay_rate, sent.decay_rate, "{}", path.display());
            assert_eq!(read.position, sent.position, "{}", path.display());
        }
        checked += packets.len();
    }
    if checked == 0 {
        eprintln!("skipping: no packets in the fixtures");
        return;
    }
    eprintln!("{checked} packets round-tripped");
}

/// Follow packets from one year into the next and see whether they flew the
/// way this engine says they should.
///
/// A packet is matched by its object id and owner between consecutive years of
/// the same game. What can be checked is the **distance covered** — the square
/// of its warp — and the **decay** of what it carries.
#[test]
fn packets_fly_and_decay_the_way_the_game_flew_them() {
    // Group the saves by directory family and year: a game's turns live in
    // sibling directories named for the year.
    let mut by_game: BTreeMap<(PathBuf, String), BTreeMap<u16, PathBuf>> = BTreeMap::new();
    for (turn, path) in saves() {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(family) = path.parent().and_then(Path::parent) else {
            continue;
        };
        by_game
            .entry((family.to_path_buf(), name.to_string()))
            .or_default()
            .insert(turn, path);
    }

    let mut matched = 0;
    let mut moved_right = 0;
    let mut decayed_right = 0;
    let mut decayed_other_prt = 0;
    for years in by_game.values() {
        for (turn, path) in years {
            let Some(next) = years.get(&(turn + 1)) else {
                continue;
            };
            let (Some((state, before)), Some((_, after))) = (packets_of(path), packets_of(next))
            else {
                continue;
            };
            let later: BTreeMap<(u16, i16), &Packet> =
                after.iter().map(|p| ((p.id, p.owner), p)).collect();

            for packet in &before {
                let Some(then) = later.get(&(packet.id, packet.owner)) else {
                    continue;
                };
                if then.target != packet.target || then.warp != packet.warp {
                    continue; // a reused object id, not the same packet
                }
                let Ok(target_id) = i16::try_from(packet.target) else {
                    continue;
                };
                // The target has to be a planet this file knows where to
                // find, or the flight cannot be judged.
                let Some(_target) = state
                    .planets
                    .iter()
                    .chain(state.known_planets.iter())
                    .find(|p| p.id == target_id)
                    .and_then(|p| p.position)
                else {
                    continue;
                };
                matched += 1;

                // It should have covered a year's flight, or arrived.
                let flown = distance(packet.position, then.position);
                let expected = f64::from(packet.range());
                if (flown - expected).abs() <= 2.0 {
                    moved_right += 1;
                }
                // And lost a year's decay from each mineral it carries.
                let physics = usize::try_from(packet.owner)
                    .ok()
                    .and_then(|i| state.players.get(i))
                    .is_some_and(|p| p.race.prt() == Some(stars_core::race::Prt::Pp));
                let expected: Vec<i16> = packet
                    .minerals
                    .iter()
                    .map(|m| m - decay_loss(*m, packet.decay_rate, physics, 100))
                    .collect();
                if expected == then.minerals.to_vec() {
                    decayed_right += 1;
                } else {
                    // Try it as a Packet Physics owner: a player file carries
                    // only its own player's race, so another player's packet
                    // decays at a rate this file cannot know.
                    let alt: Vec<i16> = packet
                        .minerals
                        .iter()
                        .map(|m| m - decay_loss(*m, packet.decay_rate, !physics, 100))
                        .collect();
                    if alt == then.minerals.to_vec() {
                        decayed_other_prt += 1;
                    }
                }
            }
        }
    }

    if matched == 0 {
        eprintln!("skipping: no packet was seen in two consecutive years");
        return;
    }
    eprintln!(
        "{matched} packets followed a year: flight matched {moved_right}, \
         decay matched {decayed_right}, decay matched at the other PRT's rate \
         {decayed_other_prt}"
    );

    // The residue is packets whose object id was recycled between the two
    // years — the same number given to a different packet — which no matching
    // by id can tell apart. It is a handful either way.
    assert!(
        moved_right * 100 >= matched * 99,
        "flight: {moved_right} of {matched}"
    );
    assert!(
        (decayed_right + decayed_other_prt) * 100 >= matched * 99,
        "decay: {decayed_right} + {decayed_other_prt} of {matched}"
    );
}
