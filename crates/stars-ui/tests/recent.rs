//! The File menu's recently-opened list.

use stars_ui::recent::Recent;

/// Nine slots, most recent first, and a game already at the front does not
/// move — which is the short-circuit `FLoadGame` takes.
#[test]
fn opening_a_game_puts_it_at_the_front() {
    let mut recent = Recent::new();
    assert!(recent.is_empty());
    assert_eq!(recent.startup_file(), None);

    assert!(recent.opened("C:\\STARS\\ONE.M1"));
    assert!(recent.opened("C:\\STARS\\TWO.M1"));
    assert_eq!(recent.paths(), ["C:\\STARS\\TWO.M1", "C:\\STARS\\ONE.M1"]);
    assert_eq!(recent.startup_file(), Some("C:\\STARS\\TWO.M1"));

    // Already first: nothing happens, and nothing needs writing out.
    assert!(!recent.opened("C:\\STARS\\TWO.M1"));
    assert!(
        !recent.opened("c:\\stars\\two.m1"),
        "and the compare is blind to case"
    );

    // Further down: it comes to the front and leaves no hole behind.
    assert!(recent.opened("c:\\stars\\one.m1"));
    assert_eq!(recent.paths(), ["c:\\stars\\one.m1", "C:\\STARS\\TWO.M1"]);
}

/// Nine is the limit — `vrgszMRU` is nine slots of `0x100` — and the tenth
/// pushes the oldest off.
#[test]
fn the_list_holds_nine() {
    let mut recent = Recent::new();
    for n in 0..12 {
        assert!(recent.opened(&format!("game{n}.m1")));
    }
    assert_eq!(recent.paths().len(), Recent::SLOTS);
    assert_eq!(recent.paths()[0], "game11.m1");
    assert_eq!(recent.paths()[8], "game3.m1", "the older three fell off");
}

/// A name shorter than four characters is no name.
#[test]
fn a_short_name_is_no_name() {
    let mut recent = Recent::new();
    assert!(!recent.opened("a.m"));
    assert!(recent.is_empty());
    assert!(recent.opened("ab.m"), "four is enough");
}

/// Captions are `&1 ` and the whole path, numbered from one.
#[test]
fn the_captions_are_numbered_from_one() {
    let mut recent = Recent::new();
    recent.opened("C:\\STARS\\GAME.M1");
    assert_eq!(recent.caption(0).as_deref(), Some("&1 C:\\STARS\\GAME.M1"));
    assert_eq!(recent.caption(1), None);
}

/// The `[Files]` section round-trips, and reading it compacts the holes the
/// way `ReadIniSettings` does.
#[test]
fn the_files_section_round_trips_and_compacts() {
    let text = "[Windows]\nMain=1,2,3,4\n\n[Files]\nFile1=first.m1\nFile3=third.m1\nFile4=x\n";
    let recent = Recent::from_ini(text);
    assert_eq!(
        recent.paths(),
        ["first.m1", "third.m1"],
        "File2 missing does not hide File3, and File4 is too short to count"
    );

    let written = recent.to_ini();
    assert!(written.starts_with("[Files]\n"), "{written}");
    assert!(written.contains("File1=first.m1\n"), "{written}");
    assert!(
        written.contains("File2=third.m1\n"),
        "compacted on the way out"
    );
    assert_eq!(Recent::from_ini(&written), recent);

    // No section at all is an empty list, not an error.
    assert!(Recent::from_ini("[Misc]\nProgress=0\n").is_empty());
    assert!(Recent::from_ini("").is_empty());
}
