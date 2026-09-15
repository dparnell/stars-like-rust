//! Dump the production queues in a saved game.
//!
//! A reverse-engineering aid: a type-28 queue block carries no planet id, so
//! this prints each queue next to the planet that owns it, which is how the
//! block layout in `docs/formats/production.md` was checked.
//!
//! ```text
//! cargo run -p stars-core --example queues -- fixtures/incoming/turn1/Game.hst
//! ```

use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    for path in std::env::args().skip(1) {
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("cannot read {path}");
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            eprintln!("cannot decode {path}");
            continue;
        };
        let (state, _) = GameState::from_file(&file);
        println!("{path} — year {}", state.year());
        for p in &state.planets {
            if p.queue.is_empty() {
                continue;
            }
            println!(
                "  planet {:>3} (owner {:?}, {} mines, {} factories)",
                p.id, p.owner, p.mines, p.factories
            );
            for q in &p.queue {
                let what = if q.ship {
                    format!("design {}", q.item)
                } else {
                    format!(
                        "item {}{}",
                        q.item,
                        if q.is_auto() { " (auto)" } else { "" }
                    )
                };
                println!("      {:>4} x {what}, {}% paid", q.count, q.completion);
            }
        }
    }
}
