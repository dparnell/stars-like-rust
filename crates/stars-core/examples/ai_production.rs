//! Score the AI production transcription against a recorded game.
//!
//! For every AI-owned planet in every turn of a saved game, work out what
//! `FFillProdMinesAndFactories` would queue and compare it with what the
//! original actually left in the queue.
//!
//! ```text
//! cargo run -p stars-core --example ai_production -- fixtures/games/all-computer-players
//! ```

use std::collections::BTreeMap;

use stars_core::ai::production::{fill_prod_mines_and_factories, Context};
use stars_core::ai::Control;
use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    let dir = std::env::args().nth(1).expect("usage: ai_production <dir>");
    let verbose = std::env::args().any(|a| a == "-v");

    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut total = 0usize;
    let mut item_ok = 0usize;
    let mut exact = 0usize;
    let mut empty_recorded = 0usize;
    let mut by_item: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut shown = 0usize;

    for year_dir in &years {
        let path = year_dir.join("Game.hst");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);
        let turn = state.year() - 2400;

        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };

            let mut tech = [0u8; 6];
            for (i, t) in player.research.levels.iter().enumerate().take(6) {
                tech[i] = *t;
            }

            // The AI decides from an empty queue: its previous entry has been
            // consumed by the production step that ran before it.
            let mut bare = planet.clone();
            bare.queue.clear();
            let ctx = Context {
                personality,
                research_pct: player.research_pct,
                tech,
                turn,
                terraform_steps: 0,
                factories_cost_all_minerals: false,
            };
            let predicted = fill_prod_mines_and_factories(&bare, &player.race, &ctx).entries();

            let recorded: Vec<_> = planet
                .queue
                .iter()
                .filter(|e| !e.ship)
                .map(|e| (e.item, e.count))
                .collect();
            let got: Vec<_> = predicted.iter().map(|e| (e.item, e.count)).collect();

            // Only the entries this routine is responsible for. Terraforming,
            // defences, scanners, starbases and packets are queued by other AI
            // routines that are not implemented yet.
            let mine_or_factory = |v: &[(u16, i32)]| -> Vec<(u16, i32)> {
                v.iter()
                    .filter(|(i, _)| *i == 7 || *i == 8)
                    .copied()
                    .collect()
            };
            let recorded = mine_or_factory(&recorded);
            let got = mine_or_factory(&got);
            if recorded.is_empty() {
                if !got.is_empty() {
                    empty_recorded += 1;
                }
                continue;
            }
            total += 1;

            let name = |v: &[(u16, i32)]| {
                v.iter()
                    .map(|(i, c)| format!("{i}x{c}"))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            *by_item.entry((name(&recorded), name(&got))).or_default() += 1;

            let items_match: Vec<u16> = recorded.iter().map(|(i, _)| *i).collect();
            let got_items: Vec<u16> = got.iter().map(|(i, _)| *i).collect();
            if items_match == got_items {
                item_ok += 1;
                if recorded == got {
                    exact += 1;
                } else if verbose && shown < 25 {
                    shown += 1;
                    println!(
                        "  {} planet {:>3} owner {owner}: recorded {} predicted {} \
                         (mines {} facts {} pop {})",
                        state.year(),
                        planet.id,
                        name(&recorded),
                        name(&got),
                        planet.mines,
                        planet.factories,
                        planet.pop
                    );
                }
            }
        }
    }

    println!("{total} AI planet-turns where the game queued mines or factories");
    println!("  {empty_recorded} more where it queued none but we would have");
    println!("  right item(s):  {item_ok} ({}%)", percent(item_ok, total));
    println!("  item and count: {exact} ({}%)", percent(exact, total));

    println!("\nmost common (recorded -> predicted):");
    let mut pairs: Vec<_> = by_item.into_iter().collect();
    pairs.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for ((rec, got), n) in pairs.into_iter().take(15) {
        let mark = if rec == got { "ok " } else { "BAD" };
        println!("  {mark} {n:>5}  {rec:<16} -> {got}");
    }
}

fn percent(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
