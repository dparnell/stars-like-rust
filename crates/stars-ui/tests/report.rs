//! The four report windows: their columns, their menus and their sort.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::report::{ColumnMenu, Data, Entry, Report, ReportState, Reports, Subsort};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "reports".to_string(),
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

/// Three more planets of our own, all named the same, with the populations
/// given. Returns the app so the caller can borrow the game.
fn planets_with_populations(app: &mut App, pops: &[i32]) {
    let game = app.game.as_mut().expect("a game");
    let template = game
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .expect("a home world")
        .clone();
    game.planets.retain(|p| p.owner != Some(0));
    for (index, pop) in pops.iter().enumerate() {
        let mut planet = template.clone();
        planet.id = 200 + i16::try_from(index).expect("a small index");
        planet.pop = *pop;
        game.planets.push(planet);
    }
}

/// The four column tables, as the string table spells them.
#[test]
fn every_report_has_the_columns_the_binary_gives_it() {
    assert_eq!(Report::Planets.columns().len(), 15);
    assert_eq!(Report::Fleets.columns().len(), 12);
    assert_eq!(Report::EnemyFleets.columns().len(), 12);
    assert_eq!(Report::Battles.columns().len(), 15);

    assert_eq!(Report::Planets.columns()[0].name, "Planet Name");
    assert_eq!(Report::Planets.columns()[2].name, "Population");
    assert_eq!(Report::Planets.columns()[4].name, "Value");
    assert_eq!(Report::Planets.columns()[11].name, "Min Conc");
    assert_eq!(Report::Planets.columns()[14].name, "Routing Dest");
    assert_eq!(Report::Fleets.columns()[10].name, "Battle Plan");
    assert_eq!(Report::EnemyFleets.columns()[3].name, "Warp");
    assert_eq!(Report::Battles.columns()[14].name, "Theirs Left");

    // Only four columns in the whole game open onto a mineral choice.
    let grouped: Vec<(i16, usize)> = Report::ALL
        .iter()
        .flat_map(|r| {
            r.columns()
                .iter()
                .enumerate()
                .filter(|(_, c)| c.subsort != Subsort::None)
                .map(move |(i, _)| (r.irpt(), i))
        })
        .collect();
    assert_eq!(grouped, vec![(0, 9), (0, 10), (0, 11), (1, 7)]);
}

/// The captions count their rows, and the `%c` at the end is the plural.
#[test]
fn the_caption_counts_what_it_shows() {
    assert_eq!(
        Report::Planets.title(24),
        "Planet Summary Report -- 24 Planets"
    );
    assert_eq!(
        Report::Planets.title(1),
        "Planet Summary Report -- 1 Planet "
    );
    assert_eq!(
        Report::EnemyFleets.title(0),
        "Others' Fleets Summary Report -- 0 Fleets"
    );
}

/// The state each `RPT` starts in, from the four blocks at `1120:1494`.
#[test]
fn a_report_starts_with_every_column_sorted_by_the_first() {
    let state = ReportState::new();
    assert_eq!(state.visible, 0x0000_ffff);
    assert_eq!(state.sort, 0);
    assert!(state.ascending);
    assert_eq!(state.subsort, 0);
    assert_eq!(state.first_field, 1);
    assert_eq!(state.first_row, 0);
    assert_eq!(state.drawn(Report::Planets).len(), 15);

    // Nothing has been sorted yet, so there is no tie-break to fall back on.
    assert!(Reports::default().previous.sort < 0);
}

/// Column 0 never scrolls away, and the horizontal scroll starts at the
/// column `cFieldFirst` names.
#[test]
fn the_first_column_stays_while_the_rest_scroll() {
    let mut state = ReportState::new();
    state.first_field = 5;
    assert_eq!(
        state.drawn(Report::Planets),
        vec![0, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    state.visible &= !(1 << 7);
    assert!(!state.drawn(Report::Planets).contains(&7));
}

/// A plain column's menu: sort, reverse sort, and — unless it is column 0 —
/// an offer to hide it.
#[test]
fn a_plain_column_offers_two_sorts_and_a_hide() {
    let state = ReportState::new();
    let menu = ColumnMenu::build(Report::Planets, 2, &state);
    assert_eq!(
        menu.entries[0],
        Entry::Item("Sort by Population".to_string())
    );
    assert_eq!(
        menu.entries[1],
        Entry::Item("Reverse Sort by Population".to_string())
    );
    assert_eq!(menu.entries[2], Entry::Separator);
    assert_eq!(
        menu.entries[3],
        Entry::Item("Hide the Population column".to_string())
    );
    assert_eq!(menu.hide, Some(3));
    assert_eq!(menu.sort_below, 3);
    // Nothing hidden yet, so the trailing separator goes too.
    assert_eq!(menu.entries.len(), 4);

    assert_eq!(menu.sort_choice(Report::Planets, 0), (true, 0));
    assert_eq!(menu.sort_choice(Report::Planets, 1), (false, 0));
}

/// The name column cannot be hidden: it is the one that never scrolls off,
/// and the original writes the Hide item then overwrites it.
#[test]
fn the_name_column_cannot_be_hidden() {
    let menu = ColumnMenu::build(Report::Planets, 0, &ReportState::new());
    assert_eq!(menu.hide, None);
    assert!(!menu
        .entries
        .iter()
        .any(|e| matches!(e, Entry::Item(text) if text.starts_with("Hide"))));
}

/// A mineral column opens two submenus, and the flat indices the original's
/// arithmetic is written against are these.
#[test]
fn a_mineral_column_opens_a_submenu_of_minerals() {
    let menu = ColumnMenu::build(Report::Planets, 11, &ReportState::new());
    assert_eq!(menu.entries[0], Entry::Submenu);
    assert_eq!(menu.entries[1], Entry::Item("Sort by Min Conc".to_string()));
    assert_eq!(menu.entries[2], Entry::Item("Ironium".to_string()));
    assert_eq!(menu.entries[3], Entry::Item("Boranium".to_string()));
    assert_eq!(menu.entries[4], Entry::Item("Germanium".to_string()));
    assert_eq!(menu.entries[5], Entry::Separator);
    assert_eq!(menu.entries[6], Entry::Item("Weighted Average".to_string()));
    assert_eq!(menu.entries[7], Entry::Submenu);
    assert_eq!(
        menu.entries[9],
        Entry::Item("Reverse Sort by Min Conc".to_string())
    );

    for (index, want) in [
        (2, (true, 0)),
        (3, (true, 1)),
        (4, (true, 2)),
        (6, (true, 3)),
    ] {
        assert_eq!(menu.sort_choice(Report::Planets, index), want);
    }
    for (index, want) in [
        (10, (false, 0)),
        (11, (false, 1)),
        (12, (false, 2)),
        (14, (false, 3)),
    ] {
        assert_eq!(menu.sort_choice(Report::Planets, index), want);
    }
}

/// The fleets' Cargo column lists colonists as well, which pushes Weighted
/// Average one place down — and the original's direction test does not
/// follow it. Sorting cargo by Weighted Average sorts **descending** from
/// either submenu. Reproduced deliberately; see `docs/ui/reports.md`.
#[test]
fn sorting_cargo_by_weighted_average_runs_backwards() {
    let menu = ColumnMenu::build(Report::Fleets, 7, &ReportState::new());
    assert_eq!(menu.entries[5], Entry::Item("Colonists".to_string()));
    assert_eq!(menu.entries[7], Entry::Item("Weighted Average".to_string()));

    assert_eq!(menu.sort_choice(Report::Fleets, 2), (true, 0));
    assert_eq!(menu.sort_choice(Report::Fleets, 5), (true, 3));
    assert_eq!(
        menu.sort_choice(Report::Fleets, 7),
        (false, 4),
        "the ascending submenu's Weighted Average sorts descending"
    );
    assert_eq!(menu.sort_choice(Report::Fleets, 16), (false, 4));
}

/// Hiding a column takes it out of the grid and puts it in every other
/// column's menu.
#[test]
fn hiding_a_column_offers_it_back() {
    let mut reports = Reports::default();
    let menu = ColumnMenu::build(Report::Planets, 6, reports.state(Report::Planets));
    let hide = menu.hide.expect("Mine can be hidden");
    reports.choose(Report::Planets, 6, hide);
    assert!(!reports.state(Report::Planets).shows(6));
    assert!(!reports
        .state(Report::Planets)
        .drawn(Report::Planets)
        .contains(&6));

    let menu = ColumnMenu::build(Report::Planets, 2, reports.state(Report::Planets));
    assert_eq!(menu.show, vec![6]);
    assert_eq!(
        menu.entries[menu.show_from],
        Entry::Item("Show the Mine column".to_string())
    );
    reports.choose(Report::Planets, 2, menu.show_from);
    assert!(reports.state(Report::Planets).shows(6));
}

/// Choosing a sort remembers the one before it, and the memory is shared by
/// all four reports because the original keeps it in globals.
#[test]
fn the_previous_sort_is_remembered_across_reports() {
    let mut reports = Reports::default();
    reports.sort_by(Report::Planets, 2, true, 0);
    assert_eq!(reports.previous.sort, 0, "the name column it started on");

    reports.sort_by(Report::Planets, 0, true, 0);
    assert_eq!(reports.previous.sort, 2);
    assert_eq!(reports.state(Report::Planets).sort, 0);

    reports.sort_by(Report::Fleets, 4, false, 0);
    assert_eq!(
        reports.previous.sort, 0,
        "the fleets report pushed down what the planets report had"
    );
    assert!(reports.previous.ascending);
}

/// Rows the chosen column cannot separate fall back to the column sorted
/// before it — the whole point of keeping the previous one.
#[test]
fn equal_rows_keep_the_order_of_the_previous_sort() {
    let mut app = a_game();
    planets_with_populations(&mut app, &[1000, 3000, 2000]);
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let mut reports = Reports::default();

    // Sort by population, ascending.
    reports.sort_by(Report::Planets, 2, true, 0);
    let mut rows = data.rows(Report::Planets);
    assert_eq!(rows.len(), 3);
    data.sort(Report::Planets, &reports, &mut rows);
    let pops: Vec<i32> = rows.iter().map(|&r| game.planets[r].pop).collect();
    assert_eq!(pops, vec![1000, 2000, 3000]);

    // Now by name. All three share one, so the population order survives.
    reports.sort_by(Report::Planets, 0, true, 0);
    let mut rows = data.rows(Report::Planets);
    data.sort(Report::Planets, &reports, &mut rows);
    let pops: Vec<i32> = rows.iter().map(|&r| game.planets[r].pop).collect();
    assert_eq!(pops, vec![1000, 2000, 3000]);

    // And a descending previous sort reverses the tie-break with it.
    reports.previous.ascending = false;
    let mut rows = data.rows(Report::Planets);
    data.sort(Report::Planets, &reports, &mut rows);
    let pops: Vec<i32> = rows.iter().map(|&r| game.planets[r].pop).collect();
    assert_eq!(pops, vec![3000, 2000, 1000]);
}

/// With nothing sorted before it there is no fallback, and the sort leaves
/// equal rows where it found them.
#[test]
fn the_first_sort_has_nothing_to_fall_back_on() {
    let mut app = a_game();
    planets_with_populations(&mut app, &[1000, 3000, 2000]);
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let reports = Reports::default();
    assert!(reports.previous.sort < 0);
    let mut rows = data.rows(Report::Planets);
    data.sort(Report::Planets, &reports, &mut rows);
    let pops: Vec<i32> = rows.iter().map(|&r| game.planets[r].pop).collect();
    assert_eq!(
        pops,
        vec![1000, 3000, 2000],
        "left as the galaxy holds them"
    );
}

/// The planets report lists your own planets and nothing you have merely
/// scanned; the fleets reports split on ownership.
#[test]
fn each_report_lists_what_the_original_puts_in_it() {
    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let planets = data.rows(Report::Planets);
    assert!(!planets.is_empty());
    assert!(planets
        .iter()
        .all(|&r| game.planets[r].owner == Some(0) && game.planets[r].detail.is_full()));

    let mine = data.rows(Report::Fleets);
    let theirs = data.rows(Report::EnemyFleets);
    assert!(mine.iter().all(|&r| game.fleets[r].owner == 0));
    assert!(theirs.iter().all(|&r| game.fleets[r].owner != 0));
    assert_eq!(mine.len() + theirs.len(), game.fleets.len());

    assert!(data.rows(Report::Battles).is_empty());
}

/// `CommaFormatLong` puts a separator every three digits, and keeps the sign
/// outside them.
#[test]
fn figures_are_written_with_thousands_separators() {
    use stars_ui::report::commas;
    assert_eq!(commas(0), "0");
    assert_eq!(commas(999), "999");
    assert_eq!(commas(1000), "1,000");
    assert_eq!(commas(1_234_567), "1,234,567");
    assert_eq!(commas(-25_000), "-25,000");
}

/// The name cell carries the planet's name and, when it has a starbase, a
/// bar for each thing that base can do.
#[test]
fn a_home_worlds_name_cell_shows_its_starbase() {
    use stars_ui::report::{Bar, Cell};

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let home = data
        .rows(Report::Planets)
        .into_iter()
        .find(|&r| game.planets[r].homeworld)
        .expect("a home world");

    let Cell::Name(name, bars) = data.cell(Report::Planets, home, 0).cell else {
        panic!("the name column draws a name");
    };
    assert!(!name.is_empty());
    assert_eq!(
        bars.first(),
        Some(&Bar::Base { cargo: true }),
        "a home world starts with a starbase that can hold cargo"
    );

    // Population reads as a figure with separators, and the starbase column
    // names the design rather than leaving a dash.
    let Cell::Right(pop) = data.cell(Report::Planets, home, 2).cell else {
        panic!("population is a figure");
    };
    assert!(pop.contains(','), "{pop} should carry a separator");
    let Cell::Left(base) = data.cell(Report::Planets, home, 1).cell else {
        panic!("the starbase column is text");
    };
    assert_ne!(base, "--");
}

/// A planet with nothing routed anywhere reads as two dashes, which is what
/// `szDblDash` puts there.
#[test]
fn nothing_there_reads_as_two_dashes() {
    use stars_ui::report::{Cell, DOUBLE_DASH};

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let row = data.rows(Report::Planets)[0];
    assert_eq!(
        data.cell(Report::Planets, row, 14).cell,
        Cell::Left(DOUBLE_DASH.to_string()),
        "a planet routes nowhere to start with"
    );
}

/// A fleet's row reads the way the original writes it: `#id`, a location, a
/// cargo hold of four figures.
#[test]
fn a_fleets_row_reads_as_the_original_writes_it() {
    use stars_ui::report::Cell;

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let row = *data.rows(Report::Fleets).first().expect("a fleet");

    let Cell::Right(id) = data.cell(Report::Fleets, row, 1).cell else {
        panic!("the id column is a figure");
    };
    assert!(id.starts_with('#'), "{id}");

    let Cell::Minerals(cargo) = data.cell(Report::Fleets, row, 7).cell else {
        panic!("cargo is four figures");
    };
    assert_eq!(cargo.len(), 4);

    let Cell::Left(location) = data.cell(Report::Fleets, row, 2).cell else {
        panic!("location is text");
    };
    assert!(!location.is_empty());
}

/// Clicking a planet's row does what clicking the same figure in the pane
/// does — `ExecuteReportClick`, column by column.
#[test]
fn a_planets_columns_raise_what_the_panes_raise() {
    use stars_ui::report::{Click, PopupKind, BAR_STRIP};

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let home = data
        .rows(Report::Planets)
        .into_iter()
        .find(|&r| game.planets[r].homeworld)
        .expect("a home world");
    let at = |column: usize, x: f32| data.click(Report::Planets, home, column, x, 100.0, false);

    // The name column is the name, except in the strip the starbase bars are
    // drawn in.
    assert_eq!(at(0, 10.0), Click::Select);
    assert_eq!(
        at(0, 100.0 - BAR_STRIP + 1.0),
        Click::Popup(PopupKind::Starbase),
        "the bars at the right edge are the starbase"
    );
    assert_eq!(at(1, 10.0), Click::Popup(PopupKind::Starbase));
    assert_eq!(at(2, 10.0), Click::Popup(PopupKind::Population));
    assert_eq!(at(3, 10.0), Click::Select, "Cap raises nothing");
    assert_eq!(at(4, 10.0), Click::Popup(PopupKind::Population));
    assert_eq!(at(5, 10.0), Click::Production);
    assert_eq!(
        at(6, 10.0),
        Click::Popup(PopupKind::Industry { factories: false })
    );
    assert_eq!(
        at(7, 10.0),
        Click::Popup(PopupKind::Industry { factories: true })
    );
    assert_eq!(at(12, 10.0), Click::Popup(PopupKind::Resources));
    assert_eq!(at(13, 10.0), Click::Select);
}

/// A mineral cell holds three figures, and which one was clicked decides
/// which mineral the pop-up is about.
#[test]
fn a_mineral_cell_is_three_figures_in_one() {
    use stars_ui::report::{Click, PopupKind};

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let row = data.rows(Report::Planets)[0];
    for column in [9, 10, 11] {
        for (x, want) in [(1.0, 0), (35.0, 1), (70.0, 2), (89.0, 2)] {
            assert_eq!(
                data.click(Report::Planets, row, column, x, 90.0, false),
                Click::Popup(PopupKind::Mineral(want)),
                "column {column} at {x}"
            );
        }
    }
}

/// While the production queue is up the report will not act at all — the
/// original beeps rather than selecting.
#[test]
fn the_planets_report_is_deaf_while_the_queue_is_open() {
    use stars_ui::report::Click;

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    let row = data.rows(Report::Planets)[0];
    assert_eq!(
        data.click(Report::Planets, row, 2, 10.0, 100.0, true),
        Click::Refused
    );
    assert_eq!(
        data.click(Report::Planets, row, 0, 10.0, 100.0, true),
        Click::Refused,
        "not even the selection"
    );
}

/// A fleet's Destination, ETA and Task take hold of its first waypoint — but
/// only when it has one to hold.
#[test]
fn a_fleets_orders_columns_take_hold_of_the_waypoint() {
    use stars_ui::report::{Click, PopupKind};

    let mut app = a_game();
    let row = {
        let game = app.game.as_ref().expect("a game");
        let data = Data {
            game,
            player: 0,
            battles: &[],
        };
        *data.rows(Report::Fleets).first().expect("a fleet")
    };

    {
        let game = app.game.as_ref().expect("a game");
        let data = Data {
            game,
            player: 0,
            battles: &[],
        };
        // A new fleet sits on waypoint 0 and is going nowhere.
        assert_eq!(game.fleets[row].waypoints.len(), 1);
        for column in [3, 4, 5] {
            assert_eq!(
                data.click(Report::Fleets, row, column, 5.0, 50.0, false),
                Click::Select,
                "column {column} with nowhere to go"
            );
        }
        assert_eq!(
            data.click(Report::Fleets, row, 6, 5.0, 50.0, false),
            Click::Transfer
        );
        assert_eq!(
            data.click(Report::Fleets, row, 7, 5.0, 50.0, false),
            Click::Transfer
        );
        assert_eq!(
            data.click(Report::Fleets, row, 8, 5.0, 50.0, false),
            Click::Popup(PopupKind::Fleet)
        );
        assert_eq!(
            data.click(Report::Fleets, row, 11, 5.0, 50.0, false),
            Click::Select
        );
    }

    // Give it somewhere to go and the three columns wake up.
    let there = app.game.as_ref().expect("a game").fleets[row].waypoints[0].clone();
    if let Some(game) = app.game.as_mut() {
        game.fleets[row].waypoints.push(there);
    }
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    for column in [3, 4, 5] {
        assert_eq!(
            data.click(Report::Fleets, row, column, 5.0, 50.0, false),
            Click::Waypoint,
            "column {column} with an order"
        );
    }
}

/// A row of the Battles report opens the recording, whichever column was
/// clicked.
#[test]
fn a_battle_row_opens_the_recording() {
    use stars_ui::report::Click;

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &[],
    };
    for column in [0, 3, 14] {
        assert_eq!(
            data.click(Report::Battles, 0, column, 5.0, 50.0, false),
            Click::Vcr
        );
    }
    // Everybody else's fleets only select, which is what puts them on the map.
    assert_eq!(
        data.click(Report::EnemyFleets, 0, 5, 5.0, 50.0, false),
        Click::Select
    );
}

/// The Defense column raises the best defence the race can build, which for
/// a starting race is the SDI.
#[test]
fn the_defense_column_names_the_best_defence() {
    let app = a_game();
    let (category, item) = app.best_defense_part().expect("a race can build an SDI");
    assert_eq!(category, stars_core::components::slot::PLANETARY);
    assert_eq!(item, 9, "SDI, with nothing better researched yet");
}

/// The mineral pop-up the three mineral columns raise: a figure a row, and
/// `Unknown` where there is no figure.
#[test]
fn the_mineral_popup_says_what_is_known() {
    use stars_ui::popup::{Popup, HOME_FLOOR, HOME_WORLD};

    let app = a_game();
    let game = app.game.as_ref().expect("a game");
    let home = game
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .expect("a home world")
        .id;

    let Some(Popup::Mineral(summary)) = app.mineral_popup(home, 0) else {
        panic!("a mineral pop-up");
    };
    assert_eq!(summary.mineral, 0);
    assert!(summary.surface.is_some(), "we live there, so we know");
    assert!(summary.rate.is_some(), "and we are mining it");
    assert!(
        summary.home_note == Some(HOME_FLOOR) || summary.home_note == Some(HOME_WORLD),
        "a home world says which"
    );

    // Somewhere we have never been: no surface total and no rate.
    let far = game
        .planets
        .iter()
        .chain(game.known_planets.iter())
        .find(|p| !p.detail.is_full())
        .map(|p| p.id);
    if let Some(far) = far {
        let Some(Popup::Mineral(summary)) = app.mineral_popup(far, 2) else {
            panic!("a mineral pop-up");
        };
        assert_eq!(summary.surface, None);
        assert_eq!(summary.rate, None);
        assert_eq!(summary.home_note, None);
    }
}

/// A battle with no planet — one in deep space — and one at a planet, so
/// the two-step click can be told apart.
fn a_battle(id: u16, planet: u16) -> stars_formats::battle::BattleRecord {
    stars_formats::battle::BattleRecord {
        id,
        players: 2,
        player_mask: 0b11,
        planet,
        position: (100, 200),
        tokens: Vec::new(),
        actions: Vec::new(),
        declared_len: 0,
    }
}

/// The Battles report lists the recordings the file carries, and its
/// Location column names the planet or the point.
#[test]
fn the_battles_report_lists_the_recordings() {
    use stars_ui::report::Cell;

    let mut app = a_game();
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .expect("a home world")
        .id;
    let named = u16::try_from(home).expect("a planet id");
    app.battles = vec![a_battle(1, named), a_battle(2, u16::MAX)];

    let game = app.game.as_ref().expect("a game");
    let data = Data {
        game,
        player: 0,
        battles: &app.battles,
    };
    assert_eq!(data.rows(Report::Battles), vec![0, 1]);

    let Cell::Left(here) = data.cell(Report::Battles, 0, 0).cell else {
        panic!("the location is text");
    };
    assert!(!here.is_empty() && here != "--", "{here}");
    let Cell::Left(there) = data.cell(Report::Battles, 1, 0).cell else {
        panic!("the location is text");
    };
    assert_eq!(there, "Space: (100, 200)", "deep space gives the point");

    // With no tokens in the recording, the SB column is blank and every
    // count is nothing.
    let Cell::Centre(sb) = data.cell(Report::Battles, 0, 1).cell else {
        panic!("SB is one centred letter");
    };
    assert_eq!(sb, " ", "nobody had a base there");
    let Cell::Right(sides) = data.cell(Report::Battles, 0, 2).cell else {
        panic!("Sides is a figure");
    };
    assert_eq!(sides, "2");
}

/// A battle at a planet takes two clicks: the first moves the selection to
/// where it happened, the second opens the recording. One in deep space has
/// nothing to select, so it opens at once.
#[test]
fn a_battle_row_selects_before_it_plays() {
    use stars_ui::report::battle_opens;

    let mut app = a_game();
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .expect("a home world")
        .id;
    app.battles = vec![a_battle(1, u16::try_from(home).expect("a planet id"))];
    app.selection.planet = None;
    app.vcr = None;

    // Nothing selected: the click moves the selection and stops.
    assert!(!battle_opens(Some(home), None, false));
    // Selected there already: it plays.
    assert!(battle_opens(Some(home), Some(home), false));
    // Somewhere else selected: back to step one.
    assert!(!battle_opens(Some(home), Some(home + 1), false));
    // Deep space has nothing to select, so it plays at once.
    assert!(battle_opens(None, None, false));
    // And a recording already up is never swapped for another.
    assert!(!battle_opens(None, None, true));
    assert!(!battle_opens(Some(home), Some(home), true));

    app.select_object(stars_ui::app::ScanObject::Planet(home));
    assert!(app.vcr.is_none(), "selecting a planet plays nothing");
    app.open_battle(0);
    assert!(app.vcr.is_some());
}

/// The industry pop-up: what is built, what fits, and what can be staffed.
#[test]
fn the_industry_popup_counts_three_ways() {
    use stars_ui::popup::Popup;

    let app = a_game();
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .expect("a home world")
        .id;

    let Some(Popup::Industry(mines)) = app.industry_popup(home, false) else {
        panic!("a mine pop-up");
    };
    assert_eq!(mines.heading(), "Mine Info");
    assert!(mines.built > 0, "a home world starts with mines");
    assert!(mines.most >= mines.built, "room for what is there");
    assert!(!mines.innate, "a Humanoid builds its own");
    let text = mines.text();
    assert!(text.contains(&mines.built.to_string()), "{text}");
    assert!(text.contains(&mines.most.to_string()), "{text}");
    assert!(text.contains("mines"), "{text}");

    let Some(Popup::Industry(factories)) = app.industry_popup(home, true) else {
        panic!("a factory pop-up");
    };
    assert_eq!(factories.heading(), "Factory Info");
    assert!(
        factories.text().contains("factories"),
        "{}",
        factories.text()
    );

    // One of a thing is singular, whichever thing it is.
    let one = stars_ui::popup::IndustrySummary {
        planet: "Stove Top".to_string(),
        factories: true,
        built: 1,
        most: 1,
        operable: 1,
        innate: false,
    };
    assert_eq!(one.noun(1), "factory");
    assert_eq!(one.noun(2), "factories");
    assert_eq!(one.noun(0), "factories");
}

/// The resources pop-up splits the year's resources between research and
/// the planet — and says nothing about a split when there is none.
#[test]
fn the_resources_popup_splits_the_year() {
    use stars_ui::popup::{Popup, ResourceSummary};

    let app = a_game();
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .expect("a home world")
        .id;
    let Some(Popup::Resources(summary)) = app.resources_popup(home) else {
        panic!("a resources pop-up");
    };
    assert_eq!(summary.heading(), "Resource Info");
    assert!(summary.total > 0);
    if let Some(spare) = summary.spare {
        assert_eq!(
            spare + summary.research,
            summary.total,
            "it all goes somewhere"
        );
    }

    // Nothing to research: the sentence stops, as the original's does.
    let none = ResourceSummary {
        planet: "Stove Top".to_string(),
        total: 100,
        research: 0,
        spare: None,
        innate: false,
    };
    let text = none.text();
    assert!(text.contains("100 resources a year"), "{text}");
    assert!(!text.contains("leaving"), "{text}");
}

/// The population pop-up: who is there, what the planet will hold, and what
/// happens next year.
#[test]
fn the_population_popup_says_who_is_there_and_what_follows() {
    use stars_ui::popup::{Inhabited, PopulationSummary, Popup};

    let app = a_game();
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .expect("a home world")
        .id;
    let Some(Popup::Population(summary)) = app.population_popup(home) else {
        panic!("a population pop-up");
    };
    assert!(matches!(summary.who, Inhabited::Ours(pop) if pop > 0));
    assert!(
        summary.capacity.is_some_and(|c| c > 0),
        "a home world holds people"
    );
    assert!(summary.value.is_some_and(|v| v > 0), "and is worth having");
    let text = summary.text();
    assert!(text.contains("colonists on"), "{text}");

    // A hostile world kills rather than holds, and says so in tenths.
    let hostile = PopulationSummary {
        planet: "Wallaby".to_string(),
        who: Inhabited::Ours(2_500),
        value: Some(-13),
        capacity: Some(0),
        growth: None,
        defenses: None,
    };
    let text = hostile.text();
    assert!(text.contains("1.3%"), "{text}");
    assert!(text.contains("kills"), "{text}");

    // Nobody's, and worth colonising.
    let empty = PopulationSummary {
        planet: "Oxygen".to_string(),
        who: Inhabited::Nobody,
        value: Some(40),
        capacity: Some(12_300),
        growth: None,
        defenses: None,
    };
    let text = empty.text();
    assert!(text.contains("uninhabited"), "{text}");
    assert!(text.contains("12300"), "{text}");

    // Somebody else's, with defences.
    let theirs = PopulationSummary {
        planet: "Sea Squared".to_string(),
        who: Inhabited::Enemy(None),
        value: None,
        capacity: None,
        growth: None,
        defenses: Some(30),
    };
    let text = theirs.text();
    assert!(text.contains("nobody knows"), "{text}");
    assert!(text.contains("30%"), "{text}");
}

/// The starbase pop-up carries the design itself, which is drawn with the
/// designer's own panel — and asking for it does not open the designer.
#[test]
fn the_starbase_popup_carries_the_design() {
    use stars_ui::popup::Popup;

    let mut app = a_game();
    let home = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .expect("a home world")
        .id;

    let Some(Popup::Design(design)) = app.starbase_popup(home) else {
        panic!("a design pop-up");
    };
    assert!(design.hull_id >= 32, "a starbase hull, not a ship's");
    assert!(!design.name.is_empty());
    assert!(app.designer.is_none(), "the dialog stays shut");

    // While a design is being peeked at, the designer's accessors answer
    // about it — that is how the panel is drawn without opening the dialog.
    assert_eq!(app.designer_subject(), None);
    app.designer_peek = Some(design.clone());
    assert_eq!(app.designer_subject().map(|d| d.name), Some(design.name));
    assert!(!app.designer_schematic().is_empty(), "it has slots to draw");
    app.designer_peek = None;
    assert_eq!(app.designer_subject(), None);
}

/// The horizontal scrollbar's range is how many columns will not fit, and
/// the walk that turns a bar position back into a first column skips the
/// hidden ones for nothing.
#[test]
fn the_horizontal_scroll_counts_columns_not_pixels() {
    use stars_ui::report::{first_field_at, scroll_columns, ScrollBy, COLUMN_PAGE};

    let mut state = ReportState::new();
    // Fifteen columns of a hundred pixels; the first never scrolls.
    let widths = [100.0_f32; 15];

    // Room for everything: no bar at all.
    let all = scroll_columns(&state, &widths, 1_400.0);
    assert_eq!(all.max, 0);
    assert_eq!(all.position, 0);

    // Room for six of the fourteen that scroll: eight do not fit.
    let tight = scroll_columns(&state, &widths, 600.0);
    assert_eq!(tight.max, 8);
    assert_eq!(tight.position, 0, "still showing the first of them");

    // Scrolled on by three, the position follows.
    state.first_field = 4;
    let moved = scroll_columns(&state, &widths, 600.0);
    assert_eq!(moved.position, 3);

    // And the position maps back to the same column.
    assert_eq!(first_field_at(&state, widths.len(), 3), 4);
    assert_eq!(first_field_at(&state, widths.len(), 0), 1);

    // A hidden column costs nothing to step over.
    state.visible &= !(1 << 2);
    assert_eq!(
        first_field_at(&state, widths.len(), 1),
        3,
        "column 2 is hidden, so one step lands on 3"
    );

    // The five fixed moves, clamped at both ends.
    assert_eq!(ScrollBy::Line(true).apply(0, 8, COLUMN_PAGE), 1);
    assert_eq!(ScrollBy::Line(false).apply(0, 8, COLUMN_PAGE), 0);
    assert_eq!(ScrollBy::Page(true).apply(0, 8, COLUMN_PAGE), 3);
    assert_eq!(ScrollBy::Page(true).apply(7, 8, COLUMN_PAGE), 8);
    assert_eq!(ScrollBy::End(true).apply(0, 8, COLUMN_PAGE), 8);
    assert_eq!(ScrollBy::End(false).apply(8, 8, COLUMN_PAGE), 0);
    assert_eq!(ScrollBy::To(5).apply(0, 8, COLUMN_PAGE), 5);
    assert_eq!(ScrollBy::To(99).apply(0, 8, COLUMN_PAGE), 8);
}

/// `SetHScrollBar` tests the visibility bit of the column **above** the one
/// it is measuring. With everything shown that cannot be told apart;
/// hiding one column takes the one to its left out of the measurement.
/// Reproduced deliberately — see `docs/ui/reports.md`.
#[test]
fn hiding_a_column_mismeasures_the_one_to_its_left() {
    use stars_ui::report::scroll_columns;

    let widths = [100.0_f32; 15];
    let shown = ReportState::new();
    let room = 600.0;
    assert_eq!(scroll_columns(&shown, &widths, room).max, 8);

    // Hide column 14, the last. The loop reads bit 15 for it — which is
    // still set — so it is measured anyway, and column 13 is the one left
    // out instead.
    let mut hidden = ReportState::new();
    hidden.visible &= !(1 << 14);
    assert_eq!(
        scroll_columns(&hidden, &widths, room).max,
        7,
        "one column's width came out of the sum, but not the hidden one's"
    );
    assert!(
        !hidden.drawn(Report::Planets).contains(&14),
        "column 14 really is hidden; only the measurement is confused"
    );
}
