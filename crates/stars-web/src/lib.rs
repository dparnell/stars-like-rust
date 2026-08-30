//! # stars-web
//!
//! Web/WebAssembly shell for the Stars! reimplementation (the delivery plan's
//! stretch goal).
//!
//! In Step 7 this crate compiles the shared `stars-ui` egui views to wasm via
//! eframe and adds browser storage/file integration, verifying that the wasm
//! build is deterministically identical to native (same seed → same result).
//!
//! For now it exposes a tiny host-testable entry point so the crate compiles
//! and participates in the workspace build/test cycle without requiring the
//! wasm toolchain.

#![forbid(unsafe_code)]

use stars_ui::App;

/// Build the shared application state for the web frontend.
///
/// Returns the initial title-screen status line; the real wasm bootstrap
/// (canvas attach, storage wiring) is added in Step 7.
pub fn boot() -> String {
    App::new().status_line()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_returns_title_status() {
        assert!(boot().contains("no game"));
    }
}
