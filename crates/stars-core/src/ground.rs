//! Landing colonists: settling an empty planet, and invading a held one.
//!
//! Source: `DropColonists` (`10b8:34e2`), called from `DoOrders`. One routine
//! does both jobs, because they are the same act: colonists are put down on a
//! planet, and what happens next depends on whether anyone was already there.
//!
//! Every drop aimed at a planet in the same turn is resolved together, so two
//! players who both send colonists to the same empty world contest it, and an
//! invasion by several players at once is a three-way fight.

use crate::race::{Prt, Race};
use crate::research::{tech_level_cost, Research};
use crate::rng::Rng;

/// How much an attacker's colonists are worth, as a percentage.
///
/// Source: `10b8:3766`. War Mongers fight at 165%, Alternate Reality races
/// cannot take a planet by force at all, and everyone else attacks at 110%.
#[must_use]
pub fn attack_weight(prt: Option<Prt>) -> i32 {
    match prt {
        Some(Prt::Wm) => 165,
        Some(Prt::Ar) => 0,
        _ => 110,
    }
}

/// How much a defender's colonists are worth, as a percentage.
///
/// Source: `10b8:3820`. Inner Strength defends at 200%; everyone else at 100%.
#[must_use]
pub fn defence_weight(prt: Option<Prt>) -> i32 {
    match prt {
        Some(Prt::Is) => 200,
        _ => 100,
    }
}

/// Whether a race will take an item from its default production queue when a
/// new planet inherits one.
///
/// Source: the template loop in `DropColonists` (`10b8:3d1a`). A newly settled
/// planet is given a queue built from its owner's default template — the "zip"
/// production queue, `PLAYER.zpq1` — with two races filtering it:
///
/// - **Alternate Reality** drops every item at or below
///   [`crate::production::item::AUTO_DEFENSE`], because it has no planetary
///   installations to build.
/// - **Claim Adjuster** drops the two terraforming items, because it terraforms
///   from orbit for free.
///
/// The item is taken from the low six bits of the template word.
#[must_use]
pub fn template_allows(prt: Option<Prt>, item: u16) -> bool {
    use crate::production::item;
    match prt {
        Some(Prt::Ar) => item > item::AUTO_DEFENSE,
        Some(Prt::Ca) => item != item::AUTO_MIN_TERRAFORM && item != item::AUTO_MAX_TERRAFORM,
        _ => true,
    }
}

/// One player's colonists arriving at a planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Landing {
    /// Whose colonists these are.
    pub player: i16,
    /// How many, in units of 100 as the planet stores them.
    pub colonists: i32,
    /// Their race's primary trait, which sets their weight.
    pub prt: Option<Prt>,
    /// Whether they may settle an empty planet (`COLDROP.fCanColonize`):
    /// set for a drop a waypoint task makes (`FQueueColonistDrop`), and for
    /// a Cargo Transfer dialog's unload only onto an inhabited planet — the
    /// dialog's colonists "forced to transport down" to an empty world die.
    pub can_colonize: bool,
}

impl Landing {
    /// The weighted strength of this landing as an attacker, before the
    /// planet's defences take their share.
    #[must_use]
    pub fn strength(self) -> i32 {
        self.colonists.saturating_mul(attack_weight(self.prt)) / 100
    }
}

/// The planet the colonists land on, as the fight sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    /// The planet's id, for the messages.
    pub id: i16,
    /// Who holds it, and with how many colonists (hundreds) and what
    /// trait; `None` for an empty planet.
    pub defender: Option<(i16, i32, Option<Prt>)>,
    /// Whether a starbase is in orbit — with an owner, it kills every
    /// landing outright.
    pub starbase: bool,
    /// The share of a bombing run the planet's defences let through
    /// (`CalcPctSurvive`); ground troops fare better, and
    /// [`resolve_landings`] works that in itself.
    pub pct_survive: f64,
}

/// What a set of landings did to a planet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nobody landed anything that counts.
    Nothing,
    /// The planet was empty and is now held by this player.
    Settled {
        /// The new owner.
        player: i16,
        /// Colonists left standing, in units of 100.
        colonists: i32,
    },
    /// The planet was held and the defender kept it.
    Held {
        /// Defenders left standing, in units of 100.
        colonists: i32,
    },
    /// The planet was held and changed hands.
    Taken {
        /// The new owner.
        player: i16,
        /// Colonists left standing, in units of 100.
        colonists: i32,
        /// Who lost it.
        loser: i16,
    },
    /// Two claims came out exactly equal: nobody survived, and a planet
    /// whose defenders had already been overrun is left empty.
    Annihilated {
        /// Who held it, if anyone did.
        loser: Option<i16>,
    },
}

/// A landing's fate and what everybody is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// What became of the planet.
    pub outcome: Outcome,
    /// The messages `DropColonists` sends, in its order.
    pub messages: Vec<crate::message::Message>,
}

/// Resolve every landing on one planet — `DropColonists` (`10b8:34e2`),
/// transcribed.
///
/// First each landing is sorted: an Alternate Reality race's colonists die
/// on any surface (`0x57`); colonists that may not colonise die on an empty
/// planet (`0x02`); an inhabited planet's starbase kills every landing
/// (`0x58`). The rest are summed per player, as colonists and as **power**:
/// `colonists × weight / 100` ([`attack_weight`]) times the share the
/// planet's defences let through, which for troops is
/// `pct + (1 − pct) / 4`.
///
/// A held planet's defence is `colonists × weight / 100`
/// ([`defence_weight`]). Attacking power short of it loses: each attacker is
/// massacred (`0x00`/`0x03`, or `0x01`/`0x04` when the defences took a
/// share first) and the defenders lose `held × power / defence`. Power that
/// matches or beats it clears the planet (`UninhabitPlanet`) and the
/// attackers fight on among themselves as on an empty one.
///
/// On an empty planet the greatest power wins — even a power of nothing,
/// which is how an Alternate Reality race settles; an exact tie kills everyone
/// (`0x06`, and `0x05` to a former holder). The winner keeps
/// `colonists × (power − defence) / power` — all of them when nothing stood
/// in the way — and, when a rival came second, `× (best − second) / best`;
/// at least one hundred either way. The "second" is the best power seen
/// before the winner in player order, which is the original's own quirk: a
/// winner with a lower number than every rival loses nothing to them.
#[must_use]
pub fn resolve_landings(ground: Ground, landings: &[Landing]) -> Resolution {
    use crate::message::{id, Message};

    let planet = ground.id;
    let mut messages = Vec::new();
    let mut tell = |player: i16, which: u16, object: i16, params: Vec<i16>| {
        if let Ok(player) = usize::try_from(player) {
            messages.push(Message {
                player,
                id: which,
                object,
                params,
            });
        }
    };
    let held_by = ground.defender.map(|(owner, _, _)| owner);
    let troops_survive = (1.0 - ground.pct_survive) / 4.0 + ground.pct_survive;

    // Per player, colonists and power.
    let mut colonists = [0i64; 16];
    let mut power = [0i64; 16];
    let mut power_total: i64 = 0;
    for landing in landings {
        let Ok(who) = usize::try_from(landing.player) else {
            continue;
        };
        if who >= 16 || landing.colonists <= 0 {
            continue;
        }
        let [lo, hi] = Message::long(landing.colonists);
        if landing.prt == Some(Prt::Ar) && (!landing.can_colonize || held_by.is_some()) {
            tell(landing.player, id::LANDING_AR_DIED, planet, vec![planet]);
        } else if held_by.is_none() && !landing.can_colonize {
            tell(
                landing.player,
                id::LANDING_NOT_COLONISED,
                planet,
                vec![lo, hi, planet],
            );
        } else if ground.starbase && held_by.is_some() {
            tell(landing.player, id::LANDING_STARBASE, planet, vec![planet]);
        } else {
            colonists[who] += i64::from(landing.colonists);
            let base = i64::from(landing.colonists) * i64::from(attack_weight(landing.prt)) / 100;
            // `(long)((double)base * pctSurvive)`: truncated.
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let scaled = (base as f64 * troops_survive) as i64;
            power[who] += scaled;
            power_total += scaled;
        }
    }

    // A held planet defends itself first.
    let mut defence: i64 = 0;
    let mut loser: Option<i16> = None;
    if let Some((owner, held, prt)) = ground.defender {
        defence = i64::from(held) * i64::from(defence_weight(prt)) / 100;
        if defence > power_total {
            let side = |p: i16| p | 0x30;
            for (who, count) in colonists.iter().enumerate() {
                if *count == 0 {
                    continue;
                }
                #[allow(clippy::cast_possible_truncation)]
                let [lo, hi] = Message::long(*count as i32);
                let attacker = i16::try_from(who).unwrap_or(0);
                #[allow(clippy::float_cmp)]
                if troops_survive == 1.0 {
                    tell(
                        attacker,
                        id::LANDING_MASSACRED,
                        planet,
                        vec![lo, hi, planet, side(owner)],
                    );
                    tell(
                        owner,
                        id::LANDING_REPELLED,
                        planet,
                        vec![planet, lo, hi, side(attacker)],
                    );
                } else {
                    #[allow(clippy::cast_possible_truncation)]
                    let pct = (10000.0 * (troops_survive - 1.0)) as i16;
                    tell(
                        attacker,
                        id::LANDING_SHOT_DOWN,
                        planet,
                        vec![lo, hi, planet, pct, side(owner)],
                    );
                    tell(
                        owner,
                        id::LANDING_REPELLED_BY_DEFENCES,
                        planet,
                        vec![planet, lo, hi, side(attacker)],
                    );
                }
            }
            let lost = if defence > 0 {
                i64::from(held) * power_total / defence
            } else {
                0
            };
            #[allow(clippy::cast_possible_truncation)]
            return Resolution {
                outcome: Outcome::Held {
                    colonists: (i64::from(held) - lost) as i32,
                },
                messages,
            };
        }
        // Overrun: the planet is emptied and the attackers settle it.
        loser = Some(owner);
    }

    // The strongest claim, an exact tie, and how many sides there were.
    let mut best: i64 = -1;
    let mut second: i64 = 0;
    let mut sides = 0i16;
    let mut tie = false;
    let mut winner = 0usize;
    for who in 0..16 {
        if colonists[who] == 0 {
            continue;
        }
        sides += 1;
        if power[who] >= best {
            if power[who] == best {
                tie = true;
            } else {
                tie = false;
                second = best;
                best = power[who];
                winner = who;
            }
        }
    }
    // `cMax` starts at −1 and the test is on its high word: a claim of no
    // power at all — an Alternate Reality race's — still counts.
    if best < 0 {
        return Resolution {
            outcome: if loser.is_some() {
                Outcome::Annihilated { loser }
            } else {
                Outcome::Nothing
            },
            messages,
        };
    }
    if tie {
        for (who, count) in colonists.iter().enumerate() {
            if *count != 0 {
                tell(
                    i16::try_from(who).unwrap_or(0),
                    id::LANDING_FREE_FOR_ALL,
                    planet,
                    vec![sides, planet],
                );
            }
        }
        if let Some(owner) = loser {
            tell(owner, id::LANDING_PRONGED, planet, vec![sides, planet]);
        }
        return Resolution {
            outcome: Outcome::Annihilated { loser },
            messages,
        };
    }

    let winner_param = i16::try_from(winner).unwrap_or(0);
    match loser {
        None => {
            if sides < 2 {
                let ar = landings
                    .iter()
                    .any(|l| l.player == winner_param && l.prt == Some(Prt::Ar));
                tell(
                    winner_param,
                    if ar {
                        id::COLONISTS_CONTROL_AR
                    } else {
                        id::COLONISTS_CONTROL
                    },
                    planet,
                    vec![planet],
                );
            } else {
                for (who, count) in colonists.iter().enumerate() {
                    if *count == 0 {
                        continue;
                    }
                    let who_param = i16::try_from(who).unwrap_or(0);
                    if who == winner {
                        tell(who_param, id::LANDING_RACE_WON, planet, vec![sides, planet]);
                    } else {
                        tell(
                            who_param,
                            id::LANDING_RACE_LOST,
                            planet,
                            vec![sides, planet, winner_param | 0xb0],
                        );
                    }
                }
            }
        }
        Some(owner) => {
            for (who, count) in colonists.iter().enumerate() {
                if *count == 0 {
                    continue;
                }
                let who_param = i16::try_from(who).unwrap_or(0);
                if who == winner {
                    tell(
                        who_param,
                        id::LANDING_CRUSHED_DEFENDERS,
                        planet,
                        vec![owner | 0x20, planet],
                    );
                } else {
                    tell(who_param, id::LANDING_LOST_THE_FIGHT, planet, vec![planet]);
                }
            }
            #[allow(clippy::cast_possible_truncation)]
            let [lo, hi] = Message::long(colonists[winner] as i32);
            tell(
                owner,
                id::LANDING_STORMED,
                planet,
                vec![winner_param | 0x30, planet, lo, hi],
            );
        }
    }

    // What the winner has left.
    let mut left = if power_total == 0 || best == 0 {
        colonists[winner]
    } else {
        let through = best * (power_total - defence) / power_total;
        colonists[winner] * through / best
    };
    if second > 0 {
        left = left * (best - second) / best;
    }
    #[allow(clippy::cast_possible_truncation)]
    let left = left.max(1) as i32;
    let outcome = match loser {
        None => Outcome::Settled {
            player: winner_param,
            colonists: left,
        },
        Some(owner) => Outcome::Taken {
            player: winner_param,
            colonists: left,
            loser: owner,
        },
    };
    Resolution { outcome, messages }
}

/// The number of technology fields wreckage can teach.
pub const TECH_FIELDS: usize = 6;

/// A player learns at most one thing per turn from wreckage or a trader
/// (`10f8:...` sets bit 3 of the player's state word and every entry checks it).
pub const ONE_PER_TURN: bool = true;

/// What a player took away from a wreck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Salvage {
    /// The technology field advanced, 0 to 5.
    pub field: usize,
    /// Resources credited toward that field's next level. This is **not** a
    /// free level: the research is paid for, and the level arrives at the next
    /// research tick like any other.
    pub resources: i32,
}

/// Learn a technology from a beaten enemy's wreckage.
///
/// Source: `ITechLearnATech` (`10f8:...`), called from `DropColonists` when an
/// inhabited planet is taken, with the loser's six technology levels copied
/// into `rgTechBattle` first.
///
/// The shape of it:
///
/// 1. A player who has already learned something this turn gets nothing —
///    one per turn, whatever the source.
/// 2. `Random(100)` must come out **above 49**, so it works half the time.
/// 3. Six attempts, each picking a field with `Random(6)`. A field where the
///    loser knew more than the winner is taken, and the cost of the winner's
///    next level in that field is credited to its research.
///
/// The Mystery Trader half of the routine — thirteen parts, each with its own
/// chance — is [`learn_from_battle`]'s, which runs the whole routine on the
/// game state; this is the research half on its own.
///
/// `slow_tech` is the game option that doubles research costs, which feeds
/// straight through [`tech_level_cost`].
///
/// # Not verified
///
/// Nothing in the fixtures exercises this. Only 56 planets change hands across
/// `fixtures/games/all-computer-players`, none of them attributable to an
/// invasion without the fleet orders, and a credited research cost is
/// indistinguishable in a save file from research the player paid for itself.
#[must_use]
pub fn learn_from_wreckage(
    winner: &Research,
    winner_race: &Race,
    loser_tech: [u8; TECH_FIELDS],
    already_learned_this_turn: bool,
    slow_tech: bool,
    rng: &mut Rng,
) -> Option<Salvage> {
    if already_learned_this_turn {
        return None;
    }
    if i32::from(rng.random(100)) <= 49 {
        return None;
    }
    for _ in 0..TECH_FIELDS {
        let field = usize::try_from(rng.random(6)).unwrap_or(0);
        let (Some(mine), Some(theirs)) = (
            winner.levels.get(field).copied(),
            loser_tech.get(field).copied(),
        ) else {
            continue;
        };
        if mine >= theirs {
            continue;
        }
        let resources = tech_level_cost(field, mine + 1, winner, winner_race, slow_tech);
        return Some(Salvage { field, resources });
    }
    None
}

/// What a player learned from wreckage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WreckageFind {
    /// Research credited to a field.
    Tech { field: usize, resources: i32 },
    /// A Mystery Trader part, by its [`crate::wormhole::part`] bit.
    Part(u16),
}

/// `ITechLearnATech` (`10f0:9918`) in full, on the game state: what a
/// player learns from the wreckage of a battle (or a planet taken), told
/// with message `idm` — `0xef`, `0xf0` or `0xf1` by how they came to be
/// there — at `place` (`x, y`, or `-1` and the planet).
///
/// One find a year (`learned_tech_this_year`), and only when `Random(100)`
/// comes out above 49. Then thirteen tries at a **Mystery Trader part**:
/// `Random(13)` names one; if the wreckage held any (`trader_seen`, the
/// count of copies seen), the player lacks it, and `Random(100)` is under
/// that count, it is theirs — told with the Trader's own wording moved up
/// by `0x2f` (`0x13a` a part, `0x13b` a hull), the item word as the
/// object. Failing that, six tries at a **field**: `Random(6)` names one,
/// and where the wreckage knew more than the player the cost of their next
/// level is credited to it (halved in a slow-tech game as the cost table
/// is), told with `idm`, object `-2`, the place, the field and the cost as
/// a long.
pub fn learn_from_battle(
    state: &mut crate::GameState,
    player: usize,
    place: [i16; 2],
    idm: u16,
    tech_seen: [u8; TECH_FIELDS],
    trader_seen: [u8; 13],
    rng: &mut Rng,
) -> Option<WreckageFind> {
    use crate::message::Message;
    let slow_tech = state.slow_tech;
    let who = state.players.get_mut(player)?;
    if who.learned_tech_this_year {
        return None;
    }
    if i32::from(rng.random(100)) <= 49 {
        return None;
    }
    for _ in 0..13 {
        let i = usize::try_from(rng.random(13)).unwrap_or(0);
        let bit = 1u16 << i;
        if trader_seen[i] == 0 || who.trader_parts & bit != 0 {
            continue;
        }
        if i32::from(rng.random(100)) >= i32::from(trader_seen[i]) {
            continue;
        }
        who.trader_parts |= bit;
        who.learned_tech_this_year = true;
        let (id, item) = crate::wormhole::part_gift(bit);
        state.messages.push(Message {
            player,
            id: id + 0x2f,
            object: item as i16,
            params: vec![place[0], place[1]],
        });
        return Some(WreckageFind::Part(bit));
    }
    for _ in 0..TECH_FIELDS {
        let field = usize::try_from(rng.random(6)).unwrap_or(0);
        let mine = who.research.levels[field];
        if mine >= tech_seen[field] {
            continue;
        }
        let resources = tech_level_cost(field, mine + 1, &who.research, &who.race, slow_tech);
        who.research.points[field] = who.research.points[field].saturating_add(resources);
        who.learned_tech_this_year = true;
        let [lo, hi] = Message::long(resources);
        state.messages.push(Message {
            player,
            id: idm,
            object: crate::message::RESEARCH_OBJECT,
            params: vec![
                place[0],
                place[1],
                i16::try_from(field).unwrap_or(0),
                lo,
                hi,
            ],
        });
        return Some(WreckageFind::Tech { field, resources });
    }
    None
}

/// The smallest research windfall an artifact can hold.
pub const ARTIFACT_MIN: i32 = 100;

/// The span of the roll above [`ARTIFACT_MIN`] (`Random(0x12d)`, so 0 to 300).
pub const ARTIFACT_SPAN: i16 = 0x12d;

/// The population, in units of 100, below which the windfall is scaled down.
pub const ARTIFACT_FULL_POP: i32 = 10;

/// What settling a planet turned up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Artifact {
    /// The technology field the find advances, 0 to 5.
    pub field: usize,
    /// Resources credited to that field's research.
    pub resources: i32,
}

/// The Mystery Trader artifact a newly settled planet yields.
///
/// Source: the tail of `DropColonists` (`10b8:4460`). A planet carrying
/// `fIsArtifact` gives its new owner a research windfall, and the flag is
/// cleared so it is found only once.
///
/// ```text
/// field     = Random(6)
/// resources = Random(301) + 100
/// if colonists < 10: resources = colonists * resources / 10
/// ```
///
/// Despite what the message looks like, this is **research**, not minerals:
/// the amount is added to `rgResSpent` for the chosen field, exactly as
/// [`learn_from_wreckage`] does. A thin first landing is worth proportionally
/// less, which is why the count is scaled below [`ARTIFACT_FULL_POP`].
///
/// The caller is responsible for the two gates the routine applies around
/// this: the planet must actually carry an artifact, and bit 7 of the game
/// options word must be clear — the option that switches artifacts off.
#[must_use]
pub fn artifact_bonus(colonists: i32, rng: &mut Rng) -> Artifact {
    let field = usize::try_from(rng.random(6)).unwrap_or(0);
    let mut resources = i32::from(rng.random(ARTIFACT_SPAN)) + ARTIFACT_MIN;
    if colonists < ARTIFACT_FULL_POP {
        resources = colonists.saturating_mul(resources) / ARTIFACT_FULL_POP;
    }
    Artifact { field, resources }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn landing(player: i16, colonists: i32, prt: Prt) -> Landing {
        Landing {
            player,
            colonists,
            prt: Some(prt),
            can_colonize: true,
        }
    }

    fn empty() -> Ground {
        Ground {
            id: 7,
            defender: None,
            starbase: false,
            pct_survive: 1.0,
        }
    }

    fn held(colonists: i32, prt: Prt) -> Ground {
        Ground {
            id: 7,
            defender: Some((0, colonists, Some(prt))),
            starbase: false,
            pct_survive: 1.0,
        }
    }

    fn outcome(ground: Ground, landings: &[Landing]) -> Outcome {
        resolve_landings(ground, landings).outcome
    }

    #[test]
    fn the_weights_are_the_ones_the_routine_uses() {
        assert_eq!(attack_weight(Some(Prt::Wm)), 165);
        assert_eq!(attack_weight(Some(Prt::Ar)), 0);
        assert_eq!(attack_weight(Some(Prt::Joat)), 110);
        assert_eq!(defence_weight(Some(Prt::Is)), 200);
        assert_eq!(defence_weight(Some(Prt::Joat)), 100);
    }

    #[test]
    fn an_empty_planet_goes_to_the_only_claimant() {
        let out = resolve_landings(empty(), &[landing(3, 250, Prt::Joat)]);
        assert_eq!(
            out.outcome,
            Outcome::Settled {
                player: 3,
                colonists: 250
            }
        );
        assert_eq!(out.messages.len(), 1);
        assert_eq!(out.messages[0].id, crate::message::id::COLONISTS_CONTROL);
    }

    /// An Alternate Reality race settles an empty planet with a power of
    /// nothing, and is told it has deployed its Orbital Construction
    /// Module; on a held planet its colonists die on the surface.
    #[test]
    fn alternate_reality_settles_but_never_invades() {
        let out = resolve_landings(empty(), &[landing(1, 5_000, Prt::Ar)]);
        assert_eq!(
            out.outcome,
            Outcome::Settled {
                player: 1,
                colonists: 5_000
            }
        );
        assert_eq!(out.messages[0].id, crate::message::id::COLONISTS_CONTROL_AR);
        let out = resolve_landings(held(10, Prt::Joat), &[landing(1, 5_000, Prt::Ar)]);
        assert_eq!(out.outcome, Outcome::Held { colonists: 10 });
        assert_eq!(out.messages[0].id, crate::message::id::LANDING_AR_DIED);
    }

    /// Colonists a cargo transfer put down on an empty planet die.
    #[test]
    fn a_forced_transport_onto_an_empty_planet_dies() {
        let mut drop = landing(1, 50, Prt::Joat);
        drop.can_colonize = false;
        let out = resolve_landings(empty(), &[drop]);
        assert_eq!(out.outcome, Outcome::Nothing);
        assert_eq!(
            out.messages[0].id,
            crate::message::id::LANDING_NOT_COLONISED
        );
        assert_eq!(out.messages[0].params, vec![50, 0, 7]);
    }

    /// A starbase over an inhabited planet kills every landing.
    #[test]
    fn a_starbase_stops_a_landing() {
        let mut ground = held(100, Prt::Joat);
        ground.starbase = true;
        let out = resolve_landings(ground, &[landing(1, 5_000, Prt::Wm)]);
        assert_eq!(out.outcome, Outcome::Held { colonists: 100 });
        assert_eq!(out.messages[0].id, crate::message::id::LANDING_STARBASE);
    }

    /// Contested: the heavier weighted claim wins, not the larger one —
    /// and, the rival having come first in player order, keeps only the
    /// margin: 100 × (16500 − 15400) / 16500 → 6, on a per-hundred
    /// power of 165 vs 154.
    #[test]
    fn a_war_monger_outweighs_a_larger_landing() {
        let out = resolve_landings(
            empty(),
            &[landing(2, 140, Prt::Joat), landing(1, 100, Prt::Wm)],
        );
        // Powers: player 1 = 165, player 2 = 154. Player 1 is seen first,
        // so player 2's 154 never becomes the "second" — the winner keeps
        // everything.
        assert_eq!(
            out.outcome,
            Outcome::Settled {
                player: 1,
                colonists: 100
            }
        );
        // The other way about, player 1 (165) comes after player 0
        // (154): 100 × (165 − 154) / 165 = 6.
        let out = resolve_landings(
            empty(),
            &[landing(0, 140, Prt::Joat), landing(1, 100, Prt::Wm)],
        );
        assert_eq!(
            out.outcome,
            Outcome::Settled {
                player: 1,
                colonists: 6
            }
        );
        let ids: Vec<u16> = out.messages.iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            vec![
                crate::message::id::LANDING_RACE_LOST,
                crate::message::id::LANDING_RACE_WON
            ]
        );
    }

    /// Equal powers: nobody survives.
    #[test]
    fn an_exact_tie_kills_everyone() {
        let out = resolve_landings(
            empty(),
            &[landing(1, 100, Prt::Joat), landing(2, 100, Prt::Joat)],
        );
        assert_eq!(out.outcome, Outcome::Annihilated { loser: None });
        assert!(out
            .messages
            .iter()
            .all(|m| m.id == crate::message::id::LANDING_FREE_FOR_ALL));
        assert_eq!(out.messages.len(), 2);
    }

    /// Inner Strength doubles the defence, which turns a losing fight.
    #[test]
    fn inner_strength_holds_a_planet_a_weaker_race_would_lose() {
        let attackers = [landing(2, 150, Prt::Joat)]; // 165 strength

        let joat = outcome(held(160, Prt::Joat), &attackers);
        assert!(
            matches!(joat, Outcome::Taken { player: 2, .. }),
            "160 defenders at 100% should fall to 165: {joat:?}"
        );

        let is = outcome(held(160, Prt::Is), &attackers);
        assert!(
            matches!(is, Outcome::Held { .. }),
            "160 defenders at 200% should hold against 165: {is:?}"
        );
    }

    /// A repelled invasion costs the defender `held × power / defence`.
    #[test]
    fn a_repelled_invasion_still_costs_the_defender() {
        let out = resolve_landings(held(1_000, Prt::Joat), &[landing(2, 100, Prt::Joat)]);
        // 1000 × 110 / 1000 = 110 lost.
        assert_eq!(out.outcome, Outcome::Held { colonists: 890 });
        let ids: Vec<u16> = out.messages.iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            vec![
                crate::message::id::LANDING_MASSACRED,
                crate::message::id::LANDING_REPELLED
            ]
        );
        assert_eq!(out.messages[0].params, vec![100, 0, 7, 0x30]);
    }

    /// A taken planet leaves the winner the share of its power that beat
    /// the defence: 200 × (220 − 100) / 220 = 109.
    #[test]
    fn a_taken_planet_keeps_the_surplus() {
        let out = resolve_landings(held(100, Prt::Joat), &[landing(2, 200, Prt::Joat)]);
        assert_eq!(
            out.outcome,
            Outcome::Taken {
                player: 2,
                colonists: 109,
                loser: 0
            }
        );
        let ids: Vec<u16> = out.messages.iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            vec![
                crate::message::id::LANDING_CRUSHED_DEFENDERS,
                crate::message::id::LANDING_STORMED
            ]
        );
    }

    /// Planetary defences take their share of the troops first — a
    /// quarter of what they would take from a bomb.
    #[test]
    fn defences_thin_the_landing() {
        let mut ground = held(100, Prt::Joat);
        // Bombs get 60% through; troops 60% + 40% / 4 = 70%.
        ground.pct_survive = 0.6;
        let out = resolve_landings(ground, &[landing(2, 200, Prt::Joat)]);
        // Power 220 × 0.7 = 154; 200 × (154 − 100) / 154 = 70.
        assert_eq!(
            out.outcome,
            Outcome::Taken {
                player: 2,
                colonists: 70,
                loser: 0
            }
        );
        // And a repelled landing says how much the defences shot down.
        let out = resolve_landings(ground, &[landing(2, 100, Prt::Joat)]);
        assert_eq!(out.messages[0].id, crate::message::id::LANDING_SHOT_DOWN);
        assert_eq!(out.messages[0].params, vec![100, 0, 7, -3000, 0x30]);
    }

    /// The two races that filter their inherited queue, and what they drop.
    #[test]
    fn the_queue_template_filter_matches_the_routine() {
        use crate::production::item;

        // Alternate Reality builds no planetary installation at all.
        for i in [item::AUTO_MINE, item::AUTO_FACTORY, item::AUTO_DEFENSE] {
            assert!(
                !template_allows(Some(Prt::Ar), i),
                "AR should drop item {i}"
            );
        }
        assert!(template_allows(Some(Prt::Ar), item::AUTO_ALCHEMY));

        // Claim Adjuster terraforms for free, so it queues no terraforming.
        for i in [item::AUTO_MIN_TERRAFORM, item::AUTO_MAX_TERRAFORM] {
            assert!(
                !template_allows(Some(Prt::Ca), i),
                "CA should drop item {i}"
            );
        }
        assert!(template_allows(Some(Prt::Ca), item::AUTO_FACTORY));

        // Everyone else takes the template as it stands.
        for i in 0..=12u16 {
            assert!(template_allows(Some(Prt::Joat), i));
        }
    }

    /// Salvage never fires twice in a turn, and never from a loser who knew
    /// less.
    #[test]
    fn wreckage_only_teaches_what_the_loser_knew_better() {
        use crate::research::Research;

        let race = Race::humanoid();
        let winner = Research {
            levels: [3, 3, 3, 3, 3, 3],
            ..Research::default()
        };

        // Already learned this turn: nothing, whatever the wreck holds.
        let mut rng = Rng::randomize(1);
        assert_eq!(
            learn_from_wreckage(&winner, &race, [9; 6], true, false, &mut rng),
            None
        );

        // A loser who knew no more teaches nothing, however many seeds we try.
        for seed in 0..50u32 {
            let mut rng = Rng::randomize(seed);
            assert_eq!(
                learn_from_wreckage(&winner, &race, [3; 6], false, false, &mut rng),
                None,
                "seed {seed}"
            );
        }

        // A better-informed loser eventually teaches something, and only in a
        // field it actually led in.
        let mut taught = 0;
        for seed in 0..200u32 {
            let mut rng = Rng::randomize(seed);
            let loser = [9, 3, 3, 3, 3, 3];
            if let Some(s) = learn_from_wreckage(&winner, &race, loser, false, false, &mut rng) {
                assert_eq!(s.field, 0, "only field 0 was ahead");
                assert!(
                    s.resources > 0,
                    "the credit should be the next level's cost"
                );
                taught += 1;
            }
        }
        assert!(
            taught > 20,
            "expected salvage to fire sometimes, got {taught}"
        );
        assert!(taught < 200, "and not always");
    }

    /// The windfall is a research credit in one field, scaled down for a thin
    /// first landing.
    #[test]
    fn an_artifact_pays_research_and_scales_with_the_landing() {
        for seed in 0..40u32 {
            let mut rng = Rng::randomize(seed);
            let full = artifact_bonus(5_000, &mut rng);
            assert!(full.field < 6, "a field of six: {full:?}");
            assert!(
                (ARTIFACT_MIN..=ARTIFACT_MIN + i32::from(ARTIFACT_SPAN)).contains(&full.resources),
                "{full:?}"
            );
        }

        // A landing of one hundredth the threshold is worth a tenth as much.
        let mut a = Rng::randomize(7);
        let mut b = Rng::randomize(7);
        let big = artifact_bonus(ARTIFACT_FULL_POP, &mut a);
        let small = artifact_bonus(1, &mut b);
        assert_eq!(small.field, big.field, "the same roll picks the same field");
        assert_eq!(small.resources, big.resources / 10);
    }

    #[test]
    fn nothing_lands_nothing_happens() {
        assert_eq!(outcome(empty(), &[]), Outcome::Nothing);
        // A held planet with nothing landing keeps everyone it has.
        assert_eq!(
            outcome(held(500, Prt::Joat), &[]),
            Outcome::Held { colonists: 500 }
        );
    }

    /// A seed whose first draw does what the closure wants.
    fn seed_where(mut want: impl FnMut(&mut Rng) -> bool) -> u32 {
        (1..100_000u32)
            .find(|s| want(&mut Rng::randomize(*s)))
            .expect("a seed")
    }

    /// Wreckage that knew more teaches a field once a year, credited at the
    /// cost of the player's next level and told with the place; wreckage
    /// carrying a Trader part can hand the part over instead.
    #[test]
    fn wreckage_teaches_once_a_year() {
        let mut state = crate::GameState::new(1);
        state
            .players
            .push(crate::Player::new(crate::Race::humanoid()));
        // Past the coin's toss, and the field then drawn is one the
        // wreckage knew better.
        let seed = seed_where(|rng| rng.random(100) > 49);
        let mut rng = Rng::randomize(seed);
        let find = learn_from_battle(
            &mut state,
            0,
            [1200, 1300],
            crate::message::id::WRECKAGE_BOOSTED_RESEARCH,
            [5; 6],
            [0; 13],
            &mut rng,
        )
        .expect("something learned");
        let WreckageFind::Tech { field, resources } = find else {
            panic!("a field, not a part: {find:?}");
        };
        let expect = tech_level_cost(field, 1, &Research::default(), &Race::humanoid(), false);
        assert_eq!(resources, expect);
        assert_eq!(state.players[0].research.points[field], expect);
        let msg = state.messages.last().expect("told");
        assert_eq!(msg.id, crate::message::id::WRECKAGE_BOOSTED_RESEARCH);
        assert_eq!(msg.object, crate::message::RESEARCH_OBJECT);
        assert_eq!(&msg.params[..3], &[1200, 1300, field as i16]);
        assert!(state.players[0].learned_tech_this_year);

        // Nothing more this year, whatever the wreckage.
        assert!(learn_from_battle(
            &mut state,
            0,
            [1200, 1300],
            crate::message::id::WRECKAGE_BOOSTED_RESEARCH,
            [9; 6],
            [25; 13],
            &mut Rng::randomize(seed),
        )
        .is_none());

        // Next year, wreckage full of Langston Shells: the part is found on
        // a seed whose draws land on it.
        state.players[0].learned_tech_this_year = false;
        let seed =
            seed_where(|rng| rng.random(100) > 49 && rng.random(13) == 2 && rng.random(100) < 25);
        let mut trader_seen = [0u8; 13];
        trader_seen[2] = 25;
        let find = learn_from_battle(
            &mut state,
            0,
            [-1, 7],
            crate::message::id::WRECKAGE_BOOSTED_RESEARCH,
            [0; 6],
            trader_seen,
            &mut Rng::randomize(seed),
        );
        assert_eq!(
            find,
            Some(WreckageFind::Part(crate::wormhole::part::SHIELD))
        );
        assert_ne!(
            state.players[0].trader_parts & crate::wormhole::part::SHIELD,
            0
        );
        let msg = state.messages.last().expect("told");
        assert_eq!(msg.id, crate::message::id::WRECKAGE_PLANS_PART);
        assert_eq!(msg.params, vec![-1, 7]);
    }
}
