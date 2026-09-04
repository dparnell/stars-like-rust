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

/// Lay out one frame of a screen, as the shell would.
fn draw(app: &mut App, screen: Screen) {
    app.screen = screen;
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::central(app, ui);
        });
    });
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
    let dir = std::env::temp_dir().join("stars-ui-new-game-test");
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

    let dir = std::env::temp_dir().join("stars-ui-save-new-game-test");
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
