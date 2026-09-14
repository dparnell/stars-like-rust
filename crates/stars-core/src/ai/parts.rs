//! The parts a computer player fits, and the fittings it fits them in.
//!
//! `FCreateAiShdef` (`1090:012c`) builds a design from a hull and a
//! **fitting**: one *AI part class* per hull slot. `FGetAIPart`
//! (`1090:043e`) turns a class into a component: each class is a short
//! list of candidates, `(slot type, item, tries)`, and the first candidate
//! the player can build wins — trying the item named and then the `tries −
//! 1` items below it, which is how one entry stands for a run of a table.
//! The class counts are `vrgcAiParts` (`1120:1450`, 45 classes) and the
//! candidate words are at the head of the AI code segment (`1090:0000`,
//! 139 of them): bits 0–3 the slot type, 4–8 the item, 9–12 the tries.
//!
//! The fittings are byte strings in the TurinDrone's own segment
//! (`1088:35c2`), reached through a table of offsets (`1088:35b0`); each
//! byte is a class, one per hull slot in hull order, and the slot is
//! filled to its capacity. A fitting with a class the player cannot yet
//! fill fails as a whole, which is why a young TurinDrone builds nothing
//! new: its engine class wants the Trans-Star 10, the scoops or the Fuel
//! Mizer, and not the Quick Jump 5.
//!
//! The candidates are the game's own tables, read out of the executable;
//! the item numbers are indices into [`crate::components`].

use crate::components::{slot, HULLS};
use crate::design::{DesignSlot, ShipDesign};
use crate::parts::{availability, Builder};

/// One candidate of a part class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// The slot type, as a [`slot`] flag.
    pub category: u16,
    /// The item to try first.
    pub item: u8,
    /// How many items to try, this one and the ones below it.
    pub tries: u8,
}

const fn cand(category: u16, item: u8, tries: u8) -> Candidate {
    Candidate {
        category,
        item,
        tries,
    }
}

/// The forty-five part classes, `vrgcAiParts` and the words at `1090:0000`.
pub static PART_CLASSES: [&[Candidate]; 45] = [
    // 0
    &[cand(slot::TORPEDO, 7, 8)],
    // 1
    &[cand(slot::TORPEDO, 11, 4)],
    // 2
    &[
        cand(slot::BEAM, 18, 1),
        cand(slot::BEAM, 20, 1),
        cand(slot::BEAM, 13, 1),
        cand(slot::BEAM, 7, 1),
    ],
    // 3
    &[
        cand(slot::BEAM, 23, 1),
        cand(slot::BEAM, 17, 1),
        cand(slot::BEAM, 11, 1),
        cand(slot::BEAM, 5, 1),
    ],
    // 4
    &[
        cand(slot::BEAM, 22, 1),
        cand(slot::BEAM, 15, 1),
        cand(slot::BEAM, 9, 1),
        cand(slot::BEAM, 3, 1),
        cand(slot::BEAM, 1, 1),
        cand(slot::BEAM, 0, 1),
    ],
    // 5
    &[
        cand(slot::BEAM, 16, 1),
        cand(slot::BEAM, 10, 1),
        cand(slot::BEAM, 4, 1),
    ],
    // 6
    &[
        cand(slot::BEAM, 21, 1),
        cand(slot::BEAM, 14, 1),
        cand(slot::BEAM, 8, 1),
        cand(slot::BEAM, 2, 1),
    ],
    // 7
    &[
        cand(slot::BEAM, 19, 1),
        cand(slot::BEAM, 12, 1),
        cand(slot::BEAM, 6, 1),
    ],
    // 8
    &[
        cand(slot::ENGINE, 15, 1),
        cand(slot::ENGINE, 8, 1),
        cand(slot::ENGINE, 14, 5),
        cand(slot::ENGINE, 2, 1),
    ],
    // 9
    &[
        cand(slot::ARMOR, 11, 1),
        cand(slot::ARMOR, 9, 1),
        cand(slot::ARMOR, 10, 1),
        cand(slot::ARMOR, 7, 1),
        cand(slot::ARMOR, 8, 1),
        cand(slot::ARMOR, 6, 7),
    ],
    // 10
    &[
        cand(slot::SHIELD, 9, 2),
        cand(slot::SHIELD, 6, 1),
        cand(slot::SHIELD, 7, 1),
        cand(slot::SHIELD, 3, 1),
        cand(slot::SHIELD, 4, 1),
        cand(slot::SHIELD, 5, 1),
        cand(slot::SHIELD, 2, 3),
    ],
    // 11
    &[cand(slot::SPECIAL_E, 7, 3)],
    // 12
    &[
        cand(slot::SPECIAL_E, 4, 1),
        cand(slot::SPECIAL_E, 11, 4),
        cand(slot::SPECIAL_M, 10, 1),
        cand(slot::SPECIAL_M, 8, 2),
    ],
    // 13
    &[
        cand(slot::SPECIAL_E, 4, 1),
        cand(slot::SPECIAL_M, 8, 2),
        cand(slot::SPECIAL_E, 3, 1),
        cand(slot::SPECIAL_M, 10, 1),
        cand(slot::SPECIAL_E, 2, 2),
    ],
    // 14
    &[cand(slot::SPECIAL_E, 13, 6), cand(slot::SPECIAL_M, 6, 2)],
    // 15
    &[
        cand(slot::SPECIAL_M, 10, 1),
        cand(slot::SPECIAL_E, 13, 6),
        cand(slot::SPECIAL_E, 11, 4),
        cand(slot::SPECIAL_M, 6, 2),
    ],
    // 16
    &[cand(slot::SPECIAL_M, 4, 3)],
    // 17
    &[cand(slot::ARMOR, 9, 1), cand(slot::ARMOR, 11, 12)],
    // 18
    &[
        cand(slot::SPECIAL_M, 8, 2),
        cand(slot::SPECIAL_M, 10, 1),
        cand(slot::SPECIAL_M, 6, 2),
    ],
    // 19
    &[cand(slot::SPECIAL_E, 11, 4), cand(slot::SPECIAL_E, 7, 3)],
    // 20
    &[
        cand(slot::SPECIAL_E, 13, 6),
        cand(slot::SPECIAL_E, 11, 4),
        cand(slot::SPECIAL_E, 4, 4),
        cand(slot::SPECIAL_E, 7, 3),
    ],
    // 21
    &[cand(slot::BOMB, 8, 1), cand(slot::BOMB, 4, 5)],
    // 22
    &[
        cand(slot::BOMB, 8, 1),
        cand(slot::BOMB, 9, 1),
        cand(slot::BOMB, 14, 5),
    ],
    // 23
    &[
        cand(slot::BOMB, 8, 1),
        cand(slot::BOMB, 14, 5),
        cand(slot::BOMB, 4, 5),
        cand(slot::BOMB, 9, 1),
    ],
    // 24
    &[cand(slot::ENGINE, 15, 1), cand(slot::ENGINE, 10, 1)],
    // 25
    &[cand(slot::MINES, 3, 4)],
    // 26
    &[
        cand(slot::SCANNER, 12, 1),
        cand(slot::SCANNER, 14, 1),
        cand(slot::SCANNER, 8, 1),
        cand(slot::SCANNER, 6, 1),
        cand(slot::SCANNER, 7, 1),
        cand(slot::SCANNER, 9, 1),
        cand(slot::SCANNER, 4, 1),
    ],
    // 27
    &[
        cand(slot::SCANNER, 14, 1),
        cand(slot::SCANNER, 5, 1),
        cand(slot::SCANNER, 6, 7),
    ],
    // 28
    &[cand(slot::MINING, 6, 7)],
    // 29
    &[cand(slot::MINING, 7, 1)],
    // 30
    &[
        cand(slot::ENGINE, 9, 2),
        cand(slot::ENGINE, 10, 1),
        cand(slot::ENGINE, 13, 3),
        cand(slot::ENGINE, 6, 4),
    ],
    // 31
    &[cand(slot::SPECIAL_M, 1, 2)],
    // 32
    &[cand(slot::MINES, 9, 3)],
    // 33
    &[cand(slot::BEAM, 18, 1)],
    // 34
    &[
        cand(slot::SPECIAL_SB, 15, 9),
        cand(slot::SPECIAL_E, 4, 4),
        cand(slot::SPECIAL_E, 11, 4),
        cand(slot::SPECIAL_E, 7, 3),
    ],
    // 35
    &[
        cand(slot::TORPEDO, 11, 1),
        cand(slot::TORPEDO, 6, 1),
        cand(slot::TORPEDO, 10, 1),
        cand(slot::TORPEDO, 5, 6),
    ],
    // 36
    &[
        cand(slot::BEAM, 23, 2),
        cand(slot::BEAM, 17, 1),
        cand(slot::BEAM, 15, 1),
        cand(slot::BEAM, 11, 1),
        cand(slot::BEAM, 9, 1),
        cand(slot::BEAM, 5, 1),
        cand(slot::BEAM, 3, 1),
        cand(slot::BEAM, 1, 2),
    ],
    // 37
    &[cand(slot::SHIELD, 6, 1), cand(slot::SHIELD, 9, 10)],
    // 38
    &[
        cand(slot::BEAM, 20, 1),
        cand(slot::BEAM, 13, 1),
        cand(slot::BEAM, 7, 1),
        cand(slot::BEAM, 5, 1),
        cand(slot::BEAM, 0, 1),
    ],
    // 39
    &[cand(slot::SPECIAL_E, 11, 4), cand(slot::SPECIAL_E, 4, 4)],
    // 40
    &[cand(slot::SPECIAL_M, 1, 1)],
    // 41
    &[
        cand(slot::ARMOR, 9, 1),
        cand(slot::SPECIAL_E, 11, 4),
        cand(slot::SPECIAL_M, 8, 2),
        cand(slot::SPECIAL_M, 10, 1),
        cand(slot::SPECIAL_M, 6, 2),
    ],
    // 42
    &[cand(slot::MINING, 6, 4)],
    // 43
    &[cand(slot::MINING, 6, 2), cand(slot::MINING, 0, 1)],
    // 44
    &[
        cand(slot::ENGINE, 15, 5),
        cand(slot::ENGINE, 2, 1),
        cand(slot::ENGINE, 0, 0),
    ],
];

/// A fitting: one part class per hull slot, in hull order.
pub type Fitting = &'static [u8];

/// The TurinDrone's fittings, from `1088:35c2` by the offsets at
/// `1088:35b0`. The hull each is for is the one `EnsureTurinDroneShdefs`
/// names beside it.
pub mod fitting {
    use super::Fitting;
    /// Offset 0: a Colony Ship (hull 15) — an engine and a colony module.
    pub const COLONY_SHIP: Fitting = &[8, 31];
    /// Offset 2: a Frigate (5) scout — engine, scanner, torpedo, shield.
    pub const SCOUT: Fitting = &[8, 26, 0, 37];
    /// Offset 6: a Destroyer (6) — three torpedoes, armour, a mechanical
    /// and an electrical.
    pub const DESTROYER: Fitting = &[8, 0, 0, 0, 9, 18, 11];
    /// Offset 13: the other Destroyer.
    pub const DESTROYER_B: Fitting = &[8, 1, 1, 11, 9, 18, 11];
    /// Offsets 48, 59, 70 and 81: the four Battleship (9) fittings one is
    /// drawn from at random.
    pub const BATTLESHIPS: [Fitting; 4] = [
        &[8, 12, 37, 6, 3, 5, 3, 7, 9, 20, 20],
        &[8, 12, 37, 0, 0, 0, 0, 0, 9, 19, 11],
        &[8, 12, 37, 6, 3, 4, 2, 7, 17, 20, 20],
        &[8, 12, 37, 1, 1, 1, 1, 1, 17, 19, 11],
    ];
    /// Offset 92: a Rogue (12).
    pub const ROGUE: Fitting = &[8, 10, 16, 27, 17, 0, 13, 39, 11];
    /// Offset 101: a Stealth Bomber (18).
    pub const STEALTH_BOMBER: Fitting = &[8, 21, 22, 12, 39];
    /// Offset 106: a Privateer (11) mine layer.
    pub const MINE_LAYER: Fitting = &[8, 10, 12, 25, 25];
    /// Offset 111: a Galleon (13).
    pub const GALLEON: Fitting = &[8, 37, 17, 0, 13, 11, 16, 27];
    /// Offset 123: a Miner (22).
    pub const MINER: Fitting = &[8, 13, 28, 28, 28, 28];
}

/// `FGetAIPart`: the first candidate of a class the player can build,
/// walking each entry's items downward.
#[must_use]
pub fn pick_part(class: u8, who: &Builder<'_>) -> Option<(u16, u8)> {
    let candidates = PART_CLASSES.get(usize::from(class))?;
    for candidate in candidates.iter() {
        for step in 0..candidate.tries {
            let Some(item) = candidate.item.checked_sub(step) else {
                break;
            };
            if availability(who, candidate.category, usize::from(item)).is_available() {
                return Some((candidate.category, item));
            }
        }
    }
    None
}

/// `FCreateAiShdef`: a design on `hull` fitted from `fitting`, every slot
/// at its capacity, or `None` when any slot's class has nothing the
/// player can build — the whole design fails, not the slot.
///
/// The name and picture are `PickANameAndBmp`'s business and are left to
/// the caller; the design comes back unnamed.
#[must_use]
pub fn create_design(hull: i16, fitting: Fitting, who: &Builder<'_>) -> Option<ShipDesign> {
    let hull_def = HULLS.get(usize::try_from(hull).ok()?)?;
    if availability(who, slot::HULL, usize::try_from(hull).ok()?)
        != crate::parts::Availability::Available
    {
        return None;
    }
    // The hull's slots run to its `chs`; the table pads the rest with empty
    // entries.
    let mut slots = Vec::with_capacity(hull_def.slots.len());
    for (index, hull_slot) in hull_def
        .slots
        .iter()
        .take_while(|s| s.allowed != 0)
        .enumerate()
    {
        let class = *fitting.get(index)?;
        let (category, item) = pick_part(class, who)?;
        slots.push(DesignSlot {
            category,
            item,
            count: hull_slot.capacity,
        });
    }
    Some(ShipDesign {
        name: String::new(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: hull,
        slots,
    })
}

/// The name groups `PickANameAndBmp` draws a class name from — the game's
/// own lists, strings `0x03d4` to `0x0431`, each a short label. Which group
/// a hull takes is decided by a word of the hull's the reader has not yet
/// pinned down; [`name_group`] chooses by the hull's role instead, which
/// agrees with what the tutorial calls the Berserkers' ships — their mine
/// layers are Saguaros, from the Prickly Pear group.
pub mod names {
    /// Ten, for scouts.
    pub const EASTER_BUNNY: &[&str] = &[
        "Easter Bunny",
        "Killjoy",
        "Momma's helper",
        "Turtle",
        "Poodle",
        "Mite",
        "Gnat",
        "Robin",
        "Ostrich",
        "Goose",
    ];
    /// Sixteen, for warships.
    pub const LYING_BASTARD: &[&str] = &[
        "Lying Bastard",
        "Pit Bull",
        "Toothless Tiger",
        "Rhode Island Red",
        "Ram Rod",
        "Spitting Cobra",
        "Venomous Dreadnought",
        "Crown Jewel",
        "Silver Serpent",
        "Xenocide",
        "Typhoon",
        "Quark",
        "Whip",
        "Lash",
        "Terror",
        "Dog of War",
    ];
    /// Twelve, for bombers.
    pub const PIDGEON: &[&str] = &[
        "Pidgeon",
        "Raging Rukh",
        "Manifest Destiny",
        "Flying Cow",
        "Bitter Harvest",
        "Peacock",
        "Saguaro",
        "Badlands Express",
        "Bright Spot",
        "Strange Love",
        "Dr. Death",
        "Scorch",
    ];
    /// Eight, for miners.
    pub const GROUND_HOG: &[&str] = &[
        "Ground Hog",
        "Naked Mole Rat",
        "Terrier",
        "Pick",
        "Gouge",
        "Gorge",
        "Gut",
        "Airdale",
    ];
    /// Eight, for colony ships.
    pub const EGG: &[&str] = &[
        "Egg", "Busy Bee", "Seeder", "Spore", "Phoenix", "Plymouth", "Duty", "Vassal",
    ];
    /// Eight, for freighters.
    pub const GLOVEBOX: &[&str] = &[
        "Glovebox",
        "Perfect Logic",
        "Boxcar",
        "Boot",
        "Peet",
        "C74",
        "Lor",
        "Black Hold",
    ];
    /// Eight, for mine layers.
    pub const PRICKLY_PEAR: &[&str] = &[
        "Prickly Pear",
        "Bristly Llama",
        "Silent Mule",
        "Saguaro",
        "Crunchy Critter",
        "Long John Silver",
        "Blackbeard",
        "Thistle",
    ];
    /// Eight, for anything else.
    pub const ZOMBIE: &[&str] = &[
        "Zombie",
        "Typhoid",
        "Zeppo",
        "Lucky Eddie",
        "Widget",
        "Poly",
        "Mog",
        "Ranger",
    ];
    /// Sixteen, for the armed haulers.
    pub const SCRAPPER: &[&str] = &[
        "Scrapper",
        "Bogey",
        "Horse Fly",
        "Hornet",
        "Dragon Fly",
        "Wasp",
        "Intruder",
        "Interceptor",
        "Quest",
        "Infinite Vision",
        "Brass Knuckle",
        "Talon",
        "Naagra",
        "Cattle Prod",
        "Asunder",
        "Blade",
    ];
}

/// The name group for a hull, by its role.
#[must_use]
pub fn name_group(hull: i16) -> &'static [&'static str] {
    match hull {
        // Scout and Frigate.
        4 | 5 => names::EASTER_BUNNY,
        // Destroyer to Dreadnought, Nubian, the Morphs.
        6..=10 | 29..=31 => names::LYING_BASTARD,
        // Privateer.
        11 => names::PRICKLY_PEAR,
        // Rogue and Galleon.
        12 | 13 => names::SCRAPPER,
        // The colony ships.
        14 | 15 => names::EGG,
        // The bombers.
        16..=19 => names::PIDGEON,
        // The miners.
        20..=24 => names::GROUND_HOG,
        // The freighters and fuel transports.
        0..=3 | 25 | 26 => names::GLOVEBOX,
        // The mine layers on their own hulls.
        27 | 28 => names::PRICKLY_PEAR,
        _ => names::ZOMBIE,
    }
}

/// `PickANameAndBmp`'s name: one of the group at random that no live design
/// of the player's already carries; after twenty misses, a name with a
/// number after it.
#[must_use]
pub fn pick_name(hull: i16, taken: &[String], rng: &mut crate::rng::Rng) -> String {
    let group = name_group(hull);
    let count = i16::try_from(group.len()).unwrap_or(1);
    for _ in 0..20 {
        let pick = usize::try_from(rng.random(count)).unwrap_or(0);
        let name = group[pick.min(group.len() - 1)];
        if !taken.iter().any(|t| t == name) {
            return name.to_string();
        }
    }
    let number = rng.random(100);
    let pick = usize::try_from(rng.random(count)).unwrap_or(0);
    format!("{} {number}", group[pick.min(group.len() - 1)])
}
