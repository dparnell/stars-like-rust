//! A new game's id — its seed — comes from the clock and can be set by
//! hand on the New Game page, so no two games are alike unless asked for.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

fn two_player(id: u32) -> NewGame {
    NewGame {
        name: "seeded".to_string(),
        id,
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    }
}

/// Opening the wizard takes the shell's clock as the id, as
/// `GenerateWorld` takes `GetTickCount()`; without one, the fallback.
#[test]
fn the_wizard_opens_with_the_clock_as_its_seed() {
    let mut app = App::new();
    app.start_new_game(4321);
    assert_eq!(app.setup.as_ref().map(|s| s.id), Some(4321));
    app.clock_ms = Some(0xdead_beef);
    app.start_new_game(4321);
    assert_eq!(app.setup.as_ref().map(|s| s.id), Some(0xdead_beef));
    assert_eq!(app.fresh_game_id(1), 0xdead_beef);
    app.clock_ms = None;
    assert_eq!(app.fresh_game_id(1), 1);
}

/// Two seeds, two universes; one seed twice, the same universe.
#[test]
fn the_seed_decides_the_universe() {
    let planets = |id: u32| {
        let mut app = App::new();
        app.new_game(&two_player(id)).expect("creates the game");
        app.universe
            .as_ref()
            .expect("a universe")
            .planets_resolved()
            .iter()
            .map(|p| (p.x, p.y, p.name_index))
            .collect::<Vec<_>>()
    };
    assert_eq!(planets(7), planets(7));
    assert_ne!(planets(7), planets(8));
    let mut app = App::new();
    app.new_game(&two_player(7)).expect("creates the game");
    assert_eq!(
        app.game.as_ref().unwrap().seed,
        7,
        "the id is the game's seed"
    );
}

/// The page shows the seed and takes a typed one.
#[test]
fn the_page_edits_the_seed() {
    let mut app = App::new();
    app.start_new_game(99);
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let _ = stars_ui::views::newgame::view(&mut app, ui);
        });
    });
    // The page draws; the seed is what the field holds.
    assert_eq!(app.setup.as_ref().map(|s| s.id), Some(99));
    // The text remembered for the field is the seed's.
    let remembered = ctx.memory(|m| {
        m.data
            .get_temp::<String>(egui::Id::new("new-game-seed-text"))
    });
    assert_eq!(remembered.as_deref(), Some("99"));
}
