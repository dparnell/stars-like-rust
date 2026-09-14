//! The built-in ship and starbase designs a new game hands out.
//!
//! Stars! ships a fixed table of designs (`rgshdefT`, 22 entries) and a second
//! one for starbases (`rgshdefSBT`, 4 entries). `GenerateWorld` copies entries
//! out of these into each player's design slots according to the race's primary
//! trait — a War Monger starts with an Armed Probe, an Inner Tech with a
//! Mayflower and a stargate — and then upgrades the components in them to
//! whatever that player's starting technology can already build (see
//! [`upgrade_slots`]).
//!
//! The slot lists here are the templates' `rghs` arrays verbatim, in hull-slot
//! order; the item numbers are indices into the component tables in
//! [`crate::components`], which is exactly what the original stores. Names are
//! the templates' own `szClass`.
//!
//! Source: `parts.c` in the reconstructed NB09 sources, cross-checked field by
//! field against the design records of the turn-0 fixture
//! (`fixtures/incoming/turn0/`).

use crate::components::slot;
use crate::design::{DesignSlot, ShipDesign};

/// One entry of the built-in design table.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    /// The design's class name (`HUL.szClass`).
    pub name: &'static str,
    /// Hull id.
    pub hull: i16,
    /// Picture index (`HUL.ibmp`), which the game draws the design with.
    pub picture: u8,
    /// Stored armour (`HUL.dp`). Zero for every ship template — a ship's
    /// armour comes from its hull and fitted plate — and 1000 for a starbase.
    pub armor: u16,
    /// The fitted slots, in hull-slot order: `(category, item, count)`.
    pub slots: &'static [(u16, u8, u8)],
}

impl Template {
    /// Turn this template into a design.
    #[must_use]
    pub fn design(&self) -> ShipDesign {
        ShipDesign {
            name: self.name.to_string(),
            picture: self.picture,
            stored_armor: self.armor,
            obsolete: false,
            hull_id: self.hull,
            slots: self
                .slots
                .iter()
                .map(|(category, item, count)| DesignSlot {
                    category: *category,
                    item: *item,
                    count: *count,
                })
                .collect(),
        }
    }
}

/// Index into [`SHIPS`], mirroring the game's `StartingShip` enum.
pub mod ship {
    /// Lilliputian Freighter.
    pub const LILLIPUTIAN_FREIGHTER: usize = 0;
    /// Shadow Transport.
    pub const SHADOW_TRANSPORT: usize = 1;
    /// Smaugarian Peeping Tom.
    pub const SMAUGARIAN_PEEPING_TOM: usize = 2;
    /// Armed Probe.
    pub const ARMED_PROBE: usize = 3;
    /// Long Range Scout.
    pub const LONG_RANGE_SCOUT: usize = 4;
    /// Shadow Sleuth.
    pub const SHADOW_SLEUTH: usize = 5;
    /// Teamster.
    pub const TEAMSTER: usize = 6;
    /// Stalwart Defender.
    pub const STALWART_DEFENDER: usize = 7;
    /// Swashbuckler.
    pub const SWASHBUCKLER: usize = 8;
    /// Santa Maria.
    pub const SANTA_MARIA: usize = 9;
    /// Pinta.
    pub const PINTA: usize = 10;
    /// Mayflower.
    pub const MAYFLOWER: usize = 11;
    /// Spore Cloud.
    pub const SPORE_CLOUD: usize = 12;
    /// Gadfly.
    pub const GADFLY: usize = 13;
    /// Cotton Picker.
    pub const COTTON_PICKER: usize = 14;
    /// Potato Bug.
    pub const POTATO_BUG: usize = 15;
    /// Little Hen.
    pub const LITTLE_HEN: usize = 16;
    /// Change of Heart.
    pub const CHANGE_OF_HEART: usize = 17;
    /// Speed Turtle.
    pub const SPEED_TURTLE: usize = 18;
    /// M.T. Lifeboat — a Mystery Trader gift, not a starting ship. Handed
    /// out by [`crate::wormhole`], and buildable by nobody.
    pub const MT_LIFEBOAT: usize = 19;
    /// M.T. Scout — a Mystery Trader gift.
    pub const MT_SCOUT: usize = 20;
    /// M.T. Probe — a Mystery Trader gift.
    pub const MT_PROBE: usize = 21;
}

/// Index into [`STARBASES`], mirroring the game's `StartingStarbase` enum.
pub mod starbase {
    /// The standard Space Station every race but Alternate Reality starts with.
    pub const STARBASE: usize = 0;
    /// Packet Physics' second design, which carries a mass driver.
    pub const ACCELERATOR_PLATFORM: usize = 1;
    /// Inner Tech's second design, which carries a stargate.
    pub const PORTHOLE_TO_BEYOND: usize = 2;
    /// The Orbital Fort an Alternate Reality race lives on.
    pub const STARTER_COLONY: usize = 3;
}

/// The design slot the first starbase design occupies (`ishdef` 16).
pub const FIRST_STARBASE_SLOT: u8 = 16;

/// The built-in ship design templates (`rgshdefT`).
pub const SHIPS: [Template; 22] = [
    Template {
        name: "Lilliputian Freighter",
        hull: 0,
        picture: 0,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::SHIELD, 0, 1),
        ],
    },
    Template {
        name: "Shadow Transport",
        hull: 0,
        picture: 2,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SPECIAL_E, 0, 1),
            (slot::SHIELD, 0, 1),
        ],
    },
    Template {
        name: "Smaugarian Peeping Tom",
        hull: 4,
        picture: 16,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::SPECIAL_M, 5, 1),
        ],
    },
    Template {
        name: "Armed Probe",
        hull: 4,
        picture: 17,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::BEAM, 1, 1),
        ],
    },
    Template {
        name: "Long Range Scout",
        hull: 4,
        picture: 18,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::SPECIAL_M, 5, 1),
        ],
    },
    Template {
        name: "Shadow Sleuth",
        hull: 4,
        picture: 19,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::SPECIAL_E, 1, 1),
        ],
    },
    Template {
        name: "Teamster",
        hull: 1,
        picture: 4,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::ARMOR, 0, 1),
        ],
    },
    Template {
        name: "Stalwart Defender",
        hull: 6,
        picture: 24,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::BEAM, 0, 1),
            (slot::TORPEDO, 0, 1),
            (slot::SCANNER, 0, 1),
            (slot::ARMOR, 0, 2),
            (slot::SPECIAL_M, 5, 1),
            (slot::SPECIAL_E, 5, 1),
        ],
    },
    Template {
        name: "Swashbuckler",
        hull: 11,
        picture: 44,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::ARMOR, 1, 2),
            (slot::SCANNER, 0, 1),
            (slot::BEAM, 0, 1),
            (slot::TORPEDO, 0, 1),
        ],
    },
    Template {
        name: "Santa Maria",
        hull: 15,
        picture: 60,
        armor: 0,
        slots: &[(slot::ENGINE, 1, 1), (slot::SPECIAL_M, 0, 1)],
    },
    Template {
        name: "Pinta",
        hull: 15,
        picture: 61,
        armor: 0,
        slots: &[(slot::ENGINE, 1, 1), (slot::SPECIAL_M, 1, 1)],
    },
    Template {
        name: "Mayflower",
        hull: 15,
        picture: 62,
        armor: 0,
        slots: &[(slot::ENGINE, 1, 1), (slot::SPECIAL_M, 0, 1)],
    },
    Template {
        name: "Spore Cloud",
        hull: 14,
        picture: 56,
        armor: 0,
        slots: &[(slot::ENGINE, 0, 1), (slot::SPECIAL_M, 0, 1)],
    },
    Template {
        name: "Gadfly",
        hull: 16,
        picture: 65,
        armor: 0,
        slots: &[(slot::ENGINE, 1, 1), (slot::BOMB, 0, 2)],
    },
    Template {
        name: "Cotton Picker",
        hull: 21,
        picture: 85,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::MINING, 1, 1),
            (slot::MINING, 1, 1),
        ],
    },
    Template {
        name: "Potato Bug",
        hull: 20,
        picture: 82,
        armor: 0,
        slots: &[(slot::ENGINE, 1, 1), (slot::MINING, 0, 2)],
    },
    Template {
        name: "Little Hen",
        hull: 27,
        picture: 108,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::MINES, 0, 2),
            (slot::MINES, 0, 2),
            (slot::SCANNER, 0, 1),
        ],
    },
    Template {
        name: "Change of Heart",
        hull: 21,
        picture: 86,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::SCANNER, 0, 1),
            (slot::MINING, 7, 1),
            (slot::MINING, 7, 1),
        ],
    },
    Template {
        name: "Speed Turtle",
        hull: 27,
        picture: 108,
        armor: 0,
        slots: &[
            (slot::ENGINE, 1, 1),
            (slot::MINES, 7, 2),
            (slot::MINES, 7, 2),
            (slot::SCANNER, 0, 1),
        ],
    },
    Template {
        name: "M.T. Lifeboat",
        hull: 29,
        picture: 108,
        armor: 0,
        slots: &[
            (slot::ENGINE, 8, 3),
            (slot::ARMOR, 9, 3),
            (slot::ARMOR, 9, 3),
            (slot::TORPEDO, 7, 3),
            (slot::TORPEDO, 7, 3),
            (slot::SHIELD, 6, 3),
            (slot::SHIELD, 6, 3),
            (slot::SPECIAL_E, 4, 3),
            (slot::SPECIAL_E, 4, 3),
            (slot::SPECIAL_M, 4, 3),
            (slot::BEAM, 18, 3),
            (slot::BEAM, 18, 3),
            (slot::BEAM, 18, 3),
        ],
    },
    Template {
        name: "M.T. Scout",
        hull: 30,
        picture: 122,
        armor: 0,
        slots: &[
            (slot::ENGINE, 8, 2),
            (slot::SHIELD, 6, 3),
            (slot::SPECIAL_E, 4, 1),
            (slot::SPECIAL_M, 4, 1),
            (slot::SPECIAL_M, 9, 1),
            (slot::TORPEDO, 7, 2),
            (slot::TORPEDO, 7, 2),
        ],
    },
    Template {
        name: "M.T. Probe",
        hull: 30,
        picture: 123,
        armor: 0,
        slots: &[
            (slot::ENGINE, 8, 2),
            (slot::ARMOR, 9, 3),
            (slot::SPECIAL_E, 4, 1),
            (slot::SPECIAL_M, 4, 1),
            (slot::SPECIAL_M, 9, 1),
            (slot::TORPEDO, 7, 2),
            (slot::TORPEDO, 7, 2),
        ],
    },
];

/// The built-in starbase design templates (`rgshdefSBT`).
pub const STARBASES: [Template; 4] = [
    Template {
        name: "Starbase",
        hull: 34,
        picture: 8,
        armor: 1000,
        slots: &[
            (slot::SPECIAL_SB, 0, 0),
            (slot::BEAM, 0, 8),
            (slot::SHIELD, 0, 8),
            (slot::BEAM, 0, 8),
            (slot::SHIELD, 0, 8),
            (slot::SHIELD, 0, 8),
            // `parts.c` prints these two sockets as `hstSpecialE | hstSpecialM`,
            // but the Space Station hull in the binary allows electrical
            // specials alone; the socket is empty either way.
            (slot::SPECIAL_E, 0, 0),
            (slot::BEAM, 0, 8),
            (slot::SPECIAL_E, 0, 0),
            (slot::BEAM, 0, 8),
            (slot::SPECIAL_SB, 1, 0),
            (slot::SHIELD, 0, 8),
        ],
    },
    Template {
        name: "Accelerator Platform",
        hull: 32,
        picture: 0,
        armor: 1000,
        slots: &[
            (slot::SPECIAL_SB, 7, 1),
            (slot::BEAM, 0, 6),
            (slot::SHIELD, 1, 6),
            (slot::BEAM, 0, 6),
            (slot::SHIELD, 1, 6),
        ],
    },
    Template {
        name: "Porthole to Beyond",
        hull: 32,
        picture: 1,
        armor: 1000,
        slots: &[
            (slot::SPECIAL_SB, 0, 1),
            (slot::BEAM, 0, 6),
            (slot::SHIELD, 0, 6),
            (slot::BEAM, 0, 6),
            (slot::SHIELD, 0, 6),
        ],
    },
    Template {
        name: "Starter Colony",
        hull: 32,
        picture: 1,
        armor: 1000,
        slots: &[
            (slot::SPECIAL_SB, 0, 0),
            (slot::BEAM, 0, 0),
            (slot::SHIELD, 0, 0),
            (slot::BEAM, 0, 0),
            (slot::SHIELD, 0, 0),
        ],
    },
];

/// Replace the placeholder components in a starting design with the best the
/// player can already build.
///
/// The templates are all written against the cheapest parts in the game, so a
/// race that starts with technology would otherwise begin with a Quick Jump 5
/// and a Bat Scanner. `GenerateWorld` walks every slot of every starting design
/// and, for the component classes below, tries a short list of substitutes in
/// order, taking the first the player's tech levels allow.
///
/// The candidate lists come from `create.c`; the order within the engine and
/// mining lists is the one the turn-0 fixture's design records actually show,
/// which differs from the order the decompiled source prints — see
/// `docs/formulas/new-game.md`.
pub fn upgrade_slots(design: &mut ShipDesign, levels: &[u8; 6]) {
    for s in &mut design.slots {
        let same = [s.item];
        let candidates: &[u8] = match s.category {
            slot::ENGINE if s.item == 1 => &[10, 5, 4, 3, 2],
            slot::SCANNER if s.item == 0 || s.item == 1 => &[4, 2, 1],
            slot::SHIELD | slot::ARMOR if s.item == 0 || s.item == 1 => &[2, 1],
            slot::BEAM if s.item == 0 || s.item == 1 => &[3, 1],
            slot::TORPEDO | slot::BOMB if s.item == 0 => &[1],
            slot::MINING if s.item == 0 || s.item == 1 => &[2],
            _ => continue,
        };
        for candidate in candidates.iter().chain(same.iter()) {
            if buildable(s.category, *candidate, levels) {
                s.item = *candidate;
                break;
            }
        }
    }
}

/// Whether the player's technology allows one component.
///
/// Every component category is covered, though [`upgrade_slots`] only ever
/// asks about the six it substitutes in.
fn buildable(category: u16, item: u8, levels: &[u8; 6]) -> bool {
    let index = usize::from(item);
    let tech: Option<&[i8; 6]> = match category {
        slot::ENGINE => crate::components::ENGINES.get(index).map(|c| &c.tech),
        slot::SCANNER => crate::components::SCANNERS.get(index).map(|c| &c.tech),
        slot::SHIELD => crate::components::SHIELDS.get(index).map(|c| &c.tech),
        slot::ARMOR => crate::components::ARMORS.get(index).map(|c| &c.tech),
        slot::BEAM => crate::components::BEAMS.get(index).map(|c| &c.tech),
        slot::TORPEDO => crate::components::TORPEDOES.get(index).map(|c| &c.tech),
        slot::BOMB => crate::components::BOMBS.get(index).map(|c| &c.tech),
        slot::MINING => crate::components::MINING.get(index).map(|c| &c.tech),
        slot::MINES => crate::components::MINE_LAYERS.get(index).map(|c| &c.tech),
        slot::SPECIAL_E => crate::components::SPECIALS_E.get(index).map(|c| &c.tech),
        slot::SPECIAL_M => crate::components::SPECIALS_M.get(index).map(|c| &c.tech),
        slot::SPECIAL_SB => crate::components::SPECIALS_SB.get(index).map(|c| &c.tech),
        slot::TERRA => crate::components::TERRAFORMING.get(index).map(|c| &c.tech),
        _ => None,
    };
    let Some(tech) = tech else { return false };
    (0..6).all(|f| i16::from(levels[f]) >= i16::from(tech[f]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::hull;

    /// Every template's slots line up with the hull it names.
    ///
    /// This is the check that catches a mis-transcribed table: the slot list is
    /// positional — the *n*th entry fills the hull's *n*th slot — so a missing
    /// or reordered entry puts a scanner in an engine socket.
    #[test]
    fn every_template_fits_its_hull() {
        for template in SHIPS.iter().chain(STARBASES.iter()) {
            let hull = hull(template.hull)
                .unwrap_or_else(|| panic!("{}: no hull {}", template.name, template.hull));
            let slots = hull.real_slots();
            assert!(
                template.slots.len() <= slots.len(),
                "{}: {} fitted slots but the hull has {}",
                template.name,
                template.slots.len(),
                slots.len()
            );
            for (i, (category, _, count)) in template.slots.iter().enumerate() {
                assert_eq!(
                    *category & slots[i].allowed,
                    *category,
                    "{} slot {i}: category {category:#06x} is not allowed by the hull",
                    template.name
                );
                assert!(
                    u16::from(*count) <= u16::from(slots[i].capacity),
                    "{} slot {i}: {count} fitted but the slot holds {}",
                    template.name,
                    slots[i].capacity
                );
            }
        }
    }

    /// Every template's items exist in the table its category names.
    #[test]
    fn every_template_names_a_real_component() {
        for template in SHIPS.iter().chain(STARBASES.iter()) {
            for (category, item, count) in template.slots {
                if *count == 0 {
                    // An empty starbase socket names item 0 as a placeholder.
                    continue;
                }
                assert!(
                    buildable(*category, *item, &[26; 6]),
                    "{}: {category:#06x} item {item} is not in the tables",
                    template.name
                );
            }
        }
    }

    /// A race with no technology keeps every placeholder part.
    #[test]
    fn no_technology_upgrades_nothing() {
        let mut design = SHIPS[ship::STALWART_DEFENDER].design();
        let before = design.slots.clone();
        upgrade_slots(&mut design, &[0; 6]);
        assert_eq!(design.slots, before);
    }

    /// A race with everything takes the best of each candidate list.
    #[test]
    fn full_technology_takes_the_first_candidate() {
        let mut design = SHIPS[ship::STALWART_DEFENDER].design();
        upgrade_slots(&mut design, &[26; 6]);
        let items: Vec<u8> = design.slots.iter().map(|s| s.item).collect();
        // Radiating Hydro-Ram Scoop, Yakimora Light Phaser, Beta Torpedo,
        // Possum Scanner, Carbonic Armor, and the two specials untouched.
        assert_eq!(items, vec![10, 3, 1, 4, 2, 5, 5]);
    }
}
