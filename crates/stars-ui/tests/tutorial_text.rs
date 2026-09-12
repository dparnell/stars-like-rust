//! The tutorial's pages in this project's own words, and the shape they
//! have to share with the game's own.
//!
//! See `crates/stars-ui/src/tutorial_text.rs` and `docs/ui/tutorial.md`.

use std::path::PathBuf;

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_formats::tutorial::{PAGES as PAGE_COUNT, PARAGRAPHS_PER_PAGE};
use stars_ui::tutorial_text::{page, PAGES};
use stars_ui::App;

fn executable() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    for name in ["stars.2.7j.exe", "stars.exe", "STARS!.EXE"] {
        if let Ok(bytes) = std::fs::read(root.join(name)) {
            return Some(bytes);
        }
    }
    None
}

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "tutorial".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

/// How `DrawTutorText` treats a paragraph: a page-ending one-character
/// paragraph, a fresh paragraph, or a continuation.
#[derive(Debug, PartialEq, Eq)]
enum Shape {
    End,
    Fresh,
    Continues,
}

fn shape(text: &str) -> Shape {
    if text.chars().count() <= 1 {
        Shape::End
    } else if text.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        Shape::Fresh
    } else {
        Shape::Continues
    }
}

/// Eighty pages of eight, and every one of them says something.
#[test]
fn there_are_eighty_pages_of_eight() {
    assert_eq!(PAGES.len(), PAGE_COUNT);
    for (index, page) in PAGES.iter().enumerate() {
        assert_eq!(page.len(), PARAGRAPHS_PER_PAGE);
        assert_eq!(
            shape(page[0]),
            Shape::Fresh,
            "page {} opens with a paragraph of its own",
            index + 1
        );
        // Once a page has ended it stays ended: nothing after a blank slot.
        let mut ended = false;
        for (slot, text) in page.iter().enumerate() {
            if shape(text) == Shape::End {
                ended = true;
            } else {
                assert!(
                    !ended,
                    "page {} has text after its end, slot {slot}",
                    index + 1
                );
            }
        }
    }
    assert_eq!(page(0), None, "pages count from one");
    assert_eq!(page(81), None);
    assert_eq!(page(1).map(|p| p.len()), Some(PARAGRAPHS_PER_PAGE));
}

/// Slot for slot, each page has the same shape as the game's own — the same
/// paragraph breaks and the same ending — so the emphasis the step machine
/// points at lands on the same instruction whichever text is showing. And
/// none of it is the game's own words.
///
/// Skipped without a copy of the original to compare against.
#[test]
fn every_page_has_the_shape_of_the_original() {
    let Some(exe) = executable() else {
        eprintln!("no copy of the original; skipping");
        return;
    };
    for number in 1..=PAGE_COUNT {
        let theirs = stars_formats::tutorial::page(&exe, number).expect("the original's page");
        let ours = page(number).expect("our page");
        for (slot, (a, b)) in theirs.iter().zip(ours.iter()).enumerate() {
            assert_eq!(
                shape(a),
                shape(b),
                "page {number} slot {slot}: {a:?} against {b:?}"
            );
            // A slot that is only a planet's name, or an "and" between two
            // of them, is a fact rather than prose and has nothing to
            // reword; anything longer must be this project's own words.
            if a.split_whitespace().count() >= 3 {
                assert_ne!(a.trim(), b.trim(), "page {number} slot {slot} is copied");
            }
        }
    }
}

/// Without a copy of the original the tutorial still has words, and they
/// are these.
#[test]
fn the_page_is_shown_without_a_copy_of_the_game() {
    let mut app = a_game();
    assert!(!app.has_art());
    app.start_tutor();
    let shown = app.tutor_page().expect("a page");
    assert_eq!(shown, page(1).expect("page one"));
    assert!(!app.tutor_page_is_original());
    assert!(
        shown[5].contains("messages"),
        "the emboldened paragraph tells you to read the messages: {:?}",
        shown[5]
    );
}

/// With one, the game's own words are shown instead.
#[test]
fn the_original_is_preferred_when_it_is_there() {
    let Some(exe) = executable() else {
        eprintln!("no copy of the original; skipping");
        return;
    };
    let mut app = a_game();
    app.load_art(exe.clone(), "test")
        .expect("reads the pictures");
    app.start_tutor();
    assert!(app.tutor_page_is_original());
    assert_eq!(
        app.tutor_page(),
        stars_formats::tutorial::page(&exe, 1),
        "the game's own page one"
    );
}
