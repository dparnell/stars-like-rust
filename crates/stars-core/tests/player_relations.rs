//! How a player regards the others, against the tables Stars! itself wrote.
//!
//! See `docs/ui/player-relations.md`.

use std::path::{Path, PathBuf};

use stars_core::relations::{self, Relation};
use stars_core::{GameState, Player, Race};
use stars_formats::{StarsFile, Universe};

fn fixture(relative: &str) -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative);
    path.exists().then_some(path)
}

/// A game read from a save and its universe, so the game's own flags are set.
fn load(save: &str, universe: &str) -> Option<GameState> {
    let file = StarsFile::decode(&std::fs::read(fixture(save)?).ok()?).ok()?;
    let (mut state, _) = GameState::from_file(&file);
    if let Some(bytes) = fixture(universe).and_then(|p| std::fs::read(p).ok()) {
        if let Ok(universe) = Universe::decode(&bytes) {
            state.apply_universe(&universe);
        }
    }
    Some(state)
}

/// The three values are the dialog's control ids less `0x7d4`, and the file
/// stores them as they are.
#[test]
fn the_values_are_the_ones_the_file_stores() {
    assert_eq!(Relation::Neutral.value(), 0);
    assert_eq!(Relation::Friend.value(), 1);
    assert_eq!(Relation::Enemy.value(), 2);
    assert_eq!(Relation::default(), Relation::Neutral);
}

/// A player with no table regards everybody as neutral, and always regards
/// themselves as neutral whatever the table says.
#[test]
fn an_absent_table_is_all_neutral() {
    let mut state = GameState::new(1);
    state.players = vec![Player::new(Race::humanoid()), Player::new(Race::humanoid())];
    assert!(state.players[0].relations.is_empty());
    assert_eq!(relations::regard(&state, 0, 1), Relation::Neutral);

    // Even a table that says otherwise about oneself.
    state.players[0].relations = vec![2, 2];
    assert_eq!(relations::regard(&state, 0, 0), Relation::Neutral);
    assert_eq!(relations::regard(&state, 0, 1), Relation::Enemy);
    // And it says nothing about how the other player regards them.
    assert_eq!(relations::regard(&state, 1, 0), Relation::Neutral);

    assert_eq!(relations::others(&state, 0), vec![1]);
    assert_eq!(relations::others(&state, 1), vec![0]);
}

/// The tutorial is the one game in the fixtures with a table actually filled
/// in — and it is also a single-player game, so its own dialog would refuse to
/// open. The table still matters: remote terraforming and the scanner's
/// minefield filters read it whether or not the dialog will edit it.
#[test]
fn the_tutorial_has_a_table_it_cannot_edit() {
    let Some(state) = load("games/tutorial/tutorial.m1", "games/tutorial/tutorial.xy") else {
        return;
    };
    assert!(state.single_player);
    assert!(!relations::can_be_set(&state), "the dialog is refused");
    assert_eq!(
        state.players[0].relations,
        vec![0, 2],
        "neutral toward itself, enemy toward player 1"
    );
}

/// A multi-player game lets relations be set. "Single player" is narrower than
/// "one human": the sixteen-player game whose other fifteen are all computers
/// is not one.
#[test]
fn a_multi_player_game_lets_them_be_set() {
    for (save, universe, players) in [
        (
            "games/no-random-events/2500/Game.m1",
            "games/no-random-events/2500/Game.xy",
            16,
        ),
        (
            "games/all-computer-players/2450/Game.m1",
            "games/all-computer-players/2450/Game.xy",
            15,
        ),
    ] {
        let Some(state) = load(save, universe) else {
            continue;
        };
        assert!(!state.single_player, "{save}");
        assert!(relations::can_be_set(&state), "{save}");
        assert_eq!(state.players.len(), players, "{save}");
        assert_eq!(relations::others(&state, 0).len(), players - 1, "{save}");
        // Neither carries a table, so everybody is neutral — a player block
        // stores the length first and these store zero.
        assert!(state.players[0].relations.is_empty(), "{save}");
    }
}
