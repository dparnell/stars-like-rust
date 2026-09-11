//! The message pane — `MessageWndProc` (`1030:5c92`).
//!
//! See `docs/ui/message-pane.md`.

/// The title bar's three decorations, and the guards `HtMsgBox` puts on each
/// of them (`1030:7d8c`).
#[test]
fn the_title_bar_hit_test_is_the_originals() {
    use stars_ui::message::{hit, Hit, MODE_WIDTH};

    // A bar 300 wide and 26 tall, so each square is 26 across.
    let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(300.0, 26.0));
    let at = |x: f32| egui::pos2(x, 13.0);

    // The left square silences the kind of message on screen — but only while
    // a real message is showing.
    assert_eq!(hit(bar, at(5.0), true, false, true, true), Hit::Filter);
    assert_eq!(hit(bar, at(5.0), false, false, true, true), Hit::None);
    assert_eq!(hit(bar, at(25.9), true, false, true, true), Hit::Filter);
    assert_eq!(hit(bar, at(26.1), true, false, true, true), Hit::None);

    // The right square reveals what has been silenced — but only where the
    // sent and filtered bitfields overlap.
    assert_eq!(hit(bar, at(290.0), true, false, true, true), Hit::Reveal);
    assert_eq!(
        hit(bar, at(290.0), true, false, false, true),
        Hit::None,
        "nothing hidden, nothing to offer"
    );

    // The mode strip is the 0x18 immediately left of that square, and only in
    // a multi-player game.
    let mode = bar.right() - bar.height() - MODE_WIDTH + 1.0;
    assert_eq!(hit(bar, at(mode), true, false, true, true), Hit::Mode);
    assert_eq!(hit(bar, at(mode), true, false, true, false), Hit::None);
    assert_eq!(
        hit(bar, at(mode - MODE_WIDTH), true, false, true, true),
        Hit::None,
        "past the strip's left edge"
    );

    // Send-message mode takes the left square out — and does *not* simply
    // disable the right one. `HtMsgBox` sends the whole right side down the
    // mode branch while a message is being written, so the right square's own
    // rectangle answers as `Mode`.
    assert_eq!(hit(bar, at(5.0), true, true, true, true), Hit::None);
    assert_eq!(
        hit(bar, at(290.0), true, true, true, true),
        Hit::Mode,
        "the square joins the mode strip while writing"
    );
    assert_eq!(
        hit(bar, at(290.0), true, true, true, false),
        Hit::None,
        "but only in a multi-player game, like the strip itself"
    );

    // And nothing outside the bar hits anything.
    assert_eq!(
        hit(bar, egui::pos2(150.0, 40.0), true, false, true, true),
        Hit::None
    );
}

/// The watermark runs down the message's own diagonal and shrinks until it
/// fits with eight pixels to spare (`DiaganolTextOut`).
#[test]
fn the_filtered_watermark_runs_corner_to_corner() {
    use stars_ui::message::{watermark_angle, watermark_scale, WATERMARK_MIN};

    // A wide box tilts gently; a square one runs at 45 degrees.
    let wide = egui::vec2(300.0, 60.0);
    let square = egui::vec2(100.0, 100.0);
    assert!(watermark_angle(wide).abs() < watermark_angle(square).abs());
    assert!((watermark_angle(square).abs() - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
    // Negative, so the text reads upwards from the bottom left.
    assert!(watermark_angle(wide) < 0.0);

    // The scale leaves the margin free: doubling the box roughly doubles it.
    let text = egui::vec2(60.0, 12.0);
    let small = watermark_scale(wide, text);
    let big = watermark_scale(wide * 2.0, text);
    assert!(small > 0.0);
    assert!(big > small, "{big} should beat {small}");

    // A box with no room at all asks for nothing, and the pane declines any
    // rectangle under ten pixels either way before it gets that far.
    assert_eq!(watermark_scale(egui::vec2(4.0, 4.0), text), 0.0);
    assert_eq!(WATERMARK_MIN, 10.0);
}

/// A game with two players, for the Goto tests.
fn a_game() -> stars_ui::App {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};

    let mut app = stars_ui::App::new();
    app.new_game(&NewGame {
        name: "messages".to_string(),
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

/// Put one message in front of the player. `object` is the word `SetMsgTitle`
/// classifies: a positive id is a planet, a negative one a fleet.
fn only_message(app: &mut stars_ui::App, object: i16) {
    if let Some(game) = app.game.as_mut() {
        game.messages = vec![stars_core::message::Message {
            player: 0,
            // `idmPlanetsProductionQueueIsEmpty`-ish: any id that is not
            // filtered by default will do, since the Goto is about the
            // object word rather than the text.
            id: 1,
            object,
            params: Vec::new(),
        }];
    }
    app.message_index = 0;
}

/// Goto selects the thing **on the map**, the way `SelectAdjPlanet` and
/// `SelectAdjFleet` do — it does not open a report. The reports are windows
/// of their own and Goto has never opened one.
#[test]
fn goto_selects_on_the_map() {
    use stars_core::message::Goto;
    use stars_ui::Screen;

    let mut app = a_game();
    let (planet, fleet) = {
        let game = app.game.as_ref().expect("a game");
        (
            game.planets
                .iter()
                .find(|p| p.owner == Some(0))
                .expect("a planet")
                .id,
            game.fleets
                .iter()
                .find(|f| f.owner == 0)
                .expect("a fleet")
                .id,
        )
    };

    app.screen = Screen::Galaxy;
    only_message(&mut app, planet);
    assert_eq!(app.message_goto(), Goto::Planet(planet));
    assert!(app.message_goto_follow());
    assert_eq!(app.selection.planet, Some(planet));
    assert!(!app.selection.on_fleet);
    assert_eq!(app.screen, Screen::Galaxy, "the map, not a report window");

    // A fleet's object word is negative, with the id in the low fifteen bits.
    #[allow(clippy::cast_possible_wrap)]
    let word = (fleet | 0x8000) as i16;
    only_message(&mut app, word);
    assert_eq!(app.message_goto(), Goto::Fleet(fleet));
    assert!(app.message_goto_follow());
    assert!(app.selection.on_fleet);
    assert_eq!(app.screen, Screen::Galaxy);
}

/// The two-stage Goto: a production message whose planet is **already**
/// selected opens Change Production instead of selecting it again. The
/// original tests the selection, not a count of clicks.
#[test]
fn a_second_goto_on_a_selected_planet_opens_production() {
    let mut app = a_game();
    let planet = app
        .game
        .as_ref()
        .expect("a game")
        .planets
        .iter()
        .find(|p| p.owner == Some(0) && p.homeworld)
        .expect("a home world")
        .id;
    only_message(&mut app, planet);
    app.selection.planet = None;

    // First: it selects.
    assert!(!app.message_goto_opens_production());
    assert!(app.message_goto_follow());
    assert_eq!(app.selection.planet, Some(planet));
    assert!(app.production.is_none());

    // Second, with it already selected: the queue.
    assert!(app.message_goto_opens_production());
    assert!(app.message_goto_follow());
    assert!(app.production.is_some());
}

/// A message about a battle opens the recording at that place, which is
/// what the button says — `View` rather than `Goto`.
#[test]
fn a_battle_message_opens_the_recording() {
    use stars_core::message::Goto;

    let mut app = a_game();
    if let Some(game) = app.game.as_mut() {
        game.messages = vec![stars_core::message::Message {
            player: 0,
            id: 1,
            // `0x4000` set: a place, from the first two parameters.
            object: 0x4000,
            params: vec![120, 240],
        }];
    }
    app.message_index = 0;
    assert_eq!(app.message_goto(), Goto::Position(120, 240));
    assert_eq!(app.message_goto_label(), "View");

    // With no recording there, the button does nothing rather than
    // pretending.
    assert!(!app.message_goto_follow());

    app.battles = vec![stars_formats::battle::BattleRecord {
        id: 7,
        players: 2,
        player_mask: 0b11,
        planet: u16::MAX,
        position: (120, 240),
        tokens: Vec::new(),
        actions: Vec::new(),
        declared_len: 0,
    }];
    assert!(app.message_goto_follow());
    assert_eq!(app.vcr.as_ref().map(|v| v.id), Some(7));
}

/// The title bar's decorations come out of two bitmaps the game keeps
/// separately — the colour strip and the one-bit `SRCAND` mask — and a
/// glyph's row in one is not its row in the other.
#[test]
fn the_decorations_name_two_strips_and_two_rows() {
    use stars_ui::message::{
        Glyph, COLOUR_SHEET, FILTER, FILTER_ON, FROM_PLAYER, MASK_SHEET, REVEAL, REVEAL_ON,
    };

    assert_eq!((COLOUR_SHEET, MASK_SHEET), (134, 199));

    // Every glyph with a mask names a different row in each strip, which is
    // the thing that would go unnoticed if the two were assumed to match.
    for glyph in [FILTER, FILTER_ON, REVEAL, REVEAL_ON] {
        let mask = glyph.mask_y.expect("a masked glyph");
        assert_ne!(mask, glyph.colour_y, "{glyph:?}");
    }

    // The filter square changes size as well as row when the kind is
    // already silenced.
    assert_eq!(FILTER.size, (15, 14));
    assert_eq!(FILTER_ON.size, (14, 12));
    assert_eq!(REVEAL.size, REVEAL_ON.size);

    // And the one from another player is not masked at all: the original
    // blacks a rectangle and copies over it.
    assert_eq!(FROM_PLAYER.mask_y, None);
    assert_eq!(FROM_PLAYER.size, (15, 9));

    // Every glyph fits inside the strips it names — 16 by 66 and 15 by 84.
    let fits = |g: Glyph| {
        g.colour_y + g.size.1 <= 66
            && g.size.0 <= 16
            && g.mask_y
                .is_none_or(|y| y + g.size.1 <= 84 && g.size.0 <= 15)
    };
    for glyph in [FILTER, FILTER_ON, REVEAL, REVEAL_ON, FROM_PLAYER] {
        assert!(fits(glyph), "{glyph:?} runs off its strip");
    }
}
