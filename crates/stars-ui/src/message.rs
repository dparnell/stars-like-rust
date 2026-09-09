//! The message pane's geometry, and its diagonal watermark.
//!
//! `MessageWndProc` (`1030:5c92`) draws the pane itself and `HtMsgBox`
//! (`1030:7d8c`) hit-tests the title bar. Neither takes a dialog template: the
//! pane is a child window of the frame and everything in it is measured from
//! `dyArial8` and from the bar's own height.
//!
//! See `docs/ui/message-pane.md`.

/// How tall the title bar is: two lines of Arial 8.
pub const TITLE_LINES: f32 = 2.0;

/// How much of the bar's width the title text is truncated to, so the
/// decorations at either end are never overdrawn.
pub const TITLE_INSET: f32 = 48.0;

/// How wide the three buttons along the foot are, and how tall — `0x2c` by
/// `dyArial8 * 3 / 2`.
pub const BUTTON_WIDTH: f32 = 0x2c as f32;
/// What the button height is a multiple of.
pub const BUTTON_LINES: f32 = 1.5;
/// The fourth button, which appears only while a message is being written.
pub const SEND_BUTTON_WIDTH: f32 = 0x32 as f32;

/// How wide the **mode** strip is: `0x18`, just left of the right-hand square.
pub const MODE_WIDTH: f32 = 0x18 as f32;

/// How many bytes the sent and filtered bitfields are — `0x31`, so 392
/// message ids fit in each.
pub const BITFIELD_BYTES: usize = 0x31;

/// What the pointer is over in the title bar.
///
/// The two squares are each as wide as the bar is **tall**, one at each end,
/// and the mode strip is `0x18` wide immediately left of the right-hand
/// square. `HtMsgBox` guards each of them separately, and the guards are as
/// much of the behaviour as the rectangles are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Nothing.
    None,
    /// The square at the left, which silences the kind of message on screen.
    /// Offered only while a real message is showing and the pane is not in
    /// send-message mode.
    Filter,
    /// The square at the right, which reveals what has been silenced. Offered
    /// only where the sent and filtered bitfields **overlap** — there is no
    /// point offering to reveal messages that were never sent.
    Reveal,
    /// The strip before it, which switches to writing a message. Offered only
    /// when the game has more than one human player — and, while a message is
    /// being written, it swallows the right-hand square's rectangle too.
    Mode,
}

/// Hit-test a point against the title bar (`HtMsgBox`, `1030:7d8c`).
///
/// `showing` is whether a real message is on screen — the original's
/// `0 <= iMsgCur < cMsg`; `writing` is send-message mode; `anything_hidden` is
/// whether the two bitfields overlap; and `multiplayer` is the game's
/// `fSinglePlr` flag, inverted.
#[must_use]
pub fn hit(
    bar: egui::Rect,
    at: egui::Pos2,
    showing: bool,
    writing: bool,
    anything_hidden: bool,
    multiplayer: bool,
) -> Hit {
    if !bar.contains(at) {
        return Hit::None;
    }
    let square = bar.height();
    if at.x < bar.left() + square && showing && !writing {
        return Hit::Filter;
    }
    if at.x >= bar.right() - square && !writing {
        return if anything_hidden {
            Hit::Reveal
        } else {
            Hit::None
        };
    }
    if multiplayer && at.x >= bar.right() - square - MODE_WIDTH {
        return Hit::Mode;
    }
    Hit::None
}

/// What the **FILTERED** watermark is written across the message with.
///
/// `DiaganolTextOut` (`1030:…`) writes it corner to corner: it builds a
/// `LOGFONT` with `lfWeight = 900` — the heaviest there is — sets
/// `lfEscapement` to the rectangle's own diagonal angle, and then **shrinks
/// the size until the text fits with eight pixels to spare in both
/// directions**, starting from the longer of the two sides. It gives up
/// entirely on a rectangle smaller than ten pixels either way.
pub const WATERMARK_MARGIN: f32 = 8.0;

/// The smallest rectangle the watermark will attempt.
pub const WATERMARK_MIN: f32 = 10.0;

/// The angle the watermark runs at, in radians: the rectangle's own diagonal,
/// running from the bottom left to the top right.
#[must_use]
pub fn watermark_angle(size: egui::Vec2) -> f32 {
    -size.y.atan2(size.x)
}

/// The largest size the watermark can be drawn at, given how big one line of
/// it comes out at a reference size.
///
/// The original loops on the font height; this solves the same question in one
/// step, because the text's extent scales with the size.
#[must_use]
pub fn watermark_scale(size: egui::Vec2, text: egui::Vec2) -> f32 {
    if text.x <= 0.0 || text.y <= 0.0 {
        return 0.0;
    }
    // The rotated text's bounding box, as a multiple of the unrotated one.
    let angle = watermark_angle(size).abs();
    let (sin, cos) = angle.sin_cos();
    let wide = text.x * cos + text.y * sin;
    let tall = text.x * sin + text.y * cos;
    let room = egui::vec2(
        (size.x - WATERMARK_MARGIN).max(0.0),
        (size.y - WATERMARK_MARGIN).max(0.0),
    );
    (room.x / wide).min(room.y / tall).max(0.0)
}
