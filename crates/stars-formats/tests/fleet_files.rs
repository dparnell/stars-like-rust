//! Differential tests for the typed fleet-block decoder
//! ([`stars_formats::fleet`]) against real host files.
//!
//! The layout comes from TotalHost's `StarsFleet.pl`; these tests assert the
//! decoded starting fleets match the known fresh-game state (each fleet orbits
//! its owner's homeworld and carries a single ship with fuel), cross-referenced
//! against the planet decoder.

use std::collections::HashMap;
use std::path::Path;

use stars_formats::{fleet_records, planet_records, StarsFile};

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

/// The sample game's host file decodes to 14 starting fleets: 6 for player 0 and
/// 4 each for players 1 and 2. Every fleet is a full record (detail 7) orbiting
/// its owner's homeworld with exactly one ship and a fuel supply.
#[test]
fn hst_fleet_records_decode_starting_fleets() {
    let Some(bytes) = fixture("incoming/turn0/Game.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();

    // Map each owner to its homeworld planet id via the planet decoder.
    let homeworld: HashMap<u8, u16> = planet_records(&file)
        .into_iter()
        .filter(|p| p.homeworld)
        .filter_map(|p| p.owner.map(|o| (o, p.id)))
        .collect();
    assert_eq!(homeworld.len(), 3, "three homeworlds");

    let fleets = fleet_records(&file);
    assert_eq!(fleets.len(), 14, "total starting fleets");

    // Per-player fleet counts.
    let mut per_owner: HashMap<u8, usize> = HashMap::new();
    for f in &fleets {
        *per_owner.entry(f.owner).or_default() += 1;
    }
    assert_eq!(per_owner.get(&0), Some(&6), "player 0 fleet count");
    assert_eq!(per_owner.get(&1), Some(&4), "player 1 fleet count");
    assert_eq!(per_owner.get(&2), Some(&4), "player 2 fleet count");

    for f in &fleets {
        assert_eq!(f.detail, 7, "starting fleets are full records");
        // Orbiting the owner's homeworld.
        assert_eq!(
            f.orbiting,
            homeworld.get(&f.owner).copied(),
            "fleet {} of player {} should orbit its homeworld",
            f.id,
            f.owner
        );
        // Exactly one ship of some design, count 1.
        assert_eq!(f.ships.len(), 1, "one ship stack per starting fleet");
        assert_eq!(f.ships[0].count, 1, "one ship in the stack");
        // Full fleets have a battle plan and a cargo hold with fuel.
        assert!(f.battle_plan.is_some(), "battle plan present");
        assert_eq!(f.waypoint_count, Some(1), "one waypoint (stay put)");
        let cargo = f.cargo.expect("full fleet has a cargo hold");
        assert!(cargo.fuel > 0, "starting ship has fuel");
        assert!(f.damage.is_empty(), "undamaged");
        assert!(!f.dead, "alive");
    }
}
