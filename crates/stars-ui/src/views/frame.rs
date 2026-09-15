//! The game's frame: the panes down the left, the dialogs over the map,
//! and the map.
//!
//! `RefitFrameChildren` (`mdi.c`) keeps three panes down the left of the
//! frame — the planet's tiles (or the fleet's, when a fleet is selected)
//! at the top, the messages under them and the survey at the foot — and
//! gives the scanner the rest. The dialogs are windows over it, and the
//! tutor's window is always on top. This is that arrangement, in one
//! place, so the desktop and the tutorial's test harness draw the very
//! same screen: a test that presses what it can see must see what the
//! player sees.

use crate::views::newgame;
use crate::{App, SurveySubject};

/// The panes down the left: one pane, three tile tables.
///
/// The pane is as wide as the tile table — two columns of tiles at their
/// pitch, `tiles::PANE_WIDTH`, plus the panel's own margins — and never
/// narrower: the tiles are laid out in pixels, and a pane squeezed below
/// its table scales the tiles down but not the buttons and text inside
/// them, which then run past the tile's edge and are clipped. That was
/// the desktop's Medium and Large layouts until the tutorial's film
/// showed the Xfer button cut off. View (Window Layout) chooses the
/// tiles' heights (`fSmallTiles`), not the pane's width. Each layout
/// keeps its own panel identity so a width dragged out by hand is
/// remembered per layout.
pub fn panes(app: &mut App, ctx: &egui::Context) {
    if app.game.is_none() {
        return;
    }
    // The panel's own margins, and the room its scroll bar takes from the
    // tiles, so that what is left is the table's width exactly.
    let spacing = ctx.style().spacing.clone();
    let margins = 2.0 * spacing.window_margin.left.max(8.0);
    let scroll = spacing.scroll.bar_width
        + spacing.scroll.bar_inner_margin
        + spacing.scroll.bar_outer_margin;
    // And the two pixels the panel's resize separator keeps for itself.
    let width = crate::tiles::PANE_WIDTH + margins + scroll + 2.0;
    egui::SidePanel::left(egui::Id::new(("planet", app.window_layout as u8)))
        .resizable(true)
        .default_width(width)
        .width_range(width..=640.0)
        .show(ctx, |ui| {
            egui::TopBottomPanel::bottom("messages")
                .resizable(true)
                .default_height(160.0)
                .show_inside(ui, |ui| crate::views::messages::view(app, ui));
            // Below the messages, the survey pane: whatever is selected,
            // summarised. It stands as tall as its contents, which it
            // measured the frame before — `RefitFrameChildren` gives each
            // pane the room it needs, not a share to scroll in.
            let margin = ctx.style().spacing.window_margin.sum().y;
            let wanted = app
                .survey_height
                .map_or(190.0, |h| (h + margin + 2.0).clamp(60.0, 480.0));
            egui::TopBottomPanel::bottom("survey")
                .resizable(false)
                .exact_height(wanted)
                .show_inside(ui, |ui| {
                    let output = egui::ScrollArea::vertical()
                        .show(ui, |ui| crate::views::survey::view(app, ui));
                    app.survey_height = Some(output.content_size.y);
                });
            // One pane, two tile tables: the original swaps the planet's
            // tiles for the fleet's when a fleet is selected.
            let fleet = matches!(app.survey_subject(), SurveySubject::Fleet(_));
            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if fleet {
                        crate::views::fleet::view(app, ui);
                    } else {
                        crate::views::planet::view(app, ui);
                    }
                });
            });
        });
}

/// The dialogs, as windows over the map: the Ship and Starbase Designer,
/// Research, the tutor's window and its notice, Player Relations, Ship
/// Transfer, Cargo Transfer and Production. Closing each does what the
/// original's close does — Research keeps what was set, the tutor hides
/// rather than stops, the transfers and the queue are dropped.
pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    if app.designer.is_some() {
        let mut open = true;
        egui::Window::new("Ship and Starbase Designer")
            .open(&mut open)
            .resizable(true)
            .default_width(660.0)
            .show(ctx, |ui| crate::views::designer::view(app, ui));
        if !open {
            app.close_designer();
        }
    }

    if app.research_dialog.is_some() {
        let mut open = true;
        egui::Window::new("Research")
            .open(&mut open)
            .resizable(true)
            .default_width(700.0)
            .show(ctx, |ui| crate::views::research::view(app, ui));
        // The original's dialog has no Cancel: closing it is Done, and
        // whatever was set is kept.
        if !open {
            app.research_ok();
        }
    }

    // The tutor window: always on top, as `TutorDlg`'s WM_INITDIALOG puts
    // it, and hidden rather than closed by its own Hide button.
    if app.tutor.as_ref().is_some_and(|t| !t.hidden) {
        let mut open = true;
        egui::Window::new("Stars! Tutor")
            .open(&mut open)
            .resizable(false)
            .default_width(crate::dialog::TUTOR.pixels().x)
            .show(ctx, |ui| crate::views::tutorial::view(app, ui));
        if !open {
            // Closing the window is the Hide button, not Stop: the tutorial
            // goes on running.
            app.tutor_notice = app.hide_tutor().map(str::to_string);
        }
    }
    if let Some(notice) = app.tutor_notice.clone() {
        let mut open = true;
        egui::Window::new("Stars!")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(notice);
                if ui.button("OK").clicked() {
                    app.tutor_notice = None;
                }
            });
        if !open {
            app.tutor_notice = None;
        }
    }

    // `BattleVCR` (`hwndVCRDlg`) is a window over the Battle Summary
    // Report, not a screen. Closing it is what lets another recording be
    // opened — the original will not swap one for another either.
    if app.vcr.is_some() {
        let mut open = true;
        egui::Window::new(crate::dialog::BATTLE_VCR.caption)
            .open(&mut open)
            .resizable(true)
            // The board at 32-pixel squares and the panel beside it.
            .default_width(640.0)
            .show(ctx, |ui| crate::views::battles::view(app, ui));
        if !open {
            app.close_battle();
        }
    }

    if app.relations_dialog.is_some() {
        let mut open = true;
        egui::Window::new(crate::dialog::RELATIONS.caption)
            .open(&mut open)
            .resizable(false)
            .default_width(crate::dialog::RELATIONS.pixels().x)
            .show(ctx, |ui| crate::views::relations::view(app, ui));
        if !open {
            app.close_relations();
        }
    }

    if app.split.is_some() {
        let mut open = true;
        egui::Window::new("Ship Transfer")
            .open(&mut open)
            .resizable(false)
            .default_width(crate::dialog::TRANSFER.pixels().x)
            .show(ctx, |ui| crate::views::split::view(app, ui));
        if !open {
            app.split_cancel();
        }
    }

    if app.xfer.is_some() {
        let mut open = true;
        egui::Window::new("Cargo Transfer")
            .open(&mut open)
            .resizable(false)
            .default_width(crate::dialog::TRANSFER.pixels().x)
            .show(ctx, |ui| crate::views::transfer::view(app, ui));
        if !open {
            app.xfer_cancel();
        }
    }

    if app.production.is_some() {
        let mut open = true;
        egui::Window::new("Production")
            .open(&mut open)
            .resizable(true)
            .default_width(700.0)
            .show(ctx, |ui| crate::views::production::view(app, ui));
        if !open {
            app.production_cancel();
        }
    }

    // The browser and the score sheet are modeless in the original, so
    // they sit alongside whatever else is open rather than blocking it —
    // and they are drawn every frame, not only on the one their key was
    // pressed.
    if app.browser.is_some() {
        let mut open = true;
        egui::Window::new("Technology Browser")
            .open(&mut open)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| crate::views::browser::view(app, ui));
        if !open {
            app.close_browser();
        }
    }

    // The report that is open, which is a window over the map rather
    // than a screen: `ReportDlg` creates one, sizes it from `ptSize`
    // and places it with `StickyDlgPos`, and the scanner goes on
    // drawing behind it.
    if let Some(report) = app.open_report_kind() {
        let state = *app.reports.state(report);
        let screen = ctx.screen_rect();
        let size = egui::vec2(f32::from(state.size.0), f32::from(state.size.1));
        let at = if state.centred() {
            screen.center() - size / 2.0
        } else {
            egui::pos2(f32::from(state.pos.0), f32::from(state.pos.1))
        };
        let min = crate::report::ReportState::MIN_SIZE;
        let mut open = true;
        let shown = egui::Window::new(crate::views::report::window_title(app, report))
            .id(egui::Id::new(("report", report.irpt())))
            .open(&mut open)
            .resizable(true)
            .min_size([f32::from(min.0), f32::from(min.1)])
            .default_pos(at)
            .default_size(size)
            .show(ctx, |ui| {
                crate::views::report::view(app, ui, report);
            });
        // `StickyDlgPos` on the way out keeps the top-left, and
        // `WM_DESTROY` the size.
        if let Some(shown) = shown {
            let rect = shown.response.rect;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a window's corner, in whole pixels"
            )]
            let round = |v: f32| v.round() as i16;
            let state = app.reports.state_mut(report);
            state.pos = (round(rect.left()), round(rect.top()));
            state.size = (round(rect.width()), round(rect.height()));
        }
        if !open {
            app.close_report();
        }
    }

    if app.score_sheet.is_some() {
        let mut open = true;
        egui::Window::new("Score")
            .open(&mut open)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| crate::views::score::view(app, ui));
        if !open {
            app.close_score_sheet();
        }
    }
}

/// The map — or the new-game wizard, or a report — in the rest of the
/// frame. Returns what the wizard asked for, if it is up.
pub fn map(app: &mut App, ctx: &egui::Context) -> Option<newgame::Action> {
    egui::CentralPanel::default()
        .show(ctx, |ui| crate::views::central(app, ui))
        .inner
}

/// The whole game screen in the frame's order, and then — after
/// everything that records a widget has drawn — the tutor's ring and the
/// question of whether the page's task is done. The original calls
/// `AdvanceTutor` from fifty-five places, after a click on the map, a
/// change of selection, a message read; asking once a frame, after the
/// frame's input has been handled, covers every one of those.
///
/// Returns the wizard's request and where the ring was painted.
pub fn game_screen(
    app: &mut App,
    ctx: &egui::Context,
) -> (Option<newgame::Action>, Option<egui::Rect>) {
    panes(app, ctx);
    dialogs(app, ctx);
    let action = map(app, ctx);
    let halo = crate::views::tutorial::halo(app, ctx);
    app.advance_tutor();
    (action, halo)
}
