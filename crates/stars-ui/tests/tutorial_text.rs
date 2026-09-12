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

/// Year zero, played as the pages say to play it, moves through the pages
/// as the original would: each task done turns the page, and generating
/// the year lands on the first page of the next.
///
/// This is the walk a new player takes, driven through the same calls the
/// panes make, so it fails if any one of those stops doing what the page
/// says it does.
#[test]
fn year_zero_can_be_played_through_from_the_pages() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    let page = |app: &App| app.tutor.as_ref().expect("running").page();
    let at = |app: &App, planet: i16| {
        app.game
            .as_ref()
            .expect("a game")
            .planets
            .iter()
            .chain(app.game.as_ref().expect("a game").known_planets.iter())
            .find(|p| p.id == planet)
            .and_then(|p| p.position)
            .expect("a placed planet")
    };
    // The worlds the pages send you to by name, which `tutorial_seed.rs`
    // pins to these ids.
    const PRUNE: i16 = 0x0c;
    const PLANET_90210: i16 = 0x10;
    const ALEXANDER: i16 = 0x0f;

    // Page 1: read the five messages.
    assert_eq!(page(&app), 1);
    for _ in 0..4 {
        app.show_next_message();
    }
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 2);

    // Page 2: Goto Armed Probe #1 and shift-click Prune.
    assert!(app.goto_fleet(0), "Armed Probe #1 is fleet 0");
    assert!(!app.advance_tutor(), "selecting it is only the first rung");
    assert_eq!(
        app.tutor.as_ref().expect("running").bold_line(),
        Some(7),
        "the emphasis has moved on to the shift-click"
    );
    let prune = at(&app, PRUNE);
    assert!(app.add_waypoint(prune.x, prune.y, 20.0));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 3);

    // Page 3: n to the next fleet, shift-click 90210.
    assert!(app.select_adjacent_fleet(1));
    assert_eq!(
        app.selection
            .fleet
            .map(|f| app.game.as_ref().expect("a game").fleets[f].id),
        Some(1)
    );
    let far = at(&app, PLANET_90210);
    assert!(app.add_waypoint(far.x, far.y, 20.0));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 4);

    // Page 4: Next three times to Stalwart Defender #5, shift-click Alexander.
    for _ in 0..3 {
        assert!(app.select_adjacent_fleet(1));
    }
    assert_eq!(
        app.selection
            .fleet
            .map(|f| app.game.as_ref().expect("a game").fleets[f].id),
        Some(4)
    );
    let alexander = at(&app, ALEXANDER);
    assert!(app.add_waypoint(alexander.x, alexander.y, 20.0));
    assert!(app.advance_tutor());
    assert_eq!(page(&app), 5);

    // Page 5: n twice round to Armed Probe #1, then Weapons in the Research
    // dialog and Done.
    assert!(app.select_adjacent_fleet(1));
    assert!(app.select_adjacent_fleet(1));
    assert_eq!(
        app.selection
            .fleet
            .map(|f| app.game.as_ref().expect("a game").fleets[f].id),
        Some(0)
    );
    app.open_research();
    app.research_dialog.as_mut().expect("the dialog").field = 1;
    app.research_ok();
    assert!(app.advance_tutor(), "Weapons chosen");
    assert_eq!(page(&app), 6, "the year's last page turned, ready for F9");

    // F9. Page 6 belongs to year 1, and the tutorial waits on it.
    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("a game").turn, 1);
    assert!(!app.advance_tutor());
    assert_eq!(page(&app), 6);
    assert_eq!(
        app.tutor_page().expect("a page")[2],
        stars_ui::tutorial_text::PAGES[5][2],
        "and it says to press Change on the Production tile"
    );
}
