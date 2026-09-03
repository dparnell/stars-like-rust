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
) -> Option<PlanetBudget> {
    let base = i32::from(resources_at_planet(planet, race)?);
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
    /// Item id. Below 256 these are the planetary items and ship designs;
    /// 256 and above are the auto-build variants of the same things.
    pub item: u16,
    /// Resources and minerals already applied to the first unit.
    pub completion: i32,
}

/// Planetary item ids (the game's `iobj` enum). Ship designs occupy their own
/// range above these.
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
    /// The first planetary-scanner id.
    pub const PLANETARY_SCANNER_FIRST: u16 = 18;
    /// Anything at or above this is the auto-build form of the item below it.
    pub const AUTO_BUILD_BASE: u16 = 256;
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

    let item = if item >= item::AUTO_BUILD_BASE {
        item - item::AUTO_BUILD_BASE
    } else {
        item
    };

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
