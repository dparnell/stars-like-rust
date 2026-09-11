//! `stars.ini`: what the reports keep between runs, and what is carried
//! through untouched.

use stars_ui::report::{Report, Reports};
use stars_ui::settings::{report_keys, Ini, WindowRect, WindowState, MISC, WINDOWS};

/// A rectangle is one letter and four fixed four-character fields, and
/// anything else is not a rectangle at all.
#[test]
fn a_window_rectangle_is_seventeen_characters() {
    let parsed = WindowRect::parse("M0000000012800960").expect("a rectangle");
    assert_eq!(parsed.state, WindowState::Maximised);
    assert_eq!(parsed.rect, (0, 0, 1280, 960));
    assert_eq!(parsed.format(), "M0000000012800960");

    // A `-` anywhere in a field makes that whole field negative.
    let negative = WindowRect::parse("R-010002000300040").expect("a rectangle");
    assert_eq!(negative.rect, (-10, 20, 30, 40));

    assert_eq!(
        WindowRect::parse("R0010002000300040").map(|r| r.state),
        Some(WindowState::Normal)
    );
    assert_eq!(
        WindowRect::parse("I0010002000300040").map(|r| r.state),
        Some(WindowState::Iconised)
    );

    // Wrong length, wrong letter, or a stray character: no rectangle.
    assert!(WindowRect::parse("M001000200030004").is_none(), "sixteen");
    assert!(
        WindowRect::parse("X0010002000300040").is_none(),
        "not M, R or I"
    );
    assert!(
        WindowRect::parse("M00100020003000x0").is_none(),
        "not a digit"
    );
    assert!(WindowRect::parse("").is_none());
}

/// The four reports' columns and sort go out and come back.
#[test]
fn a_reports_columns_and_sort_survive_a_round_trip() {
    let mut reports = Reports::default();
    reports.sort_by(Report::Planets, 2, false, 0);
    reports.sort_by(Report::Battles, 4, true, 0);
    // Hide the Mine column of the planets report.
    reports.state_mut(Report::Planets).visible &= !(1 << 6);

    let mut ini = Ini::parse("");
    reports.write_ini(&mut ini);

    let mut read = Reports::default();
    read.read_ini(&ini);
    assert_eq!(read.state(Report::Planets).sort, 2);
    assert!(!read.state(Report::Planets).ascending);
    assert!(!read.state(Report::Planets).shows(6), "still hidden");
    assert!(read.state(Report::Planets).shows(7));
    assert_eq!(read.state(Report::Battles).sort, 4);
    assert!(read.state(Report::Battles).ascending);
}

/// The sort value packs the column into the low byte and the direction into
/// bit 8, and `iSubsort` is not stored at all.
#[test]
fn the_sort_value_packs_the_direction_into_bit_eight() {
    let mut reports = Reports::default();
    // Min Conc, reversed, on the weighted average.
    reports.sort_by(Report::Planets, 0x0b, false, 3);
    let mut ini = Ini::parse("");
    reports.write_ini(&mut ini);
    assert_eq!(ini.get(MISC, "ReportPlanSort"), Some("11"));

    reports.sort_by(Report::Planets, 0x0b, true, 3);
    reports.write_ini(&mut ini);
    assert_eq!(ini.get(MISC, "ReportPlanSort"), Some("267"), "0x10b");

    let mut read = Reports::default();
    read.read_ini(&ini);
    assert_eq!(read.state(Report::Planets).sort, 0x0b);
    assert!(read.state(Report::Planets).ascending);
    assert_eq!(
        read.state(Report::Planets).subsort,
        0,
        "the mineral is not stored, so it comes back as the first"
    );
}

/// The third report's sort key is spelled `ReportEFltSort`, not the obvious
/// `ReportEFleetSort` — getting it wrong would lose that report's sort in
/// silence.
#[test]
fn the_enemy_fleets_sort_key_is_spelled_oddly() {
    assert_eq!(
        report_keys(Report::EnemyFleets),
        ("ReportEFleetFld", "ReportEFltSort", "ReportEFleetWin")
    );
    assert_eq!(
        report_keys(Report::Planets),
        ("ReportPlanFld", "ReportPlanSort", "ReportPlanWin")
    );
}

/// Everything the file holds that this project has no use for survives a
/// read and a write — including the four report windows' rectangles, which
/// belong to windows this project does not have.
#[test]
fn what_is_not_understood_is_carried_through() {
    let text = "\
[Windows]
Main=M0000000012800960
ReportPlanWin=R0100010006000400
[Misc]
DefaultPassword=hunter2
Backups=3
[Zip Orders]
Zip1=abcdabcdabcdabcdabcdMine
";
    let mut ini = Ini::parse(text);
    let mut reports = Reports::default();
    reports.read_ini(&ini);
    reports.sort_by(Report::Fleets, 5, true, 0);
    reports.write_ini(&mut ini);

    let written = ini.to_string();
    for kept in [
        "Main=M0000000012800960",
        "ReportPlanWin=R0100010006000400",
        "DefaultPassword=hunter2",
        "Backups=3",
        "[Zip Orders]",
        "Zip1=abcdabcdabcdabcdabcdMine",
    ] {
        assert!(written.contains(kept), "{kept} was lost:\n{written}");
    }
    assert!(written.contains("ReportFleetSort=261"), "{written}");

    // And the rectangle is still readable, for whenever there is a window
    // to give it to.
    let again = Ini::parse(&written);
    let rect = stars_ui::report::ReportState::window_rect(&again, Report::Planets)
        .expect("the planets report's rectangle");
    assert_eq!(rect.rect, (100, 100, 600, 400));
    assert_eq!(
        stars_ui::report::ReportState::window_rect(&again, Report::Battles),
        None,
        "no key, no rectangle"
    );
}

/// A missing key falls back to its default, the way `GetPrivateProfileInt`
/// does — and a report with no entry shows every column.
#[test]
fn a_missing_key_is_its_default() {
    let ini = Ini::parse("[Misc]\nBackups=3\n");
    assert_eq!(ini.int(MISC, "Backups", 1), 3);
    assert_eq!(ini.int(MISC, "Nothing", 7), 7);
    assert_eq!(ini.int("NoSuchSection", "Backups", 7), 7);
    assert_eq!(ini.int(MISC, "Backups", 1), 3);
    assert_eq!(ini.get(WINDOWS, "Main"), None);

    let mut reports = Reports::default();
    reports.read_ini(&ini);
    assert_eq!(reports.state(Report::Planets).visible, 0xffff);
    assert_eq!(reports.state(Report::Planets).sort, 0);
    assert!(!reports.state(Report::Planets).ascending, "0 has no bit 8");
}

/// The scanner's view, overlays, filters, zoom, toolbar and layout go out
/// and come back.
#[test]
fn the_scanner_survives_a_round_trip() {
    use stars_ui::settings::scanner;
    use stars_ui::{App, ScanView, WindowLayout};

    let mut app = App::new();
    app.scan_view = ScanView::PlanetValue;
    // Set the lot explicitly: the round trip replaces every bit, so a
    // default left standing would not prove anything.
    app.scan_overlays = stars_ui::ScanOverlays {
        names: true,
        fleet_paths: true,
        player_colours: true,
        scanner_coverage: false,
        minefields: false,
        ship_counts: false,
        idle_fleets: false,
        ship_design_filter: false,
        enemy_class_filter: false,
    };
    app.add_waypoints = true;
    app.scan_zoom = -2;
    app.scan_design_filter = 0b1010;
    app.scan_class_filter = 0b0110_0000;
    app.scan_minefield_filter = 0b0101;
    app.scan_coverage_pct = 60;
    app.toolbar_hidden = true;
    app.window_layout = WindowLayout::Small;

    let mut ini = Ini::parse("");
    app.write_scanner_ini(&mut ini);
    // The zoom is one digit, biased by five so it is always printable.
    assert_eq!(ini.get(WINDOWS, scanner::ZOOM), Some("3"));

    let mut read = App::new();
    read.read_scanner_ini(&ini);
    assert_eq!(read.scan_view, ScanView::PlanetValue);
    assert!(read.scan_overlays.names);
    assert!(read.scan_overlays.fleet_paths);
    assert!(read.scan_overlays.player_colours);
    assert!(!read.scan_overlays.minefields);
    assert!(read.add_waypoints, "even the add-waypoints mode is kept");
    assert_eq!(read.scan_zoom, -2);
    assert_eq!(read.scan_design_filter, 0b1010);
    assert_eq!(read.scan_class_filter, 0b0110_0000);
    assert_eq!(read.scan_minefield_filter, 0b0101);
    assert_eq!(read.scan_coverage_pct, 60);
    assert!(read.toolbar_hidden);
    assert_eq!(read.window_layout, WindowLayout::Small);
}

/// An empty file gives the scanner its shipped settings: planet names and
/// ship counts on, every minefield drawn, coverage at full strength.
#[test]
fn an_empty_file_gives_the_shipped_scanner() {
    use stars_ui::{App, ScanView, WindowLayout};

    let mut app = App::new();
    app.read_scanner_ini(&Ini::parse(""));
    assert_eq!(app.scan_view, ScanView::Normal);
    // `0xe0` is coverage, minefields and fleet paths — not the names and
    // ship counts a first guess would put there.
    assert!(app.scan_overlays.scanner_coverage);
    assert!(app.scan_overlays.minefields);
    assert!(app.scan_overlays.fleet_paths);
    assert!(!app.scan_overlays.names);
    assert!(!app.scan_overlays.ship_counts);
    assert_eq!(app.scan_minefield_filter, 0xf);
    assert_eq!(app.scan_coverage_pct, 100);
    assert_eq!(app.scan_zoom, -1, "the default is the digit 4");
    assert!(!app.toolbar_hidden);
    assert_eq!(app.window_layout, WindowLayout::Medium);
}

/// `grbitScan` is sanity-checked on the way in: a view the game does not
/// have, or either of the top two bits, throws away the mode **and** the
/// ship filter.
#[test]
fn a_nonsense_scan_mode_is_thrown_away_with_the_ship_filter() {
    use stars_ui::settings::scanner;
    use stars_ui::{App, ScanView};

    let mut ini = Ini::parse("");
    // View 6, which does not exist, with some overlays and a filter set.
    ini.set(WINDOWS, scanner::MODE, "2534");
    ini.set(WINDOWS, scanner::SHIP_FILTER, "255");
    let mut app = App::new();
    app.read_scanner_ini(&ini);
    assert_eq!(app.scan_view, ScanView::Normal);
    assert!(!app.scan_overlays.names, "the whole mode went");
    assert_eq!(app.scan_design_filter, 0, "and the filter with it");

    // A view the game does have is kept, filter and all.
    ini.set(WINDOWS, scanner::MODE, "1029");
    ini.set(WINDOWS, scanner::SHIP_FILTER, "255");
    let mut app = App::new();
    app.read_scanner_ini(&ini);
    assert_eq!(app.scan_view, ScanView::NoPlayerInfo, "view 5");
    assert!(app.scan_overlays.names, "0x400 is the planet names");
    assert_eq!(app.scan_design_filter, 255);
}

/// A stored zoom of `0` is rejected rather than read as the smallest one:
/// the test is `v != 0 && v < 10`.
#[test]
fn a_zoom_of_zero_is_no_zoom() {
    use stars_ui::settings::scanner;
    use stars_ui::App;

    let mut ini = Ini::parse("");
    let mut app = App::new();
    app.scan_zoom = 3;
    ini.set(WINDOWS, scanner::ZOOM, "0");
    app.read_scanner_ini(&ini);
    assert_eq!(app.scan_zoom, 3, "left alone");

    ini.set(WINDOWS, scanner::ZOOM, "9");
    app.read_scanner_ini(&ini);
    assert_eq!(app.scan_zoom, 4);
    ini.set(WINDOWS, scanner::ZOOM, "1");
    app.read_scanner_ini(&ini);
    assert_eq!(app.scan_zoom, -4);
}

/// A zip order is five four-letter words and then a name, and the word puts
/// the **action first** and then the quantity from its bottom nibble up.
#[test]
fn a_zip_order_writes_the_action_before_the_quantity() {
    use stars_formats::XferAction;
    use stars_ui::app::ZipOrder;
    use stars_ui::settings::{decode_zip, encode_zip};

    let order = ZipOrder {
        name: "Miner".to_string(),
        items: [
            (XferAction::LoadAll, 0),
            (XferAction::None, 0),
            (XferAction::None, 0),
            (XferAction::UnloadExact, 0x123),
            (XferAction::None, 0),
        ],
    };
    let text = encode_zip(&order);
    assert_eq!(&text[20..], "Miner");
    // Load All is action 1, with no quantity: `b` then three `a`s.
    assert_eq!(&text[0..4], "baaa");
    // Unload Exactly is action 4, quantity 0x123 — low nibble first.
    assert_eq!(&text[12..16], "edcb");

    let back = decode_zip(&text).expect("it reads back");
    assert_eq!(back, order);
}

/// The lengths `ReadIniSettings` insists on: over twenty characters and
/// under thirty-three, with the first twenty all in `a`–`p`.
#[test]
fn a_zip_order_of_the_wrong_shape_is_no_order() {
    use stars_ui::settings::decode_zip;

    assert!(decode_zip("").is_none());
    assert!(
        decode_zip("aaaaaaaaaaaaaaaaaaa").is_none(),
        "nineteen is one short of the five words"
    );
    assert!(
        decode_zip("aaaaaaaaaaaaaaaaaaaa").is_some(),
        "twenty is the five words and no name at all"
    );
    assert!(
        decode_zip("aaaaaaaaaaaaaaaaaaaaX").is_some(),
        "and a name fits after"
    );
    assert!(
        decode_zip("aaaaaaaaaaaaaaaaaaaaXXXXXXXXXXXXX").is_none(),
        "thirty-three is one too many"
    );
    assert!(
        decode_zip("aaaaaaaaaaaaaaaaaaqa!").is_none(),
        "a letter past p is not a nibble"
    );
}

/// The four orders and the five templates share `[ZipOrders]` and come back
/// together — and the first template slot is thrown away on the way in,
/// because it is the player's own default queue.
#[test]
fn the_zip_section_holds_both_and_round_trips() {
    use stars_formats::XferAction;
    use stars_ui::app::ZipOrder;
    use stars_ui::settings::ZIP_ORDERS;
    use stars_ui::App;

    let mut app = App::new();
    app.zip_orders[1] = ZipOrder {
        name: "Fuel".to_string(),
        items: [
            (XferAction::None, 0),
            (XferAction::None, 0),
            (XferAction::None, 0),
            (XferAction::None, 0),
            (XferAction::LoadAll, 0),
        ],
    };

    let mut ini = Ini::parse("");
    app.write_zip_ini(&mut ini);
    assert!(ini.get(ZIP_ORDERS, "ZipOrders2").is_some());
    assert!(
        ini.get(ZIP_ORDERS, "ZipOrders1").is_none(),
        "an untouched slot writes no key at all"
    );

    let mut read = App::new();
    read.read_zip_ini(&ini);
    assert_eq!(read.zip_orders[1].name, "Fuel");
    assert_eq!(read.zip_orders[1].items[4].0, XferAction::LoadAll);
    assert_eq!(read.zip_orders[0], ZipOrder::default());

    // The first template slot always reads back as the default, whatever
    // the file says, so nothing written there survives.
    assert_eq!(read.production_templates()[0].name, "<Default>");
}
