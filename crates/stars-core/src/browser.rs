//! The Technology Browser.
//!
//! `BrowserDlg` (`10d8:1ed8`), reached with **F2**, with `DisplayComponentInfo`
//! (`10d8:2ac6`) painting the panel. It shows **one component at a time**:
//! a category dropdown, Prev and Next buttons that walk the whole catalogue,
//! and a checkbox that limits the walk to what the player can build now.
//!
//! Everything it says about a component — its cost, its mass, its technology
//! requirements and whether a racial trait puts it out of reach — is relative
//! to the player looking at it, which is what the manual means by "the
//! Technology Browser always displays cost and other information relative to
//! your race type and current level of knowledge" (p. 8-3).
//!
//! See `docs/ui/technology-browser.md`.

use crate::components::{
    slot, Hull, ARMORS, BEAMS, BOMBS, ENGINES, HULLS, MINE_LAYERS, MINING, PLANETARY, SCANNERS,
    SHIELDS, SPECIALS_E, SPECIALS_M, SPECIALS_SB, STARBASE_HULLS, TERRAFORMING, TORPEDOES,
};
use crate::parts::{Availability, Builder, Requirement};
use crate::research::{TechField, TECH_FIELDS};

/// The categories the dropdown lists, in its own order: **All**, then the
/// sixteen kinds alphabetically.
///
/// `BrowserDlg` fills the list from consecutive string ids, 1087 to 1103, so
/// the order is the strings' — which is alphabetical after the first.
pub const CATEGORIES: [(u16, &str); 17] = [
    (0, "All"),
    (slot::ARMOR, "Armor"),
    (slot::BEAM, "Beam Weapons"),
    (slot::BOMB, "Bombs"),
    (slot::SPECIAL_E, "Electrical"),
    (slot::ENGINE, "Engines"),
    (slot::SPECIAL_M, "Mechanical"),
    (slot::MINES, "Mine Layers"),
    (slot::MINING, "Mining Robots"),
    (slot::SPECIAL_SB, "Orbital"),
    (slot::PLANETARY, "Planetary"),
    (slot::SCANNER, "Scanners"),
    (slot::SHIELD, "Shields"),
    (slot::HULL, "Ship Hulls"),
    (slot::SB_HULL, "Starbase Hulls"),
    (slot::TERRA, "Terraforming"),
    (slot::TORPEDO, "Torpedoes"),
];

/// How many components a category holds.
#[must_use]
pub fn category_len(category: u16) -> usize {
    match category {
        slot::ENGINE => ENGINES.len(),
        slot::SCANNER => SCANNERS.len(),
        slot::SHIELD => SHIELDS.len(),
        slot::ARMOR => ARMORS.len(),
        slot::BEAM => BEAMS.len(),
        slot::TORPEDO => TORPEDOES.len(),
        slot::BOMB => BOMBS.len(),
        slot::MINING => MINING.len(),
        slot::MINES => MINE_LAYERS.len(),
        slot::SPECIAL_SB => SPECIALS_SB.len(),
        slot::SPECIAL_E => SPECIALS_E.len(),
        slot::SPECIAL_M => SPECIALS_M.len(),
        slot::TERRA => TERRAFORMING.len(),
        slot::PLANETARY => PLANETARY.len(),
        slot::HULL => HULLS.len(),
        slot::SB_HULL => STARBASE_HULLS.len(),
        _ => 0,
    }
}

/// One technology field's requirement, and whether the player has met it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TechRequirement {
    /// Which field.
    pub field: TechField,
    /// The level it asks for.
    pub level: i8,
    /// Whether the player has that level. The original draws the ones they
    /// have in **black** and the ones they do not in **red** (p. 8-3).
    pub met: bool,
}

/// Everything the browser's panel shows about one component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    /// Which category it came from, and its index in that category.
    pub category: u16,
    /// Its index within that category.
    pub item: usize,
    /// What it is called.
    pub name: &'static str,
    /// The category's name, as the dropdown spells it.
    pub category_name: &'static str,
    /// What one costs this player: three minerals then resources.
    pub cost: [i32; 4],
    /// Mass in kT. `None` for something that is never carried.
    pub mass: Option<i32>,
    /// The technology it needs, with the fields it asks nothing of left out.
    pub tech: Vec<TechRequirement>,
    /// The type-specific figures, as label and value.
    pub stats: Vec<(String, String)>,
    /// What stands between this player and building it.
    pub notes: Vec<String>,
    /// Which of the game's component pictures it is drawn with (`ibmp`).
    pub picture: u16,
    /// Whether they can build it now.
    pub availability: Availability,
}

impl Detail {
    /// Whether the player can build it now.
    #[must_use]
    pub fn buildable(&self) -> bool {
        self.availability.is_available()
    }
}

/// What the browser shows for one component, for one player.
#[must_use]
pub fn detail(who: &Builder<'_>, category: u16, item: usize) -> Option<Detail> {
    let part = crate::parts::part(category, item)?;
    let availability = crate::parts::availability(who, category, item);
    let cost = crate::design::true_part_cost(&part, who);

    let tech = (0..TECH_FIELDS)
        .filter(|field| part.tech[*field] > 0)
        .map(|field| TechRequirement {
            field: TechField::ALL[field],
            level: part.tech[field],
            met: i16::from(who.levels[field]) >= i16::from(part.tech[field]),
        })
        .collect();

    Some(Detail {
        category,
        item,
        name: part.name,
        category_name: CATEGORIES
            .iter()
            .find(|(flag, _)| *flag == category)
            .map_or("", |(_, name)| *name),
        cost: [
            cost.minerals[0],
            cost.minerals[1],
            cost.minerals[2],
            cost.resources,
        ],
        picture: part.picture,
        // A planetary installation is never carried, so it has no mass to
        // speak of and the panel leaves the line out.
        mass: (category != slot::PLANETARY).then_some(part.mass),
        tech,
        stats: stats(category, item),
        notes: notes(who, category, item),
        availability,
    })
}

/// The figures that only make sense for one kind of component.
fn stats(category: u16, item: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut push = |label: &str, value: String| out.push((label.to_string(), value));

    match category {
        slot::ENGINE => {
            if let Some(engine) = ENGINES.get(item) {
                // The warp a ramscoop runs free to, which is what the
                // "free warp" line is about: the highest speed costing nothing.
                let free = engine
                    .fuel_used
                    .iter()
                    .enumerate()
                    .filter(|(warp, used)| *warp > 0 && **used == 0)
                    .map(|(warp, _)| warp)
                    .max()
                    .unwrap_or(0);
                if free > 0 {
                    push("Free up to warp:", free.to_string());
                }
                let ram = engine.abilities & 1 != 0;
                push(
                    "Type:",
                    if ram { "ramscoop" } else { "standard" }.to_string(),
                );
            }
        }
        slot::SCANNER => {
            if let Some(scanner) = SCANNERS.get(item) {
                // A negative range marks a scanner that also sees through a
                // planet, at half its magnitude — see `crate::scanning`.
                let range = i32::from(scanner.range.abs());
                push("Range:", format!("{range} l.y."));
                if scanner.range < 0 {
                    push("Penetrating range:", format!("{} l.y.", range / 2));
                }
            }
        }
        slot::SHIELD => {
            if let Some(shield) = SHIELDS.get(item) {
                push("Shield Strength:", format!("{}dp", shield.dp));
            }
        }
        slot::ARMOR => {
            if let Some(armor) = ARMORS.get(item) {
                push("Armor Strength:", format!("{}dp", armor.dp));
            }
        }
        slot::BEAM => {
            if let Some(beam) = BEAMS.get(item) {
                push("Range:", beam.range_max.to_string());
                push("Power:", beam.dp.to_string());
                push("Initiative:", beam.initiative.to_string());
            }
        }
        slot::TORPEDO => {
            if let Some(torpedo) = TORPEDOES.get(item) {
                push("Range:", torpedo.range_max.to_string());
                push("Power:", torpedo.dp.to_string());
                push("Initiative:", torpedo.initiative.to_string());
                push("Accuracy:", format!("{}%", torpedo.hit_chance));
            }
        }
        slot::BOMB => {
            if let Some(bomb) = BOMBS.get(item) {
                // The kill figure is in tenths of a percent.
                push(
                    "Kills:",
                    format!(
                        "{}.{}% of colonists",
                        bomb.colonist_damage / 10,
                        bomb.colonist_damage % 10
                    ),
                );
                push(
                    "Destroys:",
                    format!("{} installations", bomb.building_damage),
                );
            }
        }
        slot::HULL | slot::SB_HULL => {
            let hull: Option<&Hull> = if category == slot::HULL {
                HULLS.get(item)
            } else {
                STARBASE_HULLS.get(item)
            };
            if let Some(hull) = hull {
                push("Armor Strength:", format!("{}dp", hull.armor));
                push("Initiative:", hull.initiative.to_string());
                if hull.fuel_max > 0 {
                    push("Fuel Capacity:", format!("{}mg", hull.fuel_max));
                }
                if hull.unlimited_cargo() {
                    push("Cargo Capacity:", "Unlimited".to_string());
                } else if hull.cargo_max > 0 {
                    push("Cargo Capacity:", format!("{}kT", hull.cargo_max));
                }
                push("Slots:", hull.slot_count.to_string());
            }
        }
        slot::PLANETARY => {
            if let Some(planetary) = PLANETARY.get(item) {
                // Scanners store a range, defences a coverage rating, and a
                // negative range penetrates.
                if item < 9 {
                    let range = i32::from(planetary.ability.abs());
                    push("Range:", format!("{range} l.y."));
                    if planetary.ability < 0 {
                        push("Penetrating range:", format!("{} l.y.", range / 2));
                    }
                } else if planetary.ability > 0 {
                    push("Rating:", planetary.ability.to_string());
                }
            }
        }
        slot::SPECIAL_SB
        | slot::SPECIAL_E
        | slot::SPECIAL_M
        | slot::MINING
        | slot::MINES
        | slot::TERRA => {
            let ability = match category {
                slot::SPECIAL_SB => SPECIALS_SB.get(item).map(|p| p.ability),
                slot::SPECIAL_E => SPECIALS_E.get(item).map(|p| p.ability),
                slot::SPECIAL_M => SPECIALS_M.get(item).map(|p| p.ability),
                slot::MINING => MINING.get(item).map(|p| p.ability),
                slot::MINES => MINE_LAYERS.get(item).map(|p| p.ability),
                slot::TERRA => TERRAFORMING.get(item).map(|p| p.ability),
                _ => None,
            };
            if let Some(ability) = ability.filter(|a| *a != 0) {
                // The rating means something different in every one of these
                // tables — a mass driver's warp, a miner's kT a year, a mine
                // layer's mines a year — so it is labelled for its category.
                let label = match category {
                    slot::SPECIAL_SB if item >= 7 => "Warp:",
                    slot::MINING => "Mines:",
                    slot::MINES => "Lays:",
                    slot::TERRA => "Terraforms:",
                    _ => "Rating:",
                };
                push(label, ability.to_string());
            }
        }
        _ => {}
    }
    out
}

/// What stands between this player and building a component.
///
/// The original carries a sentence per component for this — a hundred and
/// fifty of them — saying things like which primary racial trait a stargate
/// belongs to. These are written from `crate::parts::requirements` instead, so
/// they are generated from the gate that is actually enforced and cannot drift
/// away from it.
fn notes(who: &Builder<'_>, category: u16, item: usize) -> Vec<String> {
    crate::parts::requirements(category, item)
        .into_iter()
        .filter(|need| !need.met(who.race, who.trader_parts))
        .map(|need| match need {
            Requirement::Prt(p) => {
                format!("Only a {} race can build this.", p.name())
            }
            Requirement::EitherPrt(a, b) => {
                format!("Only a {} or {} race can build this.", a.name(), b.name())
            }
            Requirement::NotPrt(p) => {
                format!("A {} race cannot build this.", p.name())
            }
            Requirement::Lrt(bit) => {
                format!("Needs the {} lesser racial trait.", lrt_name(bit))
            }
            Requirement::NotLrt(bit) => {
                format!("Not available with {}.", lrt_name(bit))
            }
            Requirement::Trader(_) => "Only the Mystery Trader can give you this.".to_string(),
        })
        .collect()
}

/// What a lesser racial trait is called.
///
/// The full table lives with the bits themselves; this only supplies the
/// wording for a trait the table does not name, which no real one is.
fn lrt_name(bit: u32) -> &'static str {
    crate::race::lrt::name(bit).unwrap_or("that lesser racial trait")
}

/// Step to the next or previous component, wrapping through the catalogue.
///
/// `BrowserDlg`'s Prev and Next buttons walk the items of a category and roll
/// over into the next or previous one, all the way round. With `buildable_only`
/// they stop only at something the player can build now; without it they stop
/// at anything the player is *allowed* to build eventually, which is what lets
/// the browser show a component and explain what is missing.
///
/// A component the Mystery Trader has not handed over is never stopped at,
/// because the player has no way of knowing it exists.
///
/// `within` limits the walk to one category, as choosing one in the dropdown
/// does; `None` is the dropdown's **All**.
#[must_use]
pub fn step(
    who: &Builder<'_>,
    within: Option<u16>,
    from: (u16, usize),
    forward: bool,
    buildable_only: bool,
) -> Option<(u16, usize)> {
    // The categories the walk covers, in the dropdown's order.
    let categories: Vec<u16> = match within {
        Some(category) => vec![category],
        None => CATEGORIES.iter().skip(1).map(|(flag, _)| *flag).collect(),
    };
    let mut at = categories.iter().position(|c| *c == from.0)?;
    let mut item = from.1;

    // At most one full pass round the catalogue, so an empty filter stops
    // rather than spinning.
    let total: usize = categories.iter().map(|c| category_len(*c)).sum();
    for _ in 0..total.max(1) {
        if forward {
            item += 1;
            if item >= category_len(categories[at]) {
                at = (at + 1) % categories.len();
                item = 0;
            }
        } else if item == 0 {
            at = (at + categories.len() - 1) % categories.len();
            item = category_len(categories[at]).saturating_sub(1);
        } else {
            item -= 1;
        }

        let category = categories[at];
        match crate::parts::availability(who, category, item) {
            Availability::Available => return Some((category, item)),
            Availability::Missing => {}
            _ if buildable_only => {}
            // Not yet researched, or a trait rules it out: still worth
            // showing, unless it is a Mystery Trader's to give.
            _ => {
                let trader = crate::parts::requirements(category, item)
                    .into_iter()
                    .any(|need| matches!(need, Requirement::Trader(_)));
                if !trader {
                    return Some((category, item));
                }
            }
        }
    }
    None
}

/// The first component the browser should show in a category.
#[must_use]
pub fn first(who: &Builder<'_>, within: Option<u16>, buildable_only: bool) -> Option<(u16, usize)> {
    let category = within.unwrap_or(slot::ARMOR);
    // Step from just before the start, so the same filtering applies.
    if matches!(
        crate::parts::availability(who, category, 0),
        Availability::Available
    ) {
        return Some((category, 0));
    }
    step(who, within, (category, 0), true, buildable_only)
}

/// Where the component in view is drawn from.
///
/// A component's picture is 64 pixels square out of `rghdibInventory`, indexed
/// by its `ibmp`; a **hull's** comes out of the ship sheets instead, which is
/// a different index space entirely — see [`crate::parts::picture_cell`]. The
/// browser shows a hull as the catalogue lists it, so the first of its four.
#[must_use]
pub fn picture(detail: &Detail) -> Option<stars_formats::resources::art::Cell> {
    crate::parts::picture_cell(detail.category, detail.picture, 0)
}
