//! The year's battles — `DoBattles` (`10f0:3a26`), which `DoOrders(1)`
//! (`10b0:179a`) runs after movement and before the arrival tasks.
//!
//! The fighting itself is [`crate::battle`]: the board, the movement
//! search, the firing loops. This module is the frame around it —
//! `LinkFleets` (which fleets share a place), `CplrBattle` (who fights
//! whom, from the battle plans and the relations), `InitializeBoard`
//! (tokens from fleets and the starbase), the round loop of
//! `FDoCoolBattle`, and what the battle leaves behind: the fleets' ship
//! counts and damage (`KillShips`), a dead starbase, salvage
//! (`CreateSalvage`), the messages (`SendBattleMessages`) and the
//! recording every player present gets to watch (`lpbBattleLog`).
//!
//! What is not here yet: `FDumpCargo` (a plan's "dump cargo" flag), the
//! tech learned from wreckage (`ITechLearnATech`), the over-255-token
//! exclusion of `CplrBattle`, and the jitter of the movement order (see
//! [`crate::battle::move_round`]). Nothing is verified against a corpus
//! battle: the recordings in the fixtures cannot be reproduced without the
//! original's generator state (`docs/rng/prng.md`).

use std::collections::BTreeMap;

use stars_formats::battle::{BattleAction, BattleRecord, BattleToken, Kill};

use crate::battle::{self, CombatToken, Damage, Square, Tactic, TargetClass, TokenState};
use crate::fleet::Fleet;
use crate::message::{id, Message};
use crate::movement::Point;
use crate::rng::Rng;
use crate::GameState;

/// How many rounds a battle lasts at most (`for (iRound = 0; iRound < 16;
/// ...)` in `FDoCoolBattle`).
pub const ROUNDS: u8 = 16;
/// The moves a disengaging token makes before it leaves (`dzDis = 7`,
/// `CheckTarget`).
pub const DISENGAGE_MOVES: u8 = 7;
/// The most tokens a battle holds (`vrgtok` is 256 `TOK`s).
pub const MAX_TOKENS: usize = 255;
/// The "attack who" values of a battle plan (`mdAttack`).
pub mod attack_who {
    /// Nobody.
    pub const NOBODY: u8 = 0;
    /// Enemies.
    pub const ENEMIES: u8 = 1;
    /// Neutrals and enemies.
    pub const NEUTRALS_AND_ENEMIES: u8 = 2;
    /// Everyone.
    pub const EVERYONE: u8 = 3;
    /// A named player: the value less this.
    pub const PLAYER_BASE: u8 = 4;
}

/// Who fights whom at one place — `CplrBattle`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encounter {
    /// Where.
    pub position: Point,
    /// The planet there, as an index into [`GameState::planets`].
    pub planet: Option<usize>,
    /// The fleets there, as indices into [`GameState::fleets`].
    pub fleets: Vec<usize>,
    /// For each player, the players they attack (`rggrfAttack`).
    pub attack: [u16; 16],
    /// The players in the battle (`grfPlayer`).
    pub present: u16,
    /// The players who see it happen without being in it — a planet's
    /// owner with no starbase, or a fleet whose plan attacks nobody
    /// (`grfSpectator`).
    pub spectators: u16,
}

impl Encounter {
    /// How many players fight.
    #[must_use]
    pub fn players(&self) -> u8 {
        self.present.count_ones().min(16) as u8
    }
}

/// What one battle did — for the turn report and the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The recording, as every player present receives it.
    pub record: BattleRecord,
    /// Ships lost, as `(player, design slot, ships)`.
    pub losses: Vec<(i16, u8, i32)>,
    /// The planet whose starbase was destroyed, if one was.
    pub starbase_destroyed: Option<i16>,
    /// Salvage left in space, as `(position, minerals)`.
    pub salvage: Vec<(Point, [i32; 3])>,
}

/// The relation a player holds toward another (`PLAYER.rgmdRelation`):
/// `0` neutral, `1` friend, `2` enemy.
fn relation(state: &GameState, player: usize, other: usize) -> u8 {
    state
        .players
        .get(player)
        .and_then(|p| p.relations.get(other))
        .copied()
        .unwrap_or(0)
}

/// The players a plan's "attack who" names, for `player`.
fn attack_mask(state: &GameState, player: usize, who: u8) -> u16 {
    let me = 1u16 << (player & 15);
    match who {
        attack_who::NOBODY => 0,
        attack_who::ENEMIES | attack_who::NEUTRALS_AND_ENEMIES => {
            let mut mask = 0u16;
            for other in 0..state.players.len().min(16) {
                if other == player {
                    continue;
                }
                let r = relation(state, player, other);
                if r == 2 || (r == 0 && who == attack_who::NEUTRALS_AND_ENEMIES) {
                    mask |= 1 << other;
                }
            }
            mask
        }
        attack_who::EVERYONE => !me,
        named => 1u16 << ((named - attack_who::PLAYER_BASE) & 15),
    }
}

/// Whether a design carries a weapon (`FHullHasTeeth`).
fn has_teeth(design: Option<&crate::design::ShipDesign>) -> bool {
    design.is_some_and(crate::design::ShipDesign::is_armed)
}

/// Whether a fleet carries any armed design (`FFleetHasTeeth`).
fn fleet_has_teeth(state: &GameState, fleet: &Fleet) -> bool {
    let Ok(owner) = usize::try_from(fleet.owner) else {
        return false;
    };
    fleet.stacks.iter().any(|s| {
        s.count > 0
            && has_teeth(
                state
                    .designs
                    .get(owner)
                    .and_then(|d| d.get(usize::from(s.design))),
            )
    })
}

/// The battle plan a fleet fights under, or the player's first plan.
fn plan_of(state: &GameState, owner: usize, plan: u8) -> Option<&stars_formats::BattlePlanRecord> {
    let plans = &state.players.get(owner)?.battle_plans;
    plans
        .iter()
        .find(|p| p.plan_id == plan && !p.deleted())
        .or_else(|| plans.first())
}

/// `CplrBattle` (`10f0:2952`): whether the fleets at one place fight, and
/// who attacks whom.
///
/// Each attacker's mask comes from its battle plan's "attack who" —
/// enemies, neutrals and enemies, everyone, or a named player — for a
/// fleet with teeth whose plan has a primary target; a starbase attacks by
/// its owner's first plan. The players attacked by anyone present are
/// **fighting**; anyone attacking a fighter joins them, and a fighter
/// fights back against everyone attacking it. A present player attacked
/// by nobody joins on a **friend's** side, taking that friend's targets,
/// unless one of those targets is a friend too. Everyone else present is
/// a spectator, and so is the owner of a planet with no starbase.
///
/// The exclusion of fleets past 255 tokens is not written.
#[must_use]
pub fn who_fights(state: &GameState, fleets: &[usize], planet: Option<usize>) -> Option<Encounter> {
    let count = state.players.len().min(16);
    let mut attack = [0u16; 16];
    let mut present = 0u16;
    let mut spectators = 0u16;
    let mut anyone_attacks = false;
    let mut starbase_owner: Option<usize> = None;

    if let Some(p) = planet.and_then(|i| state.planets.get(i)) {
        if let Some(owner) = p.owner.and_then(|o| usize::try_from(o).ok()) {
            if p.starbase {
                starbase_owner = Some(owner);
                present |= 1 << owner;
                let design = p.starbase_design.and_then(|s| {
                    state.designs.get(owner).and_then(|d| {
                        d.get(usize::from(crate::startup::FIRST_STARBASE_SLOT) + usize::from(s))
                    })
                });
                if let Some(plan) = state.players[owner].battle_plans.first() {
                    if has_teeth(design) && plan.attack_who != attack_who::NOBODY {
                        attack[owner] |= attack_mask(state, owner, plan.attack_who);
                    }
                }
            } else {
                spectators |= 1 << owner;
            }
        }
    }

    for &index in fleets {
        let fleet = &state.fleets[index];
        if fleet.is_empty() {
            continue;
        }
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        if owner >= 16 {
            continue;
        }
        present |= 1 << owner;
        let Some(plan) = plan_of(state, owner, fleet.battle_plan) else {
            continue;
        };
        if plan.primary_target != 0
            && plan.attack_who != attack_who::NOBODY
            && fleet_has_teeth(state, fleet)
        {
            anyone_attacks = true;
            attack[owner] |= attack_mask(state, owner, plan.attack_who);
        }
    }
    // Only a fleet's plan opens a battle: a starbase's mask counts once
    // one does (`bVar5` is set in the fleet loop alone).
    if !anyone_attacks {
        return None;
    }
    let _ = starbase_owner;

    // Who is attacked by someone present.
    let mut fighting = 0u16;
    for mask in attack.iter().take(count) {
        fighting |= present & mask;
    }
    if fighting == 0 {
        return None;
    }
    // Attackers of fighters fight; fighters fight back.
    for p in 0..count {
        if attack[p] & fighting != 0 {
            fighting |= 1 << p;
        }
        if fighting & (1 << p) != 0 {
            for q in 0..count {
                if attack[q] & (1 << p) != 0 {
                    attack[p] |= 1 << q;
                }
            }
        }
    }
    // The rest present: on a friend's side, or out.
    loop {
        let mut changed = false;
        for p in 0..count {
            let bit = 1u16 << p;
            if present & bit == 0 || fighting & bit != 0 {
                continue;
            }
            attack[p] = 0;
            for q in 0..count {
                if q == p || relation(state, p, q) != 1 || fighting & (1 << q) == 0 {
                    continue;
                }
                if attack[p] & (1 << q) != 0 {
                    attack[p] = 0;
                    break;
                }
                attack[p] |= attack[q];
            }
            if attack[p] == 0 {
                present &= !bit;
            } else {
                fighting |= bit;
            }
            changed = true;
        }
        if !changed {
            break;
        }
    }
    for (p, mask) in attack.iter_mut().enumerate().take(count) {
        if present & (1 << p) == 0 {
            *mask = 0;
        }
    }
    // Fleets of players not in it watch.
    for &index in fleets {
        let fleet = &state.fleets[index];
        if let Ok(owner) = usize::try_from(fleet.owner) {
            if owner < 16 && present & (1 << owner) == 0 {
                spectators |= 1 << owner;
            }
        }
    }
    if fighting.count_ones() < 2 {
        return None;
    }
    let position = fleets
        .first()
        .map(|&i| state.fleets[i].position)
        .or_else(|| planet.and_then(|i| state.planets[i].position))?;
    Some(Encounter {
        position,
        planet,
        fleets: fleets.to_vec(),
        attack,
        present,
        spectators,
    })
}

/// `SpdOfShip` (`10f0:339c`): a design's battle speed, as the stored
/// quarter-square index (`0..=8`).
///
/// The base is the engine's: 10 for the warp-10 engines (the Interspace-10,
/// Enigma Pulsar, Trans-Star 10, Trans-Galactic Mizer Scoop and Galaxy
/// Scoop), else the highest warp at which it burns no more than 120 % —
/// less four; plus one per Maneuvering Jet or Multi Function Pod, two per
/// Overthruster, one per two Enigma Pulsars or Alien Miners (rounded up),
/// and two for a War Monger; less the ship's mass (with its share of the
/// fleet's cargo) over 70, per engine; clamped to `0..=8`. A dampening
/// field takes four off every ship afterwards.
#[must_use]
pub fn battle_speed(
    design: &crate::design::ShipDesign,
    race: &crate::race::Race,
    cargo_share: i32,
) -> u8 {
    battle_speed_of(
        design,
        race.prt() == Some(crate::race::Prt::Wm),
        cargo_share,
    )
}

/// [`battle_speed`] with the War Monger bonus given outright — which
/// `SpdOfShip` leaves out when it is asked about a design with no fleet
/// behind it, as `LComputePower` asks (`1038:0d6e`).
#[must_use]
pub fn battle_speed_of(
    design: &crate::design::ShipDesign,
    war_monger: bool,
    cargo_share: i32,
) -> u8 {
    use crate::components::slot;
    let mut engine: Option<(usize, i32)> = None;
    let mut halves = 0i32;
    let mut whole = 0i32;
    for s in &design.slots {
        if s.count == 0 {
            continue;
        }
        let n = i32::from(s.count);
        let item = usize::from(s.item);
        if s.category == slot::ENGINE {
            engine = Some((item, n));
            if item == 8 {
                halves += n;
            }
        } else if s.category == slot::MINING && item == 6 {
            halves += n;
        } else if s.category == slot::SPECIAL_E && item == 4 {
            whole += n;
        } else if s.category == slot::SPECIAL_M {
            if item == 7 {
                whole += n;
            } else if item == 8 {
                whole += 2 * n;
            }
        }
    }
    let Some((engine, engines)) = engine else {
        return 0;
    };
    let base = if matches!(engine, 7 | 8 | 9 | 14 | 15) {
        10
    } else {
        let fuel = crate::components::ENGINES
            .get(engine)
            .map_or([0i16; 12], |e| e.fuel_used);
        let mut warp = 9;
        while warp > 0 && fuel[warp] > 120 {
            warp -= 1;
        }
        i32::try_from(warp).unwrap_or(0)
    };
    let mut speed = base - 4 + whole + (halves + 1) / 2;
    if war_monger {
        speed += 2;
    }
    let mass = design.mass().unwrap_or(0) + cargo_share.max(0);
    speed -= mass / 70 / engines.max(1);
    u8::try_from(speed.clamp(0, 8)).unwrap_or(0)
}

/// What `CheckWeapons` and `InitFromHuldef` read off a design: the
/// electrical and special parts' percentages, and the hull initiative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fittings {
    /// Torpedo jamming, in percent (`pctJam`): the product of the parts'
    /// `100 - jam`, from the Jammers, the Multi Function Pod, the Langston
    /// Shell, the Mega Poly Shell and the Alien Miner, at most 95.
    pub pct_jam: i32,
    /// Battle-computer accuracy bonus (`pctBC`), from the three computers
    /// and the Multi Contained Munition.
    pub pct_computer: i32,
    /// Capacitor damage bonus (`pctCap`): the product of `100 + bonus`
    /// over the Energy and Flux Capacitors, over ten; `0` for none.
    pub pct_capacitor: i32,
    /// Beam deflection (`pctBeamDef`): the product of `100 - deflection`
    /// over the Beam Deflectors, over ten; `100` for none.
    pub pct_beam_defence: i32,
    /// Hull initiative plus one, two and three per Battle Computer, Super
    /// Computer and Nexus, at most 63.
    pub initiative: i32,
    /// Whether an Energy Dampener is fitted.
    pub dampener: bool,
}

/// Read a design's [`Fittings`].
#[must_use]
pub fn fittings(design: &crate::design::ShipDesign) -> Fittings {
    use crate::components::{slot, ARMORS, SHIELDS, SPECIALS_E, SPECIALS_M};
    let mut jam: i64 = 10_000;
    let mut cap: i64 = 1_000;
    let mut deflect: i64 = 1_000;
    let mut bc: i64 = 0;
    let mut init = i32::from(design.hull().map_or(0, |h| h.initiative)) & 0x3f;
    let mut dampener = false;
    for s in &design.slots {
        if s.count == 0 {
            continue;
        }
        let n = i64::from(s.count);
        let item = usize::from(s.item);
        // A part's share of the jamming product, per copy.
        let mut jam_each: i64 = 100;
        if s.category == slot::SHIELD {
            if item == 6 {
                jam_each = 95;
            }
        } else if s.category == slot::ARMOR {
            if item == 9 {
                jam_each = 80;
            }
        } else if s.category == slot::MINING {
            if item == 6 {
                jam_each = 70;
            }
        } else if s.category == slot::SPECIAL_E {
            match item {
                4 => jam_each = 90,
                5..=7 => {
                    let ability = i64::from(SPECIALS_E.get(item).map_or(0, |p| p.ability));
                    init += i32::try_from((item as i64 - 4) * n).unwrap_or(0);
                    for _ in 0..n {
                        bc += (100 - bc) * ability / 100;
                    }
                }
                8..=11 => {
                    jam_each = 100 - i64::from(SPECIALS_E.get(item).map_or(0, |p| p.ability));
                }
                12 | 13 => {
                    let ability = i64::from(SPECIALS_E.get(item).map_or(0, |p| p.ability));
                    for _ in 0..n {
                        cap = cap * (100 + ability) / 100;
                    }
                }
                14 => dampener = true,
                _ => {}
            }
        } else if s.category == slot::SPECIAL_M && item == 10 {
            let ability = i64::from(SPECIALS_M.get(item).map_or(0, |p| p.ability));
            for _ in 0..n {
                deflect = deflect * (100 - ability) / 100;
            }
        } else if s.category == slot::BEAM && item == 18 {
            for _ in 0..n {
                bc += (100 - bc) * 10 / 100;
            }
        }
        if jam_each < 100 {
            for _ in 0..n {
                jam = jam * jam_each / 100;
            }
        }
        let _ = (ARMORS.len(), SHIELDS.len());
    }
    let pct_jam = if jam == 10_000 {
        0
    } else {
        (100 - (jam + 50) / 100).min(95)
    };
    Fittings {
        pct_jam: i32::try_from(pct_jam).unwrap_or(0),
        pct_computer: i32::try_from(bc).unwrap_or(0),
        pct_capacitor: if cap == 1_000 {
            0
        } else {
            i32::try_from(cap.min(2_550) / 10).unwrap_or(0)
        },
        pct_beam_defence: i32::try_from(deflect / 10).unwrap_or(100),
        initiative: init.min(63),
        dampener,
    }
}

/// `CheckTarget`'s class for a design (`mdTarget0`).
fn class_of(design: &crate::design::ShipDesign) -> TargetClass {
    use crate::components::slot;
    if design.is_armed() {
        TargetClass::ArmedShips
    } else if design.slots.iter().any(|s| s.count > 0 && s.is(slot::BOMB)) {
        TargetClass::BombersFreighters
    } else if design
        .slots
        .iter()
        .any(|s| s.count > 0 && s.category == slot::SPECIAL_M && (s.item == 5 || s.item == 6))
        && design.cargo_capacity().unwrap_or(0) == 0
    {
        TargetClass::FuelTransports
    } else if design.cargo_capacity().unwrap_or(0) == 0 {
        TargetClass::UnarmedShips
    } else {
        TargetClass::Freighters
    }
}

/// One token's provenance, kept beside the board.
#[derive(Debug, Clone, Copy)]
struct Origin {
    /// The fleet it came from, as an index, or `None` for the starbase.
    fleet: Option<usize>,
    /// The design slot.
    design: u8,
    /// Ships at the start.
    ships: i32,
    /// Its disengage countdown, when it is running.
    disengage: Option<u8>,
    /// The design's shields per ship, for regeneration.
    shields_max: i32,
    /// The id and class the recording names it by.
    id: u16,
    object_class: u8,
}

/// A battle's tokens, with where each came from.
struct Board {
    tokens: Vec<CombatToken>,
    origins: Vec<Origin>,
}

/// `InitializeBoard` (`10f0:45b4`): the tokens — one per design stack of
/// every fleet of a player in the battle, and the starbase — placed on
/// the starting squares by `rgbrcStart`, then shuffled
/// (`RandomizeTokOrder`).
fn build_board(state: &GameState, encounter: &Encounter, rng: &mut Rng) -> Board {
    let players = encounter.players();
    let mut side_of = [0u8; 16];
    let mut side = 0u8;
    for (p, slot) in side_of.iter_mut().enumerate() {
        if encounter.present & (1 << p) != 0 {
            *slot = side;
            side += 1;
        }
    }
    let mut tokens: Vec<CombatToken> = Vec::new();
    let mut origins: Vec<Origin> = Vec::new();
    let mut dampened = false;

    // The starbase.
    if let Some(planet) = encounter.planet.and_then(|i| state.planets.get(i)) {
        if let (Some(owner), true, Some(slot)) = (
            planet.owner.and_then(|o| usize::try_from(o).ok()),
            planet.starbase,
            planet.starbase_design,
        ) {
            if encounter.present & (1 << (owner & 15)) != 0 {
                let design = state.designs.get(owner).and_then(|d| {
                    d.get(usize::from(crate::startup::FIRST_STARBASE_SLOT) + usize::from(slot))
                });
                if let Some(design) = design {
                    let race = &state.players[owner].race;
                    let regen = race.has_lrt(crate::race::lrt::REGENERATING_SHIELDS);
                    let fit = fittings(design);
                    dampened |= fit.dampener;
                    let weapons = design.weapons();
                    let armed = !weapons.is_empty();
                    let damage = Damage {
                        pct_ships: if planet.starbase_damage != 0 { 100 } else { 0 },
                        pct_damage: i32::from(planet.starbase_damage),
                    };
                    tokens.push(CombatToken {
                        player: u8::try_from(owner).unwrap_or(0),
                        active: true,
                        square: battle::start_square(players, side_of[owner])
                            .unwrap_or(Square::new(0, 0)),
                        initiative_base: fit.initiative,
                        capacitor_pct: fit.pct_capacitor,
                        beam_deflection_pct: fit.pct_beam_defence,
                        weapon_reach: weapons.iter().map(|w| w.range).max().unwrap_or(0),
                        weapons,
                        value: design.cost().map_or(0, |c| c.resources + c.minerals[1]),
                        state: TokenState {
                            ships: 1,
                            shields: design.shields(regen),
                            armor: design.armor(regen).unwrap_or(0),
                            damage,
                        },
                        tactic: Tactic::MaximiseDamage,
                        speed_index: 0,
                        moves_left: 0,
                        class: if armed {
                            TargetClass::ArmedShips
                        } else {
                            TargetClass::UnarmedShips
                        },
                        primary_target: if armed {
                            TargetClass::Any
                        } else {
                            TargetClass::None
                        },
                        secondary_target: TargetClass::Any,
                        is_starbase: true,
                        pct_jam: fit.pct_jam * 3 / 4,
                        pct_computer: fit.pct_computer,
                        mass: i32::MAX / 2,
                        enemies: encounter.attack[owner & 15],
                    });
                    origins.push(Origin {
                        fleet: None,
                        design: 16 + slot,
                        ships: 1,
                        disengage: None,
                        shields_max: design.shields(regen),
                        id: u16::try_from(planet.id).unwrap_or(0),
                        object_class: crate::fleet::grobj::PLANET,
                    });
                }
            }
        }
    }

    // The fleets.
    for &index in &encounter.fleets {
        let fleet = &state.fleets[index];
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        if owner >= 16 || encounter.present & (1 << owner) == 0 {
            continue;
        }
        let Some(designs) = state.designs.get(owner) else {
            continue;
        };
        let race = &state.players[owner].race;
        let regen = race.has_lrt(crate::race::lrt::REGENERATING_SHIELDS);
        let plan = plan_of(state, owner, fleet.battle_plan);
        let capacity = fleet.cargo_capacity(designs).max(0);
        let cargo = fleet.cargo.mass();
        for stack in &fleet.stacks {
            if stack.count <= 0 || tokens.len() >= MAX_TOKENS {
                continue;
            }
            let Some(design) = designs.get(usize::from(stack.design)) else {
                continue;
            };
            if design.hull().is_none() {
                continue;
            }
            let fit = fittings(design);
            dampened |= fit.dampener;
            let weapons = design.weapons();
            let class = class_of(design);
            let (primary, secondary, tactic) = plan.map_or(
                (TargetClass::Any, TargetClass::Any, Tactic::MaximiseDamage),
                |p| {
                    (
                        TargetClass::from_raw(p.primary_target),
                        TargetClass::from_raw(p.secondary_target),
                        Tactic::from_raw(p.tactic_nibble()).unwrap_or(Tactic::MaximiseDamage),
                    )
                },
            );
            let tactic = if class == TargetClass::ArmedShips {
                tactic
            } else {
                Tactic::Disengage
            };
            let (primary, secondary) = if weapons.is_empty() {
                (TargetClass::None, secondary)
            } else {
                (primary, secondary)
            };
            // Its share of the fleet's cargo: the design's hold over the
            // fleet's, per ship.
            let hold = design.cargo_capacity().unwrap_or(0);
            let share = if capacity > 0 && hold > 0 {
                i32::try_from(i64::from(cargo) * i64::from(hold) / i64::from(capacity)).unwrap_or(0)
            } else {
                0
            };
            let speed = battle_speed(design, race, share);
            tokens.push(CombatToken {
                player: u8::try_from(owner).unwrap_or(0),
                active: true,
                square: battle::start_square(players, side_of[owner]).unwrap_or(Square::new(0, 0)),
                initiative_base: fit.initiative,
                capacitor_pct: fit.pct_capacitor,
                beam_deflection_pct: fit.pct_beam_defence,
                weapon_reach: weapons.iter().map(|w| w.range).max().unwrap_or(0),
                weapons,
                value: design.cost().map_or(0, |c| c.resources + c.minerals[1]),
                state: TokenState {
                    ships: stack.count,
                    shields: design.shields(regen),
                    armor: design.armor(regen).unwrap_or(0),
                    damage: Damage {
                        pct_ships: stack.damaged_pct,
                        pct_damage: stack.damage_pct,
                    },
                },
                tactic,
                speed_index: speed,
                moves_left: 0,
                class,
                primary_target: primary,
                secondary_target: secondary,
                is_starbase: false,
                pct_jam: fit.pct_jam,
                pct_computer: fit.pct_computer,
                mass: design.mass().unwrap_or(0) + share,
                enemies: encounter.attack[owner & 15],
            });
            origins.push(Origin {
                fleet: Some(index),
                design: stack.design,
                ships: stack.count,
                disengage: (tactic == Tactic::Disengage).then_some(DISENGAGE_MOVES),
                shields_max: design.shields(regen),
                id: fleet.id,
                object_class: crate::fleet::grobj::FLEET,
            });
        }
    }

    // `RandomizeTokOrder`: a forward shuffle with the game's generator.
    let n = tokens.len();
    for i in 0..n {
        let span = i16::try_from(n - i).unwrap_or(i16::MAX);
        let j = i + usize::try_from(rng.random(span)).unwrap_or(0);
        if j != i && j < n {
            tokens.swap(i, j);
            origins.swap(i, j);
        }
    }
    if dampened {
        for t in tokens.iter_mut().filter(|t| !t.is_starbase) {
            t.speed_index = t.speed_index.saturating_sub(4);
        }
    }
    Board { tokens, origins }
}

/// The players with a live token, as a mask.
fn players_alive(tokens: &[CombatToken]) -> u16 {
    tokens
        .iter()
        .filter(|t| t.alive())
        .fold(0u16, |m, t| m | (1 << (t.player & 15)))
}

/// Whether anyone left still has an enemy left (`grfPlayer & rggrfAttack`).
fn still_a_fight(tokens: &[CombatToken]) -> bool {
    let alive = players_alive(tokens);
    let mut fighters = alive;
    for p in 0..16u8 {
        if alive & (1 << p) == 0 {
            continue;
        }
        let enemies = tokens
            .iter()
            .filter(|t| t.alive() && t.player == p)
            .fold(0u16, |m, t| m | t.enemies);
        if alive & enemies == 0 {
            fighters &= !(1 << p);
        }
    }
    fighters.count_ones() >= 2
}

/// A token's record for the recording.
fn record_token(token: &CombatToken, origin: Origin) -> BattleToken {
    let init_min = token
        .firing_initiatives()
        .first()
        .copied()
        .map_or(0xff, |i| u8::try_from(i).unwrap_or(0xff));
    let init_max = token
        .firing_initiatives()
        .last()
        .copied()
        .map_or(0xff, |i| u8::try_from(i).unwrap_or(0xff));
    let min_range = token.weapons.iter().map(|w| w.range).min().unwrap_or(0);
    BattleToken {
        id: origin.id,
        player: token.player,
        object_class: origin.object_class,
        design: origin.design,
        square: stars_formats::battle::Square {
            x: token.square.x,
            y: token.square.y,
        },
        initiative_base: u8::try_from(token.initiative_base).unwrap_or(0),
        initiative_min: init_min,
        initiative_max: init_max,
        target: 0xff,
        pct_cloak: 0,
        pct_jam: u8::try_from(token.pct_jam).unwrap_or(0),
        pct_computer: u8::try_from(token.pct_computer).unwrap_or(0),
        pct_capacitor: u8::try_from(token.capacitor_pct).unwrap_or(0),
        pct_beam_defence: u8::try_from(token.beam_deflection_pct).unwrap_or(100),
        mass: u16::try_from(token.mass.clamp(0, 65_535)).unwrap_or(u16::MAX),
        shields: u16::try_from(token.state.shields.clamp(0, 65_535)).unwrap_or(u16::MAX),
        ships: u16::try_from(origin.ships.clamp(0, 65_535)).unwrap_or(u16::MAX),
        damage: token.state.damage.to_raw(),
        tactics: u16::from(token.primary_target as u8)
            | (u16::from(token.secondary_target as u8) << 4)
            | (u16::from(token.tactic as u8) << 8)
            | (u16::from(token.class as u8) << 12),
        movement: u16::try_from(token.weapon_reach.clamp(0, 15)).unwrap_or(0)
            | (u16::try_from(min_range.clamp(0, 15)).unwrap_or(0) << 4)
            | (u16::from(token.speed_index & 0x0f) << 8),
        flags: 1
            | (u16::from(token.weapons.iter().any(|w| w.torpedo)) << 2)
            | (u16::from(origin.disengage.unwrap_or(0) & 0x1f) << 5),
    }
}

/// `FDoCoolBattle` (`10f0:8bcc`): one battle, from its tokens to its
/// recording. The fleets are not touched here — [`apply`] does that from
/// the tokens afterwards.
fn fight(
    state: &GameState,
    encounter: &Encounter,
    board: &mut Board,
    battle_id: u16,
    rng: &mut Rng,
) -> BattleRecord {
    let tokens_at_start: Vec<BattleToken> = board
        .tokens
        .iter()
        .zip(board.origins.iter())
        .map(|(t, o)| record_token(t, *o))
        .collect();
    let mut actions: Vec<BattleAction> = Vec::new();

    for round in 0..ROUNDS {
        // Regenerating shields, from the second round.
        if round > 0 {
            for (t, o) in board.tokens.iter_mut().zip(board.origins.iter()) {
                if !t.alive() {
                    continue;
                }
                let owner = usize::from(t.player);
                let regen = state
                    .players
                    .get(owner)
                    .is_some_and(|p| p.race.has_lrt(crate::race::lrt::REGENERATING_SHIELDS));
                if regen && t.state.shields > 0 {
                    // `RegenShield`: a tenth of the design's shields back.
                    t.state.shields = (t.state.shields + o.shields_max / 10).min(o.shields_max);
                }
            }
        }
        if !still_a_fight(&board.tokens) {
            break;
        }

        // Movement, phase by phase, the disengaging tokens counting down.
        for t in board.tokens.iter_mut() {
            t.moves_left = if t.is_starbase {
                0
            } else {
                battle::movement_this_round(t.speed_index, round)
            };
        }
        let mut order: Vec<usize> = (0..board.tokens.len())
            .filter(|&i| board.tokens[i].alive())
            .collect();
        for _ in &order {
            let _ = rng.random(15);
        }
        order.sort_by_key(|&i| (std::cmp::Reverse(board.tokens[i].mass), i));
        for phase in (1..=battle::MOVEMENT_PHASES).rev() {
            for &mover in &order {
                if !board.tokens[mover].alive() || phase > board.tokens[mover].moves_left {
                    continue;
                }
                if let Some(left) = board.origins[mover].disengage {
                    if left == 0 {
                        board.tokens[mover].active = false;
                        actions.push(BattleAction {
                            token: u8::try_from(mover).unwrap_or(0),
                            destination: None,
                            round,
                            range: 0,
                            target: 0,
                            kills: Vec::new(),
                        });
                        continue;
                    }
                    board.origins[mover].disengage = Some(left - 1);
                }
                let primary = battle::primary_target_exists(&board.tokens, mover);
                let search = battle::move_search(&board.tokens, mover, primary);
                let square = match search.beeline {
                    Some(target) => battle::beeline_move(&board.tokens, mover, target, rng),
                    None => battle::choose_move(&board.tokens, mover, search.radius.max(1), rng),
                };
                if square != board.tokens[mover].square {
                    board.tokens[mover].square = square;
                    actions.push(BattleAction {
                        token: u8::try_from(mover).unwrap_or(0),
                        destination: Some(stars_formats::battle::Square {
                            x: square.x,
                            y: square.y,
                        }),
                        round,
                        range: 0,
                        target: 0,
                        kills: Vec::new(),
                    });
                }
            }
        }
        if !still_a_fight(&board.tokens) {
            break;
        }

        // Fire.
        let events = battle::fire_round_rng(&mut board.tokens, rng);
        for e in events {
            actions.push(BattleAction {
                token: u8::try_from(e.attacker).unwrap_or(0),
                destination: Some(stars_formats::battle::Square {
                    x: board.tokens[e.attacker].square.x,
                    y: board.tokens[e.attacker].square.y,
                }),
                round,
                range: e.range,
                target: u8::try_from(e.target).unwrap_or(0),
                kills: vec![Kill {
                    token: u8::try_from(e.target).unwrap_or(0),
                    weapon: u8::from(e.torpedo),
                    ships_killed: u16::try_from(e.ships_killed.clamp(0, 65_535)).unwrap_or(0),
                    shield_damage: u16::try_from(e.shield_damage.clamp(0, 65_535)).unwrap_or(0),
                    damage: e.damage_after,
                }],
            });
        }
        if !still_a_fight(&board.tokens) {
            break;
        }
    }

    BattleRecord {
        id: battle_id,
        players: encounter.players(),
        player_mask: encounter.present | encounter.spectators,
        planet: encounter
            .planet
            .and_then(|i| u16::try_from(state.planets[i].id).ok())
            .unwrap_or(u16::MAX),
        position: (encounter.position.x, encounter.position.y),
        tokens: tokens_at_start,
        actions,
        declared_len: 0,
    }
}

/// What the battle left: the fleets' ships and damage (`KillShips`, and
/// `FDamageTok`'s write of `rgdv`), the starbase's state, and salvage
/// (`CreateSalvage`: a third of the dead ships' minerals plus a dead
/// fleet's cargo — at a planet, eight tenths of it onto the surface with a
/// starbase there and half without; in space, half of it as a stationary
/// packet, merged with any already there).
fn apply(state: &mut GameState, encounter: &Encounter, board: &Board, outcome: &mut Outcome) {
    let mut dead_minerals: BTreeMap<Option<usize>, [i64; 3]> = BTreeMap::new();
    let mut starbase_died = false;
    for (token, origin) in board.tokens.iter().zip(board.origins.iter()) {
        let lost = origin.ships - token.state.ships.max(0);
        match origin.fleet {
            None => {
                let Some(planet) = encounter.planet else {
                    continue;
                };
                if token.state.ships <= 0 {
                    starbase_died = true;
                    outcome.losses.push((
                        state.planets[planet].owner.unwrap_or(-1),
                        origin.design,
                        1,
                    ));
                } else {
                    state.planets[planet].starbase_damage =
                        u16::try_from(token.state.damage.pct_damage.clamp(0, 500)).unwrap_or(0);
                }
            }
            Some(index) => {
                let owner = state.fleets[index].owner;
                if lost > 0 {
                    outcome.losses.push((owner, origin.design, lost));
                    // A third of the ore that built them.
                    if let Some(design) = usize::try_from(owner)
                        .ok()
                        .and_then(|o| state.designs.get(o))
                        .and_then(|d| d.get(usize::from(origin.design)))
                    {
                        let cost = design.cost().unwrap_or_default();
                        let entry = dead_minerals.entry(Some(index)).or_default();
                        for (e, m) in entry.iter_mut().zip(cost.minerals.iter()) {
                            *e += i64::from(lost) * i64::from(*m) / 3;
                        }
                    }
                }
                let fleet = &mut state.fleets[index];
                if let Some(stack) = fleet.stacks.iter_mut().find(|s| s.design == origin.design) {
                    stack.count = token.state.ships.max(0);
                    stack.damaged_pct = token.state.damage.pct_ships;
                    stack.damage_pct = token.state.damage.pct_damage;
                }
            }
        }
    }
    if starbase_died {
        if let Some(planet) = encounter.planet {
            let p = &mut state.planets[planet];
            // Not for an Alternate Reality race's, which is its people.
            let ar = p
                .owner
                .and_then(|o| usize::try_from(o).ok())
                .and_then(|o| state.players.get(o))
                .is_some_and(|pl| pl.race.is_ar());
            if !ar {
                p.starbase = false;
                p.starbase_design = None;
                p.starbase_damage = 0;
                p.queue.retain(|q| {
                    !(q.ship && q.item >= u16::from(crate::startup::FIRST_STARBASE_SLOT))
                });
                p.fling_dest = None;
            }
            outcome.starbase_destroyed = Some(p.id);
        }
    }

    // Dead fleets give up their cargo too.
    let mut salvage = [0i64; 3];
    for (fleet, minerals) in &dead_minerals {
        let mut total = *minerals;
        if let Some(index) = *fleet {
            let f = &state.fleets[index];
            if f.stacks.iter().all(|s| s.count <= 0) {
                for (t, m) in total.iter_mut().zip(f.cargo.minerals.iter()) {
                    *t += i64::from(*m);
                }
            }
        }
        for (s, t) in salvage.iter_mut().zip(total.iter()) {
            *s += t;
        }
    }
    if salvage.iter().any(|&m| m > 0) {
        match encounter.planet {
            Some(planet) => {
                let share = if state.planets[planet].starbase { 8 } else { 5 };
                for (stock, m) in state.planets[planet]
                    .surface_min
                    .iter_mut()
                    .zip(salvage.iter())
                {
                    *stock = stock.saturating_add(i32::try_from(m * share / 10).unwrap_or(0));
                }
            }
            None => {
                let left = salvage.map(|m| i32::try_from(m - m / 2).unwrap_or(0));
                if left.iter().any(|&m| m > 0) {
                    drop_salvage(state, encounter.position, left);
                    outcome.salvage.push((encounter.position, left));
                }
            }
        }
    }
    state.fleets.retain(|f| !f.is_empty());
}

/// `DropSalvage` (`10f0:24dc`): minerals left in space as a stationary
/// packet at `at`, joining one already there. Nothing is left on a
/// planet's position. Returns the id of the packet the salvage is in.
pub(crate) fn drop_salvage(state: &mut GameState, at: Point, minerals: [i32; 3]) -> Option<u16> {
    if state.planets.iter().any(|p| p.position == Some(at)) {
        return None;
    }
    if let Some(existing) = state
        .packets
        .iter_mut()
        .find(|p| p.warp == 0 && p.position == at)
    {
        for (have, m) in existing.minerals.iter_mut().zip(minerals.iter()) {
            *have = have.saturating_add(
                i16::try_from(m.clamp(&0, &i32::from(i16::MAX)).to_owned()).unwrap_or(i16::MAX),
            );
        }
        return Some(existing.id);
    }
    let id = state
        .packets
        .iter()
        .map(|p| p.id)
        .max()
        .map_or(0, |m| m + 1);
    state.packets.push(crate::packet::Packet {
        id,
        owner: -1,
        position: at,
        target: 0x3ff,
        warp: 0,
        minerals: minerals
            .map(|m| i16::try_from(m.clamp(0, i32::from(i16::MAX))).unwrap_or(i16::MAX)),
        decay_rate: 0,
        moved: true,
        include: true,
        turn: u16::try_from(state.turn).unwrap_or(0),
    });
    Some(id)
}

/// `SendBattleMessages` (`10f0:9c0e`), in outline: every player present
/// hears of the battle — the object word is the battle id with bit 14
/// set, the first two parameters the place (`-1` and the planet, or the
/// coordinates), then their ships, their losses, the enemy's ships and
/// its losses — and a spectator hears that a battle took place. The
/// original's dozen wordings by outcome are one summary here.
fn send_messages(
    state: &mut GameState,
    encounter: &Encounter,
    outcome: &Outcome,
    battle_id: u16,
    board: &Board,
) {
    let place: [i16; 2] = match encounter.planet {
        Some(i) => [-1, state.planets[i].id],
        None => [encounter.position.x, encounter.position.y],
    };
    let object = i16::from_le_bytes((battle_id | 0x4000).to_le_bytes());
    for player in 0..state.players.len().min(16) {
        let bit = 1u16 << player;
        if encounter.present & bit != 0 {
            let ours: i32 = board
                .origins
                .iter()
                .zip(board.tokens.iter())
                .filter(|(_, t)| usize::from(t.player) == player)
                .map(|(o, _)| o.ships)
                .sum();
            let theirs: i32 = board
                .origins
                .iter()
                .zip(board.tokens.iter())
                .filter(|(_, t)| usize::from(t.player) != player)
                .map(|(o, _)| o.ships)
                .sum();
            let our_losses: i32 = outcome
                .losses
                .iter()
                .filter(|(p, _, _)| usize::try_from(*p).ok() == Some(player))
                .map(|(_, _, n)| n)
                .sum();
            let their_losses: i32 = outcome
                .losses
                .iter()
                .filter(|(p, _, _)| usize::try_from(*p).ok() != Some(player))
                .map(|(_, _, n)| n)
                .sum();
            let n = |v: i32| i16::try_from(v).unwrap_or(i16::MAX);
            state.messages.push(Message {
                player,
                id: id::BATTLE,
                object,
                params: vec![
                    place[0],
                    place[1],
                    n(ours),
                    n(our_losses),
                    n(theirs),
                    n(their_losses),
                ],
            });
        } else if encounter.spectators & bit != 0 {
            state.messages.push(Message {
                player,
                id: id::BATTLE_SEEN,
                object,
                params: vec![place[0], place[1]],
            });
        }
    }
}

/// `DoBattles` (`10f0:3a26`): every place where fleets of two or more
/// players (or a fleet and a starbase) stand together is asked whether
/// they fight, and the battles are played out in fleet order. The
/// recordings go to [`GameState::battles`] for the players present.
pub fn do_battles(state: &mut GameState, rng: &mut Rng) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    // `idBattle = (game.turn & 0xf) * 0x100 + 1`.
    let mut battle_id = (u16::try_from(state.turn).unwrap_or(0) & 0xf) * 0x100 + 1;

    // `LinkFleets`: the fleets at each place, in fleet order.
    let mut places: Vec<(Point, Vec<usize>)> = Vec::new();
    for (index, fleet) in state.fleets.iter().enumerate() {
        if fleet.is_empty() {
            continue;
        }
        match places.iter_mut().find(|(p, _)| *p == fleet.position) {
            Some((_, list)) => list.push(index),
            None => places.push((fleet.position, vec![index])),
        }
    }
    // A battle removes fleets, which shifts the indices: work by fleet
    // identity instead.
    let places: Vec<(Point, Vec<(i16, u16)>)> = places
        .into_iter()
        .map(|(p, list)| {
            (
                p,
                list.iter()
                    .map(|&i| (state.fleets[i].owner, state.fleets[i].id))
                    .collect(),
            )
        })
        .collect();

    for (position, keys) in places {
        let fleets: Vec<usize> = keys
            .iter()
            .filter_map(|(owner, id)| {
                state
                    .fleets
                    .iter()
                    .position(|f| f.owner == *owner && f.id == *id && !f.is_empty())
            })
            .collect();
        let planet = state
            .planets
            .iter()
            .position(|p| p.position == Some(position));
        let owners: std::collections::BTreeSet<i16> = fleets
            .iter()
            .map(|&i| state.fleets[i].owner)
            .chain(
                planet
                    .filter(|&i| state.planets[i].starbase)
                    .and_then(|i| state.planets[i].owner),
            )
            .collect();
        if owners.len() < 2 {
            continue;
        }
        let Some(encounter) = who_fights(state, &fleets, planet) else {
            continue;
        };
        let mut board = build_board(state, &encounter, rng);
        if board.tokens.len() < 2 {
            continue;
        }
        let record = fight(state, &encounter, &mut board, battle_id, rng);
        let mut outcome = Outcome {
            record,
            losses: Vec::new(),
            starbase_destroyed: None,
            salvage: Vec::new(),
        };
        apply(state, &encounter, &board, &mut outcome);
        send_messages(state, &encounter, &outcome, battle_id, &board);
        state.battles.push(outcome.record.clone());
        outcomes.push(outcome);
        battle_id = battle_id.wrapping_add(1);
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A battle plan that attacks enemies names only the players marked
    /// enemy; one that attacks everyone names all the others.
    #[test]
    fn attack_masks_follow_the_relations() {
        let (config, seed) = crate::newgame::tutorial();
        let mut rng = Rng::randomize(seed);
        let mut state = crate::newgame::generate(&config, &mut rng)
            .expect("generates")
            .state;
        state.players[0].relations = vec![0, 2];
        assert_eq!(attack_mask(&state, 0, attack_who::ENEMIES), 0b10);
        state.players[0].relations = vec![0, 0];
        assert_eq!(attack_mask(&state, 0, attack_who::ENEMIES), 0);
        assert_eq!(
            attack_mask(&state, 0, attack_who::NEUTRALS_AND_ENEMIES),
            0b10
        );
        assert_eq!(attack_mask(&state, 0, attack_who::EVERYONE), !1u16);
        assert_eq!(attack_mask(&state, 0, attack_who::PLAYER_BASE + 1), 0b10);
    }

    /// The tutorial's Armed Probe (a Scout with a Quick Jump 5 and no
    /// thrusters) fights at the index its engine gives it.
    #[test]
    fn battle_speed_comes_from_the_engine() {
        let (config, seed) = crate::newgame::tutorial();
        let mut rng = Rng::randomize(seed);
        let state = crate::newgame::generate(&config, &mut rng)
            .expect("generates")
            .state;
        let race = &state.players[0].race;
        for design in &state.designs[0] {
            if design.hull().is_none() {
                continue;
            }
            let speed = battle_speed(design, race, 0);
            assert!(speed <= 8);
        }
    }
}
