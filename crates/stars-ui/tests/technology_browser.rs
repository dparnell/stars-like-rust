//! The Technology Browser, driven the way a player drives it.

use stars_core::components::slot;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "browser".to_string(),
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

/// It opens on the first armour, which is where `BrowserDlg` starts.
#[test]
fn the_browser_opens_on_armour() {
    let mut app = a_game();
    app.open_browser();
    let browser = app.browser.expect("it opened");
    assert_eq!(browser.category, 0, "the dropdown starts on All");
    assert_eq!(browser.showing, (slot::ARMOR, 0));
    assert!(!browser.buildable_only);

    let detail = app.browser_detail().expect("a component");
    assert_eq!(detail.category_name, "Armor");
    assert_eq!(detail.name, stars_core::components::ARMORS[0].name);
}

/// The seventeen categories, in the dropdown's order.
#[test]
fn the_dropdown_lists_all_then_the_sixteen_kinds() {
    let names: Vec<&str> = stars_core::browser::CATEGORIES
        .iter()
        .map(|(_, name)| *name)
        .collect();
    assert_eq!(names.len(), 17);
    assert_eq!(names[0], "All");
    // Alphabetical after the first, which is the order the string ids are in.
    let rest = &names[1..];
    let mut sorted = rest.to_vec();
    sorted.sort_unstable();
    assert_eq!(rest, sorted.as_slice());
    assert_eq!(rest.first(), Some(&"Armor"));
    assert_eq!(rest.last(), Some(&"Torpedoes"));

    // Every one names a real category with components in it.
    for (flag, name) in stars_core::browser::CATEGORIES.iter().skip(1) {
        assert!(
            stars_core::browser::category_len(*flag) > 0,
            "{name} is empty"
        );
    }
}

/// Prev and Next walk the catalogue and roll over between categories.
#[test]
fn next_and_prev_walk_the_whole_catalogue() {
    let mut app = a_game();
    app.open_browser();

    // Walking forward from the first armour eventually leaves the category.
    let mut seen_other = false;
    let mut last = app.browser.expect("open").showing;
    for _ in 0..60 {
        app.browser_step(true);
        let now = app.browser.expect("open").showing;
        assert_ne!(now, last, "it moved");
        if now.0 != slot::ARMOR {
            seen_other = true;
            break;
        }
        last = now;
    }
    assert!(seen_other, "Next rolls over into the next category");

    // And back again.
    let there = app.browser.expect("open").showing;
    app.browser_step(false);
    app.browser_step(true);
    assert_eq!(
        app.browser.expect("open").showing,
        there,
        "Prev undoes Next"
    );
}

/// Choosing a category confines the walk to it.
#[test]
fn a_category_confines_the_walk() {
    let mut app = a_game();
    app.open_browser();

    let engines = stars_core::browser::CATEGORIES
        .iter()
        .position(|(_, name)| *name == "Engines")
        .expect("Engines is in the list");
    app.browser_set_category(engines);
    assert_eq!(app.browser.expect("open").showing.0, slot::ENGINE);

    for _ in 0..40 {
        app.browser_step(true);
        assert_eq!(
            app.browser.expect("open").showing.0,
            slot::ENGINE,
            "it stays in the category"
        );
    }
}

/// The filter stops only at what the player can build.
#[test]
fn the_filter_shows_only_what_can_be_built() {
    let mut app = a_game();
    app.open_browser();

    // Without the filter, something unresearched turns up.
    let mut saw_unavailable = false;
    for _ in 0..80 {
        app.browser_step(true);
        if !app.browser_detail().expect("a component").buildable() {
            saw_unavailable = true;
            break;
        }
    }
    assert!(
        saw_unavailable,
        "the browser shows what you cannot build yet"
    );

    // With it on, it moves off the unavailable one and never lands on another.
    app.browser_set_buildable_only(true);
    assert!(app.browser_detail().expect("a component").buildable());
    for _ in 0..60 {
        app.browser_step(true);
        assert!(
            app.browser_detail().expect("a component").buildable(),
            "{} should not be shown",
            app.browser_detail().expect("a component").name
        );
    }
}

/// The panel says what a component costs, needs and does — for this player.
#[test]
fn the_panel_reports_cost_requirements_and_stats() {
    let mut app = a_game();
    app.open_browser();

    // A beam weapon has range, power and initiative.
    let beams = stars_core::browser::CATEGORIES
        .iter()
        .position(|(_, name)| *name == "Beam Weapons")
        .expect("Beam Weapons");
    app.browser_set_category(beams);
    let detail = app.browser_detail().expect("a beam");
    let labels: Vec<&str> = detail.stats.iter().map(|(l, _)| l.as_str()).collect();
    assert!(labels.contains(&"Range:"), "{labels:?}");
    assert!(labels.contains(&"Power:"), "{labels:?}");
    assert!(labels.contains(&"Initiative:"), "{labels:?}");
    assert!(detail.mass.is_some());
    assert!(detail.cost.iter().any(|c| *c > 0));

    // And it knows which of the game's pictures to draw it with, whether or
    // not a copy of the original has been found to draw it from.
    let cell = stars_core::browser::picture(&detail).expect("a picture cell");
    assert_eq!((cell.width, cell.height), (64, 64));
    assert!(
        stars_formats::resources::art::COMPONENT_SHEETS.contains(&cell.resource),
        "{}",
        cell.resource
    );

    // A planetary installation is never carried, so it has no mass line.
    let planetary = stars_core::browser::CATEGORIES
        .iter()
        .position(|(_, name)| *name == "Planetary")
        .expect("Planetary");
    app.browser_set_category(planetary);
    assert_eq!(app.browser_detail().expect("a scanner").mass, None);

    // Requirements are marked met or not against the player's own levels.
    let scanners = stars_core::browser::CATEGORIES
        .iter()
        .position(|(_, name)| *name == "Scanners")
        .expect("Scanners");
    app.browser_set_category(scanners);
    let mut found = false;
    for _ in 0..20 {
        let detail = app.browser_detail().expect("a scanner");
        if let Some(need) = detail.tech.first() {
            let held =
                app.game.as_ref().expect("game").players[0].research.levels[need.field as usize];
            assert_eq!(need.met, i16::from(held) >= i16::from(need.level));
            found = true;
        }
        app.browser_step(true);
    }
    assert!(found, "some scanner asks for technology");
}

/// A component a trait puts out of reach says so, in words drawn from the same
/// gate the rule is enforced from.
#[test]
fn a_forbidden_component_explains_itself() {
    let mut app = a_game();
    app.open_browser();
    let who = {
        let player = &app.game.as_ref().expect("game").players[0];
        stars_core::parts::Builder::player(player)
    };

    // A Humanoid is Jack of All Trades, so the Settler's Delight —
    // Hyper-Expansion's engine — is out of reach and says which trait it
    // needs. The trait is spelled as the race wizard's own button spells it.
    let detail = stars_core::browser::detail(&who, slot::ENGINE, 0).expect("Settler's Delight");
    assert_eq!(detail.name, "Settler's Delight");
    assert!(!detail.buildable());
    assert_eq!(detail.notes.len(), 1);
    assert!(
        detail.notes[0].contains("Hyper-Expansion"),
        "{}",
        detail.notes[0]
    );

    // A Mystery Trader's component says so rather than naming a trait.
    let detail = stars_core::browser::detail(&who, slot::ENGINE, 8).expect("Enigma Pulsar");
    assert!(detail.notes.iter().any(|n| n.contains("Mystery Trader")));

    // Something ordinary has nothing to explain.
    let detail = stars_core::browser::detail(&who, slot::ENGINE, 1).expect("Quick Jump 5");
    assert!(detail.notes.is_empty(), "{:?}", detail.notes);
    assert!(detail.buildable());
}

/// It lays out for real, over the whole catalogue.
#[test]
fn the_browser_draws() {
    let mut app = a_game();
    app.open_browser();

    let frame = |app: &mut App| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::browser::view(app, ui);
            });
        });
    };

    // Every category, and every component in it.
    for category in 0..stars_core::browser::CATEGORIES.len() {
        app.browser_set_category(category);
        frame(&mut app);
        let flag = stars_core::browser::CATEGORIES[category].0;
        let steps = if flag == 0 {
            40
        } else {
            stars_core::browser::category_len(flag)
        };
        for _ in 0..steps {
            app.browser_step(true);
            frame(&mut app);
        }
    }

    app.close_browser();
    frame(&mut app);
}
