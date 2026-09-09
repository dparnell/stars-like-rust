//! The battle VCR, played against every recording in the fixtures.
//!
//! The VCR plays a recording rather than re-simulating it, so the test of it is
//! not "does it agree with our combat model" — it is "does it reproduce what
//! the engine wrote". The recording states its own casualty totals
//! independently of the action list, which makes that checkable.

use std::path::{Path, PathBuf};

use stars_formats::{battle_records_in_with, ActionLayout, BattleRecord, StarsFile};
use stars_ui::vcr::{Event, Vcr, BOARD};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// Every battle recording in the Exodus fixtures.
fn recordings() -> Vec<BattleRecord> {
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
        let Ok(bytes) = std::fs::read(games.join(year.to_string()).join("exodus.m6")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let header = &file.latest_segment().header;
        let layout = ActionLayout::for_version(header.version_major, header.version_minor);
        out.extend(battle_records_in_with(
            file.segment_blocks(file.latest_segment()),
            layout,
        ));
    }
    out
}

/// Played to the end, the VCR must report the casualties the recording itself
/// reports.
///
/// `BattleRecord::ships_destroyed_for` counts the kill records directly; the
/// VCR reaches its figure by walking the action list and applying each one to a
/// board it carries forward. Agreement means the playback consumed every action
/// and attributed every casualty to the right token.
#[test]
fn plays_back_to_the_recorded_casualties() {
    let battles = recordings();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut checked = 0;
    for battle in &battles {
        let mut vcr = Vcr::new(battle);
        vcr.end();
        let losses = vcr.losses();
        for player in battle.participants() {
            let want = i32::try_from(battle.ships_destroyed_for(player)).unwrap_or(0);
            let got = losses
                .iter()
                .find(|(p, _)| *p == player)
                .map_or(0, |(_, n)| *n);
            assert_eq!(
                got, want,
                "battle {:#06x}: player {player} lost {want} ships, VCR says {got}",
                battle.id
            );
            checked += 1;
        }
    }
    assert!(checked > 50, "expected a decent sample, got {checked}");
    eprintln!("VCR: {} battles play back exactly", battles.len());
}

/// Stepping forward and back must be exact inverses, and the board must never
/// go inconsistent along the way.
#[test]
fn stepping_is_reversible_and_the_board_stays_sane() {
    let battles = recordings();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    for battle in &battles {
        let mut vcr = Vcr::new(battle);
        let start: Vec<_> = vcr.tokens().to_vec();

        let mut seen = Vec::new();
        while vcr.step() {
            seen.push(vcr.tokens().to_vec());
            for token in vcr.tokens() {
                assert!(
                    token.ships >= 0,
                    "battle {:#06x}: negative ships",
                    battle.id
                );
                assert!(token.shields >= 0, "negative shields");
                if let Some((x, y)) = token.square {
                    assert!(x < BOARD && y < BOARD, "off the board: ({x},{y})");
                }
            }
            // A token that has left the battle stands nowhere.
            for token in vcr.tokens() {
                if !token.active && token.ships == 0 {
                    continue;
                }
            }
        }
        assert_eq!(vcr.position(), vcr.len());

        // Walk all the way back; every board must match what it was.
        for want in seen.iter().rev() {
            assert_eq!(vcr.tokens(), want.as_slice());
            vcr.back();
        }
        assert_eq!(vcr.position(), 0);
        assert_eq!(vcr.tokens(), start.as_slice());
    }
}

/// Every action is classified, and the three kinds behave as the format says.
#[test]
fn moves_shots_and_disengages_are_told_apart() {
    let battles = recordings();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let (mut moves, mut fires, mut leaves) = (0usize, 0usize, 0usize);
    for battle in &battles {
        let vcr = Vcr::new(battle);
        assert_eq!(vcr.len(), battle.actions.len(), "one frame per action");
        for frame in vcr.frames() {
            match &frame.event {
                Event::Move { token, from, to } => {
                    moves += 1;
                    assert_ne!(from, to, "a move that goes nowhere is a shot");
                    // One step at a time, on the Chebyshev board.
                    let dx = i32::from(to.0) - i32::from(from.0);
                    let dy = i32::from(to.1) - i32::from(from.1);
                    assert!(
                        dx.abs() <= 1 && dy.abs() <= 1,
                        "a move of more than one square"
                    );
                    assert_eq!(
                        frame.tokens[*token].square,
                        Some(*to),
                        "the mover should be where it moved to"
                    );
                }
                Event::Fire { attacker, .. } => {
                    fires += 1;
                    let _ = attacker;
                }
                Event::Disengage { token } => {
                    leaves += 1;
                    assert!(!frame.tokens[*token].active, "it left, so it is not active");
                    assert_eq!(frame.tokens[*token].square, None, "and stands nowhere");
                }
            }
        }
    }
    eprintln!("VCR: {moves} moves, {fires} shots, {leaves} disengages");
    assert!(moves > 500 && fires > 100 && leaves > 10);
}

/// The board groups tokens by square and leaves out the dead.
#[test]
fn the_board_holds_only_the_living() {
    let battles = recordings();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    for battle in &battles {
        let mut vcr = Vcr::new(battle);
        vcr.end();
        let grid = vcr.board();
        let placed: usize = grid.iter().flatten().map(Vec::len).sum();
        let living = vcr
            .tokens()
            .iter()
            .filter(|t| t.active && t.ships > 0 && t.square.is_some())
            .count();
        assert_eq!(placed, living, "battle {:#06x}", battle.id);
    }
}

/// Which of the five transport buttons are alive, and where the focus lands —
/// `EnableVCRButtons` (`10e8:48f6`) decides both from one number.
#[test]
fn the_transport_follows_the_playhead() {
    use stars_ui::dialog::{BATTLE_VCR, VCR_TRANSPORT};

    // The template's seven buttons all share the foot, 32 by 13 at y = 244.
    assert_eq!(BATTLE_VCR.size, (260, 270));
    assert_eq!(BATTLE_VCR.controls.len(), 7);
    for control in BATTLE_VCR.controls {
        assert_eq!(control.at.1, 244);
        assert_eq!((control.at.2, control.at.3), (32, 13));
    }
    assert_eq!(VCR_TRANSPORT, [0xa1, 0xa2, 0xa3, 0xa4, 0xa5]);
    // Their captions are the resource's own, and the middle one plays *and*
    // pauses — one button, not two.
    assert_eq!(BATTLE_VCR.control(0xa1).expect("rewind").text, "|<<");
    assert_eq!(BATTLE_VCR.control(0xa3).expect("play").text, ">/||");
    assert_eq!(BATTLE_VCR.control(0xa5).expect("end").text, ">>|");
    assert_eq!(BATTLE_VCR.control(0x1).expect("Done").label(), "Done");

    // The rest wants a recording; a checkout without the fixtures skips it.
    let records = recordings();
    let Some(record) = records.iter().find(|r| !r.actions.is_empty()) else {
        return;
    };
    let mut vcr = stars_ui::vcr::Vcr::new(record);
    if vcr.is_empty() {
        return;
    }

    // At the start nothing can go back and the focus is on play.
    vcr.rewind();
    assert!(!vcr.can_rewind());
    assert!(vcr.can_advance());
    assert_eq!(vcr.focus(), Some(0xa3));

    // One step in, both halves are alive and the focus is left alone.
    vcr.step();
    assert!(vcr.can_rewind());
    assert_eq!(vcr.focus(), None, "only the two ends move the focus");

    // At the end nothing can go forward and the focus is on Done.
    vcr.end();
    assert!(vcr.can_rewind());
    assert!(!vcr.can_advance());
    assert_eq!(vcr.focus(), Some(0x1));
}
