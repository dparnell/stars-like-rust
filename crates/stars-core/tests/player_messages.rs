//! Letters between players: written into a player's order file, gathered by
//! the host, and delivered in the recipients' turn files.
//!
//! `FFinishPlrMsgEntry` (`1030:9bd6`), `FLoadLogFile`'s `rtPlrMsg` arm,
//! `WritePlayerMessages` (`1030:97c0`) and `ReadPlayerMessages`
//! (`1030:9a5a`). See `docs/formats/player-message.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, GameState, Race};
use stars_formats::{LogRecord, OrderLog, PlayerMessage, StarsFile};

fn a_game() -> GameState {
    let config = NewGame {
        name: "letters".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    };
    let mut rng = stars_core::rng::Rng::randomize(5);
    stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state
}

/// A message in player 0's log reaches the host as an order for delivery,
/// stamped with the file's owner as its sender whatever it said; the turn
/// delivers it; the recipient's file carries it and reads back with it,
/// while the sender's and a stranger's do not.
#[test]
fn a_letter_goes_out_with_the_turn_and_comes_back_in_the_file() {
    let mut state = a_game();
    let to_one = PlayerMessage {
        from: 9, // the file's owner overrides this
        to: 2,   // player 1
        in_re: 0,
        text: "Your freighters are welcome at our ports.".to_string(),
    };
    let to_all = PlayerMessage {
        from: 0,
        to: PlayerMessage::EVERYBODY,
        in_re: 3,
        text: "We claim the stars west of the rift.".to_string(),
    };
    let log = OrderLog {
        header: None,
        records: vec![
            LogRecord::player_message(&to_one),
            LogRecord::player_message(&to_all),
        ],
    };
    let (orders, reports) = stars_core::replay::replay_logs(&mut state, &[(0, log)]);
    assert_eq!(reports[0].messages, 2);
    assert_eq!(orders.messages.len(), 2);
    assert_eq!(orders.messages[0].from, 0, "the sender is the file's owner");

    let mut rng = stars_core::rng::Rng::randomize(1);
    stars_core::generate_turn_with_orders(&mut state, &orders, &mut rng);
    assert_eq!(state.player_messages.len(), 2);

    // Player 1 gets both; player 2 the broadcast only; player 0 nothing.
    let delivered = |player: usize| -> Vec<PlayerMessage> {
        let bytes = stars_core::save::player_file(&state, player).expect("writes");
        let file = StarsFile::decode(&bytes).expect("decodes");
        let (loaded, _) = GameState::from_file(&file);
        loaded.player_messages
    };
    let one = delivered(1);
    assert_eq!(one.len(), 2, "{one:?}");
    assert_eq!(one[0].text, to_one.text);
    assert_eq!(one[0].from, 0);
    assert_eq!(one[1].text, to_all.text);
    assert_eq!(one[1].in_re, 3);
    let two = delivered(2);
    assert_eq!(two.len(), 1);
    assert_eq!(two[0].to, PlayerMessage::EVERYBODY);
    assert!(delivered(0).is_empty(), "nothing comes back to the sender");

    // Next year's turn starts afresh: an empty post.
    let quiet = stars_core::TurnOrders::default();
    stars_core::generate_turn_with_orders(&mut state, &quiet, &mut rng);
    assert!(state.player_messages.is_empty());
}
