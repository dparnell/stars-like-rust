//! Player scores, checked against the scoreboards Stars! itself wrote.
//!
//! Every `.mN` carries its own player's score row and every `.hN` one row per
//! recorded turn, so a computed score can be held against the real thing rather
//! than only against a transcription. See `docs/formulas/scores.md`.

use std::path::{Path, PathBuf};

use stars_core::score::{pack, scores, unpack};
use stars_core::GameState;
use stars_formats::StarsFile;

/// Every `.mN` under `fixtures/` that has a universe file beside it.
fn player_files() -> Vec<PathBuf> {
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
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.starts_with('m') && e.len() == 2)
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The packing a score block uses for its ship counts round-trips small
/// numbers exactly and large ones roughly, which is what it is for.
#[test]
fn packed_counts_round_trip() {
    for value in [0, 1, 7, 100, 8_191] {
        assert_eq!(unpack(pack(value)), value, "{value} fits in thirteen bits");
    }
    // Past 8,191 the value is halved twice per step, so it comes back rounded.
    for value in [8_192, 20_000, 1_000_000] {
        let back = unpack(pack(value));
        assert!(back <= value, "{value} -> {back}");
        assert!(
            i64::from(value - back) * 100 / i64::from(value) < 5,
            "{value} -> {back}: within a few percent"
        );
    }
}

/// The counts on a real scoreboard — planets, starbases, tech levels — are what
/// this engine counts from the same game.
///
/// The score itself is compared where the file gives us enough to compute it;
/// a `.mN` describes only its own player fully, so only that player's row can
/// be checked.
#[test]
fn real_scoreboards_agree_on_what_they_count() {
    let mut checked = 0;
    let mut planets_ok = 0;
    let mut tech_ok = 0;
    let mut starbases_ok = 0;
    let mut score_ok = 0;
    let mut score_seen = 0;

    for path in player_files() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        // A player file can hold two turns; the state comes from the latest
        // segment, so the scoreboard has to come from the same one.
        let segment = file.latest_segment();
        let rows: Vec<stars_formats::ScoreRecord> = file
            .segment_blocks(segment)
            .iter()
            .filter(|b| b.type_id == 45)
            .filter_map(|b| stars_formats::ScoreRecord::decode(&b.data))
            .collect();
        if rows.is_empty() {
            continue;
        }
        // A player file's scoreboard carries a row for everybody, but the file
        // only describes its **own** player's planets and fleets; the rest it
        // sees at a distance. So only the owner's row can be recomputed.
        let owner = usize::from(segment.header.player);
        let (state, _) = GameState::from_file(&file);
        let computed = scores(&state);

        for row in rows {
            let player = usize::from(row.player_id);
            if player != owner {
                continue;
            }
            let Some(mine) = computed.get(player) else {
                continue;
            };
            // A player the file marks dead is skipped, and the reason is worth
            // stating: `UpdatePlayerScores` computes the score *first* and then
            // marks a player with nothing left as dead, so the flag in the file
            // postdates the row beside it by one turn. Their last row still
            // counts the tech levels this engine would refuse to count.
            if state.players.get(player).is_some_and(|p| p.dead) {
                continue;
            }
            checked += 1;
            planets_ok += usize::from(mine.planets == i32::from(row.planets));
            tech_ok += usize::from(mine.tech_levels == i32::from(row.tech_levels));
            starbases_ok += usize::from(mine.starbases == i32::from(row.starbases));
            if row.score > 0 {
                score_seen += 1;
                score_ok += usize::from(i64::from(mine.score) == i64::from(row.score));
            }
        }
    }

    if checked == 0 {
        eprintln!("skipping: no score rows in the fixtures");
        return;
    }
    eprintln!(
        "{checked} rows: planets {planets_ok}, starbases {starbases_ok}, \
         tech {tech_ok}, score exact {score_ok}/{score_seen}"
    );
    assert_eq!(planets_ok, checked, "planet counts");
    assert_eq!(starbases_ok, checked, "starbase counts");
    assert_eq!(tech_ok, checked, "tech levels");
    assert_eq!(score_ok, score_seen, "the score itself");
}

/// The universe file beside a save, or one directory up, which is where the
/// game's settings live.
fn universe_beside(path: &Path) -> Option<stars_formats::Universe> {
    let mut here = path.parent()?.to_path_buf();
    for _ in 0..2 {
        if let Ok(entries) = std::fs::read_dir(&here) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                if candidate.extension().and_then(|e| e.to_str()) == Some("xy") {
                    if let Ok(bytes) = std::fs::read(&candidate) {
                        if let Ok(universe) = stars_formats::Universe::decode(&bytes) {
                            return Some(universe);
                        }
                    }
                }
            }
        }
        here = here.parent()?.to_path_buf();
    }
    None
}

/// The victory flags a real scoreboard carries are the ones this engine works
/// out from the same game.
///
/// Only five of the seven can be checked here. "Exceeds second place" and
/// "holds the highest score" are about the **whole galaxy**, and a player file
/// describes one player: everybody else's score computes as zero, which would
/// make its owner the runaway leader of every game. No `.hst` in the fixtures
/// carries a scoreboard — scores live in the player and history files — so
/// there is nothing to check those two against, and the test says so rather
/// than pretending.
///
/// Most rows have no conditions met at all, the games in the fixtures being
/// young, so this is as much a check that nothing is claimed falsely as that a
/// met condition is spotted.
#[test]
fn victory_flags_match_the_scoreboards() {
    let mut checked = 0;
    let mut agreed = 0;
    let mut set_ours = 0;
    let mut set_theirs = 0;

    for path in player_files() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let segment = file.latest_segment();
        let rows: Vec<stars_formats::ScoreRecord> = file
            .segment_blocks(segment)
            .iter()
            .filter(|b| b.type_id == 45)
            .filter_map(|b| stars_formats::ScoreRecord::decode(&b.data))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let owner = usize::from(segment.header.player);
        let (mut state, _) = GameState::from_file(&file);
        // The victory settings live in the universe file, not in the save, so
        // there is nothing to check without it.
        let Some(universe) = universe_beside(&path) else {
            continue;
        };
        state.apply_universe(&universe);
        // A game whose conditions are all switched off cannot be checked
        // either. The original still sets the flags — against the defaults its
        // sliders sit at — so a mismatch there would say the universe file
        // beside these turns is not the one they were played with, which is
        // exactly what the exodus fixtures look like: every condition
        // inactive, yet their scoreboards show conditions met.
        if (0..8).all(|c| state.victory[c] & 0x80 == 0) {
            continue;
        }
        let computed = stars_core::score::scores(&state);
        // `leader` is passed false: from a player file nobody can be shown to
        // lead, so the two galaxy-wide conditions are left out of the
        // comparison above.
        let flags: Vec<stars_core::victory::Met> = computed
            .iter()
            .enumerate()
            .map(|(player, score)| stars_core::victory::met(&state, player, score, false, 0))
            .collect();

        for row in rows {
            let player = usize::from(row.player_id);
            if player != owner || state.players.get(player).is_some_and(|p| p.dead) {
                continue;
            }
            let Some(ours) = flags.get(player) else {
                continue;
            };
            let theirs = &row.victory;
            let mine = [
                ours.bits & (1 << 6) != 0,
                ours.bits & (1 << 7) != 0,
                ours.bits & (1 << 8) != 0,
                ours.bits & (1 << 10) != 0,
                ours.bits & (1 << 11) != 0,
            ];
            let then = [
                theirs.owns_planets,
                theirs.attains_tech,
                theirs.exceeds_score,
                theirs.production_capacity,
                theirs.capital_ships,
            ];
            checked += 1;
            agreed += usize::from(mine == then);
            set_ours += mine.iter().filter(|b| **b).count();
            set_theirs += then.iter().filter(|b| **b).count();
        }
    }

    if checked == 0 {
        eprintln!("skipping: no score rows in the fixtures");
        return;
    }
    eprintln!(
        "{checked} rows: victory flags agreed {agreed}, \
         conditions met ours {set_ours} theirs {set_theirs}"
    );
    assert_eq!(agreed, checked, "victory flags");
}
