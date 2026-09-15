//! Score `ideal_warp` against the warps recorded on AI fleet waypoints.
use stars_core::ai::dispatch::ideal_warp;
use stars_core::ai::Control;
use stars_core::GameState;
use stars_formats::StarsFile;
use std::collections::BTreeMap;
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();
    let (mut scored, mut exact, mut skip_exact) = (0usize, 0usize, 0usize);
    let mut diff: BTreeMap<i32, usize> = BTreeMap::new();
    let mut human = (0usize, 0usize);
    for y in &years {
        let Ok(b) = std::fs::read(y.join("Game.hst")) else {
            continue;
        };
        let Ok(f) = StarsFile::decode(&b) else {
            continue;
        };
        let (state, _) = GameState::from_file(&f);
        for fl in &state.fleets {
            let Some(pl) = state.players.get(fl.owner.max(0) as usize) else {
                continue;
            };
            let Some(w) = fl.waypoints.get(1) else {
                continue;
            };
            if w.warp == 0 {
                continue;
            }
            let warp = w.warp;
            let owner = fl.owner.max(0) as usize;
            let Some(list) = state.designs.get(owner) else {
                continue;
            };
            let designs: Vec<_> = fl
                .stacks
                .iter()
                .filter_map(|st| list.get(usize::from(st.design)).map(|d| (d, st.count)))
                .collect();
            if designs.is_empty() {
                continue;
            }
            let pred = ideal_warp(&designs, false);
            let pred_skip = ideal_warp(&designs, true);
            let ai = matches!(pl.control, Control::Computer { .. });
            if ai {
                scored += 1;
                if pred == warp {
                    exact += 1;
                }
                if pred_skip == warp {
                    skip_exact += 1;
                }
                *diff.entry(i32::from(pred) - i32::from(warp)).or_default() += 1;
            } else {
                human.1 += 1;
                if pred == warp {
                    human.0 += 1;
                }
            }
        }
    }
    println!("{scored} AI waypoint legs with a warp");
    println!("  exact: {exact} ({}%)", pct(exact, scored));
    println!("  predicted - recorded: {diff:?}");
    println!(
        "  with the back-off skipped: {skip_exact} ({}%)",
        pct(skip_exact, scored)
    );
    println!("human legs (control): {} of {}", human.0, human.1);
}

fn pct(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
