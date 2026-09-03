use stars_core::GameState;
use stars_formats::StarsFile;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let file = StarsFile::decode(&bytes).unwrap();
    let (state, _) = GameState::from_file(&file);
    for (i, p) in state.players.iter().enumerate() {
        println!(
            "  player {i:>2}: prt {:?} ar={}",
            p.race.prt(),
            p.race.is_ar()
        );
    }
}
