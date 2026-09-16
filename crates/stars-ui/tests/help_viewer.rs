//! The help viewer — every Help button's `WinHelp` call, answered by this
//! project's reader of `STARS!.HLP`. See `docs/ui/help.md`.
//!
//! The tests that read the file skip without it; the ones about a missing
//! file need nothing.

use std::path::PathBuf;

use stars_ui::help::{context, Notice};
use stars_ui::App;

fn help_bytes() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    std::fs::read(root.join("STARS!.HLP")).ok()
}

fn with_help() -> Option<App> {
    let mut app = App::new();
    app.load_help(help_bytes()?, "STARS!.HLP")
        .expect("the help file reads");
    Some(app)
}

/// Lay one frame of the dialogs out, as the shell does.
fn draw(app: &mut App) {
    app.start_frame();
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |_ui| {});
        stars_ui::views::frame::dialogs(app, ctx);
    });
}

/// Without the file, a Help button puts up the notice rather than nothing.
#[test]
fn without_the_file_a_help_button_says_so() {
    let mut app = App::new();
    assert!(!app.has_help());
    app.help_context(context::CARGO_TRANSFER);
    assert_eq!(app.help.notice, Some(Notice::NoFile));
    assert!(!app.help.open);
    draw(&mut app);
    assert!(app.drawn_button("", "OK").is_some(), "the notice's OK");
}

/// A dialog's context number opens the viewer on its page.
#[test]
fn a_context_number_opens_its_topic() {
    let Some(mut app) = with_help() else { return };
    app.help_context(context::CARGO_TRANSFER);
    assert!(app.help.open);
    assert_eq!(app.help.title(), Some("Cargo Transfer Dialogs"));
    assert_eq!(app.help.notice, None);
    app.help_context(context::BATTLE_VCR);
    assert_eq!(app.help.title(), Some("Battle VCR"));
    // Back is the page before.
    assert!(app.help.can_go_back());
    assert!(app.help_back());
    assert_eq!(app.help.title(), Some("Cargo Transfer Dialogs"));
    assert!(!app.help.can_go_back());
    // Every page of the wizard and of the new-game steps has a topic.
    for id in context::RACE_WIZARD_PAGES
        .iter()
        .chain(context::NEW_GAME_STEPS.iter())
    {
        app.help_context(*id);
        assert_eq!(app.help.notice, None, "context {id:#x}");
    }
    // The wizard's first page has no title of its own: its number points
    // into the middle of a block, at text whose header is in the block
    // before.
    app.help_context(context::RACE_WIZARD_PAGES[0]);
    assert_eq!(app.help.title(), Some("Step 1: Basic Definition"));
    // History lists everything shown, most recent first, once each.
    assert_eq!(app.help.visited.len(), 2 + 9);
}

/// A number the file has no page for puts up WinHelp's notice and leaves
/// the viewer where it was.
#[test]
fn a_missing_topic_is_a_notice() {
    let Some(mut app) = with_help() else { return };
    app.help_context(context::RESEARCH);
    app.help_context(context::PASSWORD);
    assert_eq!(app.help.notice, Some(Notice::NoTopic(context::PASSWORD)));
    assert_eq!(app.help.title(), Some("Research Dialog"));
}

/// The button bar: Contents, Search, Back, History and the browse pair.
#[test]
fn the_button_bar_browses_the_guide() {
    let Some(mut app) = with_help() else { return };
    app.help_contents();
    assert_eq!(app.help.title(), Some("Stars! Player's Guide - Contents"));
    // The contents page is the start of a browse sequence, and nothing
    // was shown before it.
    assert!(!app.help_can_browse(false));
    assert!(app.help_can_browse(true));
    draw(&mut app);
    for label in ["Contents", "Search", "Back", "History", "<<", ">>"] {
        assert!(app.drawn_button("", label).is_some(), "{label}");
    }
    assert!(!app.drawn_button("", "Back").unwrap().enabled);
    assert!(!app.drawn_button("", "<<").unwrap().enabled);
    assert!(app.drawn_button("", ">>").unwrap().enabled);
    assert!(app.help_browse(true));
    assert_eq!(app.help.title(), Some("Introduction and Player Support"));
    assert!(app.help_browse(false));
    assert_eq!(app.help.title(), Some("Stars! Player's Guide - Contents"));
    // Browsing there and back is two pages for Back to retrace.
    draw(&mut app);
    assert!(app.drawn_button("", "Back").unwrap().enabled);
    assert!(app.help_back());
    assert_eq!(app.help.title(), Some("Introduction and Player Support"));
    app.help_close();
    assert!(!app.help.open);
}

/// A hotspot: the cargo page's link back to the dialogs list, and its
/// popups.
#[test]
fn hotspots_jump_and_pop_up() {
    let Some(mut app) = with_help() else { return };
    app.help_context(context::CARGO_TRANSFER);
    let topic = app.help.topic().cloned().expect("a topic");
    let mut jumps = Vec::new();
    for paragraph in topic.paragraphs() {
        for run in &paragraph.runs {
            if let stars_formats::help::Run::Text {
                jump: Some(jump), ..
            } = run
            {
                jumps.push(jump.clone());
            }
        }
    }
    let popup = jumps
        .iter()
        .find(|j| matches!(j, stars_formats::help::Jump::Popup(_)))
        .expect("a popup");
    app.help_jump(popup);
    assert!(app.help.popup.is_some());
    assert_eq!(
        app.help.title(),
        Some("Cargo Transfer Dialogs"),
        "still there"
    );
    draw(&mut app);
    let jump = jumps
        .iter()
        .find(|j| matches!(j, stars_formats::help::Jump::Topic(_)))
        .expect("a jump");
    app.help_jump(jump);
    assert_eq!(app.help.title(), Some("Stars! dialogs"));
    assert!(app.help.popup.is_none(), "a jump takes the popup down");
    draw(&mut app);
}

/// Search: a word narrows the keyword list, Show Topics lists its pages,
/// Go To opens one.
#[test]
fn search_finds_a_page_by_keyword() {
    let Some(mut app) = with_help() else { return };
    app.help_contents();
    app.help_search_open();
    let all = app.help_keyword_matches().len();
    assert!(all > 100);
    app.help.search.as_mut().unwrap().text = "cargo".to_string();
    let some = app.help_keyword_matches();
    assert!(!some.is_empty() && some.len() < all);
    app.help.search.as_mut().unwrap().keyword = Some(some[0]);
    app.help_show_topics();
    let topics = app.help.search.as_ref().unwrap().topics.clone();
    assert!(!topics.is_empty());
    draw(&mut app);
    assert!(app.help_search_go());
    assert!(app.help.search.is_none());
    assert_eq!(app.help.title(), Some(topics[0].1.as_str()));
}

/// The pictures decode into textures once and the contents page draws.
#[test]
fn the_contents_page_draws_its_picture_map() {
    let Some(mut app) = with_help() else { return };
    app.help_contents();
    draw(&mut app);
    assert!(app.help.textures.contains_key(&0), "the screen picture");
    // "The Stars! Screen" has the tables.
    app.help_goto(0x8000);
    draw(&mut app);
    // Every titled topic lays out without complaint.
    let offsets: Vec<i32> = app
        .help
        .file()
        .unwrap()
        .titled_topics()
        .iter()
        .map(|(o, _)| *o)
        .collect();
    for offset in offsets.iter().step_by(7) {
        assert!(app.help_goto(*offset));
        draw(&mut app);
    }
}
