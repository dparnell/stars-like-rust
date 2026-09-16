//! Print Map — the dialog and the pages it prints. See
//! `docs/ui/print-map.md`.

use stars_ui::printmap::{Layout, Page, PAGE};
use stars_ui::App;

/// The arm's geometry: the map is a square as large as the sheet
/// allows, with the text below it on a tall sheet and beside it on a
/// wide one, and the larger page count along the longer side.
#[test]
fn the_layout_follows_the_sheet() {
    // One Letter page: taller than wide, so the map is the full width and
    // the text block sits under it, an inch of room kept.
    let one = Layout::new([1, 1], PAGE);
    assert_eq!((one.across, one.down), (1, 1));
    assert_eq!(one.square, 816);
    assert_eq!(one.map, 816 - 64);
    assert_eq!(one.text, (32, 848));
    assert_eq!(one.legend, (408, 848));
    // The swap: `if (down < across) == (height < width)`. On a tall
    // sheet that is "swap unless down is the smaller", so one by two
    // becomes two by one and two by one stays — either way two across,
    // 1632 by 1056, wider than tall, and the map is the height with
    // the text beside it.
    let two = Layout::new([2, 1], PAGE);
    assert_eq!((two.across, two.down), (2, 1));
    assert_eq!(two.square, 1056);
    assert_eq!(two.text, (1088, 32));
    let two = Layout::new([1, 2], PAGE);
    assert_eq!((two.across, two.down), (2, 1));
    // Two by two: 1632 by 2112, tall; the width wins again.
    let four = Layout::new([2, 2], PAGE);
    assert_eq!((four.across, four.down), (2, 2));
    assert_eq!(four.square, 1632);
    assert_eq!(four.text, (32, 1664));
    // A wide sheet: the map is the height and the text goes beside it,
    // an inch and a half kept.
    let wide = Page {
        width: 1056,
        height: 816,
        dpi: 96,
    };
    let w = Layout::new([1, 1], wide);
    assert_eq!(w.square, 816);
    assert_eq!(w.text, (848, 32));
    assert_eq!(w.legend, (848, 408));
    // A wide sheet nearly square: the map is cut to leave the inch and
    // a half.
    let squarish = Page {
        width: 900,
        height: 816,
        dpi: 96,
    };
    assert_eq!(Layout::new([1, 1], squarish).square, 900 - 144);
    // Nine by nine is clamped to 32000 pixels a side.
    let nine = Layout::new([9, 9], PAGE);
    assert_eq!(nine.square, 7344);
}

/// A planet lands where `(x − 1000) · map / dGal + margin` puts it,
/// with the y flipped.
#[test]
fn planets_are_plotted_flipped_and_scaled() {
    let layout = Layout::new([1, 1], PAGE);
    let span = 400;
    assert_eq!(layout.plot(1000, 1000, span), (32, 32 + layout.map));
    assert_eq!(layout.plot(1400, 1400, span), (32 + layout.map, 32));
    assert_eq!(
        layout.plot(1200, 1200, span),
        (32 + layout.map / 2, 32 + layout.map / 2)
    );
}

/// The boxes take one digit from 1 to 9 and throw anything else back;
/// Print complains about a bad box and prints anyway, as `PrintMapDlg`
/// does.
#[test]
fn the_dialog_takes_one_digit_each_and_prints_regardless() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    app.open_print_map();
    let dialog = app.print_map_dialog.clone().expect("open");
    assert_eq!(dialog.fields, ["1".to_string(), "1".to_string()]);
    assert!(app.print_map_type(0, "3"));
    assert!(!app.print_map_type(1, "0"), "zero beeps and is taken out");
    assert_eq!(
        app.print_map_dialog.as_ref().unwrap().fields,
        ["3".to_string(), String::new()]
    );
    assert!(!app.print_map_type(1, "x"));
    assert!(app.print_map_type(1, "2"));
    assert_eq!(app.print_map_ok(), None);
    assert_eq!(app.print_pages, [3, 2]);
    let preview = app.print_preview.clone().expect("the pages");
    assert_eq!((preview.across, preview.down), (3, 2));
    assert_eq!(preview.pages(), 6);
    app.close_print_preview();

    // A bad first box: the complaint, and the print with the counts as
    // they were.
    app.open_print_map();
    app.print_map_dialog.as_mut().unwrap().fields[0] = String::new();
    app.print_map_dialog.as_mut().unwrap().fields[1] = "5".to_string();
    assert!(app.print_map_ok().is_some());
    assert_eq!(
        app.print_pages,
        [3, 2],
        "the loop broke before the second box"
    );
    assert!(app.print_preview.is_some());
    app.close_print_preview();
    app.open_print_map();
    app.close_print_map();
    assert!(app.print_map_dialog.is_none());
}

/// A page comes out at the sheet's size with the map on it.
#[test]
fn a_page_prints_to_a_picture() {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    let image = app.print_page_image(0);
    assert_eq!((image.width, image.height), (816, 1056));
    let black = image
        .pixels
        .chunks(4)
        .filter(|p| p[0] < 0x40 && p[1] < 0x40 && p[2] < 0x40)
        .count();
    // Twenty-odd planet dots, their names, the frame and the text.
    assert!(black > 2000, "{black} dark pixels");
    // The bitmap writer: a 24-bit file of the right size.
    let bmp = stars_formats::resources::write_bmp(&image);
    assert_eq!(&bmp[..2], b"BM");
    assert_eq!(bmp.len(), 54 + 816 * 3 * 1056);
}

/// The strings are the game's own with the executable, this project's
/// without.
#[test]
fn the_print_strings_come_from_the_executable() {
    let mut app = App::new();
    let (title, year, legend) = app.print_strings();
    assert_eq!(title, stars_ui::printmap::OWN_TITLE);
    assert_eq!(year, stars_ui::printmap::OWN_YEAR);
    assert_eq!(legend.len(), 5);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    let Ok(exe) = std::fs::read(root.join("stars.2.7j.exe")) else {
        return;
    };
    app.load_art(exe, "stars.2.7j.exe").expect("art");
    let (title, year, legend) = app.print_strings();
    assert_eq!(title, "Stars! Universe Map");
    assert_eq!(year, "Year: {}");
    assert_eq!(legend[0], "   = Your Planet");
    assert_eq!(legend[4], "   = Player #2's Planet");
}
