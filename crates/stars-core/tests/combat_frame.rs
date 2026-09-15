//! The frame around the combat board, checked against the sixteen-player
//! computer game: a year is generated from its host file, and the battles
//! the engine fought are set beside the recordings the original wrote into
//! the players' files for the following year — where they happened, who was
//! there, and what each token was made of.
//!
//! The host file carries every player's designs, which the Exodus corpus
//! does not, so this is the one place a token's mass, shields, initiative,
//! speed and cloak can be checked from the fleets rather than taken from
//! the recording. Fixtures are optional: the test skips without them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stars_core::{generate_turn, GameState};
use stars_formats::{battle_records_in_with, ActionLayout, BattleRecord, StarsFile, Universe};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

fn load(path: &Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    let (mut state, _) = GameState::from_file(&file);
    let universe = Universe::decode(&std::fs::read(path.with_file_name("Game.xy")).ok()?).ok()?;
    state.apply_universe(&universe);
    Some(state)
}

/// The battles the players' files of one year record, one copy each.
fn recorded(dir: &Path) -> Vec<BattleRecord> {
    let mut out: Vec<BattleRecord> = Vec::new();
    for n in 1..=16 {
        let Ok(bytes) = std::fs::read(dir.join(format!("Game.m{n}"))) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let h = &file.latest_segment().header;
        let layout = ActionLayout::for_version(h.version_major, h.version_minor);
        for record in battle_records_in_with(file.segment_blocks(file.latest_segment()), layout) {
            if !out
                .iter()
                .any(|b| b.id == record.id && b.position == record.position)
            {
                out.push(record);
            }
        }
    }
    out
}

/// What a token is made of, as both sides state it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Make {
    player: u8,
    design: u8,
    ships: u16,
    mass: u16,
    shields: u16,
    initiative: u8,
    speed: u8,
    cloak: u8,
}

fn makes(record: &BattleRecord) -> Vec<Make> {
    let mut out: Vec<Make> = record
        .tokens
        .iter()
        .map(|t| Make {
            player: t.player,
            design: t.design,
            ships: t.ships,
            mass: t.mass,
            shields: t.shields,
            initiative: t.initiative_base,
            speed: t.speed(),
            cloak: t.pct_cloak,
        })
        .collect();
    out.sort();
    out
}

#[test]
fn the_battles_fought_are_the_ones_recorded() {
    let root = workspace_root().join("fixtures/games/all-computer-players");
    if !root.is_dir() {
        eprintln!("skipping: no sixteen-player fixtures");
        return;
    }
    let mut recorded_total = 0usize;
    let mut placed = 0usize; // a battle of ours at the same place
    let mut same_sides = 0usize; // with the same players
    let mut same_tokens = 0usize; // with the same tokens, ships for ships
    let mut same_makes = 0usize; // and every token built the same
    let mut token_total = 0usize;
    let mut token_same = 0usize;
    let mut mismatches: BTreeMap<&str, usize> = BTreeMap::new();

    for year in 2400..2500 {
        let Some(mut state) = load(&root.join(format!("{year}/Game.hst"))) else {
            break;
        };
        let next = root.join(format!("{}", year + 1));
        if !next.is_dir() {
            break;
        }
        let theirs = recorded(&next);
        if theirs.is_empty() {
            continue;
        }
        let mut rng = stars_core::rng::Rng::randomize(state.seed);
        let _ = generate_turn(&mut state, &mut rng);
        let ours = &state.battles;

        for record in &theirs {
            recorded_total += 1;
            let Some(mine) = ours.iter().find(|b| b.position == record.position) else {
                continue;
            };
            placed += 1;
            if mine.player_mask != record.player_mask {
                continue;
            }
            same_sides += 1;
            let want = makes(record);
            let have = makes(mine);
            let by_ships = |m: &[Make]| -> Vec<(u8, u8, u16)> {
                m.iter().map(|t| (t.player, t.design, t.ships)).collect()
            };
            if by_ships(&want) != by_ships(&have) {
                continue;
            }
            same_tokens += 1;
            token_total += want.len();
            let mut all = true;
            for (w, h) in want.iter().zip(have.iter()) {
                if w == h {
                    token_same += 1;
                } else {
                    all = false;
                    for (name, a, b) in [
                        ("mass", u32::from(w.mass), u32::from(h.mass)),
                        ("shields", u32::from(w.shields), u32::from(h.shields)),
                        (
                            "initiative",
                            u32::from(w.initiative),
                            u32::from(h.initiative),
                        ),
                        ("speed", u32::from(w.speed), u32::from(h.speed)),
                        ("cloak", u32::from(w.cloak), u32::from(h.cloak)),
                    ] {
                        if a != b {
                            *mismatches.entry(name).or_default() += 1;
                        }
                    }
                }
            }
            if all {
                same_makes += 1;
            }
        }
    }

    eprintln!(
        "recorded {recorded_total}: placed {placed}, same sides {same_sides}, same tokens {same_tokens}, same makes {same_makes}; tokens {token_same}/{token_total} alike; mismatches {mismatches:?}"
    );
    assert!(recorded_total > 0, "the fixtures record battles");

    // The computer players' own turns are this project's transcription and
    // send their fleets along paths of their own, so most recorded battles
    // are never joined; what matters is that when the engine fights where
    // the original fought, it is between the same sides — and that the
    // tokens it builds from the fleets are the ones the original built.
    // Measured: 343 of 1,514 placed, 306 between the same sides, 140 with
    // the same ships, and 371 of their 377 tokens alike to the kiloton, the
    // six apart carrying cargo the year's diverging orders left different.
    assert!(placed >= 250, "placed {placed} of {recorded_total}");
    assert!(
        same_sides * 100 >= placed * 85,
        "same sides {same_sides} of {placed}"
    );
    assert!(
        token_same * 100 >= token_total * 95,
        "tokens {token_same}/{token_total}: {mismatches:?}"
    );
}
