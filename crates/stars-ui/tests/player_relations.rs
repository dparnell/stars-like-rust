//! The Player Relations dialog, driven the way a player drives it.

use std::path::{Path, PathBuf};

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::relations::Relation;
use stars_core::{opponents, Race};
use stars_formats::LogRecordType;
use stars_ui::App;

fn fixture(relative: &str) -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative);
    path.exists().then_some(path)
}

fn a_game(players: usize) -> App {
    let mut app = App::new();
    let mut list = vec![NewPlayer::human(Race::humanoid())];
    for index in 1..players {
        list.push(
            opponents::opponent(index, 1)
                .expect("an opponent")
                .as_player(),
        );
    }
    app.new_game(&NewGame {
        name: "relations".to_string(),
        size: Size::Small,
        players: list,
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

fn frame(app: &mut App) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::relations::view(app, ui);
        });
    });
}

/// It lists everybody but you, and opens on the first of them.
#[test]
fn it_opens_on_the_first_other_player() {
    let mut app = a_game(4);
    assert!(app.open_relations());
    assert_eq!(app.relations_dialog, Some(1), "player 0 is looking at it");
    assert_eq!(app.relations_others(), vec![1, 2, 3]);
    // Everybody starts neutral.
    for other in app.relations_others() {
        assert_eq!(app.regard(other), Relation::Neutral);
    }
    frame(&mut app);
}

/// The radios are stacked Friend, Neutral, Enemy — which is not the order of
/// the values behind them.
#[test]
fn the_radios_are_not_in_value_order() {
    assert_eq!(
        Relation::ALL.map(|r| r.name()),
        ["Friend", "Neutral", "Enemy"]
    );
    assert_eq!(Relation::ALL.map(|r| r.value()), [1, 0, 2]);
    for relation in Relation::ALL {
        assert_eq!(Relation::from_value(relation.value()), relation);
    }
    // Anything the file does not recognise reads as neutral.
    assert_eq!(Relation::from_value(9), Relation::Neutral);
}

/// Setting a relation changes that player's row and nobody else's, and leaves
/// exactly one order behind however many times it is changed.
#[test]
fn a_change_leaves_one_order_carrying_the_whole_table() {
    let mut app = a_game(4);
    app.open_relations();

    app.relations_select(2);
    assert_eq!(app.relations_dialog, Some(2));
    assert!(app.set_regard(2, Relation::Enemy));
    frame(&mut app);
    assert_eq!(app.regard(2), Relation::Enemy);
    assert_eq!(app.regard(1), Relation::Neutral);
    assert_eq!(app.regard(3), Relation::Neutral);

    app.relations_select(3);
    assert!(app.set_regard(3, Relation::Friend));
    app.relations_select(2);
    assert!(app.set_regard(2, Relation::Friend));
    frame(&mut app);

    // Three changes, one record — it carries the whole table, so the original
    // rewinds the log rather than appending.
    let relations: Vec<_> = app
        .orders
        .iter()
        .filter(|record| record.record_type == LogRecordType::Relations)
        .collect();
    assert_eq!(relations.len(), 1);
    let table = relations[0]
        .as_relations()
        .expect("a relations order")
        .toward;
    assert_eq!(table[1], 0);
    assert_eq!(table[2], 1, "friend");
    assert_eq!(table[3], 1, "friend");

    app.close_relations();
    assert!(app.relations_dialog.is_none());
    frame(&mut app);
}

/// You cannot set a relation toward yourself, or toward a player who is not
/// there.
#[test]
fn there_is_no_row_for_yourself() {
    let mut app = a_game(3);
    app.open_relations();
    let me = app.local_player();
    assert!(!app.relations_others().contains(&me));
    assert!(!app.set_regard(me, Relation::Enemy));
    assert!(!app.set_regard(99, Relation::Enemy));

    // Selecting one is refused too, so the dialog cannot land on it.
    app.relations_select(me);
    assert_ne!(app.relations_dialog, Some(me));
    app.relations_select(99);
    assert_ne!(app.relations_dialog, Some(99));
}

/// A single-player game refuses the dialog outright, which is what the
/// original does.
#[test]
fn a_single_player_game_has_no_relations_to_set() {
    let mut app = a_game(3);
    app.game.as_mut().expect("a game").single_player = true;
    assert!(!app.open_relations());
    assert!(app.relations_dialog.is_none());
    // And nothing is drawn.
    frame(&mut app);
}

/// A real sixteen-player save opens the dialog and lists the other fifteen.
#[test]
fn a_real_save_lists_the_other_fifteen() {
    let Some(save) = fixture("games/no-random-events/2500/Game.m1") else {
        return;
    };
    let mut app = App::new();
    if app.open(&save).is_err() {
        return;
    }
    assert!(app.open_relations());
    assert_eq!(app.relations_others().len(), 15);
    for other in app.relations_others() {
        // That game's player blocks carry no table, so everybody is neutral.
        assert_eq!(app.regard(other), Relation::Neutral);
    }
    app.relations_select(9);
    frame(&mut app);
    assert!(app.set_regard(9, Relation::Enemy));
    assert_eq!(app.regard(9), Relation::Enemy);
    frame(&mut app);
}

/// The layout is the game's own resource, and the resource says several
/// things the code alone does not.
#[test]
fn the_template_is_the_games_own() {
    use stars_ui::dialog::{Class, RELATIONS, RELATION_GROUP};

    assert_eq!(RELATIONS.caption, "Player Relations");
    assert_eq!(RELATIONS.size, (198, 80));
    assert_eq!(RELATIONS.controls.len(), 7);

    // The radios are stacked Friend, Neutral, Enemy — which is not the order
    // of their values, and is the whole reason `Relation::ALL` is written the
    // way it is.
    let friend = RELATIONS.control(0x7d5).expect("Friend");
    let neutral = RELATIONS.control(0x7d4).expect("Neutral");
    let enemy = RELATIONS.control(0x7d6).expect("Enemy");
    assert!(friend.at.1 < neutral.at.1 && neutral.at.1 < enemy.at.1);
    assert_eq!(
        [friend.label(), neutral.label(), enemy.label()],
        Relation::ALL.map(|r| r.name().to_string()),
    );
    // And their ids less 0x7d4 are their stored values.
    for (id, relation) in [
        (0x7d4u16, Relation::Neutral),
        (0x7d5, Relation::Friend),
        (0x7d6, Relation::Enemy),
    ] {
        assert_eq!(u8::try_from(id - 0x7d4).expect("small"), relation.value());
    }

    // One listbox, and no Cancel: Close commits.
    assert_eq!(
        RELATIONS
            .controls
            .iter()
            .filter(|c| c.class == Class::ListBox)
            .count(),
        1
    );
    assert_eq!(RELATIONS.control(0x2).expect("Close").label(), "Close");
    assert_eq!(RELATIONS.control(0x76).expect("Help").label(), "Help");
    assert_eq!(RELATIONS.control(0xffff).expect("label").label(), "Player:");

    // The `Relation` frame is not among them — it is drawn round the radios,
    // from the first's top-left to the last's bottom-right, grown by a line
    // across and half a line down.
    assert!(RELATIONS
        .controls
        .iter()
        .all(|c| c.label() != RELATION_GROUP));
}

/// The hand-drawn group frame is measured off the radios, not off the
/// template, and its caption straddles the top edge.
#[test]
fn the_relation_frame_is_measured_off_the_radios() {
    use stars_ui::dialog::{relation_group, relation_group_caption, RELATIONS};

    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, RELATIONS.pixels());
    let friend = RELATIONS.place(rect, RELATIONS.control(0x7d5).expect("Friend"));
    let enemy = RELATIONS.place(rect, RELATIONS.control(0x7d6).expect("Enemy"));
    let line = 13.0_f32;

    let frame = relation_group(friend, enemy, line);
    assert_eq!(frame.left(), friend.left() - line);
    assert_eq!(frame.right(), enemy.right() + line);
    assert_eq!(frame.top(), friend.top() - (line / 2.0).floor());
    assert_eq!(frame.bottom(), enemy.bottom() + (line / 2.0).floor());
    // It encloses the middle radio too, which is the point of measuring from
    // the outer two.
    assert!(frame.contains_rect(RELATIONS.place(rect, RELATIONS.control(0x7d4).expect("Neutral"))));

    let caption = relation_group_caption(frame, line);
    assert_eq!(caption.x, frame.left() + 8.0);
    assert_eq!(caption.y, frame.top() - (line / 2.0).floor());
}

/// The listbox shows what `PszPlayerName` builds with every flag off: the
/// race's singular name, and nothing else.
#[test]
fn the_list_shows_the_singular_race_name() {
    let app = a_game(3);
    let game = app.game.as_ref().expect("a game");
    for (index, player) in game.players.iter().enumerate() {
        assert_eq!(app.psz_player_name(index), player.name);
        // The singular, not the plural — `fPlural` is off in the call the
        // listbox makes.
        if !player.plural_name.is_empty() && player.plural_name != player.name {
            assert_ne!(app.psz_player_name(index), player.plural_name);
        }
    }
    let listed: Vec<String> = app
        .relations_others()
        .into_iter()
        .map(|other| app.psz_player_name(other))
        .collect();
    for name in &listed {
        assert!(!name.is_empty());
        assert!(!name.contains('('), "no player number: {name}");
    }
}
