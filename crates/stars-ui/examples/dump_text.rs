//! Print the three text dumps of the tutorial's world, as File (Dump to
//! Text File) would write them.
//!
//! `cargo run -p stars-ui --example dump_text -- [per-player]`

use stars_ui::App;

fn main() {
    let per_player = std::env::args().nth(1).as_deref() == Some("per-player");
    let mut app = App::new();
    app.create_tutor_world(1024).expect("the tutorial's world");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    if let Ok(exe) = std::fs::read(root.join("stars.2.7j.exe")) {
        app.load_art(exe, "stars.2.7j.exe").expect("art");
    }
    app.per_player_dumps = per_player;
    for dump in [app.dump_universe(), app.dump_planets(), app.dump_fleets()]
        .into_iter()
        .flatten()
    {
        println!("=== {}", dump.file_name);
        print!("{}", dump.text.replace("\r\n", "\n"));
    }
}
