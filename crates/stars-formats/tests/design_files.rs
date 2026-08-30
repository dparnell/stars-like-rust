//! Differential tests for the typed design-block decoder
//! ([`stars_formats::design`]) against real host files.
//!
//! The layout comes from the stars-4x `starsapi` project (`DesignBlock.java`);
//! these tests assert the decoded values match the *known* starting designs of
//! the sample game so the decoder stays a correctness anchor.

use std::path::Path;

use stars_formats::{design_records, StarsFile};

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

/// The sample game's host file has 15 starting designs: 6 + 3 + 3 ship designs
/// for the three players plus one starbase design each. They decode to the
/// canonical hull/name/armor values.
#[test]
fn hst_design_records_decode() {
    let Some(bytes) = fixture("incoming/turn0/Game.hst") else {
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();
    let designs = design_records(&file).unwrap();
    assert_eq!(designs.len(), 15, "total starting designs");

    // Every starting design is a full design (internals present).
    for d in &designs {
        assert!(d.full_design, "design '{}' is a full design", d.name);
        assert!(!d.transferred, "starting designs are not transferred");
        assert!(d.armor.is_some(), "full design carries armor");
        assert!(d.total_built.is_some(), "full design carries a built count");
        assert_eq!(d.turn_designed, Some(1), "starting designs made on turn 1");
    }

    // Player 0's Humanoid starting fleet includes these canonical hulls/names.
    let by_name = |n: &str| designs.iter().find(|d| d.name == n);
    let probe = by_name("Armed Probe").expect("Armed Probe present");
    assert_eq!(probe.hull_id, 4, "Armed Probe is a Scout hull");
    assert_eq!(probe.slots.len(), 3, "Scout hull has 3 slots");
    assert!(!probe.starbase);

    let santa = by_name("Santa Maria").expect("Santa Maria present");
    assert_eq!(santa.hull_id, 15, "Santa Maria is a Colony Ship hull");

    // Exactly three starbase designs, all hull 34 with 1000 armor, 12 slots.
    let starbases: Vec<_> = designs.iter().filter(|d| d.starbase).collect();
    assert_eq!(starbases.len(), 3, "one starbase design per player");
    for sb in &starbases {
        assert_eq!(sb.hull_id, 34, "starbase hull");
        assert_eq!(sb.armor, Some(1000), "starbase armor");
        assert_eq!(sb.slots.len(), 12, "starbase slot count");
        assert_eq!(sb.name, "Starbase");
    }
}
