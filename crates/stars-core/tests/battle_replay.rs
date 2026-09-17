//! The combat engine checked against real battle recordings.
//!
//! `stars-formats` decodes the 47 battles the Exodus game recorded. Those
//! recordings contain both the starting state of every token and the actions
//! that followed, so the parts of combat implemented so far — where tokens
//! start, and how far they may move each round — can be checked against what
//! the original engine actually did rather than against our own expectations.
//!
//! Fixtures are optional: each test skips when the sample game is absent.

use std::path::{Path, PathBuf};

use stars_core::battle::{distance, movement_this_round, start_square, Square};
use stars_formats::{battle_records_in_with, ActionLayout, BattleRecord, StarsFile};

/// The action layout a file's version calls for.
fn layout_of(file: &StarsFile) -> ActionLayout {
    let h = &file.latest_segment().header;
    ActionLayout::for_version(h.version_major, h.version_minor)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// Every battle the fixtures record, from both games.
///
/// The sixteen-AI game keeps its recordings in the **player** files: a host
/// file carries no battle blocks at all, but each participant's `.mN` does, so
/// a two-sided battle appears twice. Duplicates are dropped by battle id and
/// position.
fn all_battles() -> Vec<(i32, BattleRecord)> {
    let mut out = exodus_battles();
    let mut seen: std::collections::HashSet<(u16, (i16, i16), i32)> =
        out.iter().map(|(y, b)| (b.id, b.position, *y)).collect();

    let ai_game = workspace_root().join("fixtures/games/all-computer-players");
    if ai_game.is_dir() {
        let mut years: Vec<i32> = std::fs::read_dir(&ai_game)
            .expect("readable fixture dir")
            .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
            .collect();
        years.sort_unstable();
        for year in years {
            for n in 1..=16 {
                let path = ai_game.join(year.to_string()).join(format!("Game.m{n}"));
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                let Ok(file) = StarsFile::decode(&bytes) else {
                    continue;
                };
                for record in battle_records_in_with(
                    file.segment_blocks(file.latest_segment()),
                    layout_of(&file),
                ) {
                    if seen.insert((record.id, record.position, year)) {
                        out.push((year, record));
                    }
                }
            }
        }
    }
    out
}

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
        for record in
            battle_records_in_with(file.segment_blocks(file.latest_segment()), layout_of(&file))
        {
            out.push((year, record));
        }
    }
    out
}

#[test]
fn tokens_start_on_the_squares_the_table_says() {
    // Exodus (2.81) only. The 2.66 game's three-player battles do not fit the
    // 2.7j table this crate transcribes — 2429 battle 0x0c02 puts its sides at
    // (3,1), (1,8) and (8,6) where the table's three-player row is (4,1),
    // (8,8), (1,8). Two-player battles agree across both versions. See
    // docs/formats/battle.md.
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }

    // Every participant is assigned a starting square by player count. All of
    // these battles are two-sided, so the two sides must be at (1,4) and (8,5).
    let mut checked = 0;
    for (year, b) in &battles {
        let participants = b.participants();
        let expected: Vec<Square> = (0..participants.len())
            .filter_map(|side| start_square(b.players, u8::try_from(side).ok()?))
            .collect();
        assert_eq!(
            expected.len(),
            participants.len(),
            "{year} battle {:#06x}: no start layout for {} players",
            b.id,
            b.players
        );

        for token in &b.tokens {
            let square = Square::new(token.square.x, token.square.y);
            assert!(
                expected.contains(&square),
                "{year} battle {:#06x}: token of player {} starts at ({},{}), \
                 not one of the {} starting squares {expected:?}",
                b.id,
                token.player,
                square.x,
                square.y,
                b.players
            );
            checked += 1;
        }

        // Each player must be on exactly one starting square, and different
        // players on different ones.
        for player in &participants {
            let squares: Vec<Square> = b
                .tokens
                .iter()
                .filter(|t| t.player == *player)
                .map(|t| Square::new(t.square.x, t.square.y))
                .collect();
            if let Some(first) = squares.first() {
                assert!(
                    squares.iter().all(|s| s == first),
                    "{year} battle {:#06x}: player {player} starts spread over {squares:?}",
                    b.id
                );
            }
        }
    }
    eprintln!("{checked} token starting positions matched the table");
    assert!(checked > 0);
}

#[test]
fn two_sided_battles_use_the_two_player_layout() {
    let battles = all_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    // The table's two-player layout, straight from the binary.
    assert_eq!(start_square(2, 0), Some(Square::new(1, 4)));
    assert_eq!(start_square(2, 1), Some(Square::new(8, 5)));

    for (year, b) in battles.iter().filter(|(_, b)| b.players == 2) {
        let mut squares: Vec<Square> = b
            .tokens
            .iter()
            .map(|t| Square::new(t.square.x, t.square.y))
            .collect();
        squares.sort_by_key(|s| (s.x, s.y));
        squares.dedup();
        assert!(
            squares.len() <= 2,
            "{year} battle {:#06x}: {} distinct starting squares",
            b.id,
            squares.len()
        );
        for s in squares {
            assert!(
                s == Square::new(1, 4) || s == Square::new(8, 5),
                "{year} battle {:#06x}: unexpected starting square ({},{})",
                b.id,
                s.x,
                s.y
            );
        }
    }
}

#[test]
fn recorded_moves_never_exceed_the_movement_allowance() {
    let battles = all_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }

    // Actions are recorded in order, so a token's successive destinations trace
    // its path. Within one round a token may be moved several times (the
    // original moves everything a square at a time across three phases), so the
    // check is that the whole round's travel fits the allowance for its speed.
    let mut checked = 0;
    let mut moves = 0;
    let mut departed = 0;
    for (year, b) in &battles {
        // Where each token currently is, and how far it has gone this round.
        let mut position: Vec<Square> = b
            .tokens
            .iter()
            .map(|t| Square::new(t.square.x, t.square.y))
            .collect();
        let mut travelled = vec![0u32; b.tokens.len()];
        let mut round = 0u8;

        for action in &b.actions {
            if action.round != round {
                round = action.round;
                travelled.iter_mut().for_each(|t| *t = 0);
            }
            let index = usize::from(action.token);
            let Some(from) = position.get(index).copied() else {
                continue;
            };
            // A token that disengages has no destination square; it is simply
            // gone, and nothing after this concerns it.
            let Some(dest) = action.destination else {
                departed += 1;
                continue;
            };
            let to = Square::new(dest.x, dest.y);
            let step = u32::from(distance(from, to));
            if step > 0 {
                moves += 1;
            }
            travelled[index] += step;
            position[index] = to;

            let speed = b.tokens[index].speed();
            let allowance = u32::from(movement_this_round(speed, action.round));
            assert!(
                travelled[index] <= allowance,
                "{year} battle {:#06x}: token {} (speed {speed}) travelled {} squares \
                 in round {}, allowance {allowance}",
                b.id,
                action.token,
                travelled[index],
                action.round
            );
            assert!(
                to.on_board(),
                "{year} battle {:#06x}: moved off the board",
                b.id
            );
            checked += 1;
        }
    }
    eprintln!(
        "{checked} recorded actions checked, {moves} of them actual moves, \
         {departed} tokens left the battle"
    );
    assert!(
        moves > 100,
        "expected a decent sample of moves, got {moves}"
    );
    assert!(departed > 0, "some token should have disengaged");
}

#[test]
fn firing_happens_within_the_recorded_range() {
    let battles = all_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    // A firing record carries the range it fired at. The longest-reaching weapon
    // in the component table is the Jihad Missile and its three larger cousins,
    // at range 5; beams reach 3 and torpedoes 4. Exodus never fires beyond 4
    // because it never researches missiles, which is why an earlier revision of
    // this test asserted 4 and the sixteen-AI game broke it.
    let mut shots = 0;
    for (year, b) in &battles {
        for action in b.actions.iter().filter(|a| !a.kills.is_empty()) {
            assert!(
                action.range <= 5,
                "{year} battle {:#06x}: fired at range {}",
                b.id,
                action.range
            );
            shots += 1;
        }
    }
    eprintln!("{shots} firing actions checked");
    assert!(shots > 0);
}

/// Damage resolution checked against the recorded kills.
///
/// A kill record says how many shield points an attack stripped and how many
/// ships it destroyed. Replaying that exactly would need the full firing order
/// and the torpedo rolls, but the **first** damage a token takes in a battle
/// is checkable on its own: at that moment the token is at full shields and
/// undamaged, which the recording states, so the outcome follows from the
/// attacker's design, the recorded range, and the damage rules.
///
/// Only beam-armed attackers are used, because torpedo hits are rolled.
#[test]
fn first_beam_hits_reproduce_the_recorded_damage() {
    use std::collections::{BTreeMap, BTreeSet};

    use stars_core::battle::{apply_damage, beam_damage, Damage, TokenState};
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_formats::{design_records, DesignRecord};

    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let to_design = |r: &DesignRecord| ShipDesign {
        name: String::new(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: i16::from(r.hull_id),
        slots: r
            .slots
            .iter()
            .map(|s| DesignSlot {
                category: s.category,
                item: s.item_id,
                count: s.count,
            })
            .collect(),
    };

    let mut checked = 0;
    let mut matched = 0;
    let mut notes = Vec::new();

    for year in years {
        let path = games.join(year.to_string()).join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let blocks = file.segment_blocks(file.latest_segment());
        let Ok(records) = design_records(&file) else {
            continue;
        };
        let by_slot: BTreeMap<u8, ShipDesign> = records
            .iter()
            .filter(|r| r.full_design && !r.starbase)
            .map(|r| (r.design_number, to_design(r)))
            .collect();

        for battle in battle_records_in_with(blocks, layout_of(&file)) {
            let mut already_hit: BTreeSet<u8> = BTreeSet::new();

            for action in &battle.actions {
                for kill in &action.kills {
                    let first = already_hit.insert(kill.token);
                    if !first {
                        continue;
                    }

                    let (Some(attacker), Some(target)) = (
                        battle.tokens.get(usize::from(action.token)),
                        battle.tokens.get(usize::from(kill.token)),
                    ) else {
                        continue;
                    };
                    // Both sides must be designs this file carries in full.
                    if attacker.is_starbase() || target.is_starbase() {
                        continue;
                    }
                    let (Some(attack_design), Some(target_design)) =
                        (by_slot.get(&attacker.design), by_slot.get(&target.design))
                    else {
                        continue;
                    };

                    let weapons = attack_design.weapons();
                    if weapons.is_empty() || weapons.iter().any(|w| w.torpedo) {
                        continue; // torpedo rolls are not reproducible here
                    }
                    let Some(armor) = target_design.armor(false) else {
                        continue;
                    };

                    let dp: i32 = weapons
                        .iter()
                        .map(|w| {
                            beam_damage(
                                *w,
                                i32::from(attacker.ships),
                                i32::from(action.range),
                                i32::from(attacker.pct_capacitor),
                                i32::from(target.pct_beam_defence),
                            )
                        })
                        .sum();

                    let state = TokenState {
                        ships: i32::from(target.ships),
                        // dpShield is per ship; the pool is that times the count.
                        shields: i32::from(target.shields),
                        armor,
                        damage: Damage::default(),
                    };
                    let result = apply_damage(state, dp, false);

                    checked += 1;
                    let want = (i32::from(kill.shield_damage), i32::from(kill.ships_killed));
                    let got = (result.shield_damage, result.ships_killed);
                    if got == want {
                        matched += 1;
                    } else if notes.len() < 8 {
                        notes.push(format!(
                            "{year} battle {:#06x}: token {} hit token {} at range {} for \
                             {dp} dp; computed {got:?}, recorded {want:?}",
                            battle.id, action.token, kill.token, action.range
                        ));
                    }
                }
            }
        }
    }

    eprintln!("first beam hits: {matched} of {checked} reproduce the recorded damage");
    for n in &notes {
        eprintln!("  {n}");
    }
    assert!(checked > 0, "no checkable first beam hit found");
    // 22 of 31 at the time this was written. The rest are cases where the
    // "first hit" assumption does not actually hold — a token can be damaged
    // by an attack that recorded no kill entry against it — or where a
    // modifier this crate does not model yet applies. A floor rather than a
    // fixed count, so the test catches a regression without pretending the
    // remainder is understood.
    assert!(
        matched * 10 >= checked * 6,
        "only {matched} of {checked} first beam hits reproduce; that is below the \
         71% this stood at when written"
    );
}

/// Replay whole battles: take the recorded movement as given, compute the
/// firing, and compare against what the engine recorded.
///
/// This is a stronger check than the first-hit one above, because it carries
/// each token's state forward through the battle instead of assuming it. Only
/// the movement is taken from the recording; every shot, target choice and
/// casualty is computed.
///
/// Battles involving torpedoes or starbases are skipped: torpedo hits are
/// rolled per shot, and a starbase's design lives in a table this file does
/// not carry.
#[test]
fn beam_only_battles_replay_to_the_recorded_casualties() {
    use std::collections::BTreeMap;

    use stars_core::battle::{
        fire_round, CombatToken, Damage, Square as CoreSquare, Tactic, TokenState,
    };
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_formats::{design_records, DesignRecord};

    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let to_design = |r: &DesignRecord| ShipDesign {
        name: String::new(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: i16::from(r.hull_id),
        slots: r
            .slots
            .iter()
            .map(|s| DesignSlot {
                category: s.category,
                item: s.item_id,
                count: s.count,
            })
            .collect(),
    };

    let mut replayed = 0;
    let mut exact = 0;
    let mut notes = Vec::new();

    for year in years {
        let path = games.join(year.to_string()).join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let blocks = file.segment_blocks(file.latest_segment());
        let Ok(records) = design_records(&file) else {
            continue;
        };
        let by_slot: BTreeMap<u8, ShipDesign> = records
            .iter()
            .filter(|r| r.full_design && !r.starbase)
            .map(|r| (r.design_number, to_design(r)))
            .collect();

        for battle in battle_records_in_with(blocks, layout_of(&file)) {
            // Every token must be a ship design we hold in full, and nothing
            // may carry a torpedo.
            let mut tokens = Vec::new();
            let mut usable = true;
            for t in &battle.tokens {
                let Some(design) = (if t.is_starbase() {
                    None
                } else {
                    by_slot.get(&t.design)
                }) else {
                    usable = false;
                    break;
                };
                // Only the file owner's designs are in the file, so a token
                // belonging to anyone else is matched against the wrong design
                // number. The recording carries the one thing that settles
                // whether it can shoot at all: `initMin` is 0xFF exactly when
                // the token has no weapons, and the firing loop gates on
                // `initMin <= init <= initMac`.
                let weapons = if t.initiative_min == 0xFF {
                    Vec::new()
                } else {
                    design.weapons()
                };
                if weapons.iter().any(|w| w.torpedo) {
                    usable = false;
                    break;
                }
                let Some(armor) = design.armor(false) else {
                    usable = false;
                    break;
                };
                tokens.push(CombatToken {
                    tactic: Tactic::from_raw(t.tactic()).unwrap_or(Tactic::MaximiseDamage),
                    speed_index: t.speed(),
                    moves_left: t.moves_left(),
                    class: stars_core::battle::TargetClass::from_raw(t.target_class()),
                    primary_target: stars_core::battle::TargetClass::from_raw(t.primary_target()),
                    secondary_target: stars_core::battle::TargetClass::from_raw(
                        t.secondary_target(),
                    ),
                    is_starbase: t.is_starbase(),
                    pct_jam: i32::from(t.pct_jam),
                    pct_computer: i32::from(t.pct_computer),
                    weapon_reach: if t.initiative_min == 0xFF {
                        0
                    } else {
                        design.weapons().iter().map(|w| w.range).max().unwrap_or(0)
                    },
                    player: t.player,
                    enemies: 0xffff,
                    active: true,
                    square: CoreSquare::new(t.square.x, t.square.y),
                    initiative_base: i32::from(t.initiative_base),
                    capacitor_pct: i32::from(t.pct_capacitor),
                    beam_deflection_pct: i32::from(t.pct_beam_defence),
                    weapons,
                    value: design.cost().map_or(0, |c| c.resources + c.minerals[1]),
                    mass: design.mass().unwrap_or(0) * i32::from(t.ships),
                    jitter: 7,
                    state: TokenState {
                        ships: i32::from(t.ships),
                        shields: i32::from(t.shields),
                        armor,
                        damage: Damage::default(),
                    },
                });
            }
            if !usable || tokens.len() < 2 {
                continue;
            }
            // Only battles that actually did something are informative.
            if battle.ships_destroyed() == 0 {
                continue;
            }

            // Walk the rounds, moving tokens where the recording says and
            // firing at the end of each.
            let mut killed = vec![0i32; tokens.len()];
            let mut round = 0u8;
            let mut fired_any = false;
            for action in battle.actions.iter().chain(std::iter::once(
                // A sentinel so the last round still fires.
                &stars_formats::BattleAction {
                    token: 0,
                    destination: None,
                    round: u8::MAX,
                    range: 0,
                    target: 0,
                    kills: Vec::new(),
                },
            )) {
                if action.round != round {
                    for event in fire_round(&mut tokens) {
                        killed[event.target] += event.ships_killed;
                        fired_any = true;
                    }
                    round = action.round;
                }
                match action.destination {
                    Some(dest) => {
                        if let Some(t) = tokens.get_mut(usize::from(action.token)) {
                            t.square = CoreSquare::new(dest.x, dest.y);
                        }
                    }
                    // `brcDest` of 0xFF is not a square: it is the record of a
                    // token leaving the battle. A Disengage token counts down
                    // `dzDis` as it moves and, when that reaches zero, the
                    // movement loop sets `fActive = 0` and writes the sentinel.
                    // Ignoring it leaves fleeing ships on the board to be shot
                    // at for the remaining rounds.
                    None => {
                        if let Some(t) = tokens.get_mut(usize::from(action.token)) {
                            t.active = false;
                        }
                    }
                }
            }
            if !fired_any {
                continue;
            }

            // Compare per-player casualties, the figure the game itself
            // reports after a battle.
            let mut want: BTreeMap<u8, u32> = BTreeMap::new();
            let mut got: BTreeMap<u8, u32> = BTreeMap::new();
            for player in battle.participants() {
                want.insert(player, battle.ships_destroyed_for(player));
                let mine: i32 = killed
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| battle.tokens[*i].player == player)
                    .map(|(_, k)| *k)
                    .sum();
                got.insert(player, u32::try_from(mine).unwrap_or(0));
            }

            replayed += 1;
            if want == got {
                exact += 1;
            } else if notes.len() < 6 {
                notes.push(format!(
                    "{year} battle {:#06x}: computed losses {got:?}, recorded {want:?}",
                    battle.id
                ));
            }
        }
    }

    eprintln!("beam-only replays: {exact} of {replayed} match the recorded casualties exactly");
    for n in &notes {
        eprintln!("  {n}");
    }
    assert!(replayed > 0, "no beam-only battle was replayable");
    // 8 of 11 when written. The three that differ are symmetric duels — two
    // identical ships, same initiative — where the outcome turns on exactly
    // when in the round each closed to range, which this implementation
    // approximates by firing once at the end of the round.
    assert!(
        exact * 10 >= replayed * 6,
        "only {exact} of {replayed} beam-only battles replay exactly; that is below the \
         8 of 11 this stood at when written"
    );
}

/// How often the movement scorer rates the square the engine actually chose
/// as one of the best available.
///
/// The engine picks a lowest-scoring square and breaks ties with `Random`, so
/// an exact match is not reproducible without the generator's state. What *is*
/// checkable is weaker but still meaningful: if the scoring were right, the
/// square the engine moved to should be among those our scorer rates best.
///
/// The control is the point of this test: the hit rate alone means nothing
/// without knowing how selective "among the best" is.
///
/// A token's move takes one of two paths, and both are checked here.
///
/// When nothing of the right class is within reach, `DzMoveRangeToConsider`
/// stops the token scoring squares at all and sends it at the nearest enemy it
/// could hurt. **105 of 111 such moves close on that target.**
///
/// Otherwise the token scores the squares within its remaining movement. The
/// engine picks a lowest-scoring square and breaks ties with `Random`, so an
/// exact match is not reproducible; what is checkable is that its choice is
/// among the squares we rate best. **379 of 450, against a 72% chance rate**
/// — the chance rate being what makes the hit rate mean anything at all.
///
/// That hit rate went *down* (from 92%) when `damage_estimate` was transcribed
/// from the disassembly, while the chance rate fell too (from 81%), leaving the
/// gap unchanged. The faithful version was kept anyway: the metric is a proxy
/// and the binary is the specification. Something in the movement path is still
/// wrong, and this records that rather than choosing whichever version happens
/// to score better.
#[test]
fn movement_scoring_rates_the_engines_choice_among_the_best() {
    use std::collections::BTreeMap;

    use stars_core::battle::{
        score_square, CombatToken, Damage, Square as CoreSquare, Tactic, TokenState,
    };
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_formats::{design_records, DesignRecord};

    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let to_design = |r: &DesignRecord| ShipDesign {
        name: String::new(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: i16::from(r.hull_id),
        slots: r
            .slots
            .iter()
            .map(|s| DesignSlot {
                category: s.category,
                item: s.item_id,
                count: s.count,
            })
            .collect(),
    };

    let mut moves = 0usize;
    let mut among_best = 0usize;
    let mut beelines = 0usize;
    let mut beelines_toward = 0usize;
    let mut candidates = 0usize;
    let mut best_set = 0usize;

    for year in years {
        let path = games.join(year.to_string()).join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let blocks = file.segment_blocks(file.latest_segment());
        let Ok(records) = design_records(&file) else {
            continue;
        };
        let by_slot: BTreeMap<u8, ShipDesign> = records
            .iter()
            .filter(|r| r.full_design && !r.starbase)
            .map(|r| (r.design_number, to_design(r)))
            .collect();

        for battle in battle_records_in_with(blocks, layout_of(&file)) {
            let mut tokens = Vec::new();
            let mut usable = true;
            for t in &battle.tokens {
                let Some(design) = (if t.is_starbase() {
                    None
                } else {
                    by_slot.get(&t.design)
                }) else {
                    usable = false;
                    break;
                };
                let Some(armor) = design.armor(false) else {
                    usable = false;
                    break;
                };
                tokens.push(CombatToken {
                    tactic: Tactic::from_raw(t.tactic()).unwrap_or(Tactic::MaximiseDamage),
                    speed_index: t.speed(),
                    moves_left: t.moves_left(),
                    class: stars_core::battle::TargetClass::from_raw(t.target_class()),
                    primary_target: stars_core::battle::TargetClass::from_raw(t.primary_target()),
                    secondary_target: stars_core::battle::TargetClass::from_raw(
                        t.secondary_target(),
                    ),
                    is_starbase: t.is_starbase(),
                    pct_jam: i32::from(t.pct_jam),
                    pct_computer: i32::from(t.pct_computer),
                    weapon_reach: if t.initiative_min == 0xFF {
                        0
                    } else {
                        design.weapons().iter().map(|w| w.range).max().unwrap_or(0)
                    },
                    player: t.player,
                    enemies: 0xffff,
                    active: true,
                    square: CoreSquare::new(t.square.x, t.square.y),
                    initiative_base: i32::from(t.initiative_base),
                    capacitor_pct: i32::from(t.pct_capacitor),
                    beam_deflection_pct: i32::from(t.pct_beam_defence),
                    // Only the file owner's designs are in the file, so a
                    // token belonging to anyone else is matched against the
                    // wrong design number. The recording carries the one thing
                    // that settles whether it can shoot at all: `initMin` is
                    // 0xFF exactly when the token has no weapons, and the
                    // firing loop gates on `initMin <= init <= initMac`.
                    weapons: if t.initiative_min == 0xFF {
                        Vec::new()
                    } else {
                        design.weapons()
                    },
                    value: design.cost().map_or(0, |c| c.resources + c.minerals[1]),
                    mass: design.mass().unwrap_or(0) * i32::from(t.ships),
                    jitter: 7,
                    state: TokenState {
                        ships: i32::from(t.ships),
                        shields: i32::from(t.shields),
                        armor,
                        damage: Damage::default(),
                    },
                });
            }
            if !usable {
                continue;
            }

            // `moves_left` is what a token has left to spend *this round*, and
            // the scorer compares the mover's against each enemy's to decide
            // whether that enemy can close the distance. Read once from the
            // token record it would be the starting allowance forever, so it is
            // reset at the head of every round and spent as tokens move.
            let mut round = u8::MAX;
            for action in &battle.actions {
                if action.round != round {
                    round = action.round;
                    for token in &mut tokens {
                        token.moves_left = movement_this_round(token.speed_index, round);
                    }
                }
                // Keep the board current. Scoring a square weighs what each
                // token could give and take, which depends on how many ships it
                // still has and what armour is left on them — so the casualties
                // the recording carries have to be applied as they happen, or
                // every round after the first is scored against a battle that is
                // no longer being fought.
                let apply_kills = |tokens: &mut Vec<CombatToken>| {
                    for kill in &action.kills {
                        let Some(hit) = tokens.get_mut(usize::from(kill.token)) else {
                            continue;
                        };
                        // `shields` is per ship and the recording's figure is
                        // the whole pool, so it has to go through the pool the
                        // way `apply_damage` does — and, as there, the per-ship
                        // value is recomputed against the ship count *before*
                        // the casualties, not after.
                        let pool = hit.state.shields * hit.state.ships;
                        let left = (pool - i32::from(kill.shield_damage)).max(0);
                        if hit.state.ships > 0 {
                            hit.state.shields = left / hit.state.ships;
                        }
                        hit.state.ships = (hit.state.ships - i32::from(kill.ships_killed)).max(0);
                        hit.state.damage = Damage::from_raw(kill.damage);
                        if hit.state.ships == 0 {
                            // `alive()` already gates on the ship count; this
                            // only zeroes the pool so the estimate cannot read
                            // shields off a stack that no longer exists.
                            hit.state.shields = 0;
                        }
                    }
                };

                let Some(dest) = action.destination else {
                    apply_kills(&mut tokens);
                    continue;
                };
                let mover = usize::from(action.token);
                let Some(token) = tokens.get(mover) else {
                    apply_kills(&mut tokens);
                    continue;
                };
                let here = token.square;
                let to = CoreSquare::new(dest.x, dest.y);
                if to == here {
                    apply_kills(&mut tokens); // a firing record, not a move
                    continue;
                }

                // What the engine would have done: if nothing is in reach
                // it heads for the nearest enemy rather than scoring squares.
                let search = stars_core::battle::move_search(&tokens, mover, true);

                if let Some(target) = search.beeline {
                    // A beeline: the move must reduce the distance to that
                    // enemy, which is what stepping toward it means.
                    beelines += 1;
                    let before = stars_core::battle::distance(here, target);
                    let after = stars_core::battle::distance(to, target);
                    if after < before || (after == before && before <= 1) {
                        beelines_toward += 1;
                    }
                    tokens[mover].square = to;
                    tokens[mover].moves_left = tokens[mover].moves_left.saturating_sub(1);
                    apply_kills(&mut tokens);
                    continue;
                }

                let allowance = search.radius.max(1)
                    + std::env::var("RADIUS_PLUS").map_or(0, |v| v.parse().unwrap_or(0));
                let mut best = i32::MAX;
                let mut best_squares = Vec::new();
                let mut candidate_count = 0;
                for x in 0..10u8 {
                    for y in 0..10u8 {
                        let square = CoreSquare::new(x, y);
                        if i32::from(stars_core::battle::distance(here, square)) > allowance {
                            continue;
                        }
                        candidate_count += 1;
                        let score = score_square(&tokens, mover, square);
                        if score < best {
                            best = score;
                            best_squares.clear();
                        }
                        if score == best {
                            best_squares.push(square);
                        }
                    }
                }

                moves += 1;
                if best_squares.contains(&to) {
                    among_best += 1;
                }
                // The control: how selective is "among the best"? If our
                // scorer rated most candidates equally, a high hit rate would
                // mean nothing.
                candidates += candidate_count;
                best_set += best_squares.len();
                tokens[mover].square = to;
                tokens[mover].moves_left = tokens[mover].moves_left.saturating_sub(1);
                apply_kills(&mut tokens);
            }
        }
    }

    let pct = (among_best * 100).checked_div(moves).unwrap_or(0);
    // If the scorer rated every square equally, "among the best" would be
    // vacuous, so the chance rate says how much the hit rate is worth.
    let chance = (best_set * 100).checked_div(candidates).unwrap_or(0);
    let beeline_pct = (beelines_toward * 100).checked_div(beelines).unwrap_or(0);
    eprintln!(
        "movement: {beelines_toward} of {beelines} beeline moves close on the target \
         ({beeline_pct}%); of the {moves} scored moves the engine's square was among \
         our best-rated {among_best} times ({pct}%), against a {chance}% chance rate"
    );
    assert!(
        moves > 100,
        "expected a decent sample of moves, got {moves}"
    );
    // Measured: 94% against a 72% chance rate, and every beeline closes. Both
    // are asserted against regression.
    assert!(
        pct >= 90,
        "movement scoring agreed with the engine on only {pct}% of scored moves, below \
         the 94% measured"
    );
    assert!(
        beeline_pct >= 95,
        "only {beeline_pct}% of beeline moves close on their target, below the 100% \
         measured"
    );
    assert!(
        chance <= 78,
        "the best set has grown to {chance}% of candidates, so the hit rate means less \
         than it did when this was written"
    );
    assert!(
        pct >= chance + 18,
        "the scorer ({pct}%) is no longer meaningfully better than chance ({chance}%)"
    );
}

/// The three-phase movement order, checked against every recorded round.
///
/// Each round sets `dMovesLeft = DxyFromSpdRound(spd, iRound)` and then runs
/// `for (j = 3; j > 0; j--)`, moving a token only when `j <= dMovesLeft`. Two
/// consequences are testable without the RNG:
///
/// * a token never moves more times in a round than that allowance;
/// * the round's moves must fit into three descending phases, each token taking
///   at most one move per phase.
///
/// The second has to be tested as **feasibility**, not by assuming a token's
/// m-th recorded move is its m-th phase. A token that chooses to stay put has
/// its record removed (`lpbBattleCur -= 6`), so a move that looks like its first
/// may belong to a later phase. Assuming otherwise reports five false failures.
#[test]
fn recorded_moves_fit_the_three_movement_phases() {
    use std::collections::BTreeMap;

    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let (mut rounds, mut feasible_rounds, mut moves, mut over) = (0usize, 0usize, 0usize, 0usize);

    for year in years {
        let path = games.join(year.to_string()).join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        for battle in
            battle_records_in_with(file.segment_blocks(file.latest_segment()), layout_of(&file))
        {
            // Group real moves by round, keeping the recorded order. A firing
            // record repeats the token's current square, so track positions.
            let mut here: Vec<(u8, u8)> = battle
                .tokens
                .iter()
                .map(|t| (t.square.x, t.square.y))
                .collect();
            let mut by_round: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
            for action in &battle.actions {
                let Some(dest) = action.destination else {
                    continue;
                };
                let Some(current) = here.get_mut(usize::from(action.token)) else {
                    continue;
                };
                if (dest.x, dest.y) == *current {
                    continue;
                }
                *current = (dest.x, dest.y);
                by_round.entry(action.round).or_default().push(action.token);
            }

            for (round, actions) in by_round {
                rounds += 1;
                let mut phase = i32::from(stars_core::battle::MOVEMENT_PHASES);
                let mut last: BTreeMap<u8, i32> = BTreeMap::new();
                let mut count: BTreeMap<u8, i32> = BTreeMap::new();
                let mut feasible = true;
                for token in &actions {
                    let Some(t) = battle.tokens.get(usize::from(*token)) else {
                        continue;
                    };
                    let allowance = i32::from(movement_this_round(t.speed(), round));
                    moves += 1;
                    let n = count.entry(*token).or_insert(0);
                    *n += 1;
                    if *n > allowance {
                        over += 1;
                    }
                    // Greedily take the highest phase still open to this token.
                    phase = phase.min(allowance);
                    if last.get(token) == Some(&phase) {
                        phase -= 1;
                    }
                    if phase < 1 {
                        feasible = false;
                        break;
                    }
                    last.insert(*token, phase);
                }
                if feasible {
                    feasible_rounds += 1;
                }
            }
        }
    }

    assert!(
        moves > 500,
        "expected a decent sample of moves, got {moves}"
    );
    eprintln!("movement phases: {feasible_rounds} of {rounds} rounds fit, over {moves} moves");
    assert_eq!(
        over, 0,
        "{over} moves exceeded the round's movement allowance"
    );
    assert_eq!(
        feasible_rounds,
        rounds,
        "{} rounds could not be fitted into three descending phases",
        rounds - feasible_rounds
    );
}
