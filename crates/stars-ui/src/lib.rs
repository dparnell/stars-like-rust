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
//! the two habitability figures the Selection Summary shows, and where planet
//! coordinates and names come from (the `.xy`, via
//! `GameState::apply_universe`).
//!
//! Two of those notes are worth repeating here because they shape every screen.
//! **Anything the game counts down is a running balance, not a record of a
//! decision** — a production queue entry shows what is left, not what was
//! ordered. And **a player's file holds only that player's own ship designs**,
//! so a screen showing an enemy ship cannot look its design up by slot number;
//! the battle recording's initiative range is all there is.

#![forbid(unsafe_code)]

pub mod vcr;

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
