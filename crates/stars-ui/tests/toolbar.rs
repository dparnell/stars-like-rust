//! The scanner's toolbar.

use std::path::PathBuf;

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::toolbar::{self, Button, Item};
use stars_ui::{App, ScanView};

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
        name: "toolbar".to_string(),
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

fn frame(app: &mut App) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::toolbar::view(app, ui);
        });
    });
}

/// Every button appears exactly once, and the row comes out in the order the
/// manual introduces them — which is *not* the order of their indices.
#[test]
fn the_layout_puts_them_in_the_manual_s_order() {
    let buttons: Vec<Button> = toolbar::items()
        .into_iter()
        .filter_map(|item| match item {
            Item::Button(button) => Some(button),
            _ => None,
        })
        .collect();
    assert_eq!(buttons.len(), 18, "all eighteen, once each");
    let mut sorted = buttons.clone();
    sorted.sort_by_key(|button| button.index());
    sorted.dedup();
    assert_eq!(sorted.len(), 18, "none twice");

    assert_eq!(
        buttons.iter().map(|b| b.name()).collect::<Vec<_>>(),
        [
            "Normal",
            "Surface Minerals",
            "Mineral Concentration",
            "Planet Value",
            "Population",
            "No Player Information",
            "Add Waypoints Mode",
            "Scanner Coverage",
            "Mine Fields",
            "Fleet Paths",
            "Planet Names",
            "Ship Count",
            "Idle Fleets",
            "Ship Design Filter",
            "Choose a ship design",
            "Enemy Ship Class Filter",
            "Choose an enemy ship class",
            "Zoom",
        ]
    );

    // The indices really are out of order, which is why the table exists.
    let indices: Vec<u8> = buttons.iter().map(|b| b.index()).collect();
    assert!(
        indices.windows(2).any(|w| w[0] > w[1]),
        "{indices:?} is not sorted"
    );

    // The coverage combo sits immediately after the Scanner Coverage button,
    // with only a two-pixel gap between them — which is what identifies that
    // button.
    let row = toolbar::items();
    let at = row
        .iter()
        .position(|item| *item == Item::Button(Button::ScannerCoverage))
        .expect("the coverage button");
    assert_eq!(row[at + 1], Item::Gap(2));
    assert_eq!(row[at + 2], Item::Combo);
}

/// Widths: two narrow buttons hung off the two filters, the rest full width.
#[test]
fn only_the_two_filter_menus_are_narrow() {
    let narrow: Vec<&str> = Button::ALL
        .iter()
        .filter(|b| b.is_narrow())
        .map(|b| b.name())
        .collect();
    assert_eq!(
        narrow,
        ["Choose a ship design", "Choose an enemy ship class"]
    );
    for button in Button::ALL {
        if button.is_narrow() {
            assert_eq!(
                (button.width(), button.art_width()),
                (11, 7),
                "{}",
                button.name()
            );
        } else {
            assert_eq!(
                (button.width(), button.art_width()),
                (29, 24),
                "{}",
                button.name()
            );
        }
    }

    // A narrow menu always sits straight after the filter it belongs to.
    let row = toolbar::items();
    for (filter, menu) in [
        (Button::ShipDesignFilter, Button::ShipDesignMenu),
        (Button::EnemyClassFilter, Button::EnemyClassMenu),
    ] {
        let at = row
            .iter()
            .position(|item| *item == Item::Button(filter))
            .expect("the filter");
        assert_eq!(row[at + 1], Item::Button(menu), "{}", filter.name());
    }
}

/// Each button's picture is one 24-pixel cell of the toolbar bitmap, and all
/// eighteen are really in it.
#[test]
fn every_button_has_a_cell_in_the_sheet() {
    for button in Button::ALL {
        let cell = button.cell();
        assert_eq!(cell.resource, toolbar::SHEET);
        assert_eq!(cell.y, 0, "one row");
        assert_eq!(cell.x, u32::from(button.index()) * 24);
        assert_eq!(cell.height, 23);
    }

    let Some(exe) = executable() else { return };
    let sheet = stars_formats::resources::read_bitmap(
        &exe,
        &stars_formats::resources::Name::Id(toolbar::SHEET),
    )
    .expect("the toolbar bitmap");
    assert_eq!(
        (sheet.width, sheet.height),
        (432, 23),
        "eighteen cells of 24, one row"
    );
    for button in Button::ALL {
        let cell = button.cell();
        assert!(
            sheet
                .crop(cell.x, cell.y, cell.width, cell.height)
                .is_some(),
            "{}",
            button.name()
        );
    }
}

/// The six views are a radio group; everything else toggles.
#[test]
fn the_views_replace_each_other_and_the_rest_toggle() {
    let mut app = a_game();
    assert!(app.toolbar_down(Button::Normal));

    app.toolbar_click(Button::Population);
    assert_eq!(app.scan_view, ScanView::Population);
    assert!(app.toolbar_down(Button::Population));
    assert!(!app.toolbar_down(Button::Normal), "only one at a time");
    // Exactly one view is ever pressed.
    let down = Button::ALL
        .iter()
        .filter(|b| b.is_view() && app.toolbar_down(**b))
        .count();
    assert_eq!(down, 1);

    // Pressing a view again leaves it on rather than turning it off.
    app.toolbar_click(Button::Population);
    assert!(app.toolbar_down(Button::Population));

    // Mine Fields is not in this list: it opens a menu rather than toggling,
    // and its own tests cover it.
    for overlay in [
        Button::PlanetNames,
        Button::FleetPaths,
        Button::IdleFleets,
        Button::ShipCount,
        Button::ScannerCoverage,
        Button::AddWaypoints,
    ] {
        let before = app.toolbar_down(overlay);
        app.toolbar_click(overlay);
        assert_ne!(app.toolbar_down(overlay), before, "{}", overlay.name());
        app.toolbar_click(overlay);
        assert_eq!(app.toolbar_down(overlay), before, "{}", overlay.name());
    }
    assert_eq!(
        app.scan_view,
        ScanView::Population,
        "and the view is untouched"
    );
}

/// Every button on the row is wired, the two ship filters included.
#[test]
fn every_button_is_wired() {
    let mut app = a_game();
    for button in Button::ALL {
        assert!(app.toolbar_enabled(button), "{}", button.name());
    }
    // The two filter toggles behave like the other overlays.
    for filter in [Button::ShipDesignFilter, Button::EnemyClassFilter] {
        assert!(!app.toolbar_down(filter));
        app.toolbar_click(filter);
        assert!(app.toolbar_down(filter), "{}", filter.name());
        app.toolbar_click(filter);
        assert!(!app.toolbar_down(filter), "{}", filter.name());
    }
    // The menu buttons open a menu instead of acting themselves.
    for menu in [Button::ShipDesignMenu, Button::EnemyClassMenu] {
        app.toolbar_click(menu);
        assert!(!app.toolbar_down(menu), "{}", menu.name());
    }
}

/// The coverage combo reads what was typed the way the original reads it.
#[test]
fn the_coverage_combo_parses_and_clamps() {
    assert_eq!(toolbar::coverage_from_text("75"), 75);
    assert_eq!(
        toolbar::coverage_from_text("75%"),
        75,
        "a trailing % is fine"
    );
    assert_eq!(toolbar::coverage_from_text("100"), 100);
    // Out of range is clamped to 2..=100 rather than refused.
    assert_eq!(toolbar::coverage_from_text("150"), 100);
    assert_eq!(toolbar::coverage_from_text("1"), 2);
    assert_eq!(toolbar::coverage_from_text("0"), 2);
    // Anything the original would not accept reads as zero, and so clamps up.
    assert_eq!(toolbar::coverage_from_text("75 percent"), 2);
    assert_eq!(toolbar::coverage_from_text("abc"), 2);
    assert_eq!(toolbar::coverage_from_text(""), 2);

    // The list runs from a hundred down in tens.
    assert_eq!(toolbar::COVERAGE_STEPS.first(), Some(&100));
    assert_eq!(toolbar::COVERAGE_STEPS.last(), Some(&10));
    for (index, step) in toolbar::COVERAGE_STEPS.iter().enumerate() {
        assert_eq!(usize::from((100 - *step) / 10), index, "{step}%");
    }

    let mut app = a_game();
    assert_eq!(app.scan_coverage_pct, 100, "full coverage to begin with");
    app.set_scan_coverage("60%");
    assert_eq!(app.scan_coverage_pct, 60);
}

/// Zoom walks the nine sizes the original's menu offers.
#[test]
fn zoom_walks_the_nine_sizes() {
    let mut app = a_game();
    assert_eq!(App::ZOOM_PERCENT.len(), 9);
    let mut seen = Vec::new();
    for _ in 0..9 {
        seen.push(app.scan_zoom_percent());
        app.toolbar_click(Button::Zoom);
    }
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 9, "all nine, once each: {seen:?}");
    assert_eq!(app.scan_zoom_percent(), seen[0], "and round again");
}

/// It draws, with the game's pictures and without them.
#[test]
fn the_toolbar_draws() {
    let mut app = a_game();
    frame(&mut app);
    for button in Button::ALL {
        app.toolbar_click(button);
        frame(&mut app);
    }
    if let Some(exe) = executable() {
        app.load_art(exe, "the test's own copy").expect("pictures");
        frame(&mut app);
        frame(&mut app);
    }
}

/// `fDown` is a distance, not a flag: a latched button pushes its picture one
/// pixel and one held under the pointer two.
#[test]
fn a_button_is_pressed_by_a_number_of_pixels() {
    assert_eq!(toolbar::Press::Up.offset(), 0.0);
    assert_eq!(toolbar::Press::Latched.offset(), 1.0);
    assert_eq!(toolbar::Press::Held.offset(), 2.0);

    assert!(!toolbar::Press::Up.is_down());
    assert!(toolbar::Press::Latched.is_down());
    assert!(toolbar::Press::Held.is_down());
}

/// The strip's own measurements, straight out of `DrawBitmapButton` and
/// `ItbFromPpt`.
#[test]
fn the_strip_is_the_size_the_original_draws() {
    assert_eq!(toolbar::BUTTON_HEIGHT, 28, "rows y..y+0x1b");
    assert_eq!(toolbar::ROW_HEIGHT, 32, "a click lands on 4 <= y < 0x20");
    assert_eq!(toolbar::MARGIN, 4, "the first button starts at x = 4");

    // And the row adds up the way the original's table does: the margin, then
    // every item's own width.
    let width: u32 = toolbar::MARGIN
        + toolbar::items()
            .iter()
            .map(|item| item.width())
            .sum::<u32>();
    assert!(width > 400 && width < 700, "a plausible strip: {width}");
}

/// The three colours are the Windows 3.1 defaults for the button system
/// colours the original asks for.
#[test]
fn the_bevel_is_the_windows_grey() {
    assert_eq!(toolbar::FACE, [0xc0, 0xc0, 0xc0]);
    assert_eq!(toolbar::HILITE, [0xff, 0xff, 0xff]);
    assert_eq!(toolbar::SHADOW, [0x80, 0x80, 0x80]);
}

/// The tooltip's own timing, which is what makes it the program's rather than
/// the toolkit's.
#[test]
fn a_tooltip_waits_the_first_time_and_not_the_next() {
    use stars_ui::toolbar::{Tip, Tooltip, TOOLTIP_DELAY, TOOLTIP_LIFETIME, TOOLTIP_REPEAT};
    let mut tips = Tooltip::default();
    let button = Tip::Button(Button::Normal);

    // Resting on a button: nothing for seven tenths of a second.
    tips.hover(Some(button), 0.0);
    assert_eq!(tips.showing(), None);
    tips.hover(Some(button), TOOLTIP_DELAY - 0.01);
    assert_eq!(tips.showing(), None);
    tips.hover(Some(button), TOOLTIP_DELAY);
    assert_eq!(tips.showing(), Some(button));

    // Moving to the next button shows its tooltip at once, because the last
    // one closed just now.
    let next = Tip::Button(Button::Population);
    tips.hover(Some(next), TOOLTIP_DELAY + 0.05);
    assert_eq!(tips.showing(), Some(next), "no wait along a row");

    // Leave the toolbar, come back after longer than the repeat window, and
    // the wait is back.
    tips.hover(None, 2.0);
    assert_eq!(tips.showing(), None);
    tips.hover(Some(button), 2.0 + TOOLTIP_REPEAT + 0.01);
    assert_eq!(tips.showing(), None, "too long ago to follow on");

    // And one left up puts itself away after ten seconds.
    let mut tips = Tooltip::default();
    tips.hover(Some(button), 0.0);
    tips.hover(Some(button), TOOLTIP_DELAY);
    assert_eq!(tips.showing(), Some(button));
    tips.hover(Some(button), TOOLTIP_DELAY + TOOLTIP_LIFETIME - 0.01);
    assert_eq!(tips.showing(), Some(button));
    tips.hover(Some(button), TOOLTIP_DELAY + TOOLTIP_LIFETIME);
    assert_eq!(tips.showing(), None, "ten seconds is its lifetime");
}

/// A click takes it away.
#[test]
fn a_click_dismisses_the_tooltip() {
    use stars_ui::toolbar::{Tip, Tooltip, TOOLTIP_DELAY};
    let mut tips = Tooltip::default();
    let button = Tip::Button(Button::MineFields);
    tips.hover(Some(button), 0.0);
    tips.hover(Some(button), TOOLTIP_DELAY);
    assert_eq!(tips.showing(), Some(button));
    tips.dismiss(TOOLTIP_DELAY);
    assert_eq!(tips.showing(), None);
}

/// The tooltips are the game's own strings, one contiguous block in button
/// order — which is what confirms the order in the first place.
#[test]
fn every_button_has_the_games_own_tooltip() {
    assert_eq!(Button::Normal.tooltip(), "Normal View");
    assert_eq!(Button::Normal.tooltip_id(), 0x16a);
    assert_eq!(Button::ShipCount.tooltip(), "Ship Counts Overlay");
    assert_eq!(Button::ShipCount.tooltip_id(), 0x17b);
    assert_eq!(toolbar::COMBO_TOOLTIP, "Scanner Effective %");

    // Every id is used once, and they run without a gap.
    let ids: Vec<u16> = Button::ALL.iter().map(|b| b.tooltip_id()).collect();
    assert_eq!(ids.first(), Some(&0x16a));
    assert_eq!(ids.last(), Some(&0x17b));
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, 0x16a + u16::try_from(i).expect("an index"));
    }
}

/// Everything the toolbar owns survives a restart: the view, the six
/// overlays, the add-waypoints mode, all three filter masks, the coverage
/// percentage, the zoom and whether the row is showing at all.
///
/// `ReadIniSettings` and `WriteIniSettings` keep the lot in `[Windows]`;
/// the spec used to say they did not, which was true only until they did.
#[test]
fn the_whole_toolbar_survives_a_restart() {
    use stars_ui::settings::Ini;
    use stars_ui::{App, ScanOverlays, ScanView};

    let mut app = App::new();
    app.scan_view = ScanView::Population;
    app.scan_overlays = ScanOverlays {
        names: true,
        scanner_coverage: true,
        minefields: false,
        fleet_paths: true,
        ship_counts: true,
        idle_fleets: false,
        ship_design_filter: true,
        enemy_class_filter: false,
        player_colours: true,
    };
    app.add_waypoints = true;
    app.scan_design_filter = 0b0000_0010_0100_0001;
    app.scan_class_filter = 0b1000_0101;
    app.scan_minefield_filter = 0b0011;
    app.scan_coverage_pct = 40;
    app.scan_zoom = 2;
    app.toolbar_hidden = true;

    let mut ini = Ini::parse("");
    app.write_scanner_ini(&mut ini);
    let mut back = App::new();
    back.read_scanner_ini(&ini);

    assert_eq!(back.scan_view, app.scan_view);
    assert_eq!(back.scan_overlays, app.scan_overlays);
    assert_eq!(back.add_waypoints, app.add_waypoints);
    assert_eq!(back.scan_design_filter, app.scan_design_filter);
    assert_eq!(back.scan_class_filter, app.scan_class_filter);
    assert_eq!(back.scan_minefield_filter, app.scan_minefield_filter);
    assert_eq!(back.scan_coverage_pct, app.scan_coverage_pct);
    assert_eq!(back.scan_zoom, app.scan_zoom);
    assert_eq!(back.toolbar_hidden, app.toolbar_hidden);

    // And every button reads back latched the way it was left.
    for button in stars_ui::toolbar::Button::ALL {
        assert_eq!(
            back.toolbar_down(button),
            app.toolbar_down(button),
            "{button:?}"
        );
    }
}

/// The two filter menus open from their eleven-pixel arrows — narrower
/// than egui's popup frame, which once tripped its minimum-width
/// assertion — and their commands act: All Designs on the design filter,
/// No Designs on the class filter.
#[test]
fn the_filter_menus_open_from_their_arrows() {
    let mut shell = stars_ui::autopilot::Shell::new(a_game());
    shell.frame();
    shell.frame();
    let arrows: Vec<egui::Rect> = shell
        .app
        .drawn
        .iter()
        .filter(|w| w.scope == "toolbar" && w.label == "▾")
        .map(|w| w.rect)
        .collect();
    assert_eq!(arrows.len(), 2, "the design and the enemy class arrows");

    shell.click_at(arrows[0].center());
    shell.frame();
    assert!(
        shell.app.drawn.iter().any(|w| w.label == "All Designs"),
        "the design menu is open"
    );
    shell.press_unchecked("toolbar", "All Designs");
    shell.frame();
    assert_eq!(shell.app.scan_design_filter, u16::MAX);
    assert!(shell.app.scan_overlays.ship_design_filter);

    shell.app.scan_class_filter = 0b101;
    shell.click_at(arrows[1].center());
    shell.frame();
    assert!(
        shell.app.drawn.iter().any(|w| w.label == "Scout"),
        "the class menu lists the classes"
    );
    shell.press_unchecked("toolbar", "No Designs");
    shell.frame();
    assert_eq!(shell.app.scan_class_filter, 0);
}
