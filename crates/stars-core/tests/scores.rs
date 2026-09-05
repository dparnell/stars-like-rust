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
