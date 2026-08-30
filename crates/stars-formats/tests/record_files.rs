//! Differential tests for the waypoint, battle-plan, production-queue and
//! score record decoders against real host/player files.
//!
//! The field layouts come from the stars-4x `decompiled` project's `Structures`
//! set (`Structure20`/`Structure30`/`Structure28`/`Structure45`). These tests
//! assert the decoded values match the *known* state of the sample games so the
//! decoders stay correctness anchors.

use std::path::Path;

use stars_formats::{
    battle_plan_records, production_queue_records, score_records, waypoint_records, StarsFile,
};

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

/// In a fresh game every fleet sits on its homeworld, so each waypoint is a
/// "waypoint zero" targeting the orbited planet (object type 17), warp 0,
/// task 0. The three fleets' waypoints share the three homeworld planet ids.
#[test]
fn hst_waypoints_are_homeworld_orbits() {
    let Some(bytes) = fixture("incoming/turn0/Game.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let wps = waypoint_records(&file);
    assert_eq!(wps.len(), 14, "one waypoint per starting fleet");

    for w in &wps {
        assert_eq!(w.task, 0, "starting fleets have no task");
        assert_eq!(w.warp, 0, "stationary fleets have warp 0");
        assert_eq!(w.object_type, 17, "waypoint targets an orbited planet");
        assert!(w.object_id.is_some(), "waypoint has a target planet");
        assert!(w.task_data.is_empty(), "task 0 carries no extra bytes");
    }

    // The distinct target planets are the three homeworlds.
    let mut targets: Vec<u16> = wps.iter().filter_map(|w| w.object_id).collect();
    targets.sort_unstable();
    targets.dedup();
    assert_eq!(targets, vec![32, 69, 112], "the three homeworld planet ids");
}

/// The host file carries every player's default battle plans; player 0's set
/// decodes to the canonical default names.
#[test]
fn hst_battle_plans_decode_default_names() {
    let Some(bytes) = fixture("incoming/turn0/Game.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let plans = battle_plan_records(&file).unwrap();
    assert_eq!(plans.len(), 15, "5 default plans for each of 3 players");

    // Player 0's plans, in file order.
    let p0: Vec<&str> = plans
        .iter()
        .filter(|p| p.race_id == 0)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        p0,
        vec![
            "Default",
            "Kill Starbase",
            "Max-Defense",
            "Sniper",
            "Chicken"
        ],
        "player 0 default battle-plan names"
    );

    // Every player has the same 5 default plans.
    for race in 0..3u8 {
        let count = plans.iter().filter(|p| p.race_id == race).count();
        assert_eq!(count, 5, "player {race} has 5 battle plans");
    }

    // The first plan of each player is "Default" with plan id 0.
    let default = plans.iter().find(|p| p.race_id == 0).unwrap();
    assert_eq!(default.name, "Default");
    assert_eq!(default.plan_id, 0);
}

/// After the first turn the host file has production queues; they decode to
/// runs of auto-build items, the first of which is partly complete.
#[test]
fn hst_production_queues_decode() {
    let Some(bytes) = fixture("incoming/turn1/Game.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let queues = production_queue_records(&file);
    assert!(!queues.is_empty(), "turn-1 host file has production queues");

    for q in &queues {
        assert!(q.planet_id.is_none(), "type-28 queues have no planet id");
        assert!(!q.items.is_empty(), "queue has at least one item");
        // The first auto-build item is the only one that can be partly built.
        for (i, item) in q.items.iter().enumerate() {
            if i > 0 {
                assert_eq!(item.completion, 0, "only the head item is in progress");
            }
        }
    }
}

/// A player file carries that player's own score row; on turn 1 it is player 0
/// at rank 1 with a single planet and starbase.
#[test]
fn m1_score_decodes() {
    let Some(bytes) = fixture("incoming/turn1/Game.m1") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let scores = score_records(&file);
    assert_eq!(scores.len(), 1, "player file has one score row");
    let s = scores[0];
    assert_eq!(s.player_id, 0);
    assert_eq!(s.rank, 1);
    assert_eq!(s.planets, 1);
    assert_eq!(s.starbases, 1);
    assert!(s.tech_levels > 0, "starting tech levels present");
    // No victory conditions are met on turn 1.
    assert_eq!(s.victory, Default::default());
}

/// The tutorial history file carries several score rows (one per turn); they
/// all decode without error and report a valid player id.
#[test]
fn tutorial_history_scores_decode() {
    let Some(bytes) = fixture("games/tutorial/tutorial.h1") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let scores = score_records(&file);
    assert!(!scores.is_empty(), "history file has score rows");
    for s in &scores {
        assert!(s.player_id < 16, "player id in range");
    }
}
