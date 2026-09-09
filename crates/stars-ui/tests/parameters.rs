/// The window is the Advanced Game wizard shown read-only — resources 390,
/// 391 and 392, three pages of 261 by 210 dialog units.
#[test]
fn the_three_templates_are_the_games_own() {
    use stars_ui::dialog::{Class, GAME_PARAMS, VICTORY_HEADING, WIZARD_FOOTER, WIZARD_FOOTER_Y};

    assert_eq!(GAME_PARAMS.len(), 3);
    for page in GAME_PARAMS {
        assert_eq!(page.size, (261, 210), "{}", page.caption);
        // The same five-button footer the race wizard carries, at the same y.
        for id in WIZARD_FOOTER {
            let control = page.control(id).expect("a footer button");
            assert_eq!(control.class, Class::Button);
            assert_eq!(control.at.1, WIZARD_FOOTER_Y);
            assert_eq!((control.at.2, control.at.3), (40, 14));
        }
    }

    // Page 1 holds the universe: five sizes, four densities, four distances
    // and seven option checkboxes.
    let one = GAME_PARAMS[0];
    for (index, name) in ["Tiny", "Small", "Medium", "Large", "Huge"]
        .into_iter()
        .enumerate()
    {
        #[allow(clippy::cast_possible_truncation)]
        let id = 0x3e8 + index as u16;
        assert_eq!(one.control(id).expect("a size").text, name);
    }
    for (index, name) in ["Sparse", "Normal", "Dense", "Packed"]
        .into_iter()
        .enumerate()
    {
        #[allow(clippy::cast_possible_truncation)]
        let id = 0x3ed + index as u16;
        assert_eq!(one.control(id).expect("a density").text, name);
    }
    for (index, name) in ["Close", "Moderate", "Farther", "Distant"]
        .into_iter()
        .enumerate()
    {
        #[allow(clippy::cast_possible_truncation)]
        let id = 0x3f1 + index as u16;
        assert_eq!(one.control(id).expect("a distance").text, name);
    }

    // Page 2 is the footer and nothing else; page 3 is its heading and seven
    // bare checkboxes, twelve units square.
    assert_eq!(GAME_PARAMS[1].controls.len(), 5, "the footer alone");
    let three = GAME_PARAMS[2];
    assert_eq!(
        three.controls.len(),
        13,
        "a heading, seven boxes, the footer"
    );
    assert_eq!(
        three.control(0xffff).expect("the heading").text,
        VICTORY_HEADING
    );
    for id in 0x123..=0x129u16 {
        let box_ = three.control(id).expect("a victory checkbox");
        assert_eq!((box_.at.2, box_.at.3), (12, 12), "just the box");
        assert_eq!(box_.text, "", "its words are built at run time");
    }
}

/// The Advanced Game dialog settles what one of the game flags means.
#[test]
fn the_option_captions_are_the_dialogs_own() {
    use stars_ui::dialog::GAME_PARAMS;

    let one = GAME_PARAMS[0];
    assert_eq!(
        one.control(0x3f8).expect("minerals").label(),
        "Beginner:  Maximum Minerals"
    );
    assert_eq!(
        one.control(0x3f9).expect("tech").label(),
        "Slower Tech Advances"
    );
    assert_eq!(
        one.control(0x3fa).expect("bbs").label(),
        "Accelerated BBS Play"
    );
    assert_eq!(
        one.control(0x3fb).expect("random").label(),
        "No Random Events"
    );
    // The correction: bit 4 is not a handicap band.
    assert_eq!(
        one.control(0x3fc).expect("alliances").label(),
        "Computer Players Form Alliances"
    );
    assert_eq!(
        one.control(0x3fd).expect("scores").label(),
        "Public Player Scores"
    );
    assert_eq!(
        one.control(0x41a).expect("clumping").label(),
        "Galaxy Clumping"
    );
}
