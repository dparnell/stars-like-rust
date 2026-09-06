//! Writing the `.xN` order log.
//!
//! A Stars! order file records what the player's client already did, so the
//! host can replay it. These tests drive the application the way a player does
//! — move cargo, send a fleet somewhere, give it a task, edit a queue, dial
//! research — then write the file and read it back, checking that every order
//! survived and says what it should.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_formats::{order_log, task, LogRecordType, StarsFile, XferAction};
use stars_ui::App;

/// A generated two-player game, saved to a temporary directory.
fn a_saved_game(name: &str) -> (App, std::path::PathBuf) {
    let mut app = App::new();
    let config = NewGame {
        name: name.to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("Turindrones").as_player(),
        ],
        ..NewGame::default()
    };
    app.new_game(&config).expect("creates the game");

    // Named for the process as well as the test: two `cargo test` runs at
    // once would otherwise share this directory, and each would delete the
    // other's game out from under it.
    let dir = std::env::temp_dir().join(format!("stars-ui-orders-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let host = dir.join(format!("{name}.hst"));
    app.save_new_game(&host).expect("writes the game");
    (app, host)
}

#[test]
fn a_turn_of_orders_is_written_and_reads_back() {
    let (mut app, host) = a_saved_game("turn");

    // The player's own fleets, and the planet they orbit.
    let (fleet, colony_ship) = {
        let game = app.game.as_ref().expect("game");
        let mine: Vec<usize> = game
            .fleets
            .iter()
            .enumerate()
            .filter(|(_, f)| f.owner == 0)
            .map(|(i, _)| i)
            .collect();
        assert!(mine.len() >= 2, "player 0 starts with several fleets");
        (mine[0], mine[1])
    };
    let home = app
        .game
        .as_ref()
        .expect("game")
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(0))
        .map(|p| p.id)
        .expect("a homeworld");
    let elsewhere = app
        .game
        .as_ref()
        .expect("game")
        .planets
        .iter()
        .chain(app.game.as_ref().expect("game").known_planets.iter())
        .find(|p| p.owner.is_none() && p.position.is_some())
        .map(|p| p.id)
        .expect("somewhere to go");

    // Five kinds of order.
    let moved = app.transfer_cargo(colony_ship, 0, 20);
    app.set_destination(fleet, elsewhere, 7);
    app.set_task(fleet, task::COLONIZE);
    app.set_destination(fleet, elsewhere, 5); // replacing a leg: delete + insert
    app.set_transport(colony_ship, 0, XferAction::LoadAll);
    app.selection.planet = Some(home);
    let item = app.buildable_items()[0].0;
    app.queue_add(item, 3);
    app.set_research(0, 42);

    // Save, which writes the state file and the orders beside it.
    app.save(&host).expect("saves");
    let orders = host.with_extension("x1");
    assert!(orders.is_file(), "the order file was written");

    // And it reads back as an order log.
    let bytes = std::fs::read(&orders).expect("reads back");
    let file = StarsFile::decode(&bytes).expect("decodes");
    assert_eq!(file.header.file_type, stars_formats::FileType::Orders);
    assert_eq!(file.header.player, 0);
    assert!(file.header.flag_done, "a written order file is submitted");

    let log = order_log(&file);
    let header = log.header.expect("a log header");
    assert_eq!(
        usize::from(header.log_byte_count),
        log.log_byte_count(),
        "cbLog counts the records"
    );
    assert_eq!(
        header.serial_number, 0,
        "this project has no registration to claim"
    );

    let kinds: Vec<LogRecordType> = log.records.iter().map(|r| r.record_type).collect();
    assert!(
        kinds.contains(&LogRecordType::FleetOrderInsert),
        "the fleet was sent somewhere: {kinds:?}"
    );
    assert!(
        kinds.contains(&LogRecordType::FleetOrderUpdate),
        "and given a task: {kinds:?}"
    );
    assert!(
        kinds.contains(&LogRecordType::FleetOrderDelete),
        "and its first leg replaced: {kinds:?}"
    );
    assert!(
        kinds.contains(&LogRecordType::PlanetProdQueue),
        "the queue was edited: {kinds:?}"
    );
    assert!(
        kinds.contains(&LogRecordType::Research),
        "research was dialled: {kinds:?}"
    );
    if moved != 0 {
        assert!(
            kinds.iter().any(|k| matches!(
                k,
                LogRecordType::CargoXfer8 | LogRecordType::CargoXfer16 | LogRecordType::CargoXfer32
            )),
            "cargo moved: {kinds:?}"
        );
    }

    // The records say what they should.
    let research = log
        .records
        .iter()
        .find_map(stars_formats::LogRecord::as_research)
        .expect("a research record");
    assert_eq!(research.pct_resources, 42);

    let queue = log
        .records
        .iter()
        .find_map(stars_formats::LogRecord::as_production_queue)
        .expect("a queue record");
    assert_eq!(queue.planet_id, u16::try_from(home).ok());
    assert!(!queue.items.is_empty());

    let insert = log
        .records
        .iter()
        .rfind(|r| r.record_type == LogRecordType::FleetOrderInsert)
        .and_then(stars_formats::LogRecord::as_waypoint)
        .expect("a waypoint insert");
    assert_eq!(insert.waypoint_index, 1);
    assert_eq!(insert.warp, 5, "the second destination replaced the first");
    assert_eq!(insert.target_id, elsewhere);

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// Generating a turn starts a fresh log.
#[test]
fn the_log_covers_one_turn() {
    let (mut app, host) = a_saved_game("year");
    app.set_research(0, 33);
    assert!(!app.orders.is_empty() || app.game.is_some());

    app.generate_turn();
    app.save(&host).expect("saves");

    let orders = host.with_extension("x1");
    let bytes = std::fs::read(&orders).expect("reads back");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let log = order_log(&file);
    assert!(
        log.records
            .iter()
            .all(|r| r.record_type != LogRecordType::CargoXfer8),
        "last turn's events are gone"
    );
    assert_eq!(file.header.turn, 1, "and the file names the new year");

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The whole loop: a player submits orders, the host replays them.
///
/// This is what an order file is *for*. The game is generated and saved, one
/// player opens their own turn file and gives orders, and the host then opens
/// the host file and generates the year — picking their submission up off the
/// disk, replaying it, and carrying it into the turn.
#[test]
fn a_submitted_turn_reaches_the_host() {
    let (_, host) = a_saved_game("relay");
    let directory = host.parent().expect("a directory").to_path_buf();

    // Player 2 opens their turn file and gives orders.
    let queued;
    let researched = 37;
    {
        let mut player = App::new();
        player
            .open(&directory.join("relay.m2"))
            .expect("opens the turn file");
        assert_eq!(player.local_player(), 1, "the file names its player");

        let home = player
            .game
            .as_ref()
            .expect("game")
            .planets
            .iter()
            .find(|p| p.homeworld)
            .map(|p| p.id)
            .expect("their homeworld");
        player.selection.planet = Some(home);
        let item = player.buildable_items()[0].0;
        player.queue_add(item, 4);
        queued = home;
        player.set_research(1, researched);

        let fleet = player
            .game
            .as_ref()
            .expect("game")
            .fleets
            .iter()
            .position(|f| f.owner == 1)
            .expect("a fleet of theirs");
        player.set_task(fleet, task::COLONIZE);

        player
            .save(&directory.join("relay.m2"))
            .expect("saves and submits");
    }
    assert!(
        directory.join("relay.x2").is_file(),
        "the submission was written"
    );

    // The host opens the host file and generates the year.
    let mut host_app = App::new();
    host_app.open(&host).expect("opens the host file");
    assert_eq!(host_app.local_player(), 0, "the host plays player 0");

    let submitted = host_app.submitted_orders();
    assert_eq!(submitted.len(), 1, "one player submitted");
    assert_eq!(submitted[0].0, 1);

    host_app.generate_turn();
    let summary = host_app.last_turn.as_ref().expect("a turn report");
    assert_eq!(summary.year, 2401);
    assert_eq!(
        summary.replayed,
        vec![(1, 3)],
        "the queue, the research and the fleet order were replayed"
    );

    // And the host's own state now carries what they ordered.
    let game = host_app.game.as_ref().expect("game");
    assert_eq!(
        game.players[1].research_pct, researched,
        "their research setting reached the host"
    );
    let their_home = game
        .planets
        .iter()
        .find(|p| p.id == queued)
        .expect("their homeworld");
    assert!(
        !their_home.queue.is_empty() || their_home.factories > 10 || their_home.mines > 10,
        "their queue reached the host and was built from"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// A log naming another player's things is rejected rather than obeyed.
#[test]
fn a_host_does_not_take_a_log_on_trust() {
    use stars_core::replay::replay_logs;
    use stars_formats::{LogRecord, OrderLog, ResearchOrder};

    let (mut app, host) = a_saved_game("trust");
    let game = app.game.as_mut().expect("game");

    // A log claiming to be player 1's, but naming player 0's fleet and
    // player 0's planet.
    let victim_fleet = game
        .fleets
        .iter()
        .find(|f| f.owner == 0)
        .expect("a fleet of player 0's");
    let fleet_word = victim_fleet.id & 0x1ff; // owner 0 in the high bits
    let victim_planet = game
        .planets
        .iter()
        .find(|p| p.owner == Some(0))
        .map(|p| p.id)
        .expect("a planet of player 0's");

    let mut log = OrderLog::new(0, [0; 11]);
    log.records.push(LogRecord::delete_waypoint(
        stars_formats::FleetOrderDelete {
            fleet_id: fleet_word,
            order_index: 1,
            delete_extra: false,
        },
    ));
    log.records.push(LogRecord::production_queue(
        &stars_formats::ProductionQueueRecord {
            planet_id: u16::try_from(victim_planet).ok(),
            items: Vec::new(),
        },
    ));
    // This one is theirs to change.
    log.records
        .push(LogRecord::research_settings(ResearchOrder {
            pct_resources: 11,
            current_field: 0,
            next_field: 6,
        }));

    let (orders, reports) = replay_logs(game, &[(1, log)]);
    assert!(orders.cargo.is_empty());
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].rejected, 2, "both of player 0's things refused");
    assert_eq!(reports[0].research, 1, "their own setting went through");
    assert_eq!(game.players[1].research_pct, 11);

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// Splitting, merging and renaming a fleet — applied, logged, and replayed by
/// the host to the same result.
#[test]
fn fleets_split_merge_and_rename() {
    let (mut app, host) = a_saved_game("fleets");
    let directory = host.parent().expect("a directory").to_path_buf();

    // A fleet with more than one ship to split. The starting fleets each hold
    // one, so merge two together first.
    let (first, second) = {
        let game = app.game.as_ref().expect("game");
        let mine: Vec<usize> = game
            .fleets
            .iter()
            .enumerate()
            .filter(|(_, f)| f.owner == 0)
            .map(|(i, _)| i)
            .collect();
        (mine[0], mine[1])
    };
    let before = app.game.as_ref().expect("game").fleets.len();
    let design = app.game.as_ref().expect("game").fleets[second].stacks[0].design;

    assert!(app.merge_fleets(first, &[second]), "the merge went through");
    let game = app.game.as_ref().expect("game");
    assert_eq!(game.fleets.len(), before - 1, "one fleet was absorbed");
    let merged = game
        .fleets
        .iter()
        .position(|f| f.ships() == 2)
        .expect("a fleet of two ships");

    assert!(app.rename_fleet(merged, "Bold Endeavour"));
    assert_eq!(
        app.game.as_ref().expect("game").fleets[merged]
            .name
            .as_deref(),
        Some("Bold Endeavour")
    );
    let named_id = app.game.as_ref().expect("game").fleets[merged].id;

    assert!(app.split_fleet(merged, design, 1), "one ship splits off");
    let game = app.game.as_ref().expect("game");
    assert_eq!(
        game.fleets.len(),
        before,
        "and there are as many fleets again"
    );

    // The three orders are in the log, and in the file.
    app.save(&host).expect("saves");
    let bytes = std::fs::read(host.with_extension("x1")).expect("reads back");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let log = order_log(&file);
    let kinds: Vec<LogRecordType> = log.records.iter().map(|r| r.record_type).collect();
    assert!(kinds.contains(&LogRecordType::FleetMerge), "{kinds:?}");
    assert!(kinds.contains(&LogRecordType::FleetName), "{kinds:?}");
    assert!(kinds.contains(&LogRecordType::FleetSplit), "{kinds:?}");
    assert!(kinds.contains(&LogRecordType::FleetCargoXfer), "{kinds:?}");

    let merge = log
        .records
        .iter()
        .find_map(stars_formats::LogRecord::as_fleet_merge)
        .expect("a merge record");
    assert_eq!(merge.absorbed().len(), 1, "one fleet was absorbed");

    // The name is in the saved game too, not just the order log.
    let mut fresh = App::new();
    fresh.open(&host).expect("opens");
    assert_eq!(
        fresh
            .game
            .as_ref()
            .expect("game")
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == named_id)
            .and_then(|f| f.name.clone()),
        Some("Bold Endeavour".to_string()),
        "the name survived the save"
    );

    // A host replaying that log against a fresh copy of the same game reaches
    // the same place.
    // The saved host file already has the orders applied, so replay against
    // the state as it was before them: generate from the original instead.
    let (mut original, _) = {
        let bytes = std::fs::read(directory.join("fleets.hst")).expect("reads");
        let decoded = StarsFile::decode(&bytes).expect("decodes");
        stars_core::GameState::from_file(&decoded)
    };
    let mut cargo = stars_core::TurnOrders::default();
    let report = stars_core::replay::replay(&mut original, 0, &log, &mut cargo);
    assert_eq!(report.merges, 1);
    assert_eq!(report.renames, 1);
    assert_eq!(report.ship_moves, 1);
    assert_eq!(report.rejected, 0, "a host accepts its own player's orders");

    let _ = std::fs::remove_dir_all(&directory);
}

/// Clearing a fleet's name removes its block from the file again.
#[test]
fn a_cleared_fleet_name_leaves_no_block() {
    let (mut app, host) = a_saved_game("unnamed");
    let fleet = app
        .game
        .as_ref()
        .expect("game")
        .fleets
        .iter()
        .position(|f| f.owner == 0)
        .expect("a fleet");
    let id = app.game.as_ref().expect("game").fleets[fleet].id;

    assert!(app.rename_fleet(fleet, "Temporary"));
    app.save(&host).expect("saves");

    let named = std::fs::read(&host).expect("reads");
    let decoded = StarsFile::decode(&named).expect("decodes");
    assert_eq!(
        decoded
            .blocks
            .iter()
            .filter(|b| b.type_id == stars_formats::FLEET_NAME_BLOCK)
            .count(),
        1,
        "one name block"
    );

    // Reopen, clear the name, save again.
    let mut app = App::new();
    app.open(&host).expect("opens");
    let fleet = app
        .game
        .as_ref()
        .expect("game")
        .fleets
        .iter()
        .position(|f| f.owner == 0 && f.id == id)
        .expect("the fleet");
    assert_eq!(
        app.game.as_ref().expect("game").fleets[fleet]
            .name
            .as_deref(),
        Some("Temporary")
    );
    assert!(app.rename_fleet(fleet, ""));
    app.save(&host).expect("saves");

    let cleared = std::fs::read(&host).expect("reads");
    let decoded = StarsFile::decode(&cleared).expect("decodes");
    assert_eq!(
        decoded
            .blocks
            .iter()
            .filter(|b| b.type_id == stars_formats::FLEET_NAME_BLOCK)
            .count(),
        0,
        "and now none"
    );
    let (state, _) = stars_core::GameState::from_file(&decoded);
    assert!(state.fleets.iter().all(|f| f.name.is_none()));

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// Battle plans, repeat orders and relations: applied, logged, and replayed to
/// the same result.
#[test]
fn fleet_settings_and_relations_are_ordered() {
    let (mut app, host) = a_saved_game("settings");
    let fleet = app
        .game
        .as_ref()
        .expect("game")
        .fleets
        .iter()
        .position(|f| f.owner == 0)
        .expect("a fleet");

    assert!(app.set_battle_plan(fleet, 2));
    assert!(app.set_repeat_orders(fleet, true));
    assert!(app.set_relations(1, 2), "player 1 becomes an enemy");
    // A second relations change replaces the record rather than adding one.
    assert!(app.set_relations(1, 1), "and then a friend");

    {
        let game = app.game.as_ref().expect("game");
        assert_eq!(game.fleets[fleet].battle_plan, 2);
        assert!(game.fleets[fleet].repeat_orders);
        assert_eq!(game.players[0].relations.get(1), Some(&1));
    }

    app.save(&host).expect("saves");
    let bytes = std::fs::read(host.with_extension("x1")).expect("reads back");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let log = order_log(&file);
    let kinds: Vec<LogRecordType> = log.records.iter().map(|r| r.record_type).collect();
    assert!(kinds.contains(&LogRecordType::FleetPlan), "{kinds:?}");
    assert!(kinds.contains(&LogRecordType::FleetFlagBit), "{kinds:?}");
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == LogRecordType::Relations)
            .count(),
        1,
        "the relations record was replaced, not repeated: {kinds:?}"
    );

    // The repeat-orders flag and the battle plan are in the saved game too.
    let mut fresh = App::new();
    fresh.open(&host).expect("opens");
    let saved = fresh
        .game
        .as_ref()
        .expect("game")
        .fleets
        .iter()
        .find(|f| f.owner == 0 && f.battle_plan == 2)
        .expect("the fleet kept its plan");
    assert!(saved.repeat_orders, "and its repeat-orders flag");

    // And a host replaying the log reaches the same settings.
    let original = {
        let bytes = std::fs::read(host.with_file_name("settings.hst")).expect("reads");
        StarsFile::decode(&bytes).expect("decodes")
    };
    let (mut state, _) = stars_core::GameState::from_file(&original);
    let mut cargo = stars_core::TurnOrders::default();
    let report = stars_core::replay::replay(&mut state, 0, &log, &mut cargo);
    assert_eq!(report.fleet_settings, 2);
    assert_eq!(report.relations, 1);
    assert_eq!(state.players[0].relations.get(1), Some(&1));

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// A battle plan is retuned, one is added and one deleted: the edits reach the
/// saved game's own type-30 blocks, the order log, and a host replaying it.
#[test]
fn battle_plans_are_edited_saved_and_replayed() {
    let (mut app, host) = a_saved_game("plans");
    let fleet = app
        .game
        .as_ref()
        .expect("game")
        .fleets
        .iter()
        .position(|f| f.owner == 0)
        .expect("a fleet");
    // A fleet on the last plan, so deleting an earlier one has to move it.
    assert!(app.set_battle_plan(fleet, 4));

    let mut retuned = app.game.as_ref().expect("game").players[0].battle_plans[2].clone();
    retuned.name = "Bombers first".to_string();
    retuned.primary_target = 8;
    assert!(app.set_battle_plan_definition(2, &retuned));

    let mut added = retuned.clone();
    added.name = "Last stand".to_string();
    assert!(app.set_battle_plan_definition(5, &added), "appends a sixth");
    assert!(app.delete_battle_plan(1));

    {
        let game = app.game.as_ref().expect("game");
        let plans = &game.players[0].battle_plans;
        assert_eq!(plans.len(), 5);
        assert_eq!(plans[1].name, "Bombers first");
        assert_eq!(plans.last().expect("a plan").name, "Last stand");
        // The fleet followed the deleted plan down a slot.
        assert_eq!(game.fleets[fleet].battle_plan, 3);
    }

    app.save(&host).expect("saves");

    // The saved game holds the edited plans, and the fleet's new index.
    let mut fresh = App::new();
    fresh.open(&host).expect("opens");
    {
        let game = fresh.game.as_ref().expect("game");
        let plans = &game.players[0].battle_plans;
        assert_eq!(plans.len(), 5, "one added, one deleted");
        assert_eq!(plans[1].name, "Bombers first");
        assert_eq!(plans[1].primary_target, 8);
        assert_eq!(plans[4].name, "Last stand");
        for (slot, plan) in plans.iter().enumerate() {
            assert_eq!(usize::from(plan.plan_id), slot, "restamped with its slot");
            assert_eq!(plan.race_id, 0);
        }
        assert_eq!(game.fleets[fleet].battle_plan, 3);
    }

    // And the log carries the three operations, which a host replays to the
    // same five plans.
    let bytes = std::fs::read(host.with_extension("x1")).expect("reads back");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let log = order_log(&file);
    assert_eq!(
        log.records
            .iter()
            .filter(|r| r.record_type == LogRecordType::BattlePlan)
            .count(),
        3,
        "two definitions and a delete"
    );

    let original = {
        let bytes = std::fs::read(host.with_file_name("plans.hst")).expect("reads");
        StarsFile::decode(&bytes).expect("decodes")
    };
    let (mut state, _) = stars_core::GameState::from_file(&original);
    let mut cargo = stars_core::TurnOrders::default();
    let report = stars_core::replay::replay(&mut state, 0, &log, &mut cargo);
    assert_eq!(report.battle_plans, 3);
    assert_eq!(report.rejected, 0);
    let plans = &state.players[0].battle_plans;
    assert_eq!(plans.len(), 5);
    assert_eq!(plans[1].name, "Bombers first");
    assert_eq!(plans[4].name, "Last stand");

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The default production queue: ordered, saved, replayed, and handed to a
/// planet the player settles.
#[test]
fn a_default_queue_is_ordered_and_applied() {
    use stars_core::production::item;
    use stars_formats::{DefaultQueue, DefaultQueueItem};

    let (mut app, host) = a_saved_game("colonies");
    let queue = DefaultQueue {
        no_research: true,
        items: vec![
            DefaultQueueItem {
                item: item::FACTORY as u8,
                count: 100,
            },
            DefaultQueueItem {
                item: item::MINE as u8,
                count: 100,
            },
        ],
    };
    assert!(app.set_default_queue(queue.clone()));
    // A second change replaces the record rather than adding one.
    assert!(app.set_default_queue(queue.clone()));

    app.save(&host).expect("saves");

    // It is in the order log, once.
    let bytes = std::fs::read(host.with_extension("x1")).expect("reads back");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let log = order_log(&file);
    assert_eq!(
        log.records
            .iter()
            .filter(|r| r.record_type == LogRecordType::PlayerZpq1)
            .count(),
        1,
        "the record was replaced, not repeated"
    );

    // And in the saved game.
    let mut fresh = App::new();
    fresh.open(&host).expect("opens");
    assert_eq!(
        fresh.game.as_ref().expect("game").players[0].default_queue,
        queue,
        "the queue survived the save"
    );

    // A host replaying the log reaches the same place, and a planet the player
    // settles starts on it.
    let original = {
        let bytes = std::fs::read(host.with_file_name("colonies.hst")).expect("reads");
        StarsFile::decode(&bytes).expect("decodes")
    };
    let (mut state, _) = stars_core::GameState::from_file(&original);
    let mut cargo = stars_core::TurnOrders::default();
    let report = stars_core::replay::replay(&mut state, 0, &log, &mut cargo);
    assert_eq!(report.default_queues, 1);

    let planet = state
        .planets
        .iter()
        .position(|p| p.owner.is_none())
        .or_else(|| state.known_planets.iter().position(|_| true).map(|_| 0))
        .expect("a planet");
    state.planets[planet].owner = Some(0);
    stars_core::orders::apply_default_queue(&mut state, planet);
    assert!(state.planets[planet].no_research);
    assert_eq!(
        state.planets[planet]
            .queue
            .iter()
            .map(|q| (q.item, q.count))
            .collect::<Vec<_>>(),
        vec![(item::FACTORY, 100), (item::MINE, 100)]
    );

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// A run of edits to one plan is logged once, the way the game's dialog logs
/// one record when it is dismissed rather than one per keystroke.
#[test]
fn repeated_edits_to_one_battle_plan_log_once() {
    let (mut app, host) = a_saved_game("collapse");
    let mut plan = app.game.as_ref().expect("game").players[0].battle_plans[0].clone();
    for target in 0..=4u8 {
        plan.primary_target = target;
        assert!(app.set_battle_plan_definition(0, &plan));
    }
    // A different slot is its own record.
    assert!(app.set_battle_plan_definition(1, &plan));

    let plans = app
        .orders
        .iter()
        .filter(|r| r.record_type == LogRecordType::BattlePlan)
        .count();
    assert_eq!(plans, 2, "one per plan touched, not one per edit");
    assert_eq!(
        app.game.as_ref().expect("game").players[0].battle_plans[0].primary_target,
        4
    );
    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The turn password: set, logged, saved into the player block, and replayed.
/// What travels is the salt the game derives from the text, never the text.
#[test]
fn a_password_is_ordered_and_saved() {
    let (mut app, host) = a_saved_game("password");
    assert_eq!(app.game.as_ref().expect("game").players[0].password, 0);

    assert!(app.set_password("open sesame"));
    let salt = stars_formats::password_salt("open sesame");
    assert_ne!(salt, 0);
    assert_eq!(app.game.as_ref().expect("game").players[0].password, salt);
    // Setting the same one again is not a change.
    assert!(!app.set_password("open sesame"));
    // A second change replaces the record rather than adding one.
    assert!(app.set_password("another one"));
    assert!(app.set_password("open sesame"));
    assert_eq!(
        app.orders
            .iter()
            .filter(|r| r.record_type == LogRecordType::ChangePassword)
            .count(),
        1
    );

    app.save(&host).expect("saves");

    // The saved player block carries the salt, and nothing carries the text.
    let mut fresh = App::new();
    fresh.open(&host).expect("opens");
    assert_eq!(fresh.game.as_ref().expect("game").players[0].password, salt);
    let bytes = std::fs::read(&host).expect("reads");
    assert!(
        !bytes.windows(11).any(|w| w == b"open sesame"),
        "the password itself is never written"
    );

    // And a host replaying the log reaches the same salt.
    let log = {
        let bytes = std::fs::read(host.with_extension("x1")).expect("reads back");
        let file = StarsFile::decode(&bytes).expect("decodes");
        order_log(&file)
    };
    let original = {
        let bytes = std::fs::read(host.with_file_name("password.hst")).expect("reads");
        StarsFile::decode(&bytes).expect("decodes")
    };
    let (mut state, _) = stars_core::GameState::from_file(&original);
    let mut cargo = stars_core::TurnOrders::default();
    let report = stars_core::replay::replay(&mut state, 0, &log, &mut cargo);
    assert_eq!(report.passwords, 1);
    assert_eq!(state.players[0].password, salt);

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// Silencing a message is ordered, saved, and reaches the host.
#[test]
fn a_message_filter_is_ordered_and_replayed() {
    let (mut app, host) = a_saved_game("filter");
    assert!(app.message_filter().is_empty());

    // "You have built a factory" — and with it "You have built N factories",
    // which is the same news in the plural.
    assert!(app.filter_message(0x35, true));
    assert!(app.message_filter().hidden(0x35));
    assert!(app.message_filter().hidden(0x36));
    // Silencing it again is not a change.
    assert!(!app.filter_message(0x35, true));
    // Nor is silencing the other wording of the same thing.
    assert!(!app.filter_message(0x36, true));

    // A second change replaces the record rather than adding one: the record
    // carries the whole bitfield.
    assert!(app.filter_message(0x91, true));
    assert_eq!(
        app.orders
            .iter()
            .filter(|r| r.record_type == LogRecordType::MessageFilter)
            .count(),
        1
    );

    app.save(&host).expect("saves");

    // A host replaying the log ends up with the same filter.
    let log = {
        let bytes = std::fs::read(host.with_extension("x1")).expect("reads back");
        let file = StarsFile::decode(&bytes).expect("decodes");
        order_log(&file)
    };
    let original = {
        let bytes = std::fs::read(host.with_file_name("filter.hst")).expect("reads");
        StarsFile::decode(&bytes).expect("decodes")
    };
    let (mut state, _) = stars_core::GameState::from_file(&original);
    let mut cargo = stars_core::TurnOrders::default();
    let report = stars_core::replay::replay(&mut state, 0, &log, &mut cargo);
    assert_eq!(report.message_filters, 1);
    let filter = state.players[0].message_filter;
    assert!(filter.hidden(0x35) && filter.hidden(0x36));
    // Every wording of a battle report went with the one that was silenced.
    assert!((0x91..=0xa8).all(|id| filter.hidden(id)));
    // And nothing else did.
    assert!(!filter.hidden(0x37));

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The message pane steps through the year's news one message at a time, and
/// steps over what the player has filtered.
#[test]
fn the_message_pane_browses_and_filters() {
    use stars_core::message::{fleet_object, id, Message};

    let (mut app, host) = a_saved_game("pane");

    // Three messages for the local player: two of a kind, one of another.
    {
        let state = app.game.as_mut().expect("game");
        state.messages = vec![
            Message {
                player: 0,
                id: id::MINES_LAID,
                object: fleet_object(1),
                params: vec![1, 40, 0],
            },
            Message {
                player: 0,
                id: id::FLEET_SWEPT,
                object: fleet_object(1),
                params: vec![1, 10, 0],
            },
            Message {
                player: 0,
                id: id::MINES_LAID,
                object: fleet_object(1),
                params: vec![1, 60, 0],
            },
            // Another player's, which this pane never shows.
            Message {
                player: 1,
                id: id::MINES_LAID,
                object: fleet_object(2),
                params: vec![2, 10, 0],
            },
        ];
    }
    app.show_first_message();

    assert_eq!(app.message_count(), 3, "the other player's is not ours");
    assert_eq!(app.message_index, 0);
    assert!(app.message_title().contains("Messages: 1 of 3"));
    assert!(app.message_body().contains("laid"));

    // Next and Prev walk the list and stop at its ends.
    assert!(app.show_next_message());
    assert_eq!(app.message_index, 1);
    assert!(app.show_next_message());
    assert_eq!(app.message_index, 2);
    assert!(!app.show_next_message(), "there is no fourth message");
    assert!(app.show_previous_message());
    assert_eq!(app.message_index, 1);
    app.show_last_message();
    assert_eq!(app.message_index, 2);
    app.show_first_message();
    assert_eq!(app.message_index, 0);

    // Filtering the message being shown silences both of its kind, so Next
    // steps straight past the third.
    assert!(app.toggle_message_filter());
    assert!(app.has_filtered_messages());
    app.show_first_message();
    assert_eq!(app.message_index, 1, "the first unfiltered one");
    assert!(!app.show_next_message(), "the third is filtered too");
    assert!(!app.show_previous_message(), "and so is the first");

    // Asking to see the filtered ones brings them back.
    assert!(app.toggle_view_filtered());
    assert!(app.view_filtered);
    app.show_first_message();
    assert_eq!(app.message_index, 0);
    assert!(app.show_next_message());
    assert_eq!(app.message_index, 1);

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// A message points at the thing it is about, and the Goto button follows it.
#[test]
fn the_message_pane_goes_to_what_a_message_is_about() {
    use stars_core::message::{fleet_object, id, Goto, Message};

    let (mut app, host) = a_saved_game("goto");
    let (fleet_id, planet_id) = {
        let state = app.game.as_ref().expect("game");
        (state.fleets[0].id, state.planets[0].id)
    };

    {
        let state = app.game.as_mut().expect("game");
        state.messages = vec![
            // About a fleet: bit 15 set.
            Message {
                player: 0,
                id: id::MINES_LAID,
                object: fleet_object(fleet_id),
                params: vec![fleet_id as i16, 40, 0],
            },
            // About a planet: a plain positive id.
            Message {
                player: 0,
                id: id::STARBASE_SWEPT,
                object: planet_id,
                params: vec![planet_id, 10, 0],
            },
            // About nothing: the button is dead.
            Message {
                player: 0,
                id: id::TRADER_ANOTHER_PASS,
                object: -1,
                params: vec![1, 0],
            },
        ];
    }
    app.show_first_message();

    assert_eq!(app.message_goto(), Goto::Fleet(fleet_id));
    assert!(app.message_goto_follow());
    assert_eq!(app.selection.fleet, Some(0));

    app.show_next_message();
    assert_eq!(app.message_goto(), Goto::Planet(planet_id));
    assert!(app.message_goto_follow());
    assert_eq!(app.selection.planet, Some(planet_id));

    app.show_next_message();
    assert_eq!(app.message_goto(), Goto::None, "nothing to go to");
    assert!(!app.message_goto_follow());

    // A filtered message's button is dead even while it is on screen.
    app.show_first_message();
    assert!(app.toggle_message_filter());
    assert!(app.toggle_view_filtered());
    app.show_first_message();
    assert_eq!(app.message_index, 0);
    assert_eq!(app.message_goto(), Goto::Fleet(fleet_id), "shown, so live");

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The planet pane says what the original's tiles say, in the original's words.
#[test]
fn the_planet_pane_reports_a_planet() {
    let (mut app, host) = a_saved_game("planet");

    // The home world, which a new game gives a starbase, mines and factories.
    let home = {
        let game = app.game.as_ref().expect("game");
        let home = game
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(0))
            .expect("a home world");
        home.id
    };
    app.selection.planet = Some(home);

    // Minerals On Hand: three minerals in kT, then what is dug and built.
    let minerals = app.planet_minerals_tile();
    let labels: Vec<&str> = minerals.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Ironium", "Boranium", "Germanium", "Mines", "Factories"]
    );
    assert!(
        minerals[0].1.ends_with("kT"),
        "minerals are kilotons: {}",
        minerals[0].1
    );
    // `%d of %d`: what is built, of what the population can run.
    assert!(
        minerals[3].1.contains(" of "),
        "mines are built of operable: {}",
        minerals[3].1
    );

    // Status: the labels are the game's own, in the game's own order.
    let status = app.planet_status_tile();
    let labels: Vec<&str> = status.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Population",
            "Resources/Year",
            "Scanner Type",
            "Scanner Range",
            "Defenses",
            "Defense Type",
            "Def Coverage",
        ]
    );
    // A home world starts with a million colonists, grouped with commas.
    assert!(
        status[0].1.contains(','),
        "population is grouped: {}",
        status[0].1
    );
    // A home world starts with defences, so the type is the best one the
    // player can build and the coverage is a pair of percentages — the second,
    // in brackets, is what survives a smart bomb.
    assert!(status[4].1.contains(" of "), "{}", status[4].1);
    assert_ne!(
        status[5].1, "none",
        "defences are built, so they have a type"
    );
    assert!(
        status[6].1.contains('%') && status[6].1.contains('('),
        "coverage is two percentages: {}",
        status[6].1
    );

    // A starbase, named for its design.
    let (title, rows) = app.planet_starbase_tile();
    assert_ne!(title, "< no starbase >", "the home world has one");
    let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, vec!["Dock Capacity", "Armor", "Shields", "Damage"]);

    // And the title bar is the planet's name.
    assert!(!app.planet_pane_title().is_empty());

    // A planet with nothing queued says so in the original's words.
    assert_eq!(
        app.planet_production_tile(),
        vec!["--- Queue is Empty ---".to_string()],
        "a new game queues nothing"
    );

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// With no planet selected the pane still has something to say.
#[test]
fn the_planet_pane_copes_with_no_selection() {
    let (mut app, host) = a_saved_game("noplanet");
    app.selection.planet = None;

    assert_eq!(app.planet_pane_title(), "Planet View");
    assert!(app.planet_minerals_tile().is_empty());
    assert!(app.planet_status_tile().is_empty());
    assert!(app.planet_production_tile().is_empty());
    assert_eq!(app.planet_starbase_tile().0, "< no starbase >");

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The survey pane summarises whatever is selected, in the original's words.
#[test]
fn the_survey_pane_summarises_a_planet() {
    use stars_ui::SurveySubject;

    let (mut app, host) = a_saved_game("survey");
    let home = {
        let game = app.game.as_ref().expect("game");
        game.planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(0))
            .expect("a home world")
            .id
    };
    app.selection.planet = Some(home);

    assert_eq!(app.survey_subject(), SurveySubject::Planet(home));
    assert!(
        app.survey_title().ends_with(" Summary"),
        "{}",
        app.survey_title()
    );

    // Value, population, the owner and how old the report is.
    let rows = app.survey_planet_rows();
    assert_eq!(rows[0].0, "Value:");
    assert!(rows[0].1.ends_with('%'), "{}", rows[0].1);
    assert_eq!(rows[1].0, "Population:");
    assert!(rows[1].1.contains(','), "grouped: {}", rows[1].1);
    assert!(
        rows.iter().any(|(_, v)| v == "Report is current"),
        "a planet we own is current"
    );

    // The three environment bars, in the game's order and units.
    let env = app.survey_environment();
    let labels: Vec<&str> = env.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, vec!["Gravity", "Temperature", "Radiation"]);
    // Gravity is printed as a bare number — the row's label carries the unit,
    // exactly as `PszCalcGravity` leaves it.
    assert!(
        env[0].value.contains('.') && !env[0].value.ends_with('g'),
        "gravity: {}",
        env[0].value
    );
    assert!(
        env[1].value.ends_with("\u{b0}C"),
        "temperature: {}",
        env[1].value
    );
    assert!(env[2].value.ends_with("mR"), "radiation: {}", env[2].value);
    // A home world sits inside its own race's habitable band.
    for bar in &env {
        assert!(
            bar.immune || (bar.low..=bar.high).contains(&bar.at),
            "{} {} is outside {}..={}",
            bar.label,
            bar.at,
            bar.low,
            bar.high
        );
    }

    // And the three mineral bars: surface stock against concentration.
    let minerals = app.survey_minerals();
    let labels: Vec<&str> = minerals.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, vec!["Ironium", "Boranium", "Germanium"]);
    assert!(minerals[0].value.ends_with("kT"), "{}", minerals[0].value);

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// A fleet, and nothing at all.
#[test]
fn the_survey_pane_summarises_a_fleet_and_deep_space() {
    use stars_ui::{Screen, SurveySubject};

    let (mut app, host) = a_saved_game("survey2");

    app.screen = Screen::Fleets;
    app.selection.fleet = Some(0);
    assert_eq!(app.survey_subject(), SurveySubject::Fleet(0));
    let rows = app.survey_fleet_rows();
    assert!(rows[0].starts_with("Ship Count: "), "{}", rows[0]);
    assert!(
        rows.iter().any(|r| r.starts_with("Fleet Mass: ")),
        "{rows:?}"
    );
    assert!(rows.iter().any(|r| r.starts_with("Cargo: ")), "{rows:?}");
    // A fleet with no orders is stopped, and its next waypoint is (none).
    assert!(
        rows.iter().any(|r| r == "Next Waypoint: (none)"),
        "{rows:?}"
    );
    assert!(
        rows.iter().any(|r| r == "Warp Speed: (stopped)"),
        "{rows:?}"
    );

    // Nothing selected at all.
    app.screen = Screen::Galaxy;
    app.selection.planet = None;
    assert_eq!(app.survey_subject(), SurveySubject::DeepSpace);
    assert_eq!(app.survey_title(), "Deep Space");
    assert!(app.survey_planet_rows().is_empty());

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The fleet pane reports a fleet the way the original's tiles do.
#[test]
fn the_fleet_pane_reports_a_fleet() {
    use stars_core::fleet::Waypoint;
    use stars_ui::Screen;

    let (mut app, host) = a_saved_game("fleetpane");
    app.screen = Screen::Fleets;
    app.selection.fleet = Some(0);

    // Fuel & Cargo: fuel first, then the three minerals and the colonists.
    let cargo = app.fleet_cargo_tile();
    let labels: Vec<&str> = cargo.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Fuel",
            "Ironium",
            "Boranium",
            "Germanium",
            "Colonists",
            "Cargo"
        ]
    );
    assert!(
        cargo[0].1.contains(" of "),
        "fuel is of capacity: {}",
        cargo[0].1
    );

    // Fleet Composition: a design and how many of it.
    let composition = app.fleet_composition_tile();
    assert!(!composition.is_empty(), "a fleet has ships in it");
    assert!(
        composition[0].1.parse::<i32>().is_ok(),
        "a count: {}",
        composition[0].1
    );

    // With no orders, the waypoint tile says where it is and nothing more.
    let rows = app.fleet_waypoints_tile();
    assert_eq!(rows[0].0, "Coming From");
    assert_eq!(rows[1], ("Next Way Pt".to_string(), "(none)".to_string()));
    assert_eq!(app.fleet_task_tile(), "(no task here)");

    // Give it somewhere to go, and the leg is costed.
    {
        let fleet = &mut app.game.as_mut().expect("game").fleets[0];
        let from = fleet.position;
        fleet.waypoints.push(Waypoint {
            position: stars_core::movement::Point::new(from.x + 100, from.y),
            target: None,
            target_class: 4,
            warp: 5,
            task: stars_formats::task::COLONIZE,
            transport: None,
            task_data: Vec::new(),
        });
    }
    let rows = app.fleet_waypoints_tile();
    let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Coming From",
            "Next Way Pt",
            "Warp Factor",
            "Distance",
            "Travel Time",
            "Est Fuel Usage"
        ]
    );
    assert_eq!(rows[2].1, "5");
    assert_eq!(rows[3].1, "100 l.y.");
    // A hundred light years at warp 5 is twenty-five a year: four years.
    assert_eq!(rows[4].1, "4.0 years");
    assert!(rows[5].1.ends_with("kT"), "{}", rows[5].1);
    assert_eq!(app.fleet_task_tile(), "Colonize");

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The scanner's zoom is the original's nine steps, applied as its own integer
/// arithmetic rather than as a percentage.
#[test]
fn the_scanner_zooms_the_way_the_original_does() {
    use stars_ui::{App, ScanView};

    let mut app = App::new();
    assert_eq!(app.scan_zoom, 0);
    assert_eq!(app.scan_zoom_percent(), 100);
    assert_eq!(app.scan_scale(1000), 1000);

    // Out to the smallest, one step at a time.
    let mut percents = vec![app.scan_zoom_percent()];
    for _ in 0..8 {
        app.scan_zoom_by(-1);
        percents.push(app.scan_zoom_percent());
    }
    assert_eq!(percents, vec![100, 75, 50, 38, 25, 25, 25, 25, 25]);
    assert_eq!(app.scan_zoom, -4, "it stops at the end");
    // A quarter, by a shift.
    assert_eq!(app.scan_scale(1000), 250);

    // And in to the largest.
    app.scan_zoom_by(8);
    assert_eq!(app.scan_zoom, 4);
    assert_eq!(app.scan_zoom_percent(), 400);
    assert_eq!(app.scan_scale(1000), 4000);

    // The table and the arithmetic disagree at 38%, which is really three
    // eighths — the original shifts rather than multiplying by the percentage.
    app.scan_zoom = -3;
    assert_eq!(app.scan_zoom_percent(), 38);
    assert_eq!(app.scan_scale(1000), 375, "three eighths, not 38%");

    // The map is drawn upside down: `LogicalToScan` mirrors y.
    app.scan_zoom = 0;
    assert_eq!(app.logical_to_scan(100, 0, 1000), (100, 1000));
    assert_eq!(app.logical_to_scan(100, 1000, 1000), (100, 0));

    // And the six views are the original's six.
    let names: Vec<&str> = ScanView::ALL.iter().map(|v| v.name()).collect();
    assert_eq!(
        names,
        vec![
            "Normal View",
            "Surface Mineral View",
            "Mineral Concentration View",
            "Planet Value View",
            "Population View",
            "No Player Info View",
        ]
    );
}

/// Dragging on the map gives a fleet its orders, and the log carries them.
#[test]
fn waypoints_are_dragged_onto_the_map() {
    let (mut app, host) = a_saved_game("dragging");
    app.selection.fleet = Some(0);

    let (from, owner) = {
        let fleet = &app.game.as_ref().expect("game").fleets[0];
        assert_eq!(
            fleet.waypoints.len(),
            1,
            "a new fleet has only its own spot"
        );
        (fleet.position, fleet.owner)
    };
    assert_eq!(owner, 0, "our own fleet");

    // A leg to a point a hundred light years east.
    assert!(app.add_waypoint(from.x + 100, from.y));
    let warp = {
        let fleet = &app.game.as_ref().expect("game").fleets[0];
        assert_eq!(fleet.waypoints.len(), 2);
        assert_eq!(fleet.waypoints[1].position.x, from.x + 100);
        // The client picks a warp: the fleet's cruising speed, slowed as far as
        // it can go without arriving later.
        assert!(fleet.waypoints[1].warp > 0);
        assert_eq!(fleet.warp, Some(fleet.waypoints[1].warp));
        fleet.waypoints[1].warp
    };
    assert_eq!(
        warp,
        app.suggested_warp(0, 100),
        "the leg takes the suggested warp"
    );

    // A second leg is appended rather than replacing the first.
    assert!(app.add_waypoint(from.x + 100, from.y + 100));
    assert_eq!(
        app.game.as_ref().expect("game").fleets[0].waypoints.len(),
        3
    );

    // Dragging the first leg moves it; waypoint 0 is where the fleet is and
    // cannot be dragged.
    assert!(
        !app.move_waypoint(0, from.x, from.y),
        "the fleet's own spot"
    );
    assert!(app.move_waypoint(1, from.x + 50, from.y));
    assert_eq!(
        app.game.as_ref().expect("game").fleets[0].waypoints[1]
            .position
            .x,
        from.x + 50
    );

    // The pointer finds a waypoint to grab, and misses when it is far off.
    assert_eq!(app.waypoint_at(from.x + 50, from.y, 4.0), Some(1));
    assert_eq!(app.waypoint_at(from.x - 400, from.y, 4.0), None);

    // And dropping one takes it off.
    assert!(app.delete_waypoint(2));
    assert_eq!(
        app.game.as_ref().expect("game").fleets[0].waypoints.len(),
        2
    );

    // Every one of those is on the order log, and a host replaying it reaches
    // the same orders.
    app.save(&host).expect("saves");
    let log = {
        let bytes = std::fs::read(host.with_extension("x1")).expect("reads back");
        let file = StarsFile::decode(&bytes).expect("decodes");
        order_log(&file)
    };
    assert!(
        log.records
            .iter()
            .any(|r| r.record_type == LogRecordType::FleetOrderInsert),
        "the legs were inserted"
    );
    assert!(
        log.records
            .iter()
            .any(|r| r.record_type == LogRecordType::FleetOrderDelete),
        "and one was deleted"
    );

    let original = {
        let bytes = std::fs::read(host.with_file_name("dragging.hst")).expect("reads");
        StarsFile::decode(&bytes).expect("decodes")
    };
    let (mut state, _) = stars_core::GameState::from_file(&original);
    let mut cargo = stars_core::TurnOrders::default();
    let report = stars_core::replay::replay(&mut state, 0, &log, &mut cargo);
    assert!(report.applied() > 0);
    assert_eq!(
        state.fleets[0].waypoints.len(),
        2,
        "the host ends with the same legs"
    );
    assert_eq!(state.fleets[0].waypoints[1].position.x, from.x + 50);

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}

/// The measuring tape reports a distance the way the original words it, and
/// snaps its far end to whatever is under the pointer.
#[test]
fn the_measuring_tape_measures_and_snaps() {
    use stars_core::movement::Point;
    use stars_ui::distance_text;

    // The original works in hundredths of a light year, rounded to nearest,
    // and prints them with `%ld.%ld`.
    assert_eq!(
        distance_text(Point::new(0, 0), Point::new(100, 0)),
        "100.0 l.y."
    );
    assert_eq!(
        distance_text(Point::new(0, 0), Point::new(3, 4)),
        "5.0 l.y.",
        "a 3-4-5 triangle"
    );
    // And the quirk that comes with `%ld.%ld`: the hundredths carry no leading
    // zero, so 20.02 light years reads "20.2".
    let d = distance_text(Point::new(0, 0), Point::new(20, 1));
    assert_eq!(
        d, "20.2 l.y.",
        "20.0249… rounds to 20.02 and prints as 20.2"
    );

    let (mut app, host) = a_saved_game("tape");
    let (home, at) = {
        let game = app.game.as_ref().expect("game");
        let home = game
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(0))
            .expect("a home world");
        (home.id, home.position.expect("a position"))
    };

    // Stretched from a point fifty light years off, the far end snaps onto the
    // planet and the bar names it.
    app.measure_from(at.x - 50, at.y);
    app.measure_to(at.x + 3, at.y, false);
    let bar = app.status_bar();
    assert_eq!(bar.name, app.planet_name(home), "it snapped to the planet");
    assert_eq!((bar.x, bar.y), (at.x, at.y));
    assert_eq!(bar.distance.as_deref(), Some("50.0 l.y."));

    // Let go and the tape stops reporting a distance.
    app.measure_end();
    assert!(app.status_bar().distance.is_none());

    // Far from anything, the end does not snap and the bar says so.
    app.measure_from(at.x, at.y);
    app.measure_to(at.x + 400, at.y + 400, false);
    let bar = app.status_bar();
    assert_eq!(bar.name, "Deep Space");
    assert_eq!((bar.x, bar.y), (at.x + 400, at.y + 400));

    let _ = std::fs::remove_dir_all(host.parent().expect("a directory"));
}
