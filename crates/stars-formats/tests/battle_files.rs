//! Battle recordings decoded from real Stars! files.
//!
//! The Exodus game recorded 40-odd battles across its 40 turns, which is what
//! makes these tests possible — and what will make a combat implementation
//! checkable rather than merely plausible.
//!
//! Fixtures are optional: each test skips when the sample game is absent.

use std::path::{Path, PathBuf};

use stars_formats::{battle_records_in, BattleRecord, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// Every battle recorded in the Exodus game, with the year it came from.
fn exodus_battles() -> Vec<(i32, BattleRecord)> {
    let games = workspace_root().join("fixtures/games/exodus");
    if !games.is_dir() {
        return Vec::new();
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let mut out = Vec::new();
    for year in years {
        let path = games.join(year.to_string()).join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        // Read only the current turn; a file may still hold an unopened one.
        let blocks = file.segment_blocks(file.latest_segment());
        for record in battle_records_in(blocks) {
            out.push((year, record));
        }
    }
    out
}

#[test]
fn battle_recordings_decode_consistently() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    eprintln!("decoded {} battles from the Exodus game", battles.len());

    for (year, b) in &battles {
        let where_ = format!("{year} battle {:#06x}", b.id);

        // Structure the header itself asserts.
        assert!(!b.tokens.is_empty(), "{where_}: no tokens");
        assert!(b.players >= 2, "{where_}: {} players", b.players);
        assert_eq!(
            usize::from(b.players),
            b.participants().len(),
            "{where_}: cplr disagrees with the player bitmask"
        );

        // Every token must name a plausible player and design.
        for (i, t) in b.tokens.iter().enumerate() {
            assert!(t.player < 16, "{where_}: token {i} player {}", t.player);
            assert!(
                b.player_mask & (1 << t.player) != 0,
                "{where_}: token {i} belongs to player {} which is not in the mask",
                t.player
            );
            assert!(
                t.square.x < 16 && t.square.y < 16,
                "{where_}: token {i} off board"
            );
            assert!(t.ships > 0, "{where_}: token {i} has no ships");
        }

        // Every action must reference real tokens and a legal round.
        for (i, a) in b.actions.iter().enumerate() {
            assert!(
                usize::from(a.token) < b.tokens.len(),
                "{where_}: action {i} acts for token {} of {}",
                a.token,
                b.tokens.len()
            );
            assert!(a.round < 16, "{where_}: action {i} round {}", a.round);
            if let Some(dest) = a.destination {
                assert!(dest.x < 16 && dest.y < 16, "{where_}: action {i} off board");
            }
            for k in &a.kills {
                assert!(
                    usize::from(k.token) < b.tokens.len(),
                    "{where_}: action {i} damages token {} of {}",
                    k.token,
                    b.tokens.len()
                );
            }
        }
    }
}

#[test]
fn recordings_consume_exactly_the_bytes_they_declare() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }

    // The strongest structural check available: a record declares its own
    // length, and walking the tokens and the variable-length action stream must
    // land exactly on it. A wrong token or action size would drift and fail
    // here on almost every battle.
    for (year, b) in &battles {
        let consumed = 14
            + b.tokens.len() * 29
            + b.actions
                .iter()
                .map(|a| 6 + a.kills.len() * 8)
                .sum::<usize>();
        assert_eq!(
            consumed,
            usize::from(b.declared_len),
            "{year} battle {:#06x}: walked {consumed} bytes of a declared {}",
            b.id,
            b.declared_len
        );
    }
}

#[test]
fn battles_have_two_sides_and_do_damage() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }

    let mut with_kills = 0;
    for (_, b) in &battles {
        // Every recorded battle involves at least two distinct players.
        let mut owners: Vec<u8> = b.tokens.iter().map(|t| t.player).collect();
        owners.sort_unstable();
        owners.dedup();
        assert!(
            owners.len() >= 2,
            "battle {:#06x} has tokens from only one player",
            b.id
        );

        if b.ships_destroyed() > 0 {
            with_kills += 1;
            // Losses must add up across the participants.
            let total: u32 = b
                .participants()
                .iter()
                .map(|p| b.ships_destroyed_for(*p))
                .sum();
            assert_eq!(
                total,
                b.ships_destroyed(),
                "battle {:#06x}: per-player losses do not sum to the total",
                b.id
            );
        }
    }
    eprintln!("{with_kills} of {} battles destroyed ships", battles.len());
    assert!(with_kills > 0, "no battle destroyed anything");
}

#[test]
fn the_players_own_fleets_appear_in_their_own_battles() {
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    // These are player 5's files, so player 5 must be in every battle they were
    // sent a recording of.
    for (year, b) in &battles {
        assert!(
            b.participants().contains(&5),
            "{year} battle {:#06x} does not involve the file's own player",
            b.id
        );
    }
}

#[test]
fn the_2424_homeworld_defence_decodes_as_it_played_out() {
    let root = workspace_root();
    let path = root.join("fixtures/games/exodus/2424/exodus.m6");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: {} absent", path.display());
        return;
    };
    let file = StarsFile::decode(&bytes).expect("fixture decodes");
    let battles = battle_records_in(file.segment_blocks(file.latest_segment()));
    assert_eq!(battles.len(), 1, "year 2424 recorded one battle");
    let b = &battles[0];

    // A single attacker from player 0 jumped player 5's home world.
    assert_eq!(b.planet, 225, "planet 225 is player 5's home world");
    assert_eq!(b.participants(), vec![0, 5]);
    assert_eq!(b.position, (1502, 1345));
    assert_eq!(b.tokens.len(), 12);

    // Token 0 is the defending starbase: an object of class 1 (a planet),
    // in a starbase design slot, with shields and the highest initiative.
    let base = &b.tokens[0];
    assert_eq!(base.player, 5);
    assert_eq!(base.object_class, 1, "a planet, not a fleet");
    assert!(
        base.is_starbase(),
        "design slot {} is a starbase",
        base.design
    );
    assert_eq!(base.shields, 1120);
    assert_eq!(base.initiative_base, 14);

    // Every defender starts on the same square, and the attacker across the
    // board from them.
    let attacker = b
        .tokens
        .iter()
        .find(|t| t.player == 0)
        .expect("player 0 brought a token");
    assert_eq!(attacker.ships, 1);
    assert_ne!(
        (attacker.square.x, attacker.square.y),
        (base.square.x, base.square.y),
        "attacker and defender start apart"
    );
    for token in b.tokens.iter().filter(|t| t.player == 5) {
        assert_eq!(
            (token.square.x, token.square.y),
            (base.square.x, base.square.y),
            "defenders share the starbase's square"
        );
    }

    // The attacker was destroyed, and nothing of player 5's was.
    assert_eq!(b.ships_destroyed(), 1);
    assert_eq!(b.ships_destroyed_for(0), 1);
    assert_eq!(b.ships_destroyed_for(5), 0);

    // The killing blow: exactly one action carries damage, and its target is
    // the token the kill names.
    let firing: Vec<_> = b.actions.iter().filter(|a| !a.kills.is_empty()).collect();
    assert_eq!(firing.len(), 1, "one action did damage");
    let shot = firing[0];
    assert_eq!(shot.kills.len(), 1);
    assert_eq!(
        shot.target, shot.kills[0].token,
        "a firing record's target names the token its kill damages"
    );
    assert_eq!(
        b.tokens[usize::from(shot.kills[0].token)].player,
        0,
        "the victim belonged to player 0"
    );
    assert_eq!(shot.kills[0].ships_killed, 1);
}
