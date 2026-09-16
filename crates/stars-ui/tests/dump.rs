//! File (Dump to Text File): the three text dumps. See `docs/ui/dump.md`.

use stars_ui::App;

fn tutorial() -> App {
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    app
}

fn executable() -> Option<Vec<u8>> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    std::fs::read(root.join("stars.2.7j.exe")).ok()
}

/// How many fields each line has, header and rows alike.
fn field_counts(text: &str) -> Vec<usize> {
    text.split("\r\n")
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').count())
        .collect()
}

/// Nothing without a game, as the original refuses.
#[test]
fn nothing_is_dumped_without_a_game() {
    let app = App::new();
    assert!(app.dump_universe().is_none());
    assert!(app.dump_planets().is_none());
    assert!(app.dump_fleets().is_none());
}

/// The universe: `#`, `X`, `Y`, `Name`, a planet a line, one-based.
#[test]
fn the_universe_dump_lists_every_planet() {
    let app = tutorial();
    let dump = app.dump_universe().expect("a game");
    assert!(dump.file_name.ends_with(".map"));
    let lines: Vec<&str> = dump.text.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(lines[0], "#\tX\tY\tName");
    let planets = app.universe.as_ref().unwrap().planets_resolved().len();
    assert_eq!(lines.len(), 1 + planets);
    assert_eq!(planets, 24);
    assert!(lines[1].starts_with("1\t"), "{}", lines[1]);
    assert!(lines.iter().any(|l| l.ends_with("\tStove Top")));
}

/// The planet dump: the header's columns match every row's, in both
/// widths, and the file is named for the width.
#[test]
fn the_planet_dump_is_a_table() {
    let mut app = tutorial();
    let dump = app.dump_planets().expect("a game");
    assert!(dump.file_name.ends_with(".pla"));
    let counts = field_counts(&dump.text);
    assert_eq!(counts[0], 20, "twenty columns in the plain dump");
    assert!(counts.iter().all(|&c| c == 20), "{counts:?}");
    let rows = counts.len() - 1;
    let known =
        app.game.as_ref().unwrap().planets.len() + app.game.as_ref().unwrap().known_planets.len();
    assert_eq!(rows, known);
    // Stove Top is the player's, with a starbase and a population.
    let stove_top = dump
        .text
        .split("\r\n")
        .find(|l| l.starts_with("Stove Top\t"))
        .expect("the home world");
    let fields: Vec<&str> = stove_top.split('\t').collect();
    assert_eq!(fields[1], "Humanoid");
    assert_eq!(fields[2], "Starbase");
    assert_eq!(fields[4], "100000");
    assert_eq!(fields[5], "100%");

    app.per_player_dumps = true;
    let dump = app.dump_planets().expect("a game");
    assert!(dump.file_name.ends_with(".p1"));
    let counts = field_counts(&dump.text);
    assert_eq!(counts[0], 36, "sixteen more in the wider dump");
    assert!(counts.iter().all(|&c| c == 36), "{counts:?}");
}

/// The fleet dump likewise.
#[test]
fn the_fleet_dump_is_a_table() {
    let mut app = tutorial();
    let dump = app.dump_fleets().expect("a game");
    assert!(dump.file_name.ends_with(".fle"));
    let counts = field_counts(&dump.text);
    assert_eq!(counts[0], 12);
    assert!(counts.iter().all(|&c| c == 12), "{counts:?}");
    assert_eq!(counts.len() - 1, app.game.as_ref().unwrap().fleets.len());
    // Every fleet is named with its owner in front, the player's own too.
    let first = dump.text.split("\r\n").nth(1).expect("a fleet");
    assert!(first.starts_with("Humanoid "), "{first}");

    app.per_player_dumps = true;
    let dump = app.dump_fleets().expect("a game");
    assert!(dump.file_name.ends_with(".f1"));
    let counts = field_counts(&dump.text);
    assert_eq!(counts[0], 29);
    assert!(counts.iter().all(|&c| c == 29), "{counts:?}");
}

/// The headers are the game's own with the executable.
#[test]
fn the_headers_come_from_the_executable() {
    let mut app = tutorial();
    let Some(exe) = executable() else { return };
    app.load_art(exe, "stars.2.7j.exe").expect("art");
    let planets = app.dump_planets().expect("a game");
    assert!(planets
        .text
        .starts_with("Planet Name\tOwner\tStarbase Type\tReport Age\t"));
    let fleets = app.dump_fleets().expect("a game");
    assert!(fleets
        .text
        .starts_with("Fleet Name\tX\tY\tPlanet\tDestination\tBattle Plan\tShip Cnt\t"));
}
