//! Native desktop shell for the Stars! reimplementation.
//!
//! In Step 5 this becomes an eframe/winit application hosting the shared
//! `stars-ui` views and wiring them to file I/O (`stars-formats`) and turn
//! generation (`stars-core`). For now it is a minimal placeholder that proves
//! the frontend can construct and query the shared application state.

#![forbid(unsafe_code)]

use stars_ui::App;

fn main() {
    let app = App::new();
    println!("{}", app.status_line());
    println!("(stars-desktop placeholder — the egui frontend is built in Step 5)");
}
