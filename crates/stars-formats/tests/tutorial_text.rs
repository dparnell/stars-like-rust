//! The tutorial's text, read out of a copy of the game.
//!
//! None of it lives in this repository; these tests skip when no copy of the
//! original is present, as every fixture-backed test here does.

use std::path::{Path, PathBuf};

fn executable() -> Option<PathBuf> {
    for name in ["stars.2.7j.exe", "stars.exe", "STARS!.EXE"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../binary")
            .join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn game() -> Option<Vec<u8>> {
    std::fs::read(executable()?).ok()
}

/// Eighty pages of eight, and the first of them says what the tutorial is.
#[test]
fn the_whole_tutorial_reads_back() {
    let Some(exe) = game() else {
        return;
    };
    assert!(stars_formats::tutorial::present(&exe));
    assert_eq!(stars_formats::tutorial::PAGES, 80);
    assert_eq!(stars_formats::tutorial::PARAGRAPHS, 640);

    let mut read = 0;
    for idt in 0..stars_formats::tutorial::PARAGRAPHS {
        let text = stars_formats::tutorial::paragraph(&exe, idt)
            .unwrap_or_else(|| panic!("paragraph {idt} did not decode"));
        if !text.trim().is_empty() {
            read += 1;
        }
        // Nothing decodes to control characters: a wrong table would show up
        // as rubbish long before it showed up as a wrong word.
        assert!(
            text.chars().all(|c| c == ' ' || !c.is_control()),
            "paragraph {idt} decoded to control characters: {text:?}"
        );
    }
    assert!(read > 400, "only {read} paragraphs had anything in them");

    // Past the end there is nothing rather than rubbish.
    assert_eq!(
        stars_formats::tutorial::paragraph(&exe, stars_formats::tutorial::PARAGRAPHS),
        None
    );
}

/// A page is eight paragraphs and its number is one-based, as the tutorial's
/// own title bar counts them.
#[test]
fn a_page_is_eight_paragraphs() {
    let Some(exe) = game() else {
        return;
    };
    let first = stars_formats::tutorial::page(&exe, 1).expect("page one");
    assert_eq!(first.len(), 8);
    for (index, text) in first.iter().enumerate() {
        assert_eq!(
            Some(text.clone()),
            stars_formats::tutorial::paragraph(&exe, index)
        );
    }
    // It opens by saying what it is and how long it runs.
    assert!(
        first[0].contains("tutorial") && first[0].contains("36"),
        "page one opens: {:?}",
        first[0]
    );

    assert_eq!(stars_formats::tutorial::page(&exe, 0), None);
    assert_eq!(stars_formats::tutorial::page(&exe, 81), None);
    let last = stars_formats::tutorial::page(&exe, 80).expect("page eighty");
    assert_eq!(last.len(), 8);
    assert!(last.iter().any(|p| !p.trim().is_empty()));
}

/// Anything that is not the game has no tutorial in it, and says so rather
/// than decoding rubbish.
#[test]
fn a_file_that_is_not_the_game_has_no_tutorial() {
    assert!(!stars_formats::tutorial::present(b"not an executable"));
    assert_eq!(stars_formats::tutorial::paragraph(b"MZ", 0), None);
    assert!(!stars_formats::tutorial::present(&[0u8; 4096]));
}

/// The segment table is read the way the loader reads it: selectors eight
/// apart from `0x1000`, offsets in units of the header's alignment shift.
#[test]
fn segments_are_found_by_selector() {
    let Some(exe) = game() else {
        return;
    };
    let (at, len) = stars_formats::resources::segment_offset(&exe, 0x1100)
        .expect("the segment the tutorial text is in");
    assert!(at + len <= exe.len());
    // The first segment is the first one, and they are eight apart.
    assert!(stars_formats::resources::segment_offset(&exe, 0x1000).is_some());
    // Not a multiple of eight past the base, and far past the end: neither is
    // a segment.
    assert_eq!(stars_formats::resources::segment_offset(&exe, 0x0fff), None);
    assert_eq!(stars_formats::resources::segment_offset(&exe, 0xf000), None);
}
