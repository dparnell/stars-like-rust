//! What a computer player puts in a planet's queue that flies.
//!
//! Ship and starbase orders differ from the installations in
//! [`super::production`] in one practical way: a ship takes several turns to
//! pay for, so its queue entry survives long enough to be recorded. That makes
//! these decisions checkable against the queue itself, which the mine and
//! factory decision is not.
//!
//! Source: `QueueAiStarbases` (`1090:8524`), reached from every personality's
//! turn routine.

use crate::planet::Planet;
use crate::production::QueueItem;

use super::production::Context;
use super::AiPersonality;

/// Design slots at or above this are starbases rather than ships.
///
/// `QueueAiStarbases` queues starbase design `n` as `AddItemToQueue(n + 0x10,
/// 1, grobjFleet, 1)`, so the 16 ship designs occupy slots 0-15 and the 10
/// starbase designs 16-25. Every ship entry in the fixtures falls in that
/// range.
pub const STARBASE_SLOT_BASE: u16 = 0x10;

/// The population, in units of 100 colonists, a planet must exceed before the
/// AI will give it a starbase (`1090:8672` tests against `0x4f`).
pub const STARBASE_MIN_POP: i32 = 79;

/// The highest real starbase design index.
///
/// `QueueAiStarbases` accepts a planet whose starbase design index is *above*
/// this (`1090:865a` tests `9 < isb & 0xf`), which is how it treats a slot
/// holding no usable starbase. Every starbase index in the fixtures is 0-9, so
/// that arm is never taken there.
pub const STARBASE_LAST_DESIGN: u8 = 9;

/// Whether a queue entry is a starbase order.
#[must_use]
pub fn is_starbase_slot(item: u16) -> bool {
    item >= STARBASE_SLOT_BASE
}

/// The AI's per-planet state that this decision reads and `stars-core` does
/// not model.
///
/// `QueueAiStarbases` consults two scratch structures that live only for the
/// duration of an AI turn: `vlpbAiPlanet`, a 16-byte record per planet whose
/// third byte gates starbase building, and `vlpbAiData`, a list of the planets
/// the AI is actively working on. Neither is written to a save file, so
/// neither can be recovered from the fixtures; the caller supplies them.
#[derive(Debug, Clone, Copy)]
pub struct PlanetTask {
    /// `vlpbAiPlanet[id * 16 + 2] == 0` — the planet is not otherwise spoken
    /// for. Defaults to true, which is the permissive reading.
    pub free: bool,
    /// Whether the planet appears in the AI's working list `vlpbAiData`.
    /// Defaults to true.
    pub tracked: bool,
}

impl Default for PlanetTask {
    fn default() -> Self {
        Self {
            free: true,
            tracked: true,
        }
    }
}

/// The starbase design slot to append to this planet's queue, if any.
///
/// Source: `QueueAiStarbases` (`1090:8524`). It declines when:
///
/// - the player has no starbase design yet (`ishdefSBLatest == -1`);
/// - the player is Macinti, which builds its starbases elsewhere;
/// - the planet already has a starbase with a real design index (0 to
///   [`STARBASE_LAST_DESIGN`]) — this routine only equips planets that have
///   none. **Replacing** an existing starbase is `FUpgradeAiStarbase`'s job,
///   not this one, and is not modelled: it is gated on `Random(100)` rolls and
///   its Macinti arm walks a per-turn recycling table (`vAiMacRecycleSB`), so
///   it can only be checked statistically. Of the 2440 starbase orders in the
///   corpus, 1203 go to planets with no starbase (this routine) and 934 are
///   upgrades to a newer design (that one);
/// - the planet's population is not above [`STARBASE_MIN_POP`];
/// - the AI's own scratch state says the planet is busy or untracked;
/// - the queue already holds a starbase order.
///
/// Exactly one is queued, with a count of one. Cyber picks its design through
/// `iBuildCyberStarbase` (`1090:8...`) instead of taking the newest, which is
/// not modelled; pass the slot that routine would choose.
///
/// `latest_starbase` is the player's newest starbase *design index* (0-9), not
/// a queue slot; the returned value has [`STARBASE_SLOT_BASE`] added.
#[must_use]
pub fn queue_ai_starbase(
    planet: &Planet,
    ctx: &Context,
    latest_starbase: Option<u8>,
    task: PlanetTask,
) -> Option<u16> {
    let design = latest_starbase?;
    if ctx.personality == Some(AiPersonality::Macinti) {
        return None;
    }
    if planet.pop <= STARBASE_MIN_POP {
        return None;
    }
    if !task.free || !task.tracked {
        return None;
    }
    // A planet that already holds a real starbase is left to the upgrade path.
    if planet.starbase
        && planet
            .starbase_design
            .is_none_or(|d| d <= STARBASE_LAST_DESIGN)
    {
        return None;
    }
    if planet
        .queue
        .iter()
        .any(|e| e.ship && is_starbase_slot(e.item))
    {
        return None;
    }
    Some(u16::from(design) + STARBASE_SLOT_BASE)
}

/// The queue entry [`queue_ai_starbase`] appends, ready to push onto the
/// planet's queue.
#[must_use]
pub fn starbase_entry(slot: u16) -> QueueItem {
    QueueItem {
        count: 1,
        item: slot,
        ship: true,
        completion: 0,
    }
}

/// The starbase hull types, which the upgrade arithmetic works in.
///
/// The ten starbase design slots are two banks of five: slot `n` and slot
/// `n + 5` are the same hull. `FUpgradeAiStarbase` works in `isb % 5`
/// throughout, which is the hull type, and the four slots it singles out —
/// 1, 3, 6 and 8 — are hull types 1 and 3 in both banks.
pub const STARBASE_HULL_TYPES: u8 = 5;

/// The hull types that take the orbital-fort branch of the upgrade decision
/// (`1090:88b6` tests the design index against 1, 3, 6 and 8).
#[must_use]
pub fn is_orbital_fort_family(design: u8) -> bool {
    matches!(design, 1 | 3 | 6 | 8)
}

/// Everything outside the planet that `FUpgradeAiStarbase` reads.
///
/// Several of these are per-turn scratch or design-table fields that are never
/// written to a save file, so they cannot come from a fixture.
#[derive(Debug, Clone, Copy)]
pub struct UpgradeInputs {
    /// The player's newest starbase design index (`ishdefSBLatest`).
    pub latest: u8,
    /// The newest design in the orbital-fort family
    /// (`IshdefAiSBLatestOF`), used when the current base is one of those.
    pub latest_orbital_fort: u8,
    /// The turn on which the design being upgraded to was created — the field
    /// at offset `0x7d` of its `0x93`-byte design record. It sets how eager the
    /// AI is: an old design is replaced more readily.
    pub design_turn: i32,
    /// Bit 9 at offset `0x7b` of the design record for index `isb + 2`, which
    /// must be **clear** for the sideways move at `1090:8952`. What the bit
    /// means is not recovered; it is taken from the caller rather than guessed.
    pub sideways_design_free: bool,
    /// The Macinti recycling table `vAiMacRecycleSB`, indexed by starbase
    /// design. Values seen are 0 to 3; 3 blocks an upgrade outright and 2
    /// blocks it nine times in ten.
    pub recycle: [u8; 10],
    /// `PctPlanetCapacity` — how full the planet is, in percent. Only the
    /// Macinti branch reads it.
    pub capacity_pct: i16,
}

impl Default for UpgradeInputs {
    fn default() -> Self {
        Self {
            latest: 0,
            latest_orbital_fort: 0,
            design_turn: 0,
            sideways_design_free: true,
            recycle: [0; 10],
            capacity_pct: 100,
        }
    }
}

/// The minimum of each surface mineral a planet needs before the AI will make
/// the sideways move (`1090:8978` compares each against 200).
pub const SIDEWAYS_MIN_SURFACE: i32 = 200;

/// The turn before which Cyber never replaces a starbase (`1090:88a2`).
pub const CYBER_UPGRADE_FIRST_TURN: i32 = 40;

/// Decide whether to replace a planet's existing starbase, and with what.
///
/// Source: `FUpgradeAiStarbase` (`1090:882a`). Returns the design *slot* to
/// queue — the index plus [`STARBASE_SLOT_BASE`] — or `None`.
///
/// # This decision is probabilistic
///
/// Every arm is gated on `Random(100)`, so a single call cannot be checked
/// against a recorded game without the RNG in the same state. What the corpus
/// confirms is the arithmetic each arm uses once it fires, which is
/// deterministic given the planet's current design; see `docs/formulas/ai.md`.
///
/// The RNG is consumed in the same order and the same number of times as the
/// original, so a caller replaying a turn stays in step.
pub fn upgrade_ai_starbase(
    planet: &Planet,
    ctx: &Context,
    inputs: &UpgradeInputs,
    rng: &mut crate::rng::Rng,
) -> Option<u16> {
    if !planet.starbase {
        return None;
    }
    // An order already in flight stops everything, whichever routine placed it.
    if planet
        .queue
        .iter()
        .any(|e| e.ship && is_starbase_slot(e.item))
    {
        return None;
    }
    let isb = planet.starbase_design?;

    if ctx.personality == Some(AiPersonality::Macinti) {
        return macinti_upgrade(isb, inputs, rng);
    }
    if ctx.personality == Some(AiPersonality::Cyber) && ctx.turn < CYBER_UPGRADE_FIRST_TURN {
        return None;
    }

    // Designs in the orbital-fort family are measured against the newest of
    // that family, and span two hull types rather than three.
    let orbital_fort = is_orbital_fort_family(isb);
    let (span, latest) = if orbital_fort {
        (2i32, inputs.latest_orbital_fort)
    } else {
        (3i32, inputs.latest)
    };

    let outdated = isb < latest || i32::from(latest) + (span - 1) * 2 < i32::from(isb);
    if outdated {
        // The older the design being replaced, the likelier the AI is to act:
        // no eagerness for the first ten turns of its life, then half the
        // elapsed turns, and a flat five percent on top.
        let mut eagerness = (ctx.turn - inputs.design_turn) - 10;
        if eagerness < 0 {
            eagerness = 0;
        } else if eagerness < 50 {
            eagerness /= 2;
        }
        if i32::from(rng.random(100)) < eagerness + 5 {
            let hull = i32::from(isb % STARBASE_HULL_TYPES);
            let base = i32::from(latest) + hull - i32::from(orbital_fort);
            return u8::try_from(base).ok().map(slot_for);
        }
        return None;
    }

    // Otherwise the AI may still move sideways to the next design up, but only
    // rarely, and only on a planet with minerals to spare.
    let hull = i32::from(isb % STARBASE_HULL_TYPES);
    if hull < (span - 1) * 2
        && inputs.sideways_design_free
        && i32::from(rng.random(100)) < 6
        && planet
            .surface_min
            .iter()
            .all(|m| *m >= SIDEWAYS_MIN_SURFACE)
    {
        return Some(u16::from(isb) + STARBASE_SLOT_BASE + 2);
    }
    None
}

/// Macinti's own starbase cycling (`1090:89b6`).
///
/// Macinti players in the corpus are Alternate Reality races, which live on
/// their starbases, so they replace them far more often than anyone else.
fn macinti_upgrade(isb: u8, inputs: &UpgradeInputs, rng: &mut crate::rng::Rng) -> Option<u16> {
    let recycling = inputs.recycle.get(usize::from(isb)).copied().unwrap_or(0);

    // A design marked 3 is never replaced; one marked 2 is replaced only one
    // time in ten.
    let blocked = recycling == 3 || (recycling == 2 && rng.random(100) <= 9);
    if !blocked {
        // A one-in-twelve nudge to the very next design, for the hull types
        // that have one above them.
        if isb > 3 && isb != 6 && isb != 9 && rng.random(100) < 8 {
            return Some(u16::from(isb + 1) + STARBASE_SLOT_BASE);
        }
        // Otherwise a crowded planet may still justify the jump below.
        if isb > 3 {
            return None;
        }
        let capacity = i32::from(inputs.capacity_pct);
        if capacity < 16 || i32::from(rng.random(100)) >= (capacity - 15) * 6 {
            return None;
        }
    }

    let next = if isb < 4 {
        // Walk up to the first design the recycling table has not claimed.
        let mut n = isb;
        loop {
            n += 1;
            if n > 8 || inputs.recycle.get(usize::from(n)).copied().unwrap_or(0) == 0 {
                break;
            }
        }
        n
    } else if isb + 3 > 9 {
        isb - 3
    } else {
        isb + 3
    };
    Some(slot_for(next))
}

/// The queue slot for a starbase design index.
#[must_use]
pub fn slot_for(design: u8) -> u16 {
    u16::from(design) + STARBASE_SLOT_BASE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::production::Context;

    fn colony(pop: i32) -> Planet {
        let mut p = Planet::unowned(0);
        p.owner = Some(0);
        p.pop = pop;
        p
    }

    #[test]
    fn a_planet_with_no_starbase_gets_the_newest_design() {
        let planet = colony(5_000);
        let slot = queue_ai_starbase(&planet, &Context::default(), Some(5), PlanetTask::default());
        assert_eq!(slot, Some(0x15));
        assert!(is_starbase_slot(0x15));
    }

    #[test]
    fn the_population_gate_and_the_macinti_exclusion_hold() {
        let ctx = Context::default();
        let task = PlanetTask::default();

        assert_eq!(
            queue_ai_starbase(&colony(STARBASE_MIN_POP), &ctx, Some(0), task),
            None
        );
        assert!(queue_ai_starbase(&colony(STARBASE_MIN_POP + 1), &ctx, Some(0), task).is_some());

        let macinti = Context {
            personality: Some(AiPersonality::Macinti),
            ..ctx
        };
        assert_eq!(
            queue_ai_starbase(&colony(5_000), &macinti, Some(0), task),
            None
        );
    }

    #[test]
    fn only_one_starbase_is_ever_queued() {
        let mut planet = colony(5_000);
        planet.queue.push(starbase_entry(0x12));
        assert_eq!(
            queue_ai_starbase(&planet, &Context::default(), Some(3), PlanetTask::default()),
            None
        );
    }

    /// A planet holding a real starbase is left to the upgrade path; a slot
    /// holding an index outside 0-9 is treated as having none.
    #[test]
    fn an_existing_starbase_is_left_to_the_upgrade_path() {
        let ctx = Context::default();
        let task = PlanetTask::default();

        let mut current = colony(5_000);
        current.starbase = true;
        current.starbase_design = Some(STARBASE_LAST_DESIGN);
        assert_eq!(queue_ai_starbase(&current, &ctx, Some(4), task), None);

        let mut outdated = current.clone();
        outdated.starbase_design = Some(STARBASE_LAST_DESIGN + 1);
        assert_eq!(
            queue_ai_starbase(&outdated, &ctx, Some(4), task),
            Some(0x14)
        );
    }

    /// A Macinti design the recycling table has claimed jumps three hulls,
    /// wrapping downward past 9. Every such pair in the corpus — (4,7), (5,8),
    /// (6,9), (7,4), (8,5), (9,6) — follows this.
    #[test]
    fn macinti_recycling_jumps_three_designs() {
        let ctx = Context {
            personality: Some(AiPersonality::Macinti),
            ..Context::default()
        };
        for (have, want) in [(4u8, 7u16), (5, 8), (6, 9), (7, 4), (8, 5), (9, 6)] {
            let mut planet = colony(5_000);
            planet.starbase = true;
            planet.starbase_design = Some(have);
            let mut recycle = [0u8; 10];
            recycle[usize::from(have)] = 3; // claimed outright
            let inputs = UpgradeInputs {
                recycle,
                ..UpgradeInputs::default()
            };
            let mut rng = crate::rng::Rng::randomize(1);
            assert_eq!(
                upgrade_ai_starbase(&planet, &ctx, &inputs, &mut rng),
                Some(slot_for(u8::try_from(want).unwrap())),
                "design {have}"
            );
        }
    }

    /// Left to itself, Macinti only ever nudges to the next design up, and
    /// only from the four hull slots that have one. Designs 6 and 9 never do.
    #[test]
    fn macinti_nudges_only_from_four_designs() {
        let ctx = Context {
            personality: Some(AiPersonality::Macinti),
            ..Context::default()
        };
        for have in 4u8..=9 {
            let mut planet = colony(5_000);
            planet.starbase = true;
            planet.starbase_design = Some(have);
            let inputs = UpgradeInputs::default();

            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..400u32 {
                let mut rng = crate::rng::Rng::randomize(seed);
                seen.insert(upgrade_ai_starbase(&planet, &ctx, &inputs, &mut rng));
            }
            let expected: std::collections::BTreeSet<_> = if matches!(have, 4 | 5 | 7 | 8) {
                [None, Some(slot_for(have + 1))].into_iter().collect()
            } else {
                [None].into_iter().collect()
            };
            assert_eq!(seen, expected, "design {have}");
        }
    }

    /// A planet with no starbase, or one that already has an order in flight,
    /// is not this routine's business.
    #[test]
    fn the_upgrade_declines_what_is_not_its_job() {
        let ctx = Context::default();
        let inputs = UpgradeInputs::default();
        let mut rng = crate::rng::Rng::randomize(1);

        let bare = colony(5_000);
        assert_eq!(upgrade_ai_starbase(&bare, &ctx, &inputs, &mut rng), None);

        let mut queued = colony(5_000);
        queued.starbase = true;
        queued.starbase_design = Some(0);
        queued.queue.push(starbase_entry(0x14));
        assert_eq!(upgrade_ai_starbase(&queued, &ctx, &inputs, &mut rng), None);
    }

    #[test]
    fn the_ai_scratch_state_can_veto() {
        let planet = colony(5_000);
        let ctx = Context::default();
        for task in [
            PlanetTask {
                free: false,
                tracked: true,
            },
            PlanetTask {
                free: true,
                tracked: false,
            },
        ] {
            assert_eq!(queue_ai_starbase(&planet, &ctx, Some(1), task), None);
        }
    }
}
