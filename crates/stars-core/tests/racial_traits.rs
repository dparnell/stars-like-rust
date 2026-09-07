//! The fourteen lesser racial traits, and which bit is which.
//!
//! See `docs/formats/race-r.md`.

/// The fourteen lesser racial traits, all named.
///
/// The names sit at fourteen consecutive string ids, `0x0132` to `0x013f`, one
/// per bit — which is what fixes **bit 5 as Ultimate Recycling**, a trait this
/// project's table had no name for at all until the race viewer had to list
/// every one of them.
#[test]
fn every_lesser_racial_trait_is_named() {
    use stars_core::race::lrt;

    assert_eq!(lrt::ALL.len(), 14, "the wizard's page has fourteen boxes");
    for (index, bit) in lrt::ALL.iter().enumerate() {
        assert_eq!(*bit as usize, index, "bit order, with no gaps");
        assert!(lrt::name(*bit).is_some(), "bit {bit} is named");
    }
    assert_eq!(
        lrt::name(lrt::ULTIMATE_RECYCLING),
        Some("Ultimate Recycling")
    );
    assert_eq!(lrt::name(lrt::IFE), Some("Improved Fuel Efficiency"));
    assert_eq!(
        lrt::name(lrt::REGENERATING_SHIELDS),
        Some("Regenerating Shields")
    );

    // The two flags that share the word but are not among the fourteen: they
    // belong to other pages of the wizard.
    assert!(!lrt::ALL.contains(&lrt::TECH3));
    assert!(!lrt::ALL.contains(&lrt::CHEAP_FACT));
    assert_eq!(lrt::name(lrt::TECH3), None);
    assert_eq!(lrt::name(lrt::CHEAP_FACT), None);
}

/// The stock AI races decode to the traits the corrected table gives them.
///
/// The bit patterns are data read out of the shipped `.r` files and recorded
/// in `docs/formats/race-r.md`; what changed is only how they are *read*.
/// Six of the seven share one paid-for advantage — IFE — funded by three
/// disadvantages that give points back, which is what a stock race looks like.
/// The old ordering gave every one of them Regenerating Shields, an expensive
/// advantage, and still looked "sensible" — which is why the argument from
/// plausibility that had confirmed it was worth nothing.
#[test]
fn the_stock_races_decode_sensibly() {
    use stars_core::race::lrt;

    let traits = |bits: u32| -> Vec<&'static str> {
        lrt::ALL
            .iter()
            .filter(|bit| bits & (1 << **bit) != 0)
            .map(|bit| lrt::name(*bit).expect("named"))
            .collect()
    };

    let base = vec![
        "Improved Fuel Efficiency",
        "No Ramscoop Engines",
        "Only Basic Remote Mining",
        "Low Starting Population",
    ];
    // DEFENDER, ECOBOOM, JUMPERS and SNEAK all carry exactly this.
    assert_eq!(traits(0x0a81), base);
    // BIGPRO adds Improved Starbases.
    assert_eq!(
        traits(0x0a89),
        vec![
            "Improved Fuel Efficiency",
            "Improved Starbases",
            "No Ramscoop Engines",
            "Only Basic Remote Mining",
            "Low Starting Population",
        ]
    );
    // OFFENDER adds Regenerating Shields — bit 13, the one the old table put
    // at bit 11 and so handed to everybody.
    assert_eq!(
        traits(0x2a81),
        vec![
            "Improved Fuel Efficiency",
            "No Ramscoop Engines",
            "Only Basic Remote Mining",
            "Low Starting Population",
            "Regenerating Shields",
        ]
    );
    // FLEXIBLE takes No Advanced Scanners in place of Low Starting Population.
    assert_eq!(
        traits(0x0681),
        vec![
            "Improved Fuel Efficiency",
            "No Ramscoop Engines",
            "Only Basic Remote Mining",
            "No Advanced Scanners",
        ]
    );

    // Every one of them is a trait the simulation knows by name, so nothing
    // here depends on a bit the engine does not use.
    for bits in [0x0a81u32, 0x0a89, 0x2a81, 0x0681] {
        assert!(!traits(bits).is_empty());
    }
}
