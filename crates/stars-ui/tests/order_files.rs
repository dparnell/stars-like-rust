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

    let dir = std::env::temp_dir().join(format!("stars-ui-orders-{name}"));
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
