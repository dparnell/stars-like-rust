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
