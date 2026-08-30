//! Differential tests for the typed planet-block decoder
//! ([`stars_formats::planet`]) against real host files.
//!
//! The field layout comes from TotalHost's `StarsPlanet.pl`; these tests assert
//! the decoded values match the *known* starting state of the sample games
//! (homeworld population, installations, environment) so the decoder stays a
//! correctness anchor.

use std::path::Path;

use stars_formats::{planet_records, PlanetRecord, StarsFile};

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

/// The sample game's host file decodes to 128 planets, of which exactly three
/// are inhabited homeworlds, each with the canonical Stars! starting state:
/// 25,000 colonists, 10 mines, 10 factories, 10 defenses, a starbase, and a
/// population estimate.
#[test]
fn hst_planet_records_decode_homeworlds() {
    let Some(bytes) = fixture("incoming/turn0/Game.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let planets = planet_records(&file);
    assert_eq!(planets.len(), 128, "planet count");

    // Ids are the contiguous 0..=127.
    let mut ids: Vec<u16> = planets.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, (0..128).collect::<Vec<_>>(), "planet ids contiguous");

    let owned: Vec<&PlanetRecord> = planets.iter().filter(|p| p.owner.is_some()).collect();
    assert_eq!(owned.len(), 3, "three owned planets");

    // Every owned planet in a fresh game is a homeworld with a starbase and
    // the canonical starting installations and population.
    let mut owners: Vec<u8> = owned.iter().map(|p| p.owner.unwrap()).collect();
    owners.sort_unstable();
    assert_eq!(owners, vec![0, 1, 2], "owners are players 0,1,2");

    for p in &owned {
        assert!(p.homeworld, "planet {} should be a homeworld", p.id);
        assert!(p.has_starbase, "planet {} should have a starbase", p.id);
        assert_eq!(p.detail, 7, "homeworld is fully visible in the host file");
        assert_eq!(p.population, Some(25_000), "homeworld starting population");

        let inst = p.installations.expect("homeworld has installations");
        assert_eq!(inst.mines, 10, "homeworld starting mines");
        assert_eq!(inst.factories, 10, "homeworld starting factories");
        assert_eq!(inst.defenses, 10, "homeworld starting defenses");

        // Surface minerals and concentrations are present and in range.
        let surf = p.surface_minerals.expect("homeworld has surface minerals");
        assert!(
            surf.ironium > 0 && surf.germanium > 0,
            "surface minerals set"
        );
        let conc = p.concentration.expect("homeworld has concentrations");
        assert!(conc.ironium > 0, "ironium concentration set");

        // Every owned planet carries a starbase design and a pop estimate.
        assert!(p.starbase.is_some(), "starbase details present");
        assert!(p.pop_guess.is_some(), "owner population estimate present");
    }

    // Player 0 in the sample game is the all-normal Humanoid race, whose
    // homeworld reads the perfectly-centred environment 50/50/50.
    let hw0 = owned.iter().find(|p| p.owner == Some(0)).unwrap();
    let env = hw0.environment.expect("environment present");
    assert_eq!(
        (env.gravity, env.temperature, env.radiation),
        (50, 50, 50),
        "Humanoid homeworld environment is centred"
    );
}

/// The tutorial host file decodes cleanly too: 24 planets, two inhabited
/// homeworlds, and no decode failures.
#[test]
fn tutorial_hst_planet_records_decode() {
    let Some(bytes) = fixture("games/tutorial/tutorial.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let planets = planet_records(&file);
    assert_eq!(planets.len(), 24, "planet count");

    let homeworlds: Vec<&PlanetRecord> = planets.iter().filter(|p| p.homeworld).collect();
    assert_eq!(homeworlds.len(), 2, "two homeworlds in the tutorial");
    for p in &homeworlds {
        assert!(p.owner.is_some(), "homeworld is owned");
        assert!(p.population.unwrap_or(0) > 0, "homeworld has population");
    }
}
