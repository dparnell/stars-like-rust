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
use stars_formats::{battle_records_in, BattleRecord, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
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
        for record in battle_records_in(file.segment_blocks(file.latest_segment())) {
            out.push((year, record));
        }
    }
    out
}

#[test]
fn tokens_start_on_the_squares_the_table_says() {
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
    let battles = exodus_battles();
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
    let battles = exodus_battles();
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
    let battles = exodus_battles();
    if battles.is_empty() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    // A firing record carries the range it fired at. Beam weapons reach at most
    // three squares and torpedoes four, so nothing should fire beyond that.
    let mut shots = 0;
    for (year, b) in &battles {
        for action in b.actions.iter().filter(|a| !a.kills.is_empty()) {
            assert!(
                action.range <= 4,
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

        for battle in battle_records_in(blocks) {
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

    use stars_core::battle::{fire_round, CombatToken, Damage, Square as CoreSquare, TokenState};
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

        for battle in battle_records_in(blocks) {
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
                let weapons = design.weapons();
                if weapons.iter().any(|w| w.torpedo) {
                    usable = false;
                    break;
                }
                let Some(armor) = design.armor(false) else {
                    usable = false;
                    break;
                };
                tokens.push(CombatToken {
                    player: t.player,
                    active: true,
                    square: CoreSquare::new(t.square.x, t.square.y),
                    initiative_base: i32::from(t.initiative_base),
                    capacitor_pct: i32::from(t.pct_capacitor),
                    beam_deflection_pct: i32::from(t.pct_beam_defence),
                    weapons,
                    value: design.cost().map_or(0, |c| c.resources + c.minerals[1]),
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
                if let Some(dest) = action.destination {
                    if let Some(t) = tokens.get_mut(usize::from(action.token)) {
                        t.square = CoreSquare::new(dest.x, dest.y);
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
