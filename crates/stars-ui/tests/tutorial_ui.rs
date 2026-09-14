//! The tutorial, played through the interface itself.
//!
//! The driver and the pages live in `stars_ui::autopilot`, so that the
//! desktop's `autoplay_tutorial` example can play the same run in a window;
//! this runs it headless, every step asserted.

use stars_ui::autopilot::{script, Shell};

/// Year zero onward, played by pressing what the pages name where the
/// panes draw it.
#[test]
fn the_tutorial_is_played_through_the_panes() {
    let mut shell = Shell::new(script::tutorial_app());
    script::tutorial(&mut shell);
}

/// The planet pane's own tile has Prev and Next as well — `SelectAdjPlanet`
/// walks the player's planets the way the fleet tile walks the fleets.
#[test]
fn the_planet_tile_walks_the_players_planets() {
    let mut shell = Shell::new(script::tutorial_app());
    shell.frame();
    let home = shell
        .app
        .selection
        .planet
        .expect("the home world is selected");
    // With one planet, Next comes back round to it.
    shell.press("planet", "Next");
    assert_eq!(shell.app.selection.planet, Some(home));
    assert!(!shell.app.selection.on_fleet, "and the planet is in front");
}
