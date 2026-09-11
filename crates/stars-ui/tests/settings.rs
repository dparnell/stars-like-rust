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
        "DefaultPassword=hunter2",
        "Backups=3",
        "[Zip Orders]",
        "Zip1=abcdabcdabcdabcdabcdMine",
    ] {
        assert!(written.contains(kept), "{kept} was lost:\n{written}");
    }
    assert!(written.contains("ReportFleetSort=261"), "{written}");

    // The planets report's own rectangle is not carried through — it is
    // read and written back, which loses nothing but the state letter,
    // since the original always writes `M`.
    let again = Ini::parse(&written);
    let mut back = Reports::default();
    back.read_ini(&again);
    assert_eq!(back.state(Report::Planets).pos, (100, 100));
    assert_eq!(back.state(Report::Planets).size, (500, 300));
    assert_eq!(
        stars_ui::report::ReportState::window_rect(&again, Report::Planets).map(|r| r.state),
        Some(WindowState::Maximised),
        "written with M whatever it was"
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

/// The frame's rectangle and the report windows' use the same seventeen
/// characters to mean **different things** in the last two fields: the
/// frame's are a width and a height, the reports' a right and a bottom.
#[test]
fn the_frame_stores_a_size_where_a_report_stores_a_corner() {
    use stars_ui::settings::{frame_window, set_frame_window, WindowRect};

    let mut ini = Ini::parse("");
    set_frame_window(
        &mut ini,
        WindowRect::frame(WindowState::Normal, (100, 80), (1200, 800)),
    );
    assert_eq!(ini.get(WINDOWS, "Main"), Some("R0100008012000800"));

    let read = frame_window(&ini);
    assert_eq!(read.position(), Some((100, 80)));
    assert_eq!(read.size(), Some((1200, 800)), "a size, not a far corner");

    // The same four numbers under a report's key are a far corner, and the
    // report reader subtracts to get the size.
    ini.set(WINDOWS, "ReportPlanWin", "R0100008012000800");
    let report =
        stars_ui::report::ReportState::window_rect(&ini, Report::Planets).expect("a rectangle");
    assert_eq!(report.rect, (100, 80, 1200, 800), "kept as written");
}

/// With no `Main` at all the frame comes up maximised, and its place is
/// left to the system — `-32768` is `CW_USEDEFAULT`, not a mistake.
#[test]
fn no_frame_rectangle_means_maximised_and_wherever() {
    use stars_ui::settings::{frame_starts_maximised, frame_window, WindowRect};

    let empty = frame_window(&Ini::parse(""));
    assert_eq!(empty.state, WindowState::Maximised);
    assert_eq!(empty.position(), None);
    assert_eq!(empty.size(), None);
    assert_eq!(empty.rect.0, WindowRect::USE_DEFAULT);
    assert!(frame_starts_maximised(empty));

    // A value that is not a rectangle goes the same way.
    let mut ini = Ini::parse("");
    ini.set(WINDOWS, "Main", "nonsense");
    assert_eq!(frame_window(&ini).state, WindowState::Maximised);

    // And a window left **minimised** comes back maximised: `InitInstance`
    // asks for `SW_SHOWMAXIMIZED` on either bit.
    ini.set(WINDOWS, "Main", "I0100008012000800");
    let iconised = frame_window(&ini);
    assert_eq!(iconised.state, WindowState::Iconised);
    assert!(
        frame_starts_maximised(iconised),
        "minimised comes back maximised, not minimised"
    );

    // A normal one does not.
    ini.set(WINDOWS, "Main", "R0100008012000800");
    assert!(!frame_starts_maximised(frame_window(&ini)));
}

/// `Selection` is three fields in a row: the kind, the player as a letter
/// from `B`, and the id.
#[test]
fn the_selection_is_a_kind_a_player_and_an_id() {
    use stars_ui::settings::{LastSelection, SelectedKind};

    let planet = LastSelection::parse("PB13").expect("a selection");
    assert_eq!(planet.kind, SelectedKind::Planet);
    assert_eq!(planet.player, 0, "B is player zero");
    assert_eq!(planet.id, 13);
    assert_eq!(planet.format(), "PB13");

    assert_eq!(
        LastSelection::parse("SD7").map(|s| (s.kind, s.player, s.id)),
        Some((SelectedKind::Fleet, 2, 7)),
        "S is a ship"
    );
    assert_eq!(
        LastSelection::parse("EQ0").map(|s| s.player),
        Some(15),
        "Q is the sixteenth player"
    );
    assert_eq!(
        LastSelection::parse("NB0").map(|s| s.kind),
        Some(SelectedKind::None)
    );

    // Under three characters is no selection, and so is a player letter
    // outside B–Q.
    assert!(LastSelection::parse("PB").is_none());
    assert!(
        LastSelection::parse("PA3").is_none(),
        "A is before the first"
    );
    assert!(LastSelection::parse("PR3").is_none(), "R is past the last");
    assert!(
        LastSelection::parse("PBx").is_none(),
        "the id has to be a number"
    );
}

/// Nothing comes back unless the player and the game id both match.
#[test]
fn a_selection_belongs_to_one_game_and_one_player() {
    use stars_ui::settings::{selection_applies, LastSelection, SelectedKind};

    let last = LastSelection {
        kind: SelectedKind::Planet,
        player: 1,
        id: 13,
    };
    assert!(selection_applies(last, 1, 0x8cef_49, 0x8cef_49));
    assert!(
        !selection_applies(last, 0, 0x8cef_49, 0x8cef_49),
        "another player's selection is not yours"
    );
    assert!(
        !selection_applies(last, 1, 0x8cef_49, 0x1234),
        "and not another game's"
    );
}

/// A fleet that has gone, or a planet that is no longer yours, falls back
/// to the home world — and a stored `E` is read as a planet id, because
/// `RestoreSelection` tests only for the other two.
#[test]
fn a_selection_that_no_longer_exists_falls_back() {
    use stars_ui::settings::{restore_selection, LastSelection, Restore, SelectedKind};

    let at = |kind| LastSelection {
        kind,
        player: 0,
        id: 9,
    };
    assert_eq!(
        restore_selection(at(SelectedKind::Fleet), true, false),
        Restore::Fleet(9)
    );
    assert_eq!(
        restore_selection(at(SelectedKind::Fleet), false, true),
        Restore::HomeWorld,
        "the fleet has gone"
    );
    assert_eq!(
        restore_selection(at(SelectedKind::Planet), false, true),
        Restore::Planet(9)
    );
    assert_eq!(
        restore_selection(at(SelectedKind::Planet), false, false),
        Restore::HomeWorld,
        "taken from you, or never yours"
    );
    assert_eq!(
        restore_selection(at(SelectedKind::Other), false, true),
        Restore::Planet(9),
        "a space object is read as a planet id"
    );
    assert_eq!(
        restore_selection(at(SelectedKind::None), true, true),
        Restore::HomeWorld
    );
}

/// The message comes back only within the same year, and the stored number
/// is one-based so that nothing stored reads as `0`.
#[test]
fn the_message_comes_back_only_in_the_same_year() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};
    use stars_ui::settings::{FILES, MESSAGE, TURN};
    use stars_ui::App;

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "settings".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.message_index = 4;

    let mut ini = Ini::parse("");
    app.write_selection_ini(&mut ini);
    assert_eq!(ini.get(WINDOWS, MESSAGE), Some("5"), "one-based");
    assert_eq!(ini.get(FILES, TURN), Some("0"));

    app.message_index = 0;
    app.read_selection_ini(&ini);
    assert_eq!(app.message_index, 4, "the same year brings it back");

    // A later year does not.
    if let Some(game) = app.game.as_mut() {
        game.turn = 3;
    }
    app.message_index = 0;
    app.read_selection_ini(&ini);
    assert_eq!(app.message_index, 0, "a new year starts at the first");

    // And nothing stored is nothing restored.
    if let Some(game) = app.game.as_mut() {
        game.turn = 0;
    }
    ini.set(WINDOWS, MESSAGE, "0");
    app.message_index = 2;
    app.read_selection_ini(&ini);
    assert_eq!(app.message_index, 2);
}

/// The four typefaces, their keys, and the lengths the reader insists on.
#[test]
fn the_font_names_default_to_arial_and_its_three_faces() {
    use stars_ui::settings::{Fonts, FONTS, FONT_DEFAULTS, FONT_KEYS};

    assert_eq!(
        FONT_KEYS,
        ["Arial", "ArialBold", "ArialItalic", "ArialBoldItalic"]
    );
    assert_eq!(
        FONT_DEFAULTS,
        ["Arial", "Arial Bold", "Arial Italic", "Arial Bold Italic"]
    );

    // Nothing in the file: the four built-in names.
    let shipped = Fonts::read_ini(&Ini::parse(""));
    assert_eq!(shipped, Fonts::default());
    assert_eq!(shipped.regular(), "Arial");
    assert_eq!(shipped.bold(), "Arial Bold");

    // A name of a sensible length replaces one.
    let mut ini = Ini::parse("");
    ini.set(FONTS, "Arial", "Helvetica");
    ini.set(FONTS, "ArialBold", "Helvetica Bold");
    let read = Fonts::read_ini(&ini);
    assert_eq!(read.regular(), "Helvetica");
    assert_eq!(read.bold(), "Helvetica Bold");
    assert_eq!(read.names[2], "Arial Italic", "the rest keep their default");

    // Four characters is too short — the test is **more than** four — and
    // thirty-two is too long for the buffer.
    ini.set(FONTS, "Arial", "Chic");
    assert_eq!(
        Fonts::read_ini(&ini).regular(),
        "Arial",
        "four is too short"
    );
    ini.set(FONTS, "Arial", "Chica");
    assert_eq!(Fonts::read_ini(&ini).regular(), "Chica", "five will do");
    ini.set(FONTS, "Arial", &"x".repeat(Fonts::LONGEST));
    assert_eq!(Fonts::read_ini(&ini).regular().len(), Fonts::LONGEST);
    ini.set(FONTS, "Arial", &"x".repeat(Fonts::LONGEST + 1));
    assert_eq!(
        Fonts::read_ini(&ini).regular(),
        "Arial",
        "thirty-two does not fit the buffer"
    );
}

/// `[Fonts]` is read and never written, so a section a player edited by
/// hand is still there after a round trip — but only because the `Ini`
/// carries through what it is not asked about.
#[test]
fn the_fonts_section_is_never_written_back() {
    use stars_ui::settings::{Fonts, FONTS};
    use stars_ui::App;

    let text = "[Fonts]\nArial=Helvetica\nArialBold=Helvetica Bold\n";
    let mut ini = Ini::parse(text);
    assert_eq!(Fonts::read_ini(&ini).regular(), "Helvetica");

    // Everything this project does write, written.
    let app = App::new();
    app.write_scanner_ini(&mut ini);
    app.write_zip_ini(&mut ini);
    stars_ui::report::Reports::default().write_ini(&mut ini);

    let written = ini.to_string();
    assert!(written.contains("Arial=Helvetica\n"), "{written}");
    assert!(written.contains("ArialBold=Helvetica Bold\n"), "{written}");
    assert_eq!(
        Fonts::read_ini(&Ini::parse(&written)).regular(),
        "Helvetica"
    );
    // And nothing added a key of its own to the section.
    assert_eq!(ini.get(FONTS, "ArialItalic"), None);
}

/// Each report window's place and size go out as a far corner and come
/// back as a position and a size.
#[test]
fn a_report_window_remembers_where_it_was() {
    use stars_ui::report::ReportState;

    let mut reports = Reports::default();
    // Every one starts centred and 600 by 400, as the four `RPT` blocks do.
    for report in Report::ALL {
        assert!(reports.state(report).centred());
        assert_eq!(reports.state(report).size, (600, 400));
    }
    assert_eq!(ReportState::MIN_SIZE, (300, 0xdc));

    let state = reports.state_mut(Report::Fleets);
    state.pos = (120, 90);
    state.size = (640, 480);

    let mut ini = Ini::parse("");
    reports.write_ini(&mut ini);
    // Written as a far corner, and always with the letter `M`.
    assert_eq!(
        ini.get(WINDOWS, "ReportFleetWin"),
        Some("M0120009007600570")
    );

    let mut read = Reports::default();
    read.read_ini(&ini);
    assert_eq!(read.state(Report::Fleets).pos, (120, 90));
    assert_eq!(read.state(Report::Fleets).size, (640, 480));
    assert!(!read.state(Report::Fleets).centred());
}

/// A rectangle the reader cannot use leaves the window where it would have
/// gone anyway, rather than putting it at `-32768`.
#[test]
fn an_unusable_report_rectangle_is_ignored() {
    let mut ini = Ini::parse("");
    ini.set(WINDOWS, "ReportPlanWin", "M-32768000000000000");
    let mut reports = Reports::default();
    reports.read_ini(&ini);
    assert!(
        reports.state(Report::Planets).centred(),
        "CW_USEDEFAULT is not a position"
    );

    ini.set(WINDOWS, "ReportPlanWin", "rubbish");
    let mut reports = Reports::default();
    reports.read_ini(&ini);
    assert!(reports.state(Report::Planets).centred());
    assert_eq!(reports.state(Report::Planets).size, (600, 400));
}

/// The mineral graph's scale: nine choices, the current one ticked, and a
/// value out of range **replaced** rather than clamped.
#[test]
fn the_mineral_scale_is_one_of_nine() {
    use stars_ui::settings::scanner;
    use stars_ui::{App, MINERAL_GRAPH_MAX, MINERAL_SCALES};

    assert_eq!(
        MINERAL_SCALES,
        [100, 500, 1000, 2500, 5000, 7500, 10000, 20000, 30000]
    );

    let mut app = App::new();
    assert_eq!(app.mineral_scale, MINERAL_GRAPH_MAX);
    let (captions, checked) = app.mineral_scale_menu();
    assert_eq!(captions.len(), 9);
    assert_eq!(captions[0], "100kT");
    assert_eq!(checked, Some(4), "5000 is the fifth");

    assert!(app.set_mineral_scale(7));
    assert_eq!(app.mineral_scale, 20000);
    assert!(
        !app.set_mineral_scale(7),
        "choosing it again changes nothing"
    );
    assert!(!app.set_mineral_scale(99), "and there is no tenth");

    // Out and back.
    let mut ini = Ini::parse("");
    app.write_scanner_ini(&mut ini);
    assert_eq!(ini.get(WINDOWS, scanner::MINERAL_SCALE), Some("20000"));
    let mut read = App::new();
    read.read_scanner_ini(&ini);
    assert_eq!(read.mineral_scale, 20000);

    // Outside 100..=30000 goes back to the shipped 5000 — not to the
    // nearest end.
    for silly in ["99", "30001", "0", "-5", "nonsense"] {
        ini.set(WINDOWS, scanner::MINERAL_SCALE, silly);
        let mut read = App::new();
        read.mineral_scale = 100;
        read.read_scanner_ini(&ini);
        assert_eq!(read.mineral_scale, MINERAL_GRAPH_MAX, "{silly}");
    }

    // A value in range that is not on the ladder is kept, and then nothing
    // is ticked.
    ini.set(WINDOWS, scanner::MINERAL_SCALE, "4321");
    let mut read = App::new();
    read.read_scanner_ini(&ini);
    assert_eq!(read.mineral_scale, 4321);
    assert_eq!(read.mineral_scale_menu().1, None);
}
