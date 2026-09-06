//! The Research dialog, driven the way a player drives it.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::research::NextField;
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "research".to_string(),
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

/// The dialog opens on what the player has set, and changes nothing until OK.
#[test]
fn nothing_is_committed_until_ok() {
    let mut app = a_game();
    let before = {
        let player = &app.game.as_ref().expect("game").players[0];
        (
            player.research.current_field,
            player.research.next_field,
            player.research_pct,
        )
    };

    app.open_research();
    let dialog = app.research_dialog.expect("it opened");
    assert_eq!(
        (dialog.field, dialog.next, dialog.percent),
        before,
        "it opens on what is already set"
    );

    // Change all three and cancel.
    let d = app.research_dialog.as_mut().expect("open");
    d.field = 3;
    d.next = NextField::Field(1);
    d.percent = 42;
    app.research_cancel();
    assert!(app.research_dialog.is_none());
    let player = &app.game.as_ref().expect("game").players[0];
    assert_eq!(
        (
            player.research.current_field,
            player.research.next_field,
            player.research_pct
        ),
        before,
        "Cancel changed nothing"
    );

    // Change them again and OK.
    app.open_research();
    let d = app.research_dialog.as_mut().expect("open");
    d.field = 3;
    d.next = NextField::Field(1);
    d.percent = 42;
    app.research_ok();
    assert!(app.research_dialog.is_none());
    let player = &app.game.as_ref().expect("game").players[0];
    assert_eq!(player.research.current_field, 3);
    assert_eq!(player.research.next_field, NextField::Field(1));
    assert_eq!(player.research_pct, 42);
    assert!(app.dirty);
}

/// The six fields, their levels, and the dropdown's eight choices.
#[test]
fn the_dialog_lists_the_fields_and_the_choices() {
    let mut app = a_game();
    app.open_research();

    let levels = app.research_levels();
    assert_eq!(levels.len(), 6);
    assert_eq!(levels[0].0, "Energy");
    assert_eq!(levels[5].0, "Biotechnology");
    // A Humanoid is Jack of All Trades and starts at three in every field.
    assert!(levels.iter().all(|(_, level)| *level == 3), "{levels:?}");

    // The dropdown: <Same field>, the six fields, <Lowest field>.
    let choices = App::research_next_choices();
    assert_eq!(choices.len(), 8);
    assert_eq!(choices[0].0, NextField::Same);
    assert_eq!(choices[0].1, "<Same field>");
    assert_eq!(choices[1].0, NextField::Field(0));
    assert_eq!(choices[6].0, NextField::Field(5));
    assert_eq!(choices[7].0, NextField::Lowest);
    assert_eq!(choices[7].1, "<Lowest field>");
}

/// The Currently Researching box, in the original's wording.
#[test]
fn the_status_box_says_what_is_owed_and_how_long() {
    let mut app = a_game();
    app.open_research();

    let rows = app.research_status();
    assert_eq!(rows.len(), 3);
    // "%s, Tech Level %d" — the level being worked on, one above what is held.
    assert_eq!(rows[0].1, "Energy, Tech Level 4");
    assert_eq!(rows[1].0, "Resources needed to complete:");
    assert!(rows[1].1.parse::<i32>().is_ok(), "{}", rows[1].1);
    assert_eq!(rows[2].0, "Estimated time to completion:");

    // Selecting another field changes the first line.
    app.research_dialog.as_mut().expect("open").field = 2;
    assert_eq!(app.research_status()[0].1, "Propulsion, Tech Level 4");

    // With nothing budgeted the estimate reads Never.
    app.research_dialog.as_mut().expect("open").percent = 0;
    let rows = app.research_status();
    assert!(
        rows[2].1 == "Never" || rows[2].1.contains("year"),
        "{}",
        rows[2].1
    );

    // A maxed field reads Maxed Out on both lines.
    if let Some(game) = app.game.as_mut() {
        game.players[0].research.levels[2] = stars_core::research::MAX_TECH_LEVEL;
    }
    let rows = app.research_status();
    assert_eq!(rows[1].1, "Maxed Out");
    assert_eq!(rows[2].1, "Maxed Out");
}

/// The Resource Allocation box, and the projection that moves with the slider.
#[test]
fn the_allocation_box_projects_next_years_budget() {
    let mut app = a_game();
    app.open_research();

    let rows = app.research_allocation();
    let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Annual resources from all planets:",
            "Total resources spent on research last year:",
            "Resources budgeted for research:",
            "Next year's projected research budget:",
        ]
    );
    // The percentage row shows the dialog's figure, not the player's.
    app.research_dialog.as_mut().expect("open").percent = 33;
    assert_eq!(app.research_allocation()[2].1, "33%");

    // A home world starts with an empty queue, so research gets everything it
    // makes whatever the percentage — which is the manual's rule on p. 8-4.
    let none = app.research_projected(0);
    let all = app.research_projected(100);
    assert_eq!(
        none, all,
        "an idle empire gives research everything either way"
    );
    assert!(all > 0);
}

/// The benefits list is sorted by how soon, and lists nothing already built.
#[test]
fn the_benefits_list_is_nearest_first() {
    let mut app = a_game();
    app.open_research();

    let benefits = app.research_benefits();
    assert!(!benefits.is_empty());
    assert!(benefits
        .windows(2)
        .all(|w| w[0].levels_away <= w[1].levels_away));
    assert!(benefits.iter().all(|b| (1..=9).contains(&b.levels_away)));
}

/// It lays out for real, on a real game.
#[test]
fn the_dialog_draws() {
    let mut app = a_game();
    app.open_research();

    let frame = |app: &mut App| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::research::view(app, ui);
            });
        });
    };

    // Every field selected in turn, so each one's status box is built.
    for field in 0..6 {
        app.research_dialog.as_mut().expect("open").field = field;
        frame(&mut app);
    }
    // And every choice of next field.
    for (next, _) in App::research_next_choices() {
        app.research_dialog.as_mut().expect("open").next = next;
        frame(&mut app);
    }
    // A maxed field, and a race with both research traits.
    if let Some(game) = app.game.as_mut() {
        game.players[0].research.levels[0] = stars_core::research::MAX_TECH_LEVEL;
        game.players[0].race.lrt_bits |= 1 << stars_core::race::lrt::GENERALIZED_RESEARCH;
        game.players[0].race.lrt_bits |= 1 << stars_core::race::lrt::BLEEDING_EDGE_TECH;
    }
    app.research_dialog.as_mut().expect("open").field = 0;
    assert_eq!(app.research_notes().len(), 2);
    frame(&mut app);

    app.research_cancel();
    frame(&mut app);
}
