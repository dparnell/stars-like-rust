//! Production: how a planet's resources are split between research and the
//! build queue.
//!
//! Source: `Produce` (`10b8:0000`) in `stars.2.7j.exe`, cross-checked against
//! the reconstructed NB09 C (`turn2.c`) and `MANUAL.PDF` p. 20-13. Full
//! derivation in `docs/formulas/production.md`.
//!
//! The build queue itself needs the components table (part costs), which is
//! not decoded yet; what is implemented here is the resource accounting around
//! it, which is what feeds research.

use crate::planet::Planet;
use crate::race::Race;
use crate::resources::resources_at_planet;

/// A planet's resource budget for one year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlanetBudget {
    /// Resources the planet generated, after any recycled surplus.
    pub total: i32,
    /// Resources skimmed for research before the queue runs.
    pub research: i32,
    /// Resources left for the production queue.
    pub production: i32,
}

/// Combine a planet's own output with surplus resources recycled onto it.
///
/// Extra resources are deliberately *not* additive; the manual gives the rule
/// directly (p. 20-13):
///
/// ```text
/// Resources = (Current_production x Extra_resources) /
///             (Current_production + Extra_resources)
/// ```
///
/// which the original adds on top of the planet's own output. The effect is
/// strongly diminishing: a planet already producing a lot gains little from
/// recycling.
#[must_use]
pub fn with_recycled(resources: i32, extra: i32) -> i32 {
    if resources == 0 || extra == 0 {
        return resources;
    }
    let denominator = resources + extra;
    if denominator == 0 {
        return resources;
    }
    resources + (resources * extra) / denominator
}

/// Split a planet's yearly output between research and production.
///
/// `research_pct` is the player's research allocation, and `extra` is any
/// surplus recycled onto this planet. A planet flagged "don't contribute to
/// research" keeps everything for its queue.
///
/// Note what happens to whatever the queue does not spend: it goes to research
/// too. A planet with an empty queue therefore puts **all** of its output into
/// research, which is why players who let their queues run dry still make
/// research progress.
#[must_use]
pub fn planet_budget(
    planet: &Planet,
    race: &Race,
    research_pct: u8,
    extra: i32,
    no_research: bool,
    energy_tech: i16,
) -> Option<PlanetBudget> {
    let base = i32::from(resources_at_planet(planet, race, energy_tech)?);
    let total = with_recycled(base, extra);
    if total == 0 {
        return Some(PlanetBudget::default());
    }

    let research = if no_research {
        0
    } else {
        total * i32::from(research_pct) / 100
    };

    Some(PlanetBudget {
        total,
        research,
        production: total - research,
    })
}

/// A production-queue item, as a planet's build list stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueItem {
    /// How many to build.
    pub count: i32,
    /// Item id: an [`item`] constant when [`Self::ship`] is false, otherwise a
    /// ship or starbase design slot.
    pub item: u16,
    /// Whether this entry builds a ship rather than a planetary installation.
    pub ship: bool,
    /// How far the first unit has been paid for, as a percentage.
    pub completion: i32,
}

impl QueueItem {
    /// Whether this entry is an auto-build installation, which keeps building
    /// as the planet grows rather than counting down to zero.
    #[must_use]
    pub fn is_auto(&self) -> bool {
        !self.ship && item::auto_builds(self.item).is_some()
    }
}

/// Planetary item ids — the game's `ProdItemType` enum, recovered from the
/// binary's own debug symbols.
///
/// A queue entry's 7-bit item field holds one of these when the entry's class
/// is [`stars_formats::QueueClass::Planet`], and a ship design slot when it is
/// `Fleet`.
///
/// The `AUTO_*` ids are the game's `mdIdle*` values: they are the **auto-build**
/// form of the item, which keeps building as the planet grows instead of
/// counting down. Auto-build is therefore its own set of item ids, not a flag
/// on an ordinary one.
pub mod item {
    /// A mine.
    pub const MINE: u16 = 0;
    /// A factory.
    pub const FACTORY: u16 = 1;
    /// A planetary defence.
    pub const DEFENSE: u16 = 2;
    /// Mineral alchemy: resources into one kT of each mineral.
    pub const ALCHEMY: u16 = 3;
    /// Terraform one step toward the race's ideal.
    pub const MIN_TERRAFORM: u16 = 4;
    /// Terraform as far as technology allows.
    pub const MAX_TERRAFORM: u16 = 5;
    /// A mineral packet.
    pub const PACKET: u16 = 6;
    /// Auto-build factories (`mdIdleFactory`).
    pub const AUTO_FACTORY: u16 = 7;
    /// Auto-build mines (`mdIdleMine`).
    pub const AUTO_MINE: u16 = 8;
    /// Auto-build defences (`mdIdleDefense`).
    pub const AUTO_DEFENSE: u16 = 9;
    /// Auto-build mineral alchemy (`mdIdleAlchemy`).
    pub const AUTO_ALCHEMY: u16 = 11;
    /// Auto-build terraforming (`mdIdleTerraform`).
    pub const AUTO_TERRAFORM: u16 = 12;
    /// The first planetary-scanner id.
    pub const PLANETARY_SCANNER_FIRST: u16 = 18;

    /// The ordinary item an auto-build id builds, or `None` if `item` is not
    /// an auto-build id.
    #[must_use]
    pub fn auto_builds(item: u16) -> Option<u16> {
        Some(match item {
            AUTO_FACTORY => FACTORY,
            AUTO_MINE => MINE,
            AUTO_DEFENSE => DEFENSE,
            AUTO_ALCHEMY => ALCHEMY,
            AUTO_TERRAFORM => MAX_TERRAFORM,
            _ => return None,
        })
    }
}

/// What one unit of a queue item costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemCost {
    /// Ironium, boranium, germanium.
    pub minerals: [i32; 3],
    /// Resources.
    pub resources: i32,
}

/// The cost of one planetary item.
///
/// Source: `GetProductionCosts` (`produce.c`). Ship designs are costed by
/// [`crate::design::ShipDesign::cost`] instead; this covers the things a
/// planet builds directly.
///
/// Returns `None` for an id this does not cover, which includes ship designs
/// and the packet and scanner ranges.
#[must_use]
pub fn planetary_item_cost(item: u16, race: &Race, tutorial: bool) -> Option<ItemCost> {
    use crate::race::{lrt, Prt, RaceStat};

    let item = item::auto_builds(item).unwrap_or(item);

    Some(match item {
        item::MINE => ItemCost {
            minerals: [0, 0, 0],
            resources: i32::from(race.stat(RaceStat::MineBuild)),
        },
        item::FACTORY => {
            // Cheap Factories saves a germanium.
            let saving = i32::from(race.has_lrt(lrt::CHEAP_FACT));
            ItemCost {
                minerals: if tutorial {
                    [2 - saving, 2 - saving, 2 - saving]
                } else {
                    [0, 0, 4 - saving]
                },
                resources: i32::from(race.stat(RaceStat::FactBuild)),
            }
        }
        item::DEFENSE => {
            // The SDI's entry in the planetary table is the cost of any
            // defence; Inner Strength pays three fifths of it.
            let sdi = crate::components::PLANETARY
                .iter()
                .find(|p| p.name == "SDI")?;
            let mut cost = ItemCost {
                minerals: [
                    i32::from(sdi.ore_cost[0]),
                    i32::from(sdi.ore_cost[1]),
                    i32::from(sdi.ore_cost[2]),
                ],
                resources: i32::from(sdi.resource_cost),
            };
            if race.prt() == Some(Prt::Is) {
                cost.resources = cost.resources * 3 / 5;
                for m in &mut cost.minerals {
                    *m = *m * 3 / 5;
                }
            }
            cost
        }
        item::ALCHEMY => ItemCost {
            minerals: [0, 0, 0],
            resources: if race.has_lrt(lrt::MINERAL_ALCHEMY) {
                25
            } else {
                100
            },
        },
        item::MIN_TERRAFORM | item::MAX_TERRAFORM => {
            let mut resources = if race.has_lrt(lrt::TT) { 70 } else { 100 };
            if race.prt() == Some(Prt::Ca) {
                resources /= 2;
            }
            ItemCost {
                minerals: [0, 0, 0],
                resources,
            }
        }
        _ => return None,
    })
}

/// The four things a build consumes: ironium, boranium, germanium, resources.
pub const COST_PARTS: usize = 4;

/// What one queue item did when the planet tried to build it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildOutcome {
    /// Units completed this year.
    pub built: i32,
    /// Units still wanted.
    pub remaining: i32,
    /// Progress on the next unit, as a percentage, carried to next year.
    pub completion_pct: i32,
    /// Whether the item stopped because minerals ran out rather than
    /// resources. An auto-build item blocked this way banks nothing.
    pub mineral_blocked: bool,
}

/// Try to build `count` of an item from what the planet has available.
///
/// Source: `CBuildProdItem` (`10b8:0c92`). Whole units are completed while
/// they can be afforded outright; when the next one cannot be, as much of it
/// as possible is paid for and banked as a percentage, so a poor colony
/// finishes a factory over several years.
///
/// `available` is `[ironium, boranium, germanium, resources]` and is debited
/// in place. `auto_build` marks the automatic queue items, which differ in one
/// way: if they are blocked for want of **minerals** they bank nothing and
/// stop, rather than part-paying a unit they cannot finish.
pub fn build_item(
    cost: ItemCost,
    count: i32,
    completion_pct: i32,
    available: &mut [i32; COST_PARTS],
    auto_build: bool,
) -> BuildOutcome {
    let unit = [
        cost.minerals[0],
        cost.minerals[1],
        cost.minerals[2],
        cost.resources,
    ];
    // What the carried percentage has already paid for.
    let mut paid = unit.map(|c| c * completion_pct / 100);

    let mut built = 0;
    let mut remaining = count.max(0);
    let mut pct = completion_pct;

    loop {
        if remaining == 0 {
            return BuildOutcome {
                built,
                remaining,
                completion_pct: pct,
                mineral_blocked: false,
            };
        }

        // Can the next whole unit be afforded outright?
        if (0..COST_PARTS).all(|i| available[i] >= unit[i] - paid[i]) {
            built += 1;
            remaining -= 1;
            pct = 0;
            for i in 0..COST_PARTS {
                available[i] -= unit[i] - paid[i];
                paid[i] = 0;
            }
            continue;
        }

        // Not affordable: work out how much of it can be paid for, taking the
        // most constrained of the four.
        let mut best = 100;
        let mut mineral_blocked = false;
        for i in 0..COST_PARTS {
            if unit[i] <= 0 {
                continue;
            }
            let mut share = if available[i] < unit[i] {
                let exact = (available[i] + paid[i]) * 100 / unit[i];
                // The original nudges up to just under the next percent when
                // one more unit of input would cross the boundary.
                let generous = (available[i] + paid[i] + 1) * 100 / unit[i];
                if exact < generous {
                    generous - 1
                } else {
                    exact
                }
            } else {
                100
            };
            if share > 100 {
                share = 100;
            }
            if share < best {
                best = share;
                mineral_blocked = i < 3;
            }
        }

        if mineral_blocked && auto_build {
            // An auto-build item does not part-pay for something it cannot
            // finish for want of minerals.
            return BuildOutcome {
                built,
                remaining,
                completion_pct: pct,
                mineral_blocked: true,
            };
        }

        for i in 0..COST_PARTS {
            let add = unit[i] * best / 100 - paid[i];
            available[i] -= add;
            paid[i] += add;
        }
        return BuildOutcome {
            built,
            remaining,
            completion_pct: best,
            mineral_blocked,
        };
    }
}

/// How many of an auto-build installation a planet may still add.
///
/// Auto-build mines, factories and defences are capped by what the planet will
/// be able to *operate* next year, not by what it could ever hold — which is
/// why an auto-build queue keeps pace with population rather than racing ahead
/// of it.
#[must_use]
pub fn auto_build_cap(planet: &Planet, race: &Race, item: u16) -> i32 {
    use crate::resources::{max_operable_factories, max_operable_mines};

    let item = item::auto_builds(item).unwrap_or(item);
    let cap = match item {
        item::MINE => i32::from(max_operable_mines(planet, race, true)) - i32::from(planet.mines),
        item::FACTORY => {
            i32::from(max_operable_factories(planet, race, true)) - i32::from(planet.factories)
        }
        _ => return 1000,
    };
    cap.max(0)
}
