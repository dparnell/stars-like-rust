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

    for overlay in [
        Button::PlanetNames,
        Button::MineFields,
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
    // The two menus never show pressed; they open a menu instead.
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
