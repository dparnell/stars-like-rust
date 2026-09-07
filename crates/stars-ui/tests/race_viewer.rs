//! The race viewer — View (Race), F8.
//!
//! See `docs/ui/race-wizard.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::race::lrt;
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "race".to_string(),
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

/// Six pages, in the wizard's own order.
#[test]
fn there_are_six_pages_in_the_wizard_s_order() {
    let app = a_game();
    let pages = app.race_pages(0);
    assert_eq!(
        pages.iter().map(|p| p.title).collect::<Vec<_>>(),
        [
            "Race",
            "Habitability",
            "Economy",
            "Primary Racial Trait",
            "Lesser Racial Traits",
            "Research Costs",
        ]
    );
    assert!(pages.iter().all(|p| !p.rows.is_empty()));
    // A player who is not there has no pages rather than empty ones.
    assert!(app.race_pages(9).is_empty());
}

/// The first page is who they are.
#[test]
fn the_first_page_names_the_race() {
    let app = a_game();
    let page = &app.race_pages(0)[0];
    let value = |label: &str| {
        page.rows
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, v)| v.clone())
            .expect("a row")
    };
    assert_eq!(value("Race name"), "Humanoid");
    assert_eq!(value("Password"), "none");
}

/// Habitability reads as a range with an ideal, or as "immune" — a negative
/// upper bound is the immunity marker, so the row is one thing or the other.
#[test]
fn habitability_shows_a_range_or_immunity() {
    let mut app = a_game();
    let rows = app.race_pages(0)[1].rows.clone();
    assert_eq!(rows.len(), 4, "three variables and the growth rate");
    assert!(rows[0].1.contains("ideal"), "{}", rows[0].1);
    assert!(rows[3].1.ends_with('%'), "{}", rows[3].1);

    // Make the race immune to gravity and the row changes shape.
    if let Some(game) = app.game.as_mut() {
        game.players[0].race.env_max[0] = -1;
    }
    let rows = app.race_pages(0)[1].rows.clone();
    assert_eq!(rows[0].1, "immune");
    assert!(rows[1].1.contains("ideal"), "the others are unaffected");
}

/// The primary trait is named, and all fourteen lesser ones are listed so that
/// what is *not* taken is as plain as what is.
#[test]
fn the_traits_are_all_listed() {
    let mut app = a_game();
    let prt = &app.race_pages(0)[3];
    assert_eq!(prt.rows.len(), 1);
    assert_eq!(prt.rows[0].1, "Jack of All Trades", "a Humanoid's");

    let lrts = &app.race_pages(0)[4];
    assert_eq!(lrts.rows.len(), 14);
    assert!(lrts.rows.iter().all(|(_, v)| v == "yes" || v == "no"));

    // Turning one on shows in its row and nobody else's.
    if let Some(game) = app.game.as_mut() {
        game.players[0].race.lrt_bits |= 1 << lrt::ULTIMATE_RECYCLING;
    }
    let lrts = &app.race_pages(0)[4];
    let row = lrts
        .rows
        .iter()
        .find(|(l, _)| l == "Ultimate Recycling")
        .expect("bit 5 is named");
    assert_eq!(row.1, "yes");
    assert_eq!(
        lrts.rows.iter().filter(|(_, v)| v == "yes").count(),
        1,
        "and only that one"
    );
}

/// Research costs, field by field, with the three settings spelled out as the
/// wizard spells them.
#[test]
fn research_costs_read_one_field_at_a_time() {
    let mut app = a_game();
    if let Some(game) = app.game.as_mut() {
        let race = &mut game.players[0].race;
        race.attrs[8] = 0; // energy
        race.attrs[9] = 1;
        race.attrs[10] = 2;
    }
    let page = &app.race_pages(0)[5];
    assert_eq!(page.rows.len(), 7, "six fields and the tech-3 option");
    assert_eq!(page.rows[0].1, "costs 75% extra");
    assert_eq!(page.rows[1].1, "costs the standard amount");
    assert_eq!(page.rows[2].1, "costs 50% less");
    assert_eq!(page.rows[6].0, "All techs start at 3");
}

/// Back and Next walk the pages and **stop at the ends** rather than wrapping,
/// because a wizard's do.
#[test]
fn the_pages_stop_at_the_ends() {
    let mut app = a_game();
    app.open_race_viewer(0);
    assert_eq!(app.race_viewer, Some((0, 0)));

    app.race_viewer_page(false);
    assert_eq!(app.race_viewer, Some((0, 0)), "no wrapping backwards");

    for expect in 1..6 {
        app.race_viewer_page(true);
        assert_eq!(app.race_viewer, Some((0, expect)));
    }
    app.race_viewer_page(true);
    assert_eq!(app.race_viewer, Some((0, 5)), "and none forwards");

    app.close_race_viewer();
    assert!(app.race_viewer.is_none());
    // A player who is not there does not open it.
    app.open_race_viewer(9);
    assert!(app.race_viewer.is_none());
}

/// It draws, on every page.
#[test]
fn the_viewer_draws() {
    let mut app = a_game();
    app.open_race_viewer(0);
    for _ in 0..7 {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::race::view(&mut app, ui);
            });
        });
        app.race_viewer_page(true);
    }
    app.close_race_viewer();
}
