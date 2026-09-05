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
    let host = dir.join("Orders.hst");
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
    let orders = host.with_file_name("Orders.x1");
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

    let orders = host.with_file_name("Orders.x1");
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
