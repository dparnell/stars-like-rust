//! The Production dialog, driven the way a player drives it.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::production::item;
use stars_core::{opponents, Race};
use stars_ui::App;

/// A generated game, with the player's home world selected.
fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "production".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");

    let home = app
        .game
        .as_ref()
        .expect("game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0) && p.homeworld)
        .map(|p| p.id)
        .expect("the player has a home world");
    app.selection.planet = Some(home);
    app
}

/// The dialog opens on the selected planet with a copy of its queue.
#[test]
fn the_dialog_edits_a_copy_of_the_queue() {
    let mut app = a_game();
    app.open_production();
    let dialog = app.production.as_ref().expect("it opened");
    let planet = app.selected_planet().expect("a planet");
    assert_eq!(dialog.planet, planet.id);
    assert_eq!(dialog.queue, planet.queue);
    assert_eq!(dialog.queue_index, None, "the top of the queue");

    // Editing the copy leaves the planet alone until OK.
    let before = app.selected_planet().expect("a planet").queue.clone();
    app.production_add(1);
    assert_ne!(
        app.production.as_ref().expect("open").queue,
        before,
        "the copy has changed"
    );
    assert_eq!(
        app.selected_planet().expect("a planet").queue,
        before,
        "and the planet has not"
    );

    app.production_cancel();
    assert!(app.production.is_none());
    assert_eq!(
        app.selected_planet().expect("a planet").queue,
        before,
        "Cancel threw the changes away"
    );
}

/// A home world's inventory holds the things it can build, and queueing a
/// unique item takes it out of the list.
#[test]
fn the_inventory_shrinks_as_things_are_queued() {
    let mut app = a_game();
    app.open_production();

    let rows = app.production_inventory();
    assert!(!rows.is_empty());
    let ids: Vec<u16> = rows.iter().filter(|r| !r.ship).map(|r| r.item).collect();
    assert!(ids.contains(&item::FACTORY), "{ids:?}");
    assert!(ids.contains(&item::MINE), "{ids:?}");
    assert!(ids.contains(&item::ALCHEMY), "{ids:?}");
    // The auto-build items are all there and are marked as such.
    assert!(rows.iter().any(|r| r.auto));
    // A home world starts with a starbase, so its designs are on offer.
    assert!(rows.iter().any(|r| r.ship), "{ids:?}");

    // Queue the planetary scanner — a unique item — and it goes.
    let scanner = rows
        .iter()
        .position(|r| !r.ship && r.item == item::PLANETARY_SCANNER)
        .expect("a home world has no scanner yet");
    app.production.as_mut().expect("open").inventory_index = scanner;
    app.production_add(1);
    assert!(!app
        .production_inventory()
        .iter()
        .any(|r| r.item == item::PLANETARY_SCANNER));

    // And it is in the queue, once.
    let queued = app.production_queue_rows();
    let count = queued
        .iter()
        .filter(|(_, name)| name == "Planetary Scanner")
        .count();
    assert_eq!(count, 1, "{queued:?}");
}

/// Add puts an item under the selected row, and merges with a neighbour that
/// already holds the same thing.
#[test]
fn add_inserts_under_the_selection_and_merges() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    let pick = |app: &mut App, id: u16| {
        let index = app
            .production_inventory()
            .iter()
            .position(|r| !r.ship && r.item == id)
            .unwrap_or_else(|| panic!("item {id} should be on offer"));
        app.production.as_mut().expect("open").inventory_index = index;
    };

    // With the top of the queue selected, the first item goes first.
    pick(&mut app, item::FACTORY);
    app.production_add(2);
    assert_eq!(
        app.production.as_ref().expect("open").queue[0].item,
        item::FACTORY
    );
    assert_eq!(app.production.as_ref().expect("open").queue[0].count, 2);

    // Adding the same item again merges rather than making a second row.
    app.production_add(3);
    assert_eq!(app.production.as_ref().expect("open").queue.len(), 1);
    assert_eq!(app.production.as_ref().expect("open").queue[0].count, 5);

    // A different item goes under the selected row.
    pick(&mut app, item::MINE);
    app.production_add(1);
    let queue = &app.production.as_ref().expect("open").queue;
    assert_eq!(queue.len(), 2);
    assert_eq!(queue[1].item, item::MINE);

    // Selecting the top again puts the next one in front of everything.
    app.production.as_mut().expect("open").queue_index = None;
    pick(&mut app, item::ALCHEMY);
    app.production_add(1);
    let queue = &app.production.as_ref().expect("open").queue;
    assert_eq!(queue[0].item, item::ALCHEMY);
    assert_eq!(queue[1].item, item::FACTORY);
    assert_eq!(queue[2].item, item::MINE);
}

/// Remove takes some off, and empties the row when nothing is left.
#[test]
fn remove_takes_items_back_off() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::FACTORY)
        .expect("factories");
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(5);
    app.production.as_mut().expect("open").queue_index = Some(0);

    app.production_remove(2);
    assert_eq!(app.production.as_ref().expect("open").queue[0].count, 3);
    app.production_remove(100);
    assert!(
        app.production.as_ref().expect("open").queue.is_empty(),
        "removing more than is there empties the row"
    );
    assert_eq!(app.production.as_ref().expect("open").queue_index, None);
}

/// Auto alchemy comes off whole, because its count is only a placeholder.
#[test]
fn auto_alchemy_is_removed_all_at_once() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::AUTO_ALCHEMY)
        .expect("auto alchemy is always on offer");
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(1);
    app.production.as_mut().expect("open").queue_index = Some(0);

    app.production_remove(1);
    assert!(app.production.as_ref().expect("open").queue.is_empty());
}

/// What Shift and Ctrl are worth.
#[test]
fn the_modifier_keys_change_how_many_are_added() {
    assert_eq!(App::production_step(false, false), 1);
    assert_eq!(App::production_step(false, true), 10);
    assert_eq!(App::production_step(true, false), 100);
    assert_eq!(App::production_step(true, true), 1020);
}

/// Item Up and Item Down move a row; Clear empties the queue.
#[test]
fn the_queue_can_be_reordered_and_cleared() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    for id in [item::FACTORY, item::MINE, item::ALCHEMY] {
        let index = app
            .production_inventory()
            .iter()
            .position(|r| !r.ship && r.item == id)
            .expect("on offer");
        app.production.as_mut().expect("open").inventory_index = index;
        app.production_add(1);
    }
    let ids = |app: &App| -> Vec<u16> {
        app.production
            .as_ref()
            .expect("open")
            .queue
            .iter()
            .map(|e| e.item)
            .collect()
    };
    assert_eq!(ids(&app), vec![item::FACTORY, item::MINE, item::ALCHEMY]);

    app.production.as_mut().expect("open").queue_index = Some(2);
    app.production_move(true);
    assert_eq!(ids(&app), vec![item::FACTORY, item::ALCHEMY, item::MINE]);
    assert_eq!(app.production.as_ref().expect("open").queue_index, Some(1));

    app.production_move(false);
    assert_eq!(ids(&app), vec![item::FACTORY, item::MINE, item::ALCHEMY]);

    // Moving off either end does nothing.
    app.production.as_mut().expect("open").queue_index = Some(0);
    app.production_move(true);
    assert_eq!(ids(&app), vec![item::FACTORY, item::MINE, item::ALCHEMY]);

    app.production_clear();
    assert!(ids(&app).is_empty());
}

/// OK writes the queue back and marks the game changed; stepping to another
/// planet writes it back too, which is what keeps the edits.
#[test]
fn ok_and_stepping_planets_both_write_the_queue_back() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();
    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::FACTORY)
        .expect("factories");
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(4);
    let here = app.production.as_ref().expect("open").planet;

    app.production_ok();
    assert!(app.production.is_none());
    assert!(app.dirty);
    let planet = app
        .game
        .as_ref()
        .expect("game")
        .planets
        .iter()
        .find(|p| p.id == here)
        .expect("the planet");
    assert_eq!(planet.queue.len(), 1);
    assert_eq!(planet.queue[0].item, item::FACTORY);
    assert_eq!(planet.queue[0].count, 4);

    // Stepping planets keeps what was edited on the one being left.
    app.open_production();
    app.production.as_mut().expect("open").queue_index = Some(0);
    app.production_remove(1);
    app.production_step_planet(true, false);
    let moved = app.production.as_ref().expect("open").planet;
    let planet = app
        .game
        .as_ref()
        .expect("game")
        .planets
        .iter()
        .find(|p| p.id == here)
        .expect("the planet");
    assert_eq!(
        planet.queue[0].count, 3,
        "the edit was written on the way out"
    );
    if moved != here {
        // The dialog now holds the other planet's queue, not this one's.
        assert_eq!(
            app.production.as_ref().expect("open").queue,
            app.game
                .as_ref()
                .expect("game")
                .planets
                .iter()
                .find(|p| p.id == moved)
                .expect("the other planet")
                .queue
        );
    }
}

/// The cost panel prices what is selected, times how many are queued.
#[test]
fn the_cost_panel_prices_the_selection() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::FACTORY)
        .expect("factories");
    app.production.as_mut().expect("open").inventory_index = index;

    // With nothing selected in the queue it prices one.
    let one = app.production_cost_rows();
    let labels: Vec<&str> = one.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Ironium", "Boranium", "Germanium", "Resources"]
    );
    let resources_of = |rows: &[(String, String)]| -> i32 {
        rows.iter()
            .find(|(l, _)| l == "Resources")
            .map(|(_, v)| v.parse().expect("a number"))
            .expect("a resource row")
    };
    let unit = resources_of(&one);
    assert!(unit > 0);

    // Queue four and the panel prices all four.
    app.production_add(4);
    app.production.as_mut().expect("open").queue_index = Some(0);
    assert_eq!(resources_of(&app.production_cost_rows()), unit * 4);
}

/// The dialog lays out for real, on a real game.
#[test]
fn the_dialog_draws() {
    let mut app = a_game();
    app.open_production();

    let frame = |app: &mut App| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::production::view(app, ui);
            });
        });
    };

    frame(&mut app);
    // Every inventory row selected in turn, so each one's cost panel is built.
    for index in 0..app.production_inventory().len() {
        app.production.as_mut().expect("open").inventory_index = index;
        frame(&mut app);
    }
    // And with a queue, including an auto-build row.
    let auto = app
        .production_inventory()
        .iter()
        .position(|r| r.auto)
        .expect("an auto-build item");
    app.production.as_mut().expect("open").inventory_index = auto;
    app.production_add(10);
    app.production.as_mut().expect("open").queue_index = Some(0);
    frame(&mut app);

    // The <Customize> panel, on each slot in turn.
    for slot in 0..stars_formats::TEMPLATE_SLOTS {
        app.customize_template = Some(slot);
        frame(&mut app);
    }
    app.customize_template = None;

    app.production_cancel();
    frame(&mut app);
}

/// Every queue row carries a year and a colour.
#[test]
fn the_queue_says_when_each_row_will_be_done() {
    use stars_core::production::EtaMark;

    let mut app = a_game();
    app.open_production();
    app.production_clear();

    // One factory on a home world: next year.
    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::FACTORY)
        .expect("factories");
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(1);

    let schedule = app.production_schedule();
    assert_eq!(schedule.len(), 1);
    let (when, mark) = &schedule[0];
    assert_eq!(when, "1 year", "a home world can afford one factory");
    assert_eq!(*mark, EtaMark::AllNextYear);

    // A thousand of them will not be finished in a century.
    app.production_clear();
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(1000);
    let (when, mark) = app.production_schedule().remove(0);
    assert!(
        when == "Never" || when.contains("???") || when.contains("years"),
        "{when}"
    );
    assert_ne!(mark, EtaMark::AllNextYear);

    // An auto-build item that has nothing to do reads as skipped, and one
    // standing by reads "As Needed".
    app.production_clear();
    let alchemy = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::AUTO_ALCHEMY)
        .expect("auto alchemy");
    app.production.as_mut().expect("open").inventory_index = alchemy;
    app.production_add(1);
    app.production.as_mut().expect("open").queue_index = Some(0);
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(1);
    // Alchemy is now in front of the factory, so it is standing by.
    let schedule = app.production_schedule();
    assert_eq!(schedule[0].0, "As Needed", "{schedule:?}");
    assert_eq!(schedule[0].1, EtaMark::Idle);
}

/// Applying a template replaces the queue's auto-build items and leaves the
/// ordinary ones alone.
#[test]
fn a_template_replaces_only_the_auto_build_items() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    let pick = |app: &mut App, id: u16| {
        let index = app
            .production_inventory()
            .iter()
            .position(|r| !r.ship && r.item == id)
            .unwrap_or_else(|| panic!("item {id} should be on offer"));
        app.production.as_mut().expect("open").inventory_index = index;
    };

    // A queue with one ordinary item and two auto-build ones.
    pick(&mut app, item::FACTORY);
    app.production_add(3);
    pick(&mut app, item::AUTO_MINE);
    app.production_add(50);
    pick(&mut app, item::AUTO_DEFENSE);
    app.production_add(5);

    // Import it into slot 1, then wipe the queue and apply it back.
    app.customize_template = Some(1);
    app.production_import_template(1, "Colony");
    assert_eq!(app.production_template_name(1), "Colony");
    assert!(app.production_template_editable(1));

    app.production_clear();
    pick(&mut app, item::MINE);
    app.production_add(2);
    app.production_apply_template(1);

    let queue = &app.production.as_ref().expect("open").queue;
    // The ordinary mine survived, at the front, and the template's two
    // auto-build items came in behind it. The ordinary factory was NOT part
    // of the template.
    assert_eq!(queue[0].item, item::MINE);
    assert_eq!(queue[0].count, 2);
    assert_eq!(queue[1].item, item::AUTO_MINE);
    assert_eq!(queue[1].count, 50);
    assert_eq!(queue[2].item, item::AUTO_DEFENSE);
    assert_eq!(queue[2].count, 5);
    assert_eq!(queue.len(), 3);
}

/// The default template is slot 0, is the player's own default queue, and
/// cannot be renamed or deleted.
#[test]
fn the_default_template_is_the_players_default_queue() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    assert_eq!(app.production_template_name(0), "<Default>");
    assert!(
        !app.production_template_editable(0),
        "the default cannot be renamed or deleted"
    );
    app.production_delete_template(0);
    assert_eq!(app.production_template_name(0), "<Default>");

    // Importing into slot 0 changes the player's default queue, which is a
    // real order.
    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::AUTO_FACTORY)
        .expect("auto factories");
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(100);
    app.production_import_template(0, "ignored");

    let player = &app.game.as_ref().expect("game").players[0];
    assert_eq!(player.default_queue.items.len(), 1);
    assert_eq!(player.default_queue.items[0].item, 1); // AUTO_FACTORY
    assert_eq!(player.default_queue.items[0].count, 100);
}

/// An empty slot reads `<Unused n>`, and deleting one empties it again.
#[test]
fn an_empty_template_slot_says_so() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();

    assert_eq!(app.production_template_name(2), "<Unused 3>");
    assert!(!app.production_template_editable(2));

    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::AUTO_MINE)
        .expect("auto mines");
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(10);
    app.production_import_template(2, "Mines");
    assert_eq!(app.production_template_name(2), "Mines");

    app.production_rename_template(2, "Digging");
    assert_eq!(app.production_template_name(2), "Digging");

    app.production_delete_template(2);
    assert_eq!(app.production_template_name(2), "<Unused 3>");
}

/// The templates go out to `stars.ini` and come back, as the original keeps
/// them there rather than in a save.
#[test]
fn templates_survive_a_trip_through_the_ini() {
    let mut app = a_game();
    app.open_production();
    app.production_clear();
    let index = app
        .production_inventory()
        .iter()
        .position(|r| !r.ship && r.item == item::AUTO_FACTORY)
        .expect("auto factories");
    app.production.as_mut().expect("open").inventory_index = index;
    app.production_add(25);
    app.production_import_template(1, "Growth");

    let ini = app.production_templates_ini();
    assert_eq!(ini[0].0, "ZipOrdersP2", "slot 1 is the second key");
    assert!(!ini[0].1.is_empty());

    let mut fresh = a_game();
    fresh.load_production_templates(&ini);
    assert_eq!(fresh.production_template_name(1), "Growth");
    let queue = fresh.production_templates()[1]
        .queue
        .clone()
        .expect("it came back");
    assert_eq!(queue.items.len(), 1);
    assert_eq!(queue.items[0].count, 25);
}

/// The `<Customize>` panel lists what a template holds, in the game's wording.
#[test]
fn the_customize_panel_lists_the_template() {
    use stars_core::production::template_lines;
    use stars_formats::{DefaultQueue, DefaultQueueItem};

    let empty = DefaultQueue::default();
    assert_eq!(template_lines(None), vec!["<No Auto Build Orders>"]);
    assert_eq!(template_lines(Some(&empty)), vec!["<No Auto Build Orders>"]);

    let queue = DefaultQueue {
        no_research: false,
        items: vec![
            DefaultQueueItem {
                item: 1,
                count: 100,
            },
            DefaultQueueItem { item: 0, count: 1 },
            DefaultQueueItem { item: 3, count: 1 },
        ],
    };
    assert_eq!(
        template_lines(Some(&queue)),
        vec![
            "Factories (100)".to_string(),
            // A count of one is shown by name alone...
            "Mines".to_string(),
            // ...and so is alchemy, whatever its count.
            "Alchemy".to_string(),
        ]
    );
}
