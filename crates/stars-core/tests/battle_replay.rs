//! The combat engine checked against real battle recordings.
//!
//! `stars-formats` decodes the 47 battles the Exodus game recorded. Those
//! recordings contain both the starting state of every token and the actions
//! that followed, so the parts of combat implemented so far — where tokens
//! start, and how far they may move each round — can be checked against what
//! the original engine actually did rather than against our own expectations.
//!
//! Fixtures are optional: each test skips when the sample game is absent.

use std::path::{Path, PathBuf};

use stars_core::battle::{distance, movement_this_round, start_square, Square};
use stars_formats::{battle_records_in, BattleRecord, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

fn exodus_battles() -> Vec<(i32, BattleRecord)> {
    let games = workspace_root().join("fixtures/games/exodus");
    if !games.is_dir() {
        return Vec::new();
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let mut out = Vec::new();
    for year in years {
        let path = games.join(year.to_string()).join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        for record in battle_records_in(file.segment_blocks(file.latest_segment())) {
            out.push((year, record));
        }
    }
    out
}

#[test]
fn tokens_start_on_the_squares_the_table_says() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }

    // Every participant is assigned a starting square by player count. All of
    // these battles are two-sided, so the two sides must be at (1,4) and (8,5).
    let mut checked = 0;
    for (year, b) in &battles {
        let participants = b.participants();
        let expected: Vec<Square> = (0..participants.len())
            .filter_map(|side| start_square(b.players, u8::try_from(side).ok()?))
            .collect();
        assert_eq!(
            expected.len(),
            participants.len(),
            "{year} battle {:#06x}: no start layout for {} players",
            b.id,
            b.players
        );

        for token in &b.tokens {
            let square = Square::new(token.square.x, token.square.y);
            assert!(
                expected.contains(&square),
                "{year} battle {:#06x}: token of player {} starts at ({},{}), \
                 not one of the {} starting squares {expected:?}",
                b.id,
                token.player,
                square.x,
                square.y,
                b.players
            );
            checked += 1;
        }

        // Each player must be on exactly one starting square, and different
        // players on different ones.
        for player in &participants {
            let squares: Vec<Square> = b
                .tokens
                .iter()
                .filter(|t| t.player == *player)
                .map(|t| Square::new(t.square.x, t.square.y))
                .collect();
            if let Some(first) = squares.first() {
                assert!(
                    squares.iter().all(|s| s == first),
                    "{year} battle {:#06x}: player {player} starts spread over {squares:?}",
                    b.id
                );
            }
        }
    }
    eprintln!("{checked} token starting positions matched the table");
    assert!(checked > 0);
}

#[test]
fn two_sided_battles_use_the_two_player_layout() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    // The table's two-player layout, straight from the binary.
    assert_eq!(start_square(2, 0), Some(Square::new(1, 4)));
    assert_eq!(start_square(2, 1), Some(Square::new(8, 5)));

    for (year, b) in battles.iter().filter(|(_, b)| b.players == 2) {
        let mut squares: Vec<Square> = b
            .tokens
            .iter()
            .map(|t| Square::new(t.square.x, t.square.y))
            .collect();
        squares.sort_by_key(|s| (s.x, s.y));
        squares.dedup();
        assert!(
            squares.len() <= 2,
            "{year} battle {:#06x}: {} distinct starting squares",
            b.id,
            squares.len()
        );
        for s in squares {
            assert!(
                s == Square::new(1, 4) || s == Square::new(8, 5),
                "{year} battle {:#06x}: unexpected starting square ({},{})",
                b.id,
                s.x,
                s.y
            );
        }
    }
}

#[test]
fn recorded_moves_never_exceed_the_movement_allowance() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }

    // Actions are recorded in order, so a token's successive destinations trace
    // its path. Within one round a token may be moved several times (the
    // original moves everything a square at a time across three phases), so the
    // check is that the whole round's travel fits the allowance for its speed.
    let mut checked = 0;
    let mut moves = 0;
    let mut departed = 0;
    for (year, b) in &battles {
        // Where each token currently is, and how far it has gone this round.
        let mut position: Vec<Square> = b
            .tokens
            .iter()
            .map(|t| Square::new(t.square.x, t.square.y))
            .collect();
        let mut travelled = vec![0u32; b.tokens.len()];
        let mut round = 0u8;

        for action in &b.actions {
            if action.round != round {
                round = action.round;
                travelled.iter_mut().for_each(|t| *t = 0);
            }
            let index = usize::from(action.token);
            let Some(from) = position.get(index).copied() else {
                continue;
            };
            // A token that disengages has no destination square; it is simply
            // gone, and nothing after this concerns it.
            let Some(dest) = action.destination else {
                departed += 1;
                continue;
            };
            let to = Square::new(dest.x, dest.y);
            let step = u32::from(distance(from, to));
            if step > 0 {
                moves += 1;
            }
            travelled[index] += step;
            position[index] = to;

            let speed = b.tokens[index].speed();
            let allowance = u32::from(movement_this_round(speed, action.round));
            assert!(
                travelled[index] <= allowance,
                "{year} battle {:#06x}: token {} (speed {speed}) travelled {} squares \
                 in round {}, allowance {allowance}",
                b.id,
                action.token,
                travelled[index],
                action.round
            );
            assert!(
                to.on_board(),
                "{year} battle {:#06x}: moved off the board",
                b.id
            );
            checked += 1;
        }
    }
    eprintln!(
        "{checked} recorded actions checked, {moves} of them actual moves, \
         {departed} tokens left the battle"
    );
    assert!(
        moves > 100,
        "expected a decent sample of moves, got {moves}"
    );
    assert!(departed > 0, "some token should have disengaged");
}

#[test]
fn firing_happens_within_the_recorded_range() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    // A firing record carries the range it fired at. Beam weapons reach at most
    // three squares and torpedoes four, so nothing should fire beyond that.
    let mut shots = 0;
    for (year, b) in &battles {
        for action in b.actions.iter().filter(|a| !a.kills.is_empty()) {
            assert!(
                action.range <= 4,
                "{year} battle {:#06x}: fired at range {}",
                b.id,
                action.range
            );
            shots += 1;
        }
    }
    eprintln!("{shots} firing actions checked");
    assert!(shots > 0);
}
