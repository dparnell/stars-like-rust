//! Compare a player's own view of the galaxy with the host's.
use stars_core::GameState;
use stars_formats::StarsFile;

fn load(path: &str) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    Some(GameState::from_file(&file).0)
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: known <year dir>");
    let host = load(&format!("{dir}/Game.hst")).expect("host file");
    println!(
        "host: {} owned planets, {} known but not owned",
        host.planets.len(),
        host.known_planets.len()
    );
    for n in 1..=16 {
        let Some(view) = load(&format!("{dir}/Game.m{n}")) else {
            continue;
        };
        let owner = i16::try_from(n - 1).unwrap_or(-1);
        let owns = host
            .planets
            .iter()
            .filter(|p| p.owner == Some(owner))
            .count();
        println!(
            "  player {:>2}: host says it owns {owns:>3}; its own file has {:>3} full, {:>3} known",
            n - 1,
            view.planets.len(),
            view.known_planets.len()
        );
    }
}
