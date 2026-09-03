//! What a computer player puts in a planet's production queue.
//!
//! Source: `FFillProdMinesAndFactories` (`10a8:2d72`), called once per planet
//! by `FillProductionQueue` (`10a8:2ce2`), which is in turn called from each
//! personality's turn routine. The AI's queue is almost always a single
//! auto-build entry, because this function is the only thing that writes to it
//! on most turns.
//!
//! The decompilation of `FFillProdMinesAndFactories` loses the arguments to the
//! bitfield helper at `1118:0e32` at every call site, which made the item tests
//! unreadable until the queue-entry layout was pinned down from
//! `AddItemToQueue` (see `docs/formats/production.md`). With that layout the
//! two shifts are unambiguous: `>> 0x11 & 7` is the entry's `GrobjClass` and
//! `>> 0xa & 0x7f` is its item id.

use crate::mining::{minerals_mined, mines_operating};
use crate::planet::Planet;
use crate::production::{item, planetary_item_cost, QueueItem, COST_PARTS};
use crate::race::Race;
use crate::resources::{
    factories_operating, max_operable_factories, max_operable_mines, resources_at_planet,
};

use super::AiPersonality;

/// The parts of the game state outside the planet that the decision reads.
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// Which opponent is playing. Only Macinti takes the terraform branch.
    pub personality: Option<AiPersonality>,
    /// The player's research allocation, in percent. Resources handed to
    /// research are not available to build with.
    pub research_pct: u8,
    /// The player's six technology levels.
    pub tech: [u8; 6],
    /// The turn number, counting from 0 at year 2400. Mineral alchemy is only
    /// ever queued past turn 100.
    pub turn: i32,
    /// How many terraform steps the planet still has available. Only the
    /// Macinti branch uses this; zero disables it, which is also the right
    /// answer for a planet that is already at its race's ideal.
    pub terraform_steps: i32,
    /// Whether factories cost all three minerals rather than germanium alone
    /// (bit 11 of the game flags word tested at `10a8:3130`). Off in every
    /// game in `fixtures/`.
    pub factories_cost_all_minerals: bool,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            personality: None,
            research_pct: 15,
            tech: [0; 6],
            turn: 0,
            terraform_steps: 0,
            factories_cost_all_minerals: false,
        }
    }
}

/// The technology level every field must reach before the AI will build
/// mineral alchemy (`10a8:3690` compares each of the six against `0x1a`).
const ALCHEMY_TECH_LEVEL: u8 = 26;

/// The turn past which mineral alchemy may be queued (`100 < game.turn`).
const ALCHEMY_FIRST_TURN: i32 = 100;

/// What one call to the routine decided to add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Decision {
    /// Auto-build factories to append to the queue.
    pub factories: i32,
    /// Auto-build mines to insert at the front of the queue.
    pub mines: i32,
    /// Auto-build mineral alchemy.
    pub alchemy: i32,
    /// Auto-build terraforming, from the Macinti branch. When this is set the
    /// routine returns immediately and the other three are zero.
    pub terraform: i32,
}

impl Decision {
    /// Whether anything was added — the routine's `int16_t` return value.
    #[must_use]
    pub fn changed(self) -> bool {
        self.factories + self.mines + self.alchemy + self.terraform > 0
    }

    /// The decision as queue entries, in the order `AddItemToQueue` leaves
    /// them: mines are inserted at the front, factories appended.
    #[must_use]
    pub fn entries(self) -> Vec<QueueItem> {
        let mut out = Vec::new();
        let mut push = |item, count| {
            if count > 0 {
                out.push(QueueItem {
                    count,
                    item,
                    ship: false,
                    completion: 0,
                });
            }
        };
        push(item::AUTO_TERRAFORM, self.terraform);
        push(item::AUTO_MINE, self.mines);
        push(item::AUTO_FACTORY, self.factories);
        push(item::AUTO_ALCHEMY, self.alchemy);
        out
    }
}

/// What the planet can spend this year: `[ironium, boranium, germanium,
/// resources]`.
///
/// Source: `GetResourcesAvailable` (`1090:56d0`). Minerals are next year's
/// estimated mining plus what is already on the surface; resources are the
/// planet's output less the player's research share. The mining estimate
/// truncates rather than rolling the leftover hundredths, which is why
/// [`minerals_mined`] is called with no RNG.
#[must_use]
pub fn resources_available(planet: &Planet, race: &Race, research_pct: u8) -> [i32; COST_PARTS] {
    let mined = minerals_mined(planet, race, None, None);
    let mut out = [0i32; COST_PARTS];
    for (slot, (mined, surface)) in out
        .iter_mut()
        .zip(mined.iter().zip(planet.surface_min.iter()))
    {
        *slot = mined.saturating_add(*surface);
    }

    let mut resources = i32::from(resources_at_planet(planet, race).unwrap_or(0));
    if !planet.no_research {
        resources -= resources * i32::from(research_pct) / 100;
    }
    out[3] = resources;
    out
}

/// What the queue already committed, `[ironium, boranium, germanium,
/// resources]`.
///
/// Source: `GetProdQCost` (`1090:57c0`), which sums `GetProductionCosts` over
/// every entry. Ship entries are skipped here: the AI's own queues hold only
/// planetary items, and costing a design needs the player's design list.
#[must_use]
pub fn queue_cost(queue: &[QueueItem], race: &Race) -> [i32; COST_PARTS] {
    let mut out = [0i32; COST_PARTS];
    for entry in queue {
        if entry.ship {
            continue;
        }
        let base = item::auto_builds(entry.item).unwrap_or(entry.item);
        let Some(cost) = planetary_item_cost(base, race, false) else {
            continue;
        };
        for (slot, unit) in out.iter_mut().zip(cost.minerals.iter()) {
            *slot = slot.saturating_add(unit.saturating_mul(entry.count));
        }
        out[3] = out[3].saturating_add(cost.resources.saturating_mul(entry.count));
    }
    out
}

/// How many of an item `resources` will buy, guarding the zero-cost case.
fn affordable(resources: i32, unit: i32) -> i32 {
    if unit <= 0 {
        return 0;
    }
    resources / unit
}

/// Decide what to add to one planet's production queue.
///
/// Source: `FFillProdMinesAndFactories` (`10a8:2d72`). In outline:
///
/// 1. Work out what is left after the existing queue is paid for, and give up
///    if no resources remain.
/// 2. Cap mines and factories at what the planet will be able to *operate*
///    next year, less what it already has running and what the queue already
///    holds.
/// 3. If any mineral has run out, build mines; otherwise build factories
///    first and then mines with whatever resources survive.
/// 4. Queue mineral alchemy only once every technology has reached 26 and the
///    game is past turn 100.
///
/// # This transcription is not verified
///
/// Scored against `fixtures/games/all-computer-players` — 101 turns of sixteen
/// computer players — the original queued mines or factories on **85**
/// planet-turns. This function would queue them on **11,825**: it fires about
/// 140 times too often, and where the original does queue, it almost always
/// queues one at a time while this predicts far larger batches.
///
/// The decision logic above is a faithful reading of the routine. What is
/// missing is the gate in front of it: `FillProductionQueue` (`10a8:2ce2`)
/// walks `vrglpplAi[0..vclpplAi]`, a working list of planets the personality
/// routine selects, not every planet the player owns. Until that selection is
/// recovered this cannot be scored properly, so nothing asserts it — see
/// `docs/formulas/ai.md`.
#[must_use]
pub fn fill_prod_mines_and_factories(planet: &Planet, race: &Race, ctx: &Context) -> Decision {
    let mut decision = Decision::default();

    let available = resources_available(planet, race, ctx.research_pct);
    let committed = queue_cost(&planet.queue, race);
    let mut left = [0i32; COST_PARTS];
    for i in 0..COST_PARTS {
        left[i] = available[i].saturating_sub(committed[i]);
    }

    if left[3] <= 0 {
        return decision;
    }

    // What the queue already holds, so the caps below are not spent twice.
    let queued = |want: u16| -> i32 {
        planet
            .queue
            .iter()
            .filter(|e| !e.ship && e.item == want)
            .map(|e| e.count)
            .sum()
    };
    let mines_queued = queued(item::AUTO_MINE);
    let factories_queued = queued(item::AUTO_FACTORY);

    let mines_wanted = (i32::from(max_operable_mines(planet, race, true))
        - i32::from(mines_operating(planet, race))
        - mines_queued)
        .max(0);
    let factories_wanted = (i32::from(max_operable_factories(planet, race, true))
        - i32::from(factories_operating(planet, race))
        - factories_queued)
        .max(0);

    let cost_mine = planetary_item_cost(item::MINE, race, false).unwrap_or_default();
    let cost_factory = planetary_item_cost(item::FACTORY, race, false).unwrap_or_default();
    let cost_alchemy = planetary_item_cost(item::ALCHEMY, race, false).unwrap_or_default();

    // Macinti spends spare resources on terraforming instead, and returns as
    // soon as it queues any. The divisor is the terraform cost the routine
    // hard-codes at `10a8:3050`, rounded up.
    if ctx.terraform_steps > 0 && ctx.personality == Some(AiPersonality::Macinti) {
        let n = ((left[3] + 69) / 70).min(ctx.terraform_steps);
        if n > 0 {
            decision.terraform = n;
            return decision;
        }
    }

    // A mineral has run out: mines are the only thing worth building.
    let mineral_short = left[0] <= 0 || left[1] <= 0 || left[2] <= 0;
    let mut resources = left[3];

    if mineral_short {
        // Do not fight an alchemy order that is already at the head of the
        // queue — it is there precisely to fix this shortage.
        if let Some(first) = planet.queue.first() {
            if !first.ship && (first.item == item::ALCHEMY || first.item == item::AUTO_ALCHEMY) {
                return decision;
            }
        }
        if mines_wanted > 0 {
            decision.mines = mines_wanted.min(affordable(resources, cost_mine.resources));
            resources -= decision.mines * cost_mine.resources;
        }
    } else {
        let mut factories = factories_wanted;
        if factories > 0 {
            // Factories are germanium-limited, unless the game charges all
            // three minerals for them.
            let by_minerals = if ctx.factories_cost_all_minerals {
                (0..3)
                    .map(|i| affordable(left[i], cost_factory.minerals[i]))
                    .min()
                    .unwrap_or(0)
            } else {
                affordable(left[2], cost_factory.minerals[2])
            };
            factories = factories.min(by_minerals);
        }
        if factories > 0 {
            decision.factories = factories.min(affordable(resources, cost_factory.resources));
            resources -= decision.factories * cost_factory.resources;
        }
        if mines_wanted > 0 {
            decision.mines = mines_wanted.min(affordable(resources, cost_mine.resources));
            resources -= decision.mines * cost_mine.resources;
        }
    }

    // Mineral alchemy is a late-game sink for resources a fully researched
    // empire has nothing better to do with.
    if cost_alchemy.resources > 0
        && ctx.tech.iter().all(|t| *t == ALCHEMY_TECH_LEVEL)
        && ctx.turn > ALCHEMY_FIRST_TURN
    {
        decision.alchemy = affordable(resources, cost_alchemy.resources) + 1;
    }

    decision
}

/// The most auto-terraforming the AI will queue in one go (`10a8:8e6a`
/// clamps the catalogue's available count to 4).
pub const MAX_TERRAFORM_QUEUED: i32 = 4;

/// The population, in units of 100 colonists, a planet must exceed before the
/// AI will terraform it (`1090:8d4a`).
pub const TERRAFORM_MIN_POP: i32 = 199;

/// Decide how much auto-terraforming to add to a planet's queue.
///
/// Source: `FQueueAiTerraforming` (`1090:8d28`), reached from every
/// personality through `HandleBasicAiTasks` (`1090:95a4`). It refuses if:
///
/// - the player is Cyber;
/// - the planet's population is not above [`TERRAFORM_MIN_POP`];
/// - the queue already holds an auto-terraform entry;
/// - no environment variable differs from the race's ideal.
///
/// Otherwise it queues `min(steps available, 4)`.
///
/// `ctx.terraform_steps` stands in for the catalogue count `InitProduction`
/// computes, which needs the terraforming model this crate does not have yet;
/// pass 0 and the routine declines, as it does for a planet already at its
/// race's ideal.
#[must_use]
pub fn queue_ai_terraforming(planet: &Planet, race: &Race, ctx: &Context) -> i32 {
    if ctx.personality == Some(AiPersonality::Cyber) {
        return 0;
    }
    if planet.pop <= TERRAFORM_MIN_POP {
        return 0;
    }
    if planet
        .queue
        .iter()
        .any(|e| !e.ship && e.item == item::AUTO_TERRAFORM)
    {
        return 0;
    }
    // Only terraform a planet that is actually off its race's ideal. A zero
    // difference never wins the comparison at `1090:8dbe`, so a planet already
    // at its ideal in all three variables is skipped.
    let off_ideal = (0..3).any(|i| planet.env[i] != race.env_center[i]);
    if !off_ideal {
        return 0;
    }
    ctx.terraform_steps.clamp(0, MAX_TERRAFORM_QUEUED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race::Race;

    fn colony(mines: i16, factories: i16, pop: i32) -> Planet {
        let mut p = Planet::unowned(0);
        p.owner = Some(0);
        p.pop = pop;
        p.mines = mines;
        p.factories = factories;
        p.min_conc = [80, 80, 80];
        p.surface_min = [500, 500, 500];
        p
    }

    /// With minerals in hand the AI builds factories before mines.
    #[test]
    fn factories_come_before_mines() {
        let race = Race::humanoid();
        let planet = colony(0, 0, 20_000);
        let d = fill_prod_mines_and_factories(&planet, &race, &Context::default());
        assert!(d.factories > 0, "expected factories, got {d:?}");
        assert!(d.changed());
    }

    /// With no germanium on the surface and none being mined, factories cannot
    /// be paid for, so the AI falls back to mines.
    #[test]
    fn a_mineral_shortage_switches_to_mines() {
        let race = Race::humanoid();
        let mut planet = colony(0, 0, 20_000);
        planet.surface_min = [0, 0, 0];
        planet.min_conc = [0, 0, 0];
        let d = fill_prod_mines_and_factories(&planet, &race, &Context::default());
        assert_eq!(d.factories, 0, "{d:?}");
        assert!(d.mines > 0, "{d:?}");
    }

    /// Alchemy waits for every technology to reach 26 *and* for turn 100.
    #[test]
    fn alchemy_needs_full_tech_and_a_late_turn() {
        let race = Race::humanoid();
        let planet = colony(0, 0, 20_000);

        let full = Context {
            tech: [26; 6],
            turn: 101,
            ..Context::default()
        };
        assert!(fill_prod_mines_and_factories(&planet, &race, &full).alchemy > 0);

        let early = Context { turn: 100, ..full };
        assert_eq!(
            fill_prod_mines_and_factories(&planet, &race, &early).alchemy,
            0
        );

        let untrained = Context {
            tech: [26, 26, 26, 26, 26, 25],
            ..full
        };
        assert_eq!(
            fill_prod_mines_and_factories(&planet, &race, &untrained).alchemy,
            0
        );
    }

    /// A planet that earns nothing decides nothing.
    #[test]
    fn no_resources_means_no_decision() {
        let race = Race::humanoid();
        let planet = colony(0, 0, 0);
        let d = fill_prod_mines_and_factories(&planet, &race, &Context::default());
        assert_eq!(d, Decision::default());
        assert!(!d.changed());
    }
}
