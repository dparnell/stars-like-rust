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

pub mod item {
    //! Planetary item ids — the game's `ProdItemType` enum.
    //!
    //! A queue entry's 7-bit item field holds one of these when the entry's
    //! class is [`stars_formats::QueueClass::Planet`], and a ship design slot
    //! when it is `Fleet`.
    //!
    //! Two families, and which is which is easy to get backwards. Ids **0..=6
    //! are the auto-build items** — the ones the manual writes as `Factories
    //! Up to 50` — and ids 7 upward are the things a planet builds one of.
    //!
    //! Three independent pieces of evidence say so:
    //!
    //! * `FillProdSrcLB` (`10d0:3b00`) labels an inventory row with
    //!   ` (Auto Build)` and draws it italic exactly when `iItem < 7`
    //!   (`10d0:3c42: CMP AX,0x7 / JC`).
    //! * `PszNameProdItem` names 0..=6 in the **plural** — `Mines`,
    //!   `Factories`, `Min Terraform` — and 7..=12 in the singular: `Factory`,
    //!   `Mine`, `Terraform Environment`.
    //! * Every one of the 1649 entries for ids 0, 1 and 2 across the fixtures
    //!   has a count of exactly **100** and nothing else, which is an "up to
    //!   100" auto-build order; the plain ids carry ordinary varying counts.
    //!
    //! The AI agrees from the other side: `FFillProdMinesAndFactories`
    //! (`10a8:2d72`) queues with `AddItemToQueue(7, …)` for factories and
    //! `(8, …)` for mines, having counted the queue's existing 7s and 8s
    //! against what the planet can *operate*.

    /// Auto-build mines: keep building as the planet grows.
    pub const AUTO_MINE: u16 = 0;
    /// Auto-build factories.
    pub const AUTO_FACTORY: u16 = 1;
    /// Auto-build defences.
    pub const AUTO_DEFENSE: u16 = 2;
    /// Auto-build mineral alchemy — "as needed", only when minerals are short.
    pub const AUTO_ALCHEMY: u16 = 3;
    /// Auto-build terraforming, only as far as the planet needs.
    pub const AUTO_MIN_TERRAFORM: u16 = 4;
    /// Auto-build terraforming, as far as technology allows.
    pub const AUTO_MAX_TERRAFORM: u16 = 5;
    /// Auto-build mineral packets.
    pub const AUTO_PACKET: u16 = 6;

    /// One factory.
    pub const FACTORY: u16 = 7;
    /// One mine.
    pub const MINE: u16 = 8;
    /// One planetary defence.
    pub const DEFENSE: u16 = 9;
    /// One unit of mineral alchemy: resources into a kT of each mineral.
    pub const ALCHEMY: u16 = 11;
    /// One terraforming step toward the race's ideal.
    pub const TERRAFORM: u16 = 12;
    /// A Genesis Device.
    pub const GENESIS: u16 = 13;
    /// An ironium mineral packet.
    pub const PACKET_IRONIUM: u16 = 14;
    /// A boranium mineral packet.
    pub const PACKET_BORANIUM: u16 = 15;
    /// A germanium mineral packet.
    pub const PACKET_GERMANIUM: u16 = 16;
    /// A packet of all three minerals.
    pub const PACKET_MIXED: u16 = 17;
    /// The first planetary-scanner id (`Viewer 50`).
    pub const PLANETARY_SCANNER_FIRST: u16 = 18;
    /// The last planetary-scanner id (`Snooper 620X`).
    pub const PLANETARY_SCANNER_LAST: u16 = 26;
    /// The generic planetary scanner, which is what the inventory offers: a
    /// planet builds one and it upgrades itself as technology arrives, which
    /// is why the inventory drops it once the planet has one.
    pub const PLANETARY_SCANNER: u16 = 27;
    /// The id a planet with no scanner stores (`PLANET.iScanner`, five bits).
    pub const NO_SCANNER: u16 = 31;

    /// Whether an id is one of the auto-build items.
    #[must_use]
    pub fn is_auto(item: u16) -> bool {
        item <= AUTO_PACKET
    }

    /// The ordinary item an auto-build id builds, or `None` if `item` is not
    /// an auto-build id.
    ///
    /// Both terraforming variants build the same thing; the difference is how
    /// far they go, not what they make. The auto packet is costed as a mixed
    /// packet, which is what `GetProductionCosts` does with it.
    #[must_use]
    pub fn auto_builds(item: u16) -> Option<u16> {
        Some(match item {
            AUTO_MINE => MINE,
            AUTO_FACTORY => FACTORY,
            AUTO_DEFENSE => DEFENSE,
            AUTO_ALCHEMY => ALCHEMY,
            AUTO_MIN_TERRAFORM | AUTO_MAX_TERRAFORM => TERRAFORM,
            AUTO_PACKET => PACKET_MIXED,
            _ => return None,
        })
    }
}

/// What the game calls a queue item.
///
/// `PszNameProdItem` (`10d0:3c92`) reads the name out of the string table at
/// `idsMines + iItem` for everything but the planetary scanners, which take
/// their names from the components table. The auto-build items are the plural
/// ones — `Mines`, `Factories` — and the plain items the singular: `Mine`,
/// `Factory`. Id 10 is unused and reads as a single space.
///
/// A **ship** entry is named by its design instead, so this covers only the
/// `grobjPlanet` half of a queue.
#[must_use]
pub fn item_name(id: u16) -> &'static str {
    const NAMES: [&str; 18] = [
        "Mines",
        "Factories",
        "Defenses",
        "Alchemy",
        "Min Terraform",
        "Max Terraform",
        "Mineral Packets",
        "Factory",
        "Mine",
        "Defenses",
        " ",
        "Mineral Alchemy",
        "Terraform Environment",
        "Genesis Device",
        "Ironium Mineral Packet",
        "Boranium Mineral Packet",
        "Germanium Mineral Packet",
        "Mixed Mineral Packet",
    ];
    if let Some(name) = NAMES.get(usize::from(id)) {
        return name;
    }
    if (item::PLANETARY_SCANNER_FIRST..=item::PLANETARY_SCANNER_LAST).contains(&id) {
        let index = usize::from(id - item::PLANETARY_SCANNER_FIRST);
        return crate::components::PLANETARY
            .get(index)
            .map_or("", |p| p.name);
    }
    if id == item::PLANETARY_SCANNER {
        return "Planetary Scanner";
    }
    ""
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
///
/// `tutorial` is [`crate::GameState::tutorial`]: while the tutorial runs
/// (`gd` bit 11, tested at `10d0:4885`) a factory costs **two of every
/// mineral** rather than four germanium alone, which is what lets the
/// tutorial's home world keep building factories for as long as its pages
/// assume.
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
        item::TERRAFORM => {
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
    /// How many units of mineral alchemy were performed on this item's behalf
    /// — nothing unless there was auto alchemy in front of it in the queue.
    pub alchemised: i32,
    /// How the year went for this item, which is what decides whether the
    /// queue carries on behind it.
    pub status: BuildStatus,
}

/// What became of one queue item this year (`mdProdStat`).
///
/// The numbers are the game's own, and the order matters: **anything above
/// `NoneAuto` stops the queue for the year**, which is `Produce`'s
/// `if (mdStatus > 4)`. An auto-build item that could not be finished does
/// *not* stop it — the manual's "auto-build items that require only resources
/// will continue to be produced" (p. 7-1) — but an ordinary one does, so an
/// unaffordable ship at the front of a queue holds up everything behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BuildStatus {
    /// An ordinary item, finished.
    #[default]
    Complete = 0,
    /// An auto-build item that reached its cap this year.
    CompleteAuto = 1,
    /// An auto-build item with nothing to do — it is already at its cap.
    SkippedAuto = 2,
    /// An auto-build item that built some of what it wanted.
    SomeAuto = 3,
    /// An auto-build item that built none of it.
    NoneAuto = 4,
    /// An ordinary item that built some but not all of what was asked.
    Some = 5,
    /// An ordinary item that could not build even one.
    ///
    /// The original distinguishes two of these — `mdProdStatBlockedSame` and
    /// `mdProdStatBlockedDiff`, by whether the item id changed under the
    /// auto-build clamp — but only to choose a message; both stop the queue.
    Blocked = 6,
}

impl BuildStatus {
    /// Whether the queue stops here for the year.
    #[must_use]
    pub fn stops_the_queue(self) -> bool {
        (self as u8) > (BuildStatus::NoneAuto as u8)
    }

    /// Whether the item finished everything it was asked for.
    #[must_use]
    pub fn is_complete(self) -> bool {
        matches!(self, BuildStatus::Complete | BuildStatus::CompleteAuto)
    }
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
///
/// `count` is what the item is allowed to build this year — for an auto-build
/// item that is its cap, not the "up to N" the player typed — and the outcome's
/// [`BuildStatus`] says whether the queue carries on behind it.
///
/// `alchemy` is what one unit of mineral alchemy costs in resources — 100, or
/// 25 with the trait — and is `Some` only when the queue entry immediately
/// before this one is **auto alchemy**. It turns resources into the minerals
/// this item is short of; see [`Alchemy`].
pub fn build_item(
    cost: ItemCost,
    count: i32,
    completion_pct: i32,
    available: &mut [i32; COST_PARTS],
    auto_build: bool,
    alchemy: Option<i32>,
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
    let mut alchemised = 0;

    loop {
        if remaining == 0 {
            return BuildOutcome {
                built,
                remaining,
                completion_pct: pct,
                mineral_blocked: false,
                alchemised,
                status: status_of(auto_build, built, remaining, false),
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
        //
        // Two flags come out of this, and they are not opposites. `short_of_a
        // _mineral` is **sticky**: it records that a mineral was, at some
        // point, the tightest input so far, and it is what decides whether an
        // auto-build item gives up. `resources_bind` says resources were the
        // tightest in the end, and it is what says alchemy cannot help —
        // alchemy is paid for in resources.
        let mut best = 100;
        let mut short_of_a_mineral = false;
        let mut resources_bind = false;
        let mut shortfall = 0;
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
                shortfall = (unit[i] - paid[i]) - available[i];
                if i == 3 {
                    resources_bind = true;
                } else {
                    short_of_a_mineral = true;
                }
            }
        }

        if short_of_a_mineral && auto_build {
            // An auto-build item does not part-pay for something it cannot
            // finish for want of minerals — unless there is alchemy in front
            // of it, in which case it goes straight to asking for some.
            if alchemy.is_none() {
                return BuildOutcome {
                    built,
                    remaining,
                    completion_pct: pct,
                    mineral_blocked: true,
                    alchemised,
                    status: status_of(auto_build, built, remaining, true),
                };
            }
        } else {
            for i in 0..COST_PARTS {
                let add = unit[i] * best / 100 - paid[i];
                available[i] -= add;
                paid[i] += add;
            }
            pct = best;
            // Alchemy is bought with resources, so it is no help at all when
            // resources are what ran out.
            if alchemy.is_none() || resources_bind {
                return BuildOutcome {
                    built,
                    remaining,
                    completion_pct: pct,
                    mineral_blocked: short_of_a_mineral,
                    alchemised,
                    status: status_of(auto_build, built, remaining, false),
                };
            }
        }

        // Alchemy: turn resources into the minerals this item is short of, up
        // to the shortfall and no further. Every unit gives one kT of each of
        // the three.
        let each = alchemy.unwrap_or(0).max(1);
        let bought = (available[3] / each).min(shortfall).max(0);
        if bought > 0 {
            for mineral in available.iter_mut().take(3) {
                *mineral += bought;
            }
            available[3] -= each * bought;
            alchemised += bought;
        }
        if bought != shortfall {
            // It could not make up the whole gap, so there is no point going
            // round again.
            return BuildOutcome {
                built,
                remaining,
                completion_pct: pct,
                mineral_blocked: short_of_a_mineral,
                alchemised,
                status: status_of(auto_build, built, remaining, false),
            };
        }
    }
}

/// `CBuildProdItem`'s closing `mdStatus` decision.
fn status_of(auto: bool, built: i32, remaining: i32, mineral_blocked: bool) -> BuildStatus {
    if auto && mineral_blocked {
        // An auto item that ran out of minerals: it does not hold the queue up.
        return if built < 1 {
            BuildStatus::NoneAuto
        } else {
            BuildStatus::SomeAuto
        };
    }
    if !auto || remaining != 0 {
        return if built == 0 {
            BuildStatus::Blocked
        } else if remaining == 0 {
            BuildStatus::Complete
        } else {
            BuildStatus::Some
        };
    }
    if built < 1 {
        BuildStatus::SkippedAuto
    } else {
        BuildStatus::CompleteAuto
    }
}

/// How many of an auto-build item a planet may still build this year.
///
/// Source: the opening of `CBuildProdItem` (`10b8:0c92`), which clamps an
/// auto-build item's count to this before spending anything on it. The cap is
/// not the same as the plain item's: mines, factories and defences are held to
/// what the planet will be able to **operate** next year rather than to what it
/// could ever hold, which is what keeps an auto-build queue in step with the
/// population instead of racing ahead of it.
///
/// The other four:
///
/// * **alchemy** has no cap;
/// * **maximum terraforming** is capped by how much terraforming is left;
/// * **minimum terraforming** by the same, but drops to nothing while the
///   planet is growing *and* habitable — which is what makes it the *minimum*:
///   it only acts when people would otherwise be dying;
/// * **packets** need a mass driver and something on the surface to fling.
///
/// An id that is not an auto-build item has no cap.
#[must_use]
pub fn auto_build_cap(
    planet: &Planet,
    race: &Race,
    tech: [u8; 6],
    designs: &[crate::design::ShipDesign],
    id: u16,
) -> i32 {
    use crate::resources::{max_operable_defenses, max_operable_factories, max_operable_mines};

    let cap = match id {
        item::AUTO_MINE => {
            i32::from(max_operable_mines(planet, race, true)) - i32::from(planet.mines)
        }
        item::AUTO_FACTORY => {
            i32::from(max_operable_factories(planet, race, true)) - i32::from(planet.factories)
        }
        item::AUTO_DEFENSE => {
            i32::from(max_operable_defenses(planet, race)) - i32::from(planet.defenses)
        }
        item::AUTO_ALCHEMY => return UNLIMITED,
        item::AUTO_MIN_TERRAFORM | item::AUTO_MAX_TERRAFORM => {
            let left = crate::terraform::terraform_steps(planet, race, tech);
            if id == item::AUTO_MIN_TERRAFORM && left > 0 {
                // Growing and habitable: nothing needs doing yet.
                let growing = crate::population::chg_pop_from_planet(planet, race)
                    .is_some_and(|change| change.delta >= 0);
                if growing && crate::hab::pct_planet_desirability(planet, race) > 0 {
                    return 0;
                }
            }
            left
        }
        item::AUTO_PACKET => {
            if mass_driver_warp(planet, designs) == 0 || planet.surface_min.iter().all(|m| *m == 0)
            {
                return 0;
            }
            return UNLIMITED;
        }
        _ => return UNLIMITED,
    };
    cap.max(0)
}

/// What a planet may add to its queue, and how many more of it.
///
/// The count of an item there is no limit on. The game stores the inventory
/// count in a ten-bit field, so this is what "unlimited" looks like to it.
pub const UNLIMITED: i32 = 0x3ff;

/// One row of the production inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Available {
    /// The item id, or a design slot when [`Self::ship`] is set.
    pub item: u16,
    /// Whether it builds a ship or starbase rather than a planetary item.
    pub ship: bool,
    /// How many more may be queued, or [`UNLIMITED`].
    pub count: i32,
    /// What the game calls it.
    pub name: String,
    /// Whether it is one of the auto-build items, which the original draws in
    /// italic and labels ` (Auto Build)`.
    pub auto: bool,
}

impl Available {
    /// Whether there is no limit on how many may be queued.
    #[must_use]
    pub fn unlimited(&self) -> bool {
        self.count >= UNLIMITED
    }
}

/// The production inventory: everything this planet can build right now.
///
/// Source: `InitProduction` (`10d0:015e`), in its own order — ship designs,
/// starbase designs, the Genesis Device, mineral packets, the three
/// installations, alchemy, a planetary scanner, terraforming, and finally the
/// seven auto-build items. `FillProdSrcLB` then drops any row whose count has
/// fallen to zero, which is what makes a unique item disappear from the list
/// once it is queued.
///
/// `designs` is the owning player's full design list — ship slots `0..16` and
/// starbase slots `16..26` — and `queue` is the planet's queue, whose contents
/// are **subtracted** from the counts, so what comes back is what may still be
/// added.
///
/// See `docs/ui/production.md`.
#[must_use]
pub fn inventory(
    planet: &Planet,
    who: &crate::parts::Builder<'_>,
    designs: &[crate::design::ShipDesign],
    queue: &[QueueItem],
) -> Vec<Available> {
    use crate::components::slot;
    use crate::race::Prt;

    let race = who.race;
    let prt = race.prt();
    let first_base = usize::from(crate::startup::FIRST_STARBASE_SLOT);
    let mut out: Vec<Available> = Vec::new();

    let mut push = |item: u16, ship: bool, count: i32| {
        // The count lives in the same ten-bit field the queue uses, so a
        // capacity above 1023 is stored as — and is indistinguishable from —
        // "no limit". A well-grown planet reaches that for mines and
        // factories long before it runs out of room for them.
        let count = count.min(UNLIMITED);
        if count > 0 {
            out.push(Available {
                item,
                ship,
                count,
                name: if ship {
                    designs
                        .get(usize::from(item))
                        .map_or_else(String::new, |d| d.name.clone())
                } else {
                    item_name(item).to_string()
                },
                auto: !ship && item::is_auto(item),
            });
        }
    };

    // Ships, but only from a starbase with a space dock, and only designs the
    // dock is big enough for: the hull's cargo capacity is the dock's size.
    let dock = planet
        .starbase
        .then(|| planet.starbase_design.map(usize::from))
        .flatten()
        .and_then(|slot| designs.get(first_base + slot))
        .and_then(|base| crate::components::hull(base.hull_id))
        .map(|hull| i32::from(hull.cargo_max))
        .filter(|dock| *dock != 0);
    if let Some(dock) = dock {
        for (slot, design) in designs
            .iter()
            .take(crate::design::MAX_SHIP_DESIGNS)
            .enumerate()
        {
            if design.hull_id < 0 {
                continue;
            }
            if design.mass().is_some_and(|mass| mass <= dock) {
                push(u16::try_from(slot).unwrap_or(0), true, UNLIMITED);
            }
        }
    }

    // Starbases: every design but the one already in orbit. A planet needs no
    // starbase to build a starbase.
    for slot in 0..MAX_STARBASE_DESIGNS_SHOWN {
        let Some(design) = designs.get(first_base + slot) else {
            continue;
        };
        if design.hull_id < 0 {
            continue;
        }
        if planet.starbase && planet.starbase_design == u8::try_from(slot).ok() {
            continue;
        }
        push(u16::try_from(first_base + slot).unwrap_or(0), true, 1);
    }

    // The Genesis Device, if the Mystery Trader has handed it over and the
    // technology is there.
    if crate::parts::availability(who, slot::PLANETARY, GENESIS_PART).is_available() {
        push(item::GENESIS, false, 1);
    }

    // Mineral packets, once the planet has a mass driver to fling them with.
    if mass_driver_warp(planet, designs) > 0 {
        for id in item::PACKET_IRONIUM..=item::PACKET_MIXED {
            push(id, false, UNLIMITED);
        }
    }

    // The three installations, each limited by what the planet can run.
    push(
        item::FACTORY,
        false,
        i32::from(crate::resources::max_factories(planet, race)) - i32::from(planet.factories),
    );
    push(
        item::MINE,
        false,
        i32::from(crate::resources::max_mines(planet, race)) - i32::from(planet.mines),
    );
    push(
        item::DEFENSE,
        false,
        i32::from(crate::resources::max_defenses(planet, race)) - i32::from(planet.defenses),
    );

    // Alchemy is always on offer.
    push(item::ALCHEMY, false, UNLIMITED);

    // A planetary scanner, once and only once: a planet that has one upgrades
    // it for free as technology arrives. Alternate Reality scans from its
    // starbases and never builds one.
    if planet.scanner.is_none() && prt != Some(Prt::Ar) {
        push(item::PLANETARY_SCANNER, false, 1);
    }

    // Terraforming, as far as there is any left to do.
    let steps = crate::terraform::terraform_steps(planet, race, who.levels);
    push(item::TERRAFORM, false, steps);

    // And the auto-build items. Alternate Reality has no mines, factories or
    // defences to keep topped up, and a Claim Adjuster terraforms for free.
    for id in item::AUTO_MINE..=item::AUTO_PACKET {
        if !crate::ground::template_allows(prt, id) {
            continue;
        }
        push(id, false, UNLIMITED);
    }

    // What is already queued comes off the counts, and a unique item queued
    // once drops out of the list entirely.
    for entry in queue {
        let Some(row) = out
            .iter_mut()
            .find(|row| row.ship == entry.ship && row.item == entry.item)
        else {
            continue;
        };
        if row.count < UNLIMITED {
            row.count = (row.count - entry.count).max(0);
        }
    }
    out.retain(|row| row.count > 0);
    out
}

/// How many starbase designs the inventory walks (`FillBuildDD`'s ten).
const MAX_STARBASE_DESIGNS_SHOWN: usize = crate::design::MAX_STARBASE_DESIGNS;

/// The Genesis Device's index in [`crate::components::PLANETARY`].
const GENESIS_PART: usize = 14;

/// A planet's **mass driver**: what its starbase flings packets with.
///
/// A mass driver is an orbital special fitted to the starbase, and its rating
/// *is* the warp: `Mass Driver 5` flings at warp 5 and the `Ultra Driver 13`
/// at warp 13. The stargates share the same table and are the first seven
/// entries, which is why only the rest count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MassDriver {
    /// The warp of the best driver fitted, or `0` when there is none.
    pub warp: i32,
    /// Whether a **second** driver of that same warp is fitted in another
    /// slot.
    ///
    /// The planet pane writes a `+` after the warp when it is, and the packet
    /// rules count a pair as one warp faster — see
    /// [`docs/formulas/packets.md`](../../../docs/formulas/packets.md).
    ///
    /// It is set slot by slot, so **two drivers in one slot are not a pair**:
    /// `IWarpMAFromLppl` walks the slots and never looks at a slot's count
    /// beyond checking that it is not empty.
    pub paired: bool,
}

/// The warp a planet's mass driver flings at, or `0` when it has none.
///
/// [`mass_driver`] with the pair flag dropped.
#[must_use]
pub fn mass_driver_warp(planet: &Planet, designs: &[crate::design::ShipDesign]) -> i32 {
    mass_driver(planet, designs).warp
}

/// What a planet's starbase can fling packets with.
///
/// Source: `IWarpMAFromLppl` (`1048:7b10`). The turn generator does not use
/// this yet — a planet catching a packet is still modelled without its own
/// driver, which `docs/formulas/packets.md` records — but the production
/// inventory needs it to decide whether to offer packets at all, and the
/// planet pane's Starbase tile reads it for the Mass Driver row.
///
/// The original has one gate this cannot express: for **another player's**
/// planet it reports a driver only when their starbase design is known in
/// full (`det == 7`), and a design here carries no such detail level.
#[must_use]
pub fn mass_driver(planet: &Planet, designs: &[crate::design::ShipDesign]) -> MassDriver {
    use crate::components::{slot, SPECIALS_SB};
    /// Orbital specials below this index are stargates.
    const FIRST_DRIVER: usize = 7;

    // An unowned planet has no starbase to ask about, whatever else is
    // recorded against it.
    if !planet.starbase || planet.owner.is_none() {
        return MassDriver::default();
    }
    let base = planet
        .starbase_design
        .map(usize::from)
        .map(|s| usize::from(crate::startup::FIRST_STARBASE_SLOT) + s)
        .and_then(|s| designs.get(s));
    let Some(base) = base else {
        return MassDriver::default();
    };

    // Slot by slot, keeping the best: a strictly better driver takes over and
    // clears the pair flag, an equal one sets it. Which is why three drivers
    // rated 5, 5 and 7 are not a pair but 5, 7 and 7 are.
    let mut out = MassDriver::default();
    for fitted in &base.slots {
        if fitted.count == 0 || fitted.category & slot::SPECIAL_SB == 0 {
            continue;
        }
        if usize::from(fitted.item) < FIRST_DRIVER {
            continue;
        }
        let Some(driver) = SPECIALS_SB.get(usize::from(fitted.item)) else {
            continue;
        };
        let warp = i32::from(driver.ability);
        if warp > out.warp {
            out = MassDriver {
                warp,
                paired: false,
            };
        } else if warp == out.warp {
            out.paired = true;
        }
    }
    out
}

/// What one of a queue item costs this player.
///
/// The superset of [`planetary_item_cost`]: it also covers the four mineral
/// packets, the Genesis Device and the planetary scanners, which need the
/// player's technology because the last two are **miniaturised** like any
/// other component.
///
/// Source: `GetProductionCosts` (`10d0:3f20`). Returns `None` for a ship
/// design, which is costed by [`crate::design::ShipDesign::true_cost`].
#[must_use]
pub fn item_cost(id: u16, who: &crate::parts::Builder<'_>) -> Option<ItemCost> {
    use crate::components::slot;
    use crate::race::Prt;

    let id = item::auto_builds(id).unwrap_or(id);
    if let Some(cost) = planetary_item_cost(id, who.race, who.tutorial) {
        return Some(cost);
    }

    // A packet's minerals depend only on the primary trait: Packet Physics
    // packs them tightest and Interstellar Traveler worst.
    let packet = |each: i32| -> ItemCost {
        let pp = who.race.prt() == Some(Prt::Pp);
        ItemCost {
            minerals: [each, each, each],
            resources: if pp { 5 } else { 10 },
        }
    };
    let single = |which: usize| -> ItemCost {
        let each = match who.race.prt() {
            Some(Prt::Pp) => 70,
            Some(Prt::It) => 120,
            _ => 110,
        };
        let mut cost = packet(0);
        cost.minerals[which] = each;
        cost
    };

    // The two that are really components, and are miniaturised as such.
    let part = |index: usize| -> Option<ItemCost> {
        let part = crate::parts::part(slot::PLANETARY, index)?;
        let cost = crate::design::true_part_cost(&part, who);
        Some(ItemCost {
            minerals: cost.minerals,
            resources: cost.resources,
        })
    };

    Some(match id {
        item::PACKET_MIXED => packet(match who.race.prt() {
            Some(Prt::Pp) => 25,
            Some(Prt::It) => 48,
            _ => 44,
        }),
        item::PACKET_IRONIUM => single(0),
        item::PACKET_BORANIUM => single(1),
        item::PACKET_GERMANIUM => single(2),
        item::GENESIS => part(GENESIS_PART)?,
        // The generic scanner is costed as a Viewer 50 whatever the planet
        // will actually end up with — `GetProductionCosts` rewrites id 27 to
        // id 18 before looking the part up.
        item::PLANETARY_SCANNER => part(0)?,
        id if (item::PLANETARY_SCANNER_FIRST..=item::PLANETARY_SCANNER_LAST).contains(&id) => {
            part(usize::from(id - item::PLANETARY_SCANNER_FIRST))?
        }
        _ => return None,
    })
}

/// When a queue item will be built, as two years: the one the **first** of them
/// is finished in and the one the **last** is.
///
/// Both are counted from next year, so `1` means "next year". Three values are
/// not years at all:
///
/// * `100` — a hundred years or more, which the game calls **never**;
/// * `0` — nothing to do, which it calls **skipped**;
/// * `-1` — auto alchemy standing by, which it calls **as needed**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eta {
    /// The year the first one is finished.
    pub first: i16,
    /// The year the last one is finished.
    pub last: i16,
}

impl Eta {
    /// What the original prints for this (`PszProductionETA`, `1048:310c`).
    ///
    /// The wording is the game's, string ids 815 to 820 and 596.
    #[must_use]
    pub fn text(self, id: u16, ship: bool) -> String {
        if self.first == 100 {
            // An auto-build item is never "never" — it simply has no schedule.
            return if !ship && id <= item::AUTO_PACKET {
                "Unknown".to_string()
            } else {
                "Never".to_string()
            };
        }
        if self.last == 100 {
            return format!("{} - ??? years", self.first);
        }
        if self.first == self.last {
            return match self.first {
                0 => "Skipped".to_string(),
                -1 => "As Needed".to_string(),
                1 => "1 year".to_string(),
                n => format!("{n} years"),
            };
        }
        format!("{} - {} years", self.first, self.last)
    }

    /// How the row is drawn.
    ///
    /// `FillPlanetProdLB` puts one of five characters in front of a queue row
    /// and `DrawProductionItem` reads it back. The interesting one is
    /// [`EtaMark::Never`], which is the manual's red row: the item's minerals
    /// are so far out of reach that it will practically never be finished
    /// (p. 7-7).
    #[must_use]
    pub fn mark(self, id: u16, ship: bool) -> EtaMark {
        if (self.first == 0 && self.last == 0) || (self.first == -1 && self.last == -1) {
            return EtaMark::Idle;
        }
        // An auto-build item at 100 is "unknown", not "never", and is drawn
        // like any ordinary row.
        let unknown = self.first == 100 && !ship && id <= item::AUTO_PACKET;
        if (self.first < 2 || self.first > 99) && !unknown {
            if self.first == 1 && self.last == 1 {
                return EtaMark::AllNextYear;
            }
            return if self.first < 100 {
                EtaMark::FirstNextYear
            } else {
                EtaMark::Never
            };
        }
        EtaMark::Ordinary
    }
}

/// How a production-queue row is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtaMark {
    /// `&` — nothing to do, or nothing known.
    Idle,
    /// `*` — all of them finish next year.
    AllNextYear,
    /// `#` — the first finishes next year, the rest take longer.
    FirstNextYear,
    /// ` ` — somewhere between two and ninety-nine years.
    Ordinary,
    /// `!` — a hundred years or more. The original draws this row **red**.
    Never,
}

/// When one item in a planet's queue will be finished.
///
/// Source: `EstimateItemProdSched` (`10d0:4f40`), which is a real simulation:
/// it takes a copy of the planet and runs **up to ninety-nine years** of the
/// whole queue over it — mining, resources, research skim, the queue's own
/// stopping rules and population growth — until the item in question completes
/// or the century runs out. Nothing cheaper would do, because an item's date
/// depends on everything ahead of it and on how the planet grows underneath it.
///
/// `index` is the entry in `planet.queue` being asked about.
#[must_use]
pub fn eta(
    planet: &Planet,
    who: &crate::parts::Builder<'_>,
    research_pct: u8,
    designs: &[crate::design::ShipDesign],
    index: usize,
) -> Eta {
    /// The century the original gives up after.
    const PASSES: i16 = 100;
    /// What auto alchemy's count becomes when it is the last item in the
    /// queue, and so is allowed to run flat out.
    const ALCHEMY_FLAT_OUT: i32 = 1020;

    let race = who.race;
    let tech = who.levels;
    if index >= planet.queue.len() {
        return Eta { first: 0, last: 0 };
    }

    let mut pl = planet.clone();
    let mut first: i16 = 0;
    let mut last: i16 = 0;

    for pass in 1..PASSES {
        // This year's minerals and resources. The estimate truncates the
        // mining remainder rather than rolling for it, which is what
        // `EstMineralsMined`'s `fTrue` means.
        let mined = crate::mining::minerals_mined(&pl, race, None, None);
        for (surface, add) in pl.surface_min.iter_mut().zip(mined.iter()) {
            *surface += add;
        }
        let Some(budget) = planet_budget(
            &pl,
            race,
            research_pct,
            0,
            pl.no_research,
            i16::from(tech[0]),
        ) else {
            break;
        };
        let mut available = [
            pl.surface_min[0],
            pl.surface_min[1],
            pl.surface_min[2],
            budget.production,
        ];

        let mut queue = std::mem::take(&mut pl.queue);
        let end = queue.len().saturating_sub(1);
        let alchemy_cost = planetary_item_cost(item::ALCHEMY, race, false).map(|c| c.resources);
        let mut alchemy: Option<i32> = None;
        // The index is the point: it says whether this is the entry being
        // asked about and whether it is the last in the queue.
        #[allow(clippy::needless_range_loop)]
        for i in 0..queue.len() {
            let entry = queue[i];
            if entry.count == 0 && !entry.is_auto() {
                continue;
            }

            // Auto alchemy stands aside unless it is the last item, in which
            // case it runs flat out. An entry asked about while it is standing
            // aside has no schedule at all.
            let mut wanted = entry.count;
            if !entry.ship && entry.item == item::AUTO_ALCHEMY {
                if i != end {
                    if i == index {
                        return Eta {
                            first: -1,
                            last: -1,
                        };
                    }
                    alchemy = alchemy_cost;
                    continue;
                }
                wanted = ALCHEMY_FLAT_OUT;
            }

            let auto = entry.is_auto();
            let cost = if entry.ship {
                designs
                    .get(usize::from(entry.item))
                    .filter(|d| d.hull_id >= 0)
                    .and_then(|d| d.true_cost(who))
                    .map(|c| ItemCost {
                        minerals: c.minerals,
                        resources: c.resources,
                    })
            } else {
                item_cost(entry.item, who)
            };
            let Some(cost) = cost else {
                continue;
            };
            if auto {
                wanted = wanted.min(auto_build_cap(&pl, race, tech, designs, entry.item));
            }

            let outcome = build_item(
                cost,
                wanted,
                entry.completion,
                &mut available,
                auto,
                alchemy.take(),
            );

            if i == index {
                if outcome.built > 0 && first == 0 {
                    first = pass;
                }
                match outcome.status {
                    BuildStatus::SkippedAuto => {
                        if first != 0 {
                            last = pass - 1;
                        }
                        return Eta { first, last };
                    }
                    status if status.is_complete() => {
                        return Eta { first, last: pass };
                    }
                    _ => {}
                }
            }

            // The planet grows under the estimate, which is what lets a
            // colony that cannot afford a factory this year afford one later.
            if outcome.built > 0 {
                match item::auto_builds(entry.item).unwrap_or(entry.item) {
                    item::MINE => pl.mines += i16::try_from(outcome.built).unwrap_or(0),
                    item::FACTORY => pl.factories += i16::try_from(outcome.built).unwrap_or(0),
                    item::DEFENSE => pl.defenses += i16::try_from(outcome.built).unwrap_or(0),
                    _ => {}
                }
            }
            if !auto {
                queue[i].count = outcome.remaining;
            }
            queue[i].completion = outcome.completion_pct;
            if outcome.status.stops_the_queue() {
                break;
            }
        }
        pl.queue = queue;

        pl.surface_min = [available[0], available[1], available[2]];
        if let Some(change) = crate::population::chg_pop_from_planet(&pl, race) {
            pl.pop += change.delta;
        }
    }

    if first == 0 {
        first = PASSES;
    }
    Eta {
        first,
        last: PASSES,
    }
}

/// Apply a production template to a planet's queue.
///
/// Every **auto-build** item already in the queue is dropped and the
/// template's put in its place, at the end; the ordinary items keep their
/// order and their progress. That is the manual's "all auto-build items
/// currently in the queue are replaced with the list of items in the template"
/// (p. 7-4).
///
/// The same two racial filters the default queue uses apply: Alternate Reality
/// takes no mines, factories or defences, and a Claim Adjuster no terraforming
/// — see [`crate::ground::template_allows`].
///
/// Source: `ProdCommandHandler`'s `0x816` arm (`10d0:1994`).
#[must_use]
pub fn apply_template(
    queue: &[QueueItem],
    template: &stars_formats::DefaultQueue,
    prt: Option<crate::race::Prt>,
) -> Vec<QueueItem> {
    let mut out: Vec<QueueItem> = queue
        .iter()
        .filter(|entry| entry.ship || !item::is_auto(entry.item))
        .copied()
        .collect();
    for entry in &template.items {
        let id = u16::from(entry.item);
        if !crate::ground::template_allows(prt, id) {
            continue;
        }
        out.push(QueueItem {
            count: i32::from(entry.count),
            item: id,
            ship: false,
            completion: 0,
        });
    }
    out
}

/// Take a planet's auto-build items into a template.
///
/// The `<Customize>` dialog's **Import** button: the queue's auto-build
/// entries, in order, up to the twelve a template holds — nothing else, since
/// a template can only carry the auto-build items.
///
/// Source: `ZipProdDlg`'s `0x816` arm (`10d0:5490`).
#[must_use]
pub fn import_template(queue: &[QueueItem], no_research: bool) -> stars_formats::DefaultQueue {
    stars_formats::DefaultQueue {
        no_research,
        items: queue
            .iter()
            .filter(|entry| !entry.ship && item::is_auto(entry.item))
            .take(stars_formats::DEFAULT_QUEUE_MAX)
            .map(|entry| stars_formats::DefaultQueueItem {
                item: u8::try_from(entry.item).unwrap_or(0) & 0x3f,
                count: u16::try_from(entry.count).unwrap_or(0) & 0x03ff,
            })
            .collect(),
    }
}

/// How the `<Customize>` dialog lists a template's contents
/// (`FillZipProdLB`, `10d0:5e58`).
///
/// One line per entry — `Factories (100)` — except that an entry of exactly
/// one, and auto alchemy whatever its count, are shown by name alone. An empty
/// or unused template reads `<No Auto Build Orders>`.
#[must_use]
pub fn template_lines(template: Option<&stars_formats::DefaultQueue>) -> Vec<String> {
    let Some(template) = template.filter(|t| !t.items.is_empty()) else {
        return vec!["<No Auto Build Orders>".to_string()];
    };
    template
        .items
        .iter()
        .map(|entry| {
            let id = u16::from(entry.item);
            let name = item_name(id);
            if entry.count == 1 || id == item::AUTO_ALCHEMY {
                name.to_string()
            } else {
                format!("{name} ({})", entry.count)
            }
        })
        .collect()
}

/// What research gets out of one planet next year.
///
/// The Research dialog's **Next year's projected research budget** is the sum
/// of this over the player's planets (`ProjectedResearchSpending`,
/// `10d8:65ae`), and the answer is not simply the research percentage:
///
/// * a planet with an **empty queue** gives research everything it makes —
///   which is the manual's "you receive all resources from planets with
///   nothing in the production queue" (p. 8-4);
/// * a planet with a queue gives the research skim **plus whatever the queue
///   fails to spend**, which is why a blocked queue quietly funds research.
///
/// The second is `EstimateItemProdSched` called with a negative item, which
/// runs one year of the queue and returns what is left over. So the projection
/// shares the whole of the production simulation — the queue's stopping rules,
/// alchemy and all.
#[must_use]
pub fn research_from_planet(
    planet: &Planet,
    who: &crate::parts::Builder<'_>,
    research_pct: u8,
    designs: &[crate::design::ShipDesign],
) -> i32 {
    let race = who.race;
    let tech = who.levels;

    let Some(budget) = planet_budget(
        planet,
        race,
        research_pct,
        0,
        planet.no_research,
        i16::from(tech[0]),
    ) else {
        return 0;
    };
    if planet.queue.is_empty() {
        return budget.total;
    }

    // One year of the queue over a copy of the planet; what production does
    // not spend goes to research, on top of the skim.
    let mut pl = planet.clone();
    let mined = crate::mining::minerals_mined(&pl, race, None, None);
    for (surface, add) in pl.surface_min.iter_mut().zip(mined.iter()) {
        *surface += add;
    }
    let mut available = [
        pl.surface_min[0],
        pl.surface_min[1],
        pl.surface_min[2],
        budget.production,
    ];
    let queue = std::mem::take(&mut pl.queue);
    let end = queue.len().saturating_sub(1);
    let alchemy_cost = planetary_item_cost(item::ALCHEMY, race, false).map(|c| c.resources);
    let mut alchemy: Option<i32> = None;

    for (i, entry) in queue.iter().enumerate() {
        if entry.count == 0 && !entry.is_auto() {
            continue;
        }
        let mut wanted = entry.count;
        if !entry.ship && entry.item == item::AUTO_ALCHEMY {
            if i != end {
                alchemy = alchemy_cost;
                continue;
            }
            wanted = 1020;
        }
        let auto = entry.is_auto();
        let cost = if entry.ship {
            designs
                .get(usize::from(entry.item))
                .filter(|d| d.hull_id >= 0)
                .and_then(|d| d.true_cost(who))
                .map(|c| ItemCost {
                    minerals: c.minerals,
                    resources: c.resources,
                })
        } else {
            item_cost(entry.item, who)
        };
        let Some(cost) = cost else { continue };
        if auto {
            wanted = wanted.min(auto_build_cap(&pl, race, tech, designs, entry.item));
        }
        let outcome = build_item(
            cost,
            wanted,
            entry.completion,
            &mut available,
            auto,
            alchemy.take(),
        );
        if outcome.status.stops_the_queue() {
            break;
        }
    }

    // What production left, plus the skim it never had.
    available[3].max(0) + budget.research
}

/// The whole empire's projected research budget for next year.
///
/// `ProjectedResearchSpending(pct)` with the percentage the dialog is showing,
/// which is how moving the slider changes the figure before anything is
/// committed.
#[must_use]
pub fn projected_research_spending(
    planets: &[Planet],
    owner: i16,
    who: &crate::parts::Builder<'_>,
    research_pct: u8,
    designs: &[crate::design::ShipDesign],
) -> i32 {
    planets
        .iter()
        .filter(|p| p.owner == Some(owner))
        .map(|p| research_from_planet(p, who, research_pct, designs))
        .sum()
}
