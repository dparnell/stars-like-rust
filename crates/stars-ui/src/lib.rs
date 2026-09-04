//! # stars-ui
//!
//! Shared, frontend-agnostic view code for the Stars! reimplementation.
//!
//! The intent is that every screen — galaxy map/starfield, the race-creation
//! wizard (mirroring `RACEWIZARDDLG1-6`), the production queue (`ZIPPRODDLG`),
//! and the ship/planet browsers and reports (`BROWSERWNDPROC`,
//! `ORDERINFODLG`) — is written once here (against `stars-core` state) and
//! reused by both the native (`stars-desktop`) and wasm (`stars-web`) shells.
//!
//! The egui/eframe views are built in Step 5 of the delivery plan. For now
//! this crate only holds the shared application-state container so the frontend
//! crates have something concrete to wire up.
//!
//! Before starting those views, read the Step 5 notes in
//! `docs/plans/stars-re-reimplementation.md`. Several things the screens need
//! are already settled and should be driven off `stars-core` rather than
//! reimplemented: which planets a player merely knows about
//! (`Planet::detail`), which production items a race may build
//! (`ground::template_allows`), what a queue entry's fields actually mean, and
//! the two habitability figures the Selection Summary shows. It also records
//! the one hard dependency — the turn generator does not execute waypoint
//! tasks yet, so there is no playable game to attach a UI to until it does.

#![forbid(unsafe_code)]

use stars_core::GameState;

/// Frontend-agnostic application state shared by all Stars! frontends.
///
/// This owns the loaded [`GameState`] (if any) plus, eventually, the
/// view-model state for the open screens. Keeping it here (rather than in a
/// specific frontend) is what lets desktop and web share the same UI logic.
#[derive(Debug, Default)]
pub struct App {
    /// The currently loaded game, or `None` on the title screen.
    pub game: Option<GameState>,
}

impl App {
    /// Create an empty application (no game loaded).
    pub fn new() -> Self {
        Self::default()
    }

    /// Human-readable one-line status used by the frontends' title bars.
    pub fn status_line(&self) -> String {
        match &self.game {
            Some(state) => format!("Stars! — turn {}", state.turn),
            None => "Stars! — no game loaded".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_reflects_loaded_state() {
        let mut app = App::new();
        assert!(app.status_line().contains("no game"));
        app.game = Some(GameState::new(1));
        assert!(app.status_line().contains("turn 0"));
    }
}
