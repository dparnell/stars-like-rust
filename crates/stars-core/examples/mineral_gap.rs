//! Break down the whole-turn replay's surface-mineral disagreement.
use stars_core::rng::Rng;
use stars_core::{generate_turn, GameState};
use stars_formats::StarsFile;
use std::collections::{BTreeMap, HashMap};

fn load(p: &std::path::Path) -> Option<GameState> {
    let b = std::fs::read(p).ok()?;
    let f = StarsFile::decode(&b).ok()?;
    Some(GameState::from_file(&f).0)
}

fn main() {
    let games = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let (mut readings, mut exact, mut within1, mut all3) = (0usize, 0usize, 0usize, 0usize);
    let mut planets = 0usize;
    let mut err: BTreeMap<i32, usize> = BTreeMap::new();
    for w in years.windows(2) {
        if w[1] != w[0] + 1 {
            continue;
        }
        let (Some(mut before), Some(after)) = (
            load(&games.join(w[0].to_string()).join("exodus.m6")),
            load(&games.join(w[1].to_string()).join("exodus.m6")),
        ) else {
            continue;
        };
        let actual: HashMap<i16, _> = after.planets.iter().map(|p| (p.id, p.clone())).collect();
        for p in &mut before.planets {
            if let Some(n) = actual.get(&p.id) {
                p.env = n.env;
            }
        }
        let mut rng = Rng::randomize(before.seed);
        generate_turn(&mut before, &mut rng);
        for p in &before.planets {
            let Some(want) = actual.get(&p.id) else {
                continue;
            };
            planets += 1;
            let mut ok = 0;
            for i in 0..3 {
                readings += 1;
                let d = p.surface_min[i] - want.surface_min[i];
                *err.entry(d.clamp(-6, 6)).or_default() += 1;
                if d == 0 {
                    exact += 1;
                    ok += 1;
                }
                if d.abs() <= 1 {
                    within1 += 1;
                }
            }
            if ok == 3 {
                all3 += 1;
            }
        }
    }
    let pct = |n: usize, t: usize| {
        n.checked_mul(100)
            .and_then(|x| x.checked_div(t))
            .unwrap_or(0)
    };
    println!("{planets} planet-years, {readings} mineral readings");
    println!(
        "  per mineral exact:    {exact} ({}%)",
        pct(exact, readings)
    );
    println!(
        "  per mineral within 1: {within1} ({}%)",
        pct(within1, readings)
    );
    println!("  all three exact:      {all3} ({}%)", pct(all3, planets));
    println!("  error (ours - recorded, clamped +/-6): {err:?}");
}
