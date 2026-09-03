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
