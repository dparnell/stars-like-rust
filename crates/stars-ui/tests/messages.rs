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
