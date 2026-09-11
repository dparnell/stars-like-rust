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
