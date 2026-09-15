//! Print a history file's score timeline.
use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    let path = std::env::args().nth(1).expect("a history file");
    let file = StarsFile::decode(&std::fs::read(&path).expect("read")).expect("decode");
    let (state, _) = GameState::from_file(&file);
    for (player, years) in state.timeline.iter().enumerate() {
        for year in years {
            println!("player {player} {year:?}");
        }
    }
}
