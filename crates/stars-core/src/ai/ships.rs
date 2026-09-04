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
