//! Differential tests for the typed player-block decoder
//! ([`stars_formats::player`]) against real host files.
//!
//! The layout comes from the stars-4x `starsapi` project (`PlayerBlock.java`);
//! these tests assert the decoded values match the *known* starting state of the
//! sample game so the decoder stays a correctness anchor.

use std::path::Path;

use stars_formats::{player_records, Prt, StarsFile};

/// Load a fixture under `fixtures/`, returning `None` (with a skip note) if it
/// is not present so the suite still passes on a bare checkout.
fn fixture(rel: &str) -> Option<Vec<u8>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel);
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            eprintln!("skipping: {rel} not present");
            None
        }
    }
}

/// The sample game's host file has three players. Player 0 is the all-normal
/// Humanoid race (JOAT); the two AI players are Tritizoid and Golem.
#[test]
fn hst_player_records_decode() {
    let Some(bytes) = fixture("incoming/turn0/Game.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let players = player_records(&file).unwrap();
    assert_eq!(players.len(), 3, "three players");

    // Player numbers are the contiguous 0..=2.
    let nums: Vec<u8> = players.iter().map(|p| p.player_number).collect();
    assert_eq!(nums, vec![0, 1, 2], "player numbers contiguous");

    // Every player in a real host file carries the full race struct.
    for p in &players {
        assert!(p.full_data, "player {} has full data", p.player_number);
        assert!(
            p.race.is_some(),
            "player {} embeds a race record",
            p.player_number
        );
        assert_eq!(
            p.starbase_design_count, 1,
            "each player starts with one starbase design"
        );
    }

    // Player 0: the human Humanoid race.
    let p0 = &players[0];
    assert_eq!(p0.singular_name, "Humanoid");
    assert_eq!(p0.plural_name, "Humanoids");
    assert_eq!(p0.ship_design_count, 6, "Humanoid starting ship designs");
    assert_eq!(p0.race.as_ref().unwrap().prt, Prt::JOAT);

    // The two AI players.
    assert_eq!(players[1].singular_name, "Tritizoid");
    assert_eq!(players[2].singular_name, "Golem");
    for ai in &players[1..] {
        assert_eq!(ai.ship_design_count, 3, "AI starting ship designs");
    }
}

/// The tutorial host file decodes its two players cleanly too.
#[test]
fn tutorial_hst_player_records_decode() {
    let Some(bytes) = fixture("games/tutorial/tutorial.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let players = player_records(&file).unwrap();
    assert_eq!(players.len(), 2, "two players in the tutorial");
    for p in &players {
        assert!(p.full_data);
        assert!(!p.singular_name.is_empty(), "player has a name");
    }
}
