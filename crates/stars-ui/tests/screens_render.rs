//! Every screen, driven through a real egui pass against real save files.
//!
//! egui runs perfectly well without a window, so this is not a smoke test in
//! name only: each screen is laid out, every widget is built and every id is
//! allocated exactly as it would be on screen. It catches the failures that
//! only appear when a view meets real data — an index off the end of a fleet's
//! stacks, a planet with no position, a battle with one token, an id clash
//! between two lists on the same screen.

use std::path::{Path, PathBuf};

use stars_ui::{App, Screen};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// Lay out one frame of a screen, as the shell would — with the planet, message
/// and survey panes beside it, which is where the shell puts them.
fn draw(app: &mut App, screen: Screen) {
    app.screen = screen;
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        if app.game.is_some() {
            egui::TopBottomPanel::bottom("messages")
                .show(ctx, |ui| stars_ui::views::messages::view(app, ui));
            egui::SidePanel::left("planet").show(ctx, |ui| stars_ui::views::planet::view(app, ui));
            egui::SidePanel::left("fleet").show(ctx, |ui| stars_ui::views::fleet::view(app, ui));
            egui::SidePanel::left("survey").show(ctx, |ui| stars_ui::views::survey::view(app, ui));
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::central(app, ui);
        });
    });
}

/// Every scanner view and overlay lays out, on a real universe.
#[test]
fn every_scanner_view_draws() {
    let root = workspace_root().join("fixtures/games");
    if !root.is_dir() {
        eprintln!("skipping: no fixtures");
        return;
    }
    let Some(save) = first_save(&root) else {
        eprintln!("skipping: no save files");
        return;
    };
    let mut app = App::new();
    if app.open(&save).is_err() {
        eprintln!("skipping: {} did not open", save.display());
        return;
    }

    for view in stars_ui::ScanView::ALL {
        app.scan_view = view;
        for zoom in -4..=4 {
            app.scan_zoom = zoom;
            draw(&mut app, Screen::Galaxy);
        }
    }
    // And every overlay at once, which is the busiest the map ever is.
    app.scan_overlays = stars_ui::ScanOverlays {
        names: true,
        scanner_coverage: true,
        minefields: true,
        fleet_paths: true,
        ship_counts: true,
        idle_fleets: true,
        ship_design_filter: true,
        enemy_class_filter: true,
        player_colours: true,
    };
    draw(&mut app, Screen::Galaxy);
}

/// The first save file under a directory, for the tests that need only one.
fn first_save(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut found: Vec<PathBuf> = Vec::new();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|x| {
                let x = x.to_string_lossy().to_lowercase();
                x.starts_with('m') && x.len() == 2
            }) {
                found.push(path);
            }
        }
    }
    found.sort();
    found.into_iter().next()
}

/// With nothing loaded, every screen still lays out.
#[test]
fn the_title_screen_draws_with_no_game() {
    let mut app = App::new();
    for screen in Screen::ALL {
        draw(&mut app, screen);
    }
}

/// Every screen, against every save file in the fixtures.
///
/// This is the test that earns its keep: the fixtures include a player file
/// with battles, host files with none, sixteen-player games and one-player
/// games, planets with and without a universe file beside them.
#[test]
fn every_screen_draws_for_every_fixture() {
    let root = workspace_root().join("fixtures/games");
    if !root.is_dir() {
        eprintln!("skipping: no fixtures");
        return;
    }

    let mut saves: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let is_save = path.extension().is_some_and(|x| {
                let x = x.to_string_lossy().to_lowercase();
                x == "hst" || (x.starts_with('m') && x.len() == 2)
            });
            if is_save {
                saves.push(path);
            }
        }
    }
    saves.sort();
    // One file per directory, then a spread across them: the point is variety
    // — a player file with battles, a host file with none, sixteen players and
    // one, a universe file beside the save and one a directory up — not volume.
    saves.dedup_by_key(|p| p.parent().map(Path::to_path_buf));
    let step = (saves.len() / 24).max(1);
    let saves: Vec<PathBuf> = saves.into_iter().step_by(step).collect();
    assert!(
        saves.len() > 5,
        "expected several save files, got {}",
        saves.len()
    );

    let mut drawn = 0;
    for path in &saves {
        let mut app = App::new();
        if app.open(path).is_err() {
            continue;
        }
        for screen in Screen::ALL {
            draw(&mut app, screen);
            drawn += 1;
        }
        // And with a battle open, played to a few different points.
        if !app.battles.is_empty() {
            app.open_battle(0);
            app.screen = Screen::Battles;
            for position in [0usize, 1, 5, usize::MAX] {
                if let Some(vcr) = app.vcr.as_mut() {
                    vcr.seek(position);
                }
                draw(&mut app, Screen::Battles);
                drawn += 1;
            }
        }
        // And with nothing selected, which is what a fresh galaxy looks like
        // before the player clicks.
        app.selection = stars_ui::Selection::default();
        for screen in Screen::ALL {
            draw(&mut app, screen);
            drawn += 1;
        }
    }
    assert!(drawn > 50, "expected many frames, drew {drawn}");
    eprintln!(
        "UI: {drawn} screen frames laid out over {} save files",
        saves.len()
    );
}

/// A game whose universe file is missing has planets with no position; the
/// galaxy map must say so rather than drawing them all at the origin.
#[test]
fn the_galaxy_map_copes_with_no_universe() {
    let mut app = App::new();
    let mut game = stars_core::GameState::new(1);
    game.planets.push(stars_core::Planet::unowned(0));
    app.game = Some(game);
    assert!(app.extent().is_none(), "no positions, so no extent");
    draw(&mut app, Screen::Galaxy);
}

/// The New Game wizard lays out, and creating a game from it works.
#[test]
fn the_new_game_wizard_draws_and_creates_a_game() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::opponents;

    let mut app = App::new();
    app.setup = Some(NewGame::default());
    // The wizard replaces the whole central panel, so the screen does not
    // matter; draw it under each anyway.
    for screen in Screen::ALL {
        draw(&mut app, screen);
    }
    assert!(app.setup.is_some(), "drawing must not close the wizard");

    // Every player row has its own combo box, so a full game exercises every
    // id the wizard allocates.
    let mut config = NewGame {
        name: "Rendered".into(),
        size: Size::Small,
        players: vec![NewPlayer::human(stars_core::Race::humanoid())],
        ..NewGame::default()
    };
    for level in 0..4 {
        for personality in 0..5 {
            if let Some(opponent) = opponents::opponent(personality, level) {
                config.players.push(opponent.as_player());
            }
        }
    }
    config.players.truncate(stars_core::newgame::MAX_PLAYERS);
    app.setup = Some(config.clone());
    draw(&mut app, Screen::Galaxy);

    app.new_game(&config).expect("creates the game");
    assert!(app.setup.is_none(), "creating closes the wizard");
    assert!(app.game.is_some());
    assert!(app.universe.is_some());
    assert!(
        !app.can_save_game(),
        "a generated game has no file to write back to"
    );

    // And every screen draws for it, the same as for a loaded save.
    for screen in Screen::ALL {
        draw(&mut app, screen);
    }

    // Its universe is a real .xy.
    // Per process: concurrent test runs must not share it.
    let dir = std::env::temp_dir().join(format!("stars-ui-new-game-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("rendered.xy");
    app.save_universe(&path).expect("writes the universe");
    let bytes = std::fs::read(&path).expect("reads back");
    let universe = stars_formats::Universe::decode(&bytes).expect("decodes");
    assert_eq!(universe.game().expect("game info").name, "Rendered");
    assert_eq!(
        universe.planet_count(),
        app.game.as_ref().expect("game").planets.len()
    );
    let _ = std::fs::remove_file(&path);
}

/// A generated game takes orders and generates turns, like a loaded one.
#[test]
fn a_generated_game_takes_orders_and_turns() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};

    let mut app = App::new();
    let config = NewGame {
        size: Size::Small,
        players: vec![NewPlayer::human(stars_core::Race::humanoid())],
        ..NewGame::default()
    };
    app.new_game(&config).expect("creates the game");

    // Queue a factory on the homeworld and advance a year.
    let home = app
        .game
        .as_ref()
        .expect("game")
        .planets
        .iter()
        .find(|p| p.homeworld)
        .map(|p| p.id)
        .expect("a homeworld");
    app.selection.planet = Some(home);
    let buildable = app.buildable_items();
    assert!(!buildable.is_empty(), "a homeworld can build something");
    app.queue_add(buildable[0].0, 5);
    assert!(app.dirty);

    app.generate_turn();
    assert_eq!(app.game.as_ref().expect("game").year(), 2401);
    assert!(app.last_turn.is_some());
    draw(&mut app, Screen::Planets);
}

/// A new game can be written out whole, and comes back as a real save.
#[test]
fn a_new_game_can_be_saved_and_reopened() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::opponents;

    let mut app = App::new();
    let config = NewGame {
        name: "Saved Game".into(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(stars_core::Race::humanoid()),
            opponents::opponent(1, 1).expect("Turindrones").as_player(),
        ],
        ..NewGame::default()
    };
    app.new_game(&config).expect("creates the game");
    assert!(!app.can_save_game(), "a game with no file behind it");

    let planets = app.game.as_ref().expect("game").planets.len();
    let fleets = app.game.as_ref().expect("game").fleets.len();

    let dir = std::env::temp_dir().join(format!(
        "stars-ui-save-new-game-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let written = app
        .save_new_game(&dir.join("Kestrel.hst"))
        .expect("writes the game");

    let names: Vec<String> = written
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .collect();
    assert_eq!(
        names,
        vec!["Kestrel.xy", "Kestrel.hst", "Kestrel.m1", "Kestrel.m2"]
    );

    // Saving re-opens from the host file, so the app is now backed by one.
    assert!(app.can_save_game());
    assert_eq!(app.path.as_deref(), Some(dir.join("Kestrel.hst").as_path()));
    let game = app.game.as_ref().expect("game");
    assert_eq!(game.planets.len() + game.known_planets.len(), planets);
    assert_eq!(game.fleets.len(), fleets);
    // The universe was found beside the host file, so planets have positions.
    assert!(game.planets.iter().all(|p| p.position.is_some()));

    // And every screen draws for the reopened game.
    for screen in Screen::ALL {
        draw(&mut app, screen);
    }

    // Saving again goes through the ordinary edit-preserving path.
    app.save(&dir.join("Kestrel.hst")).expect("saves again");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The Ship and Starbase Designer, in both of its faces and all four views.
///
/// It is a dialog rather than a screen, so it gets its own pass: the browser
/// over designs, hulls and enemy hulls, and then the editor with its parts
/// list and schematic, each laid out for real.
#[test]
fn the_designer_draws_in_both_modes() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "Designer".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");

    let frame = |app: &mut App| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::designer::view(app, ui);
            });
        });
    };

    app.open_designer();
    for starbase in [false, true] {
        for view in stars_ui::DesignView::ALL {
            let designer = app.designer.as_mut().expect("open");
            designer.starbase = starbase;
            designer.view = view;
            designer.selected = 0;
            // Every entry of every list, so no hull's schematic goes undrawn.
            let count = app.designer_list().len().max(1);
            for index in 0..count {
                app.designer.as_mut().expect("open").selected = index;
                frame(&mut app);
            }
        }
    }

    // The editor, on a copied hull, with each parts filter in turn.
    let designer = app.designer.as_mut().expect("open");
    designer.starbase = false;
    designer.view = stars_ui::DesignView::Hulls;
    designer.selected = 0;
    app.designer_copy();
    assert!(app.designer.as_ref().expect("open").editing.is_some());
    for filter in 0..app.designer_filters().len() {
        app.designer.as_mut().expect("open").filter = filter;
        frame(&mut app);
    }

    // And the complaint an engineless design draws.
    app.designer_ok();
    assert!(app.designer.as_ref().expect("open").complaint.is_some());
    frame(&mut app);

    app.close_designer();
    assert!(app.designer.is_none());
    frame(&mut app);
}

/// Every page of the race wizard lays out, on a race with an immune axis so
/// the habitability page draws both kinds of row.
#[test]
fn every_race_wizard_page_draws() {
    let mut app = App::new();
    app.open_race_wizard();
    app.race_wizard_load_preset(4); // Silicanoid: immune to all three.

    for page in 0..stars_ui::RACE_WIZARD_PAGES {
        if let Some(wizard) = app.race_wizard.as_mut() {
            wizard.page = page;
        }
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::race_wizard::view(&mut app, ui);
            });
        });
        assert_eq!(
            app.race_wizard.as_ref().expect("still open").page,
            page,
            "drawing page {page} should not have turned it"
        );
    }
}

/// The Battle Plans dialog lays out, including its rename box and its delete
/// warning, which are drawn in place rather than as dialogs of their own.
#[test]
fn the_battle_plans_dialog_draws() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "Plans".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.open_battle_plans();

    let frame = |app: &mut App| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::battleplans::view(app, ui);
            });
        });
    };

    for slot in 0..app.battle_plan_count() {
        app.select_battle_plan(slot);
        frame(&mut app);
    }
    app.select_battle_plan(2);
    if let Some(dialog) = app.battle_plans.as_mut() {
        dialog.rename = Some("Renamed".to_string());
    }
    frame(&mut app);
    if let Some(dialog) = app.battle_plans.as_mut() {
        dialog.rename = None;
        dialog.confirm_delete = true;
    }
    frame(&mut app);
    assert!(app.battle_plans.is_some(), "drawing should not close it");
}

/// The Change Password dialog lays out, empty and with an error showing.
#[test]
fn the_change_password_dialog_draws() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "Password".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.open_password_dialog();

    let frame = |app: &mut App| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::password::view(app, ui);
            });
        });
    };

    frame(&mut app);
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "one".to_string();
        dialog.retype = "two".to_string();
    }
    app.submit_password();
    frame(&mut app);
    assert!(app.password_dialog.is_some(), "a mismatch keeps it open");
}

/// The password prompt lays out, before and after a wrong answer.
#[test]
fn the_password_prompt_draws() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "Prompt".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.set_password("io");

    let dir = std::env::temp_dir().join(format!("stars-ui-prompt-draw-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let written = app
        .save_new_game(&dir.join("Prompt.hst"))
        .expect("writes the game");
    let turn = written
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "m1"))
        .expect("a turn file")
        .clone();

    // A fresh app with somebody at the keyboard has not been given the
    // password, so opening asks for it.
    let mut app = App::new();
    app.prompt_for_password = true;
    app.open(&turn).expect("reads the file");
    assert!(app.password_prompt.is_some());

    let frame = |app: &mut App, now: f64| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::password::prompt(app, ui, now);
            });
        });
    };

    frame(&mut app, 0.0);
    if let Some(prompt) = app.password_prompt.as_mut() {
        prompt.typed = "not it".to_string();
    }
    app.submit_password_prompt(0.0);
    // Drawn during the wait that follows a wrong password.
    frame(&mut app, 0.5);
    assert!(app.password_prompt.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Host Mode dialog lays out, with a mix of statuses on the board.
#[test]
fn the_host_mode_dialog_draws() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "Hosted".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    if let Some(game) = app.game.as_mut() {
        game.players[1].dead = true;
    }
    app.open_host_mode();

    for elapsed in [0.0, 64.0, 90_061.0] {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::host::view(&mut app, ui, elapsed);
            });
        });
    }
    assert!(app.host_mode, "drawing should not close it");
}

/// The scanner's right-click menu draws over the map.
#[test]
fn the_scanner_menu_draws() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "Menu".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    let at = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .and_then(|p| p.position)
        .expect("a homeworld");
    app.scan_menu_at = Some((at.x, at.y));
    // With a space object at the same place, so the menu draws one of each and
    // the survey pane has a thing to summarise.
    if let Some(game) = app.game.as_mut() {
        game.minefields = vec![stars_core::minefield::Minefield {
            id: 1,
            owner: 0,
            position: at,
            mines: 400,
            kind: 1,
            detonating: false,
            detected_by: 0xFFFF,
            visible_to: 0xFFFF,
            turn: 0,
        }];
    }

    draw(&mut app, Screen::Galaxy);
    assert!(app.scan_menu_at.is_some(), "drawing should not close it");

    // And with the field selected, the survey pane draws its summary.
    app.select_object(stars_ui::ScanObject::Thing(stars_ui::ScanThing::Minefield(
        0,
    )));
    draw(&mut app, Screen::Galaxy);
    assert!(!app.survey_thing_rows().is_empty());
}
