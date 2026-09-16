//! Reading the player's guide, `STARS!.HLP`.
//!
//! These tests need the help file from the original's directory; without it
//! they skip, like every test that reads a game asset.

use std::path::PathBuf;

use stars_formats::help::{Block, Jump, Run};
use stars_formats::HelpFile;

fn help_file() -> Option<HelpFile> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    let bytes = std::fs::read(root.join("STARS!.HLP")).ok()?;
    Some(HelpFile::read(bytes).expect("the help file reads"))
}

#[test]
fn the_file_names_itself_and_its_window() {
    let Some(help) = help_file() else { return };
    assert_eq!(help.title(), "Stars! Player's Guide");
    assert_eq!(help.contents(), 0);
    let window = help.window().expect("a main window");
    assert_eq!((window.width, window.height), (710, 909));
    assert!(!window.maximised);
    // The non-scrolling band is white; the scrolling region's colour was
    // never set, so it is the viewer's own.
    assert_eq!(window.band_background, Some([0xff, 0xff, 0xff]));
    assert_eq!(window.background, None);
    assert_eq!(help.fonts().len(), 43);
    let bold = help.font(1).expect("font 1");
    assert!(bold.bold && !bold.italic);
    assert_eq!(bold.face, "Helv");
    assert_eq!(bold.half_points, 20);
}

#[test]
fn every_titled_topic_is_found_at_its_offset() {
    let Some(help) = help_file() else { return };
    let titles = help.titled_topics();
    assert_eq!(titles.len(), 414);
    for (offset, title) in titles {
        let topic = help
            .topic(*offset)
            .unwrap_or_else(|e| panic!("topic {offset:#x} ({title}): {e}"));
        assert_eq!(&topic.title, title, "at {offset:#x}");
    }
}

#[test]
fn a_dialogs_help_button_finds_its_topic() {
    let Some(help) = help_file() else { return };
    // `TransferDlg` (`1050:59be`) asks for 0x433 for a cargo transfer.
    let offset = help
        .topic_for_context(0x433)
        .expect("the cargo transfer topic");
    let topic = help.topic(offset).expect("it reads");
    assert_eq!(topic.title, "Cargo Transfer Dialogs");
    assert!(!topic.band.is_empty(), "the title band");
    let text = topic.plain_text();
    assert!(text.contains("Cargo Transfer"), "{text}");
    // The Battle VCR (`10e8:18cb`, 0x43a) and the research dialog
    // (`10d8:088f`, 0x42e).
    assert_eq!(
        help.title_of(help.topic_for_context(0x43a).unwrap()),
        Some("Battle VCR")
    );
    assert_eq!(
        help.title_of(help.topic_for_context(0x42e).unwrap()),
        Some("Research Dialog")
    );
    // The password dialog (`1040:5c5f`) asks for 0x441, which the file
    // does not have — WinHelp says so, and so must this project.
    assert_eq!(help.topic_for_context(0x441), None);
}

#[test]
fn hotspots_resolve_to_topics_and_the_contents_page_has_a_picture_map() {
    let Some(help) = help_file() else { return };
    let contents = help.topic(help.contents()).expect("the contents topic");
    let mut jumps = 0;
    let mut pictures = Vec::new();
    for paragraph in contents.paragraphs() {
        for run in &paragraph.runs {
            match run {
                Run::Text {
                    jump: Some(Jump::Topic(target)),
                    ..
                } => {
                    jumps += 1;
                    assert!(help.topic(*target).is_ok(), "jump to {target:#x}");
                }
                Run::Picture { number, .. } => pictures.push(*number),
                _ => {}
            }
        }
    }
    // The contents page is a picture of the screen and nothing else: its
    // links are the picture's hotspots.
    assert_eq!(jumps, 0);
    assert_eq!(pictures, vec![0]);
    let picture = help.picture(0).expect("the contents picture decodes");
    assert_eq!((picture.image.width, picture.image.height), (323, 396));
    assert_eq!(picture.hotspots.len(), 8);
    for hotspot in &picture.hotspots {
        let Jump::Topic(target) = hotspot.jump else {
            panic!("{hotspot:?}");
        };
        assert!(help.topic(target).is_ok(), "hotspot to {target:#x}");
    }
    // A topic with text links: the cargo transfer page links back to the
    // dialogs list and opens four popups.
    let cargo = help.topic(help.topic_for_context(0x433).unwrap()).unwrap();
    let mut topics = 0;
    let mut popups = 0;
    for paragraph in cargo.paragraphs() {
        for run in &paragraph.runs {
            match run {
                Run::Text {
                    jump: Some(Jump::Topic(t)),
                    ..
                } => {
                    topics += 1;
                    assert_eq!(help.title_of(*t), Some("Stars! dialogs"));
                }
                Run::Text {
                    jump: Some(Jump::Popup(t)),
                    ..
                } => {
                    popups += 1;
                    assert!(help.topic(*t).is_ok());
                }
                _ => {}
            }
        }
    }
    assert_eq!((topics, popups), (1, 4));
    // Every picture but the five metafiles decodes.
    let decoded = (0..help.picture_count())
        .filter(|n| help.picture(*n as u16).is_ok())
        .count();
    assert_eq!(help.picture_count(), 134);
    assert_eq!(decoded, 129);
}

#[test]
fn tables_keep_their_columns() {
    let Some(help) = help_file() else { return };
    // "The Stars! Screen" opens with a two-column table naming the panes.
    let offset = help.topic_for_context(0x1195).expect("the introduction");
    assert_eq!(help.title_of(offset), Some("Welcome to Stars!"));
    let screen = help
        .titled_topics()
        .iter()
        .find(|(_, t)| t == "The Stars! Screen")
        .map(|(o, _)| *o)
        .expect("the screen topic");
    let topic = help.topic(screen).expect("it reads");
    let table = topic
        .body
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            Block::Text(_) => None,
        })
        .expect("a table");
    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].len(), 2);
    let first = table.rows[0][0].plain();
    assert!(first.contains("Command pane"), "{first}");
}

#[test]
fn keywords_index_the_search_dialog() {
    let Some(help) = help_file() else { return };
    let keywords = help.keywords();
    assert!(!keywords.is_empty());
    let cargo = keywords
        .iter()
        .find(|k| k.word.eq_ignore_ascii_case("cargo transfer"))
        .or_else(|| {
            keywords
                .iter()
                .find(|k| k.word.to_ascii_lowercase().contains("cargo"))
        })
        .expect("a cargo keyword");
    assert!(!cargo.topics.is_empty());
    for topic in &cargo.topics {
        assert!(help.topic(*topic).is_ok());
    }
    // Sorted, as the tree hands them out.
    let words: Vec<String> = keywords
        .iter()
        .map(|k| k.word.to_ascii_lowercase())
        .collect();
    let mut sorted = words.clone();
    sorted.sort();
    assert_eq!(words, sorted);
}
