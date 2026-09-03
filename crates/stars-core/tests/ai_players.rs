//! Who controls each player, read from real saved games.
//!
//! Stars! records the computer opponents in the player block's flags byte,
//! which is the high half of the mode word `DoAiTurn` (`1088:0000`) switches
//! on. These tests pin that decoding to the games in `fixtures/`.

use std::path::{Path, PathBuf};

use stars_core::ai::{AiPersonality, Control};
use stars_core::GameState;
use stars_formats::StarsFile;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

fn load(path: &Path) -> GameState {
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let file = StarsFile::decode(&bytes)
        .unwrap_or_else(|e| panic!("cannot decode {}: {e}", path.display()));
    GameState::from_file(&file).0
}

/// The three-player `incoming` game is one person and two computer opponents.
///
/// Player 0 (Humanoid) carries flags `0x01`; players 1 and 2 (Tritizoid and
/// Golem) carry `0x27`, which is bit 1 set for "computer" and `0x27 >> 5 == 1`
/// for Turindrone.
#[test]
fn incoming_game_has_two_turindrone_opponents() {
    let root = workspace_root();
    for turn in ["turn0", "turn1"] {
        let path = root.join(format!("fixtures/incoming/{turn}/Game.hst"));
        if !path.is_file() {
            eprintln!("skipping: {} absent", path.display());
            return;
        }
        let state = load(&path);
        let controls: Vec<Control> = state.players.iter().map(|p| p.control).collect();

        assert_eq!(controls[0], Control::Human, "{turn}: player 0");
        for i in [1, 2] {
            assert_eq!(
                controls[i],
                Control::Computer {
                    personality: Some(AiPersonality::TurinDrone),
                    skill_bits: 1,
                },
                "{turn}: player {i}"
            );
        }
    }
}

/// The Exodus race files are human races, not computer opponents.
///
/// This is worth pinning because their names — `OFFENDER`, `DEFENDER`,
/// `SNEAK` and so on — read like AI archetypes. They are not: every one has a
/// zero flags byte, so a computer opponent cannot be mistaken for one of them.
#[test]
fn exodus_race_files_are_human_races() {
    let races = workspace_root().join("fixtures/games/exodus/Races");
    if !races.is_dir() {
        eprintln!("skipping: {} absent", races.display());
        return;
    }
    let mut checked = 0;
    for entry in std::fs::read_dir(&races).unwrap() {
        let path = entry.unwrap().path();
        let state = load(&path);
        for player in &state.players {
            assert_eq!(player.control, Control::Human, "{}", path.display());
        }
        checked += 1;
    }
    assert_eq!(checked, 8, "expected eight Exodus race files");
}

/// Every personality `DoAiTurn` dispatches to has a distinct name, and the one
/// mode value its jump table omits runs no AI.
#[test]
fn personalities_cover_the_dispatch_table() {
    let named: Vec<_> = (0..8)
        .map(|m| AiPersonality::from_mode(m << 13).map(AiPersonality::name))
        .collect();
    assert_eq!(
        named,
        vec![
            Some("Robotoid"),
            Some("Turindrone"),
            Some("Automitron"),
            Some("Rototill"),
            Some("Cyber"),
            Some("Macinti"),
            None,
            Some("Maid"),
        ]
    );
}
