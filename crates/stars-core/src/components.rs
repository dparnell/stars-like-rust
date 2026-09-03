//! The ship and planetary component tables.
//!
//! Every component the game can build is a static table in the executable's
//! data segment. These are transcribed here so that fuel use, scanner ranges
//! and build costs can be computed without the binary.
//!
//! | Table | Address | Entries |
//! |-------|---------|---------|
//! | [`ENGINES`] | `1008:0000` | 16 |
//! | [`ARMORS`] | `1008:04e0` | 12 |
//! | [`SCANNERS`] | `1008:0768` | 16 |
//! | [`SHIELDS`] | `1008:0ae8` | 10 |
//! | [`PLANETARY`] | `1008:16b8` | 15 |
//! | [`TORPEDOES`] | `1008:2180` | 12 |
//! | [`BEAMS`] | `1008:2450` | 24 |
//! | [`SPECIALS_E`] / [`SPECIALS_M`] / [`SPECIALS_SB`] | `rgspecialE` / `rgspecialM` / `rgspecialSB` | 17 / 11 / 16 |
//! | [`MINING`] / [`MINE_LAYERS`] / [`TERRAFORMING`] | `rgmining` / `rgmines` / `rgterra` | 8 / 10 / 20 |
//! | [`BOMBS`] | `rgbomb` | 15 |
//! | [`HULLS`] | `rghuldef` | 32 |
//! | [`STARBASE_HULLS`] | `rghuldefSB` | 5 |
//!
//! Every component shares a common header — id, the six tech levels needed to
//! build it, name, mass, and the resource and mineral cost — followed by
//! type-specific fields.
//!
//! The tables are **generated**, from the reconstructed NB09 source
//! (`parts.c`), by the script recorded in `docs/formulas/components.md`, and
//! spot-checked against our own binary by
//! `crates/stars-core/tests/component_tables.rs`, which reads the same bytes
//! out of the executable's data segment.

/// Number of technology fields a component's requirements are expressed in.
pub const TECH_FIELDS: usize = 6;

/// Tech levels required in each field to build a component.
pub type TechRequirement = [i8; TECH_FIELDS];

/// Mineral cost: ironium, boranium, germanium.
pub type OreCost = [i16; 3];

/// An engine. `fuel_used` is indexed by warp factor `0..=11`; a zero entry
/// means the engine runs free at that speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Engine {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// Ability flags (bit 0 marks a ramscoop).
    pub abilities: i16,
    /// Fuel used per warp factor, in the scaled units [`crate::movement`] uses.
    pub fuel_used: [i16; 12],
}

/// Armour: adds damage points to a hull.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Armor {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// Damage points added.
    pub dp: i16,
}

/// A shield: damage points that regenerate between battles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shield {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// Shield points added.
    pub dp: i16,
}

/// A ship scanner. A **negative** range marks a planet-penetrating scanner,
/// whose penetrating range is half its magnitude — see [`crate::scanning`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scanner {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// Scanning range in light years; negative means penetrating.
    pub range: i16,
    /// Ability flags.
    pub abilities: i16,
}

/// A planetary installation: scanners and defences share this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planetary {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT (always zero — these are not carried).
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// For scanners, the range (negative means penetrating); for defences, the
    /// coverage rating.
    pub ability: i16,
}

/// A beam weapon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beam {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// Maximum range in battle squares.
    pub range_max: i16,
    /// Damage points per shot.
    pub dp: i16,
    /// Firing initiative.
    pub initiative: i16,
    /// Ability flags (sappers, gattlings and the like).
    pub abilities: i16,
}

/// A special-purpose component: cargo pods, fuel tanks, capacitors, jammers,
/// mining robots, mine layers, terraforming modules and starbase specials all
/// share this shape. `ability` means different things per table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Special {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// The component's rating; its meaning depends on the table.
    pub ability: i16,
}

/// A bomb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bomb {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// Bombing rounds.
    pub rounds: i16,
    /// Colonists killed, in tenths of a percent.
    pub colonist_damage: i16,
    /// Installations destroyed.
    pub building_damage: i16,
}

/// Which components a hull slot accepts, and how many fit.
///
/// `allowed` is a bitmask of [`slot`] flags; a slot that takes either a shield
/// or armour has both bits set. `capacity` is how many copies of the chosen
/// component fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HullSlot {
    /// Bitmask of the component categories this slot accepts.
    pub allowed: u16,
    /// How many components fit in the slot.
    pub capacity: u8,
}

/// Hull-slot category flags (the game's `HullSlotType`).
pub mod slot {
    /// Engine.
    pub const ENGINE: u16 = 0x0001;
    /// Scanner.
    pub const SCANNER: u16 = 0x0002;
    /// Shield.
    pub const SHIELD: u16 = 0x0004;
    /// Armour.
    pub const ARMOR: u16 = 0x0008;
    /// Beam weapon.
    pub const BEAM: u16 = 0x0010;
    /// Torpedo launcher.
    pub const TORPEDO: u16 = 0x0020;
    /// Bomb.
    pub const BOMB: u16 = 0x0040;
    /// Mining robot.
    pub const MINING: u16 = 0x0080;
    /// Mine layer.
    pub const MINES: u16 = 0x0100;
    /// Starbase-only special.
    pub const SPECIAL_SB: u16 = 0x0200;
    /// Starbase hull.
    pub const SB_HULL: u16 = 0x0400;
    /// Electrical special.
    pub const SPECIAL_E: u16 = 0x0800;
    /// Mechanical special.
    pub const SPECIAL_M: u16 = 0x1000;
    /// Terraforming module.
    pub const TERRA: u16 = 0x2000;
    /// Ship hull.
    pub const HULL: u16 = 0x4000;
    /// Planetary installation.
    pub const PLANETARY: u16 = 0x8000;
}

/// A hull: the chassis a design is built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hull {
    /// Hull id. Ship hulls are 0..=31 and starbase hulls 32..=36.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass of the bare hull, in kT.
    pub empty_mass: u16,
    /// Resource cost of the bare hull.
    pub resource_cost: u16,
    /// Mineral cost of the bare hull.
    pub ore_cost: [u16; 3],
    /// Cargo capacity, in kT.
    pub cargo_max: u16,
    /// Fuel capacity, in mg.
    pub fuel_max: u16,
    /// Armour of the bare hull, in damage points.
    pub armor: u16,
    /// Base battle initiative.
    pub initiative: u8,
    /// Hull category, used for battle targeting.
    pub category: u8,
    /// How many of [`Hull::slots`] are real.
    pub slot_count: u8,
    /// The slot layout; only the first [`Hull::slot_count`] are meaningful.
    pub slots: [HullSlot; 16],
}

impl Hull {
    /// The slots that actually exist on this hull.
    #[must_use]
    pub fn real_slots(&self) -> &[HullSlot] {
        &self.slots[..usize::from(self.slot_count).min(self.slots.len())]
    }
}

/// A torpedo or missile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Torpedo {
    /// Component id.
    pub id: i16,
    /// Tech levels required.
    pub tech: TechRequirement,
    /// Display name.
    pub name: &'static str,
    /// Mass in kT.
    pub mass: i16,
    /// Resource cost to build.
    pub resource_cost: u16,
    /// Mineral cost.
    pub ore_cost: OreCost,
    /// Maximum range in battle squares.
    pub range_max: i16,
    /// Damage points on a hit.
    pub dp: i16,
    /// Firing initiative.
    pub initiative: i16,
    /// Base accuracy, as a percentage.
    pub hit_chance: i16,
}

// --- rgengine (16 entries) ---
pub const ENGINES: [Engine; 16] = [
    Engine {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Settler's Delight",
        mass: 2,
        resource_cost: 2,
        ore_cost: [1, 0, 1],
        abilities: 1,
        fuel_used: [0, 0, 0, 0, 0, 0, 0, 140, 275, 480, 576, 0],
    },
    Engine {
        id: 2,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Quick Jump 5",
        mass: 4,
        resource_cost: 3,
        ore_cost: [3, 0, 1],
        abilities: 0,
        fuel_used: [0, 0, 25, 100, 100, 100, 180, 500, 800, 900, 1080, 0],
    },
    Engine {
        id: 3,
        tech: [0, 0, 2, 0, 0, 0],
        name: "Fuel Mizer",
        mass: 6,
        resource_cost: 11,
        ore_cost: [8, 0, 0],
        abilities: 3,
        fuel_used: [0, 0, 0, 0, 0, 35, 120, 175, 235, 360, 420, 0],
    },
    Engine {
        id: 4,
        tech: [0, 0, 3, 0, 0, 0],
        name: "Long Hump 6",
        mass: 9,
        resource_cost: 6,
        ore_cost: [5, 0, 1],
        abilities: 0,
        fuel_used: [0, 0, 20, 60, 100, 100, 105, 450, 750, 900, 1080, 0],
    },
    Engine {
        id: 5,
        tech: [0, 0, 5, 0, 0, 0],
        name: "Daddy Long Legs 7",
        mass: 13,
        resource_cost: 12,
        ore_cost: [11, 0, 3],
        abilities: 0,
        fuel_used: [0, 0, 20, 60, 70, 100, 100, 110, 600, 750, 900, 0],
    },
    Engine {
        id: 6,
        tech: [0, 0, 7, 0, 0, 0],
        name: "Alpha Drive 8",
        mass: 17,
        resource_cost: 28,
        ore_cost: [16, 0, 3],
        abilities: 0,
        fuel_used: [0, 0, 15, 50, 60, 70, 100, 100, 115, 700, 840, 0],
    },
    Engine {
        id: 7,
        tech: [0, 0, 9, 0, 0, 0],
        name: "Trans-Galactic Drive",
        mass: 25,
        resource_cost: 50,
        ore_cost: [20, 20, 9],
        abilities: 0,
        fuel_used: [0, 0, 15, 35, 45, 55, 70, 80, 90, 100, 120, 0],
    },
    Engine {
        id: 8,
        tech: [0, 0, 11, 0, 0, 0],
        name: "Interspace-10",
        mass: 25,
        resource_cost: 60,
        ore_cost: [18, 25, 10],
        abilities: 5,
        fuel_used: [0, 0, 10, 30, 40, 50, 60, 70, 80, 90, 100, 0],
    },
    Engine {
        id: 9,
        tech: [7, 0, 13, 5, 9, 0],
        name: "Enigma Pulsar",
        mass: 20,
        resource_cost: 40,
        ore_cost: [12, 15, 11],
        abilities: 6,
        fuel_used: [0, 0, 0, 0, 0, 0, 65, 75, 85, 95, 105, 0],
    },
    Engine {
        id: 10,
        tech: [0, 0, 23, 0, 0, 0],
        name: "Trans-Star 10",
        mass: 5,
        resource_cost: 10,
        ore_cost: [3, 0, 3],
        abilities: 0,
        fuel_used: [0, 0, 5, 15, 20, 25, 30, 35, 40, 45, 50, 0],
    },
    Engine {
        id: 11,
        tech: [2, 0, 6, 0, 0, 0],
        name: "Radiating Hydro-Ram Scoop",
        mass: 10,
        resource_cost: 8,
        ore_cost: [3, 2, 9],
        abilities: 2,
        fuel_used: [0, 0, 0, 0, 0, 0, 0, 165, 375, 600, 720, 0],
    },
    Engine {
        id: 12,
        tech: [2, 0, 8, 0, 0, 0],
        name: "Sub-Galactic Fuel Scoop",
        mass: 20,
        resource_cost: 12,
        ore_cost: [4, 4, 7],
        abilities: 0,
        fuel_used: [0, 0, 0, 0, 0, 0, 85, 105, 210, 380, 456, 0],
    },
    Engine {
        id: 13,
        tech: [3, 0, 9, 0, 0, 0],
        name: "Trans-Galactic Fuel Scoop",
        mass: 19,
        resource_cost: 18,
        ore_cost: [5, 4, 12],
        abilities: 0,
        fuel_used: [0, 0, 0, 0, 0, 0, 0, 88, 100, 145, 174, 0],
    },
    Engine {
        id: 14,
        tech: [4, 0, 12, 0, 0, 0],
        name: "Trans-Galactic Super Scoop",
        mass: 18,
        resource_cost: 24,
        ore_cost: [6, 4, 16],
        abilities: 0,
        fuel_used: [0, 0, 0, 0, 0, 0, 0, 0, 65, 90, 108, 0],
    },
    Engine {
        id: 15,
        tech: [4, 0, 16, 0, 0, 0],
        name: "Trans-Galactic Mizer Scoop",
        mass: 11,
        resource_cost: 20,
        ore_cost: [5, 2, 13],
        abilities: 0,
        fuel_used: [0, 0, 0, 0, 0, 0, 0, 0, 0, 70, 84, 0],
    },
    Engine {
        id: 16,
        tech: [5, 0, 20, 0, 0, 0],
        name: "Galaxy Scoop",
        mass: 8,
        resource_cost: 12,
        ore_cost: [4, 2, 9],
        abilities: 4,
        fuel_used: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 60, 0],
    },
];

// --- rgarmor (12 entries) ---
pub const ARMORS: [Armor; 12] = [
    Armor {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Tritanium",
        mass: 60,
        resource_cost: 10,
        ore_cost: [5, 0, 0],
        dp: 50,
    },
    Armor {
        id: 2,
        tech: [0, 0, 0, 3, 0, 0],
        name: "Crobmnium",
        mass: 56,
        resource_cost: 13,
        ore_cost: [6, 0, 0],
        dp: 75,
    },
    Armor {
        id: 3,
        tech: [0, 0, 0, 0, 0, 4],
        name: "Carbonic Armor",
        mass: 25,
        resource_cost: 15,
        ore_cost: [0, 0, 5],
        dp: 100,
    },
    Armor {
        id: 4,
        tech: [0, 0, 0, 6, 0, 0],
        name: "Strobnium",
        mass: 54,
        resource_cost: 18,
        ore_cost: [8, 0, 0],
        dp: 120,
    },
    Armor {
        id: 5,
        tech: [0, 0, 0, 0, 0, 7],
        name: "Organic Armor",
        mass: 15,
        resource_cost: 20,
        ore_cost: [0, 0, 6],
        dp: 175,
    },
    Armor {
        id: 6,
        tech: [0, 0, 0, 9, 0, 0],
        name: "Kelarium",
        mass: 50,
        resource_cost: 25,
        ore_cost: [9, 1, 0],
        dp: 180,
    },
    Armor {
        id: 7,
        tech: [4, 0, 0, 10, 0, 0],
        name: "Fielded Kelarium",
        mass: 50,
        resource_cost: 28,
        ore_cost: [10, 0, 2],
        dp: 175,
    },
    Armor {
        id: 8,
        tech: [0, 0, 0, 10, 3, 0],
        name: "Depleted Neutronium",
        mass: 50,
        resource_cost: 28,
        ore_cost: [10, 0, 2],
        dp: 200,
    },
    Armor {
        id: 9,
        tech: [0, 0, 0, 12, 0, 0],
        name: "Neutronium",
        mass: 45,
        resource_cost: 30,
        ore_cost: [11, 2, 1],
        dp: 275,
    },
    Armor {
        id: 10,
        tech: [14, 0, 0, 14, 14, 6],
        name: "Mega Poly Shell",
        mass: 20,
        resource_cost: 65,
        ore_cost: [18, 6, 6],
        dp: 400,
    },
    Armor {
        id: 11,
        tech: [0, 0, 0, 16, 0, 0],
        name: "Valanium",
        mass: 40,
        resource_cost: 50,
        ore_cost: [15, 0, 0],
        dp: 500,
    },
    Armor {
        id: 12,
        tech: [0, 0, 0, 24, 0, 0],
        name: "Superlatanium",
        mass: 30,
        resource_cost: 100,
        ore_cost: [25, 0, 0],
        dp: 1500,
    },
];

// --- rgshield (10 entries) ---
pub const SHIELDS: [Shield; 10] = [
    Shield {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Mole-skin Shield",
        mass: 1,
        resource_cost: 4,
        ore_cost: [1, 0, 1],
        dp: 25,
    },
    Shield {
        id: 2,
        tech: [3, 0, 0, 0, 0, 0],
        name: "Cow-hide Shield",
        mass: 1,
        resource_cost: 5,
        ore_cost: [2, 0, 2],
        dp: 40,
    },
    Shield {
        id: 3,
        tech: [6, 0, 0, 0, 0, 0],
        name: "Wolverine Diffuse Shield",
        mass: 1,
        resource_cost: 6,
        ore_cost: [3, 0, 3],
        dp: 60,
    },
    Shield {
        id: 4,
        tech: [7, 0, 0, 4, 0, 0],
        name: "Croby Sharmor",
        mass: 10,
        resource_cost: 15,
        ore_cost: [7, 0, 4],
        dp: 60,
    },
    Shield {
        id: 5,
        tech: [7, 0, 0, 0, 3, 0],
        name: "Shadow Shield",
        mass: 2,
        resource_cost: 7,
        ore_cost: [3, 0, 3],
        dp: 75,
    },
    Shield {
        id: 6,
        tech: [10, 0, 0, 0, 0, 0],
        name: "Bear Neutrino Barrier",
        mass: 1,
        resource_cost: 8,
        ore_cost: [4, 0, 4],
        dp: 100,
    },
    Shield {
        id: 7,
        tech: [12, 0, 9, 0, 9, 0],
        name: "Langston Shell",
        mass: 10,
        resource_cost: 20,
        ore_cost: [10, 2, 6],
        dp: 125,
    },
    Shield {
        id: 8,
        tech: [14, 0, 0, 0, 0, 0],
        name: "Gorilla Delagator",
        mass: 1,
        resource_cost: 11,
        ore_cost: [5, 0, 6],
        dp: 175,
    },
    Shield {
        id: 9,
        tech: [18, 0, 0, 0, 0, 0],
        name: "Elephant Hide Fortress",
        mass: 1,
        resource_cost: 15,
        ore_cost: [8, 0, 10],
        dp: 300,
    },
    Shield {
        id: 10,
        tech: [22, 0, 0, 0, 0, 0],
        name: "Complete Phase Shield",
        mass: 1,
        resource_cost: 20,
        ore_cost: [12, 0, 15],
        dp: 500,
    },
];

// --- rgscanner (16 entries) ---
pub const SCANNERS: [Scanner; 16] = [
    Scanner {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Bat Scanner",
        mass: 2,
        resource_cost: 1,
        ore_cost: [1, 0, 1],
        range: 0,
        abilities: 0,
    },
    Scanner {
        id: 2,
        tech: [0, 0, 0, 0, 1, 0],
        name: "Rhino Scanner",
        mass: 5,
        resource_cost: 3,
        ore_cost: [3, 0, 2],
        range: 50,
        abilities: 0,
    },
    Scanner {
        id: 3,
        tech: [0, 0, 0, 0, 4, 0],
        name: "Mole Scanner",
        mass: 2,
        resource_cost: 9,
        ore_cost: [2, 0, 2],
        range: 100,
        abilities: 0,
    },
    Scanner {
        id: 4,
        tech: [0, 0, 3, 0, 0, 6],
        name: "DNA Scanner",
        mass: 2,
        resource_cost: 5,
        ore_cost: [1, 1, 1],
        range: 125,
        abilities: 0,
    },
    Scanner {
        id: 5,
        tech: [0, 0, 0, 0, 5, 0],
        name: "Possum Scanner",
        mass: 3,
        resource_cost: 18,
        ore_cost: [3, 0, 3],
        range: 150,
        abilities: 0,
    },
    Scanner {
        id: 6,
        tech: [4, 0, 0, 0, 4, 4],
        name: "Pick Pocket Scanner",
        mass: 15,
        resource_cost: 35,
        ore_cost: [8, 10, 6],
        range: 80,
        abilities: 4,
    },
    Scanner {
        id: 7,
        tech: [3, 0, 0, 0, 6, 0],
        name: "Chameleon Scanner",
        mass: 6,
        resource_cost: 25,
        ore_cost: [4, 6, 4],
        range: 160,
        abilities: 4,
    },
    Scanner {
        id: 8,
        tech: [3, 0, 0, 0, 7, 2],
        name: "Ferret Scanner",
        mass: 2,
        resource_cost: 36,
        ore_cost: [2, 0, 8],
        range: 185,
        abilities: 1,
    },
    Scanner {
        id: 9,
        tech: [5, 0, 0, 0, 10, 4],
        name: "Dolphin Scanner",
        mass: 4,
        resource_cost: 40,
        ore_cost: [5, 5, 10],
        range: 220,
        abilities: 2,
    },
    Scanner {
        id: 10,
        tech: [4, 0, 0, 0, 8, 0],
        name: "Gazelle Scanner",
        mass: 5,
        resource_cost: 24,
        ore_cost: [4, 0, 5],
        range: 225,
        abilities: 0,
    },
    Scanner {
        id: 11,
        tech: [0, 0, 5, 0, 0, 10],
        name: "RNA Scanner",
        mass: 2,
        resource_cost: 20,
        ore_cost: [1, 1, 2],
        range: 230,
        abilities: 0,
    },
    Scanner {
        id: 12,
        tech: [5, 0, 0, 0, 11, 0],
        name: "Cheetah Scanner",
        mass: 4,
        resource_cost: 50,
        ore_cost: [3, 1, 13],
        range: 275,
        abilities: 0,
    },
    Scanner {
        id: 13,
        tech: [6, 0, 0, 0, 16, 7],
        name: "Elephant Scanner",
        mass: 6,
        resource_cost: 70,
        ore_cost: [8, 5, 14],
        range: 300,
        abilities: 3,
    },
    Scanner {
        id: 14,
        tech: [6, 0, 0, 0, 14, 0],
        name: "Eagle Eye Scanner",
        mass: 3,
        resource_cost: 64,
        ore_cost: [3, 2, 21],
        range: 335,
        abilities: 0,
    },
    Scanner {
        id: 15,
        tech: [10, 0, 0, 0, 15, 10],
        name: "Robber Baron Scanner",
        mass: 20,
        resource_cost: 90,
        ore_cost: [10, 10, 10],
        range: 220,
        abilities: 4,
    },
    Scanner {
        id: 16,
        tech: [7, 0, 0, 0, 24, 0],
        name: "Peerless Scanner",
        mass: 4,
        resource_cost: 90,
        ore_cost: [3, 2, 30],
        range: 500,
        abilities: 0,
    },
];

// --- rgplanetary (15 entries) ---
pub const PLANETARY: [Planetary; 15] = [
    Planetary {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Viewer 50",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: 50,
    },
    Planetary {
        id: 2,
        tech: [0, 0, 0, 0, 1, 0],
        name: "Viewer 90",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: 90,
    },
    Planetary {
        id: 3,
        tech: [0, 0, 0, 0, 3, 0],
        name: "Scoper 150",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: 150,
    },
    Planetary {
        id: 4,
        tech: [0, 0, 0, 0, 6, 0],
        name: "Scoper 220",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: 220,
    },
    Planetary {
        id: 5,
        tech: [0, 0, 0, 0, 8, 0],
        name: "Scoper 280",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: 280,
    },
    Planetary {
        id: 6,
        tech: [3, 0, 0, 0, 10, 3],
        name: "Snooper 320X",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: -320,
    },
    Planetary {
        id: 7,
        tech: [4, 0, 0, 0, 13, 6],
        name: "Snooper 400X",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: -400,
    },
    Planetary {
        id: 8,
        tech: [5, 0, 0, 0, 16, 7],
        name: "Snooper 500X",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: -500,
    },
    Planetary {
        id: 9,
        tech: [7, 0, 0, 0, 23, 9],
        name: "Snooper 620X",
        mass: 0,
        resource_cost: 100,
        ore_cost: [10, 10, 70],
        ability: -620,
    },
    Planetary {
        id: 10,
        tech: [0, 0, 0, 0, 0, 0],
        name: "SDI",
        mass: 0,
        resource_cost: 15,
        ore_cost: [5, 5, 5],
        ability: 10,
    },
    Planetary {
        id: 11,
        tech: [5, 0, 0, 0, 0, 0],
        name: "Missile Battery",
        mass: 0,
        resource_cost: 15,
        ore_cost: [5, 5, 5],
        ability: 20,
    },
    Planetary {
        id: 12,
        tech: [10, 0, 0, 0, 0, 0],
        name: "Laser Battery",
        mass: 0,
        resource_cost: 15,
        ore_cost: [5, 5, 5],
        ability: 24,
    },
    Planetary {
        id: 13,
        tech: [16, 0, 0, 0, 0, 0],
        name: "Planetary Shield",
        mass: 0,
        resource_cost: 15,
        ore_cost: [5, 5, 5],
        ability: 30,
    },
    Planetary {
        id: 14,
        tech: [23, 0, 0, 0, 0, 0],
        name: "Neutron Shield",
        mass: 0,
        resource_cost: 15,
        ore_cost: [5, 5, 5],
        ability: 38,
    },
    Planetary {
        id: 15,
        tech: [20, 10, 10, 20, 10, 20],
        name: "Genesis Device",
        mass: 0,
        resource_cost: 5000,
        ore_cost: [0, 0, 0],
        ability: 0,
    },
];

// --- rgbeam (24 entries) ---
pub const BEAMS: [Beam; 24] = [
    Beam {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Laser",
        mass: 1,
        resource_cost: 5,
        ore_cost: [0, 6, 0],
        range_max: 1,
        dp: 10,
        initiative: 9,
        abilities: 0,
    },
    Beam {
        id: 2,
        tech: [0, 3, 0, 0, 0, 0],
        name: "X-Ray Laser",
        mass: 1,
        resource_cost: 6,
        ore_cost: [0, 6, 0],
        range_max: 1,
        dp: 16,
        initiative: 9,
        abilities: 0,
    },
    Beam {
        id: 3,
        tech: [0, 5, 0, 0, 0, 0],
        name: "Mini Gun",
        mass: 3,
        resource_cost: 10,
        ore_cost: [0, 16, 0],
        range_max: 2,
        dp: 13,
        initiative: 12,
        abilities: 2,
    },
    Beam {
        id: 4,
        tech: [0, 6, 0, 0, 0, 0],
        name: "Yakimora Light Phaser",
        mass: 1,
        resource_cost: 7,
        ore_cost: [0, 8, 0],
        range_max: 1,
        dp: 26,
        initiative: 9,
        abilities: 0,
    },
    Beam {
        id: 5,
        tech: [0, 7, 0, 0, 0, 0],
        name: "Blackjack",
        mass: 10,
        resource_cost: 7,
        ore_cost: [0, 16, 0],
        range_max: 0,
        dp: 90,
        initiative: 10,
        abilities: 0,
    },
    Beam {
        id: 6,
        tech: [0, 8, 0, 0, 0, 0],
        name: "Phaser Bazooka",
        mass: 2,
        resource_cost: 11,
        ore_cost: [0, 8, 0],
        range_max: 2,
        dp: 26,
        initiative: 7,
        abilities: 0,
    },
    Beam {
        id: 7,
        tech: [5, 9, 0, 0, 0, 0],
        name: "Pulsed Sapper",
        mass: 1,
        resource_cost: 12,
        ore_cost: [0, 0, 4],
        range_max: 3,
        dp: 82,
        initiative: 14,
        abilities: 1,
    },
    Beam {
        id: 8,
        tech: [0, 10, 0, 0, 0, 0],
        name: "Colloidal Phaser",
        mass: 2,
        resource_cost: 18,
        ore_cost: [0, 14, 0],
        range_max: 3,
        dp: 26,
        initiative: 5,
        abilities: 0,
    },
    Beam {
        id: 9,
        tech: [0, 11, 0, 0, 0, 0],
        name: "Gatling Gun",
        mass: 3,
        resource_cost: 13,
        ore_cost: [0, 20, 0],
        range_max: 2,
        dp: 31,
        initiative: 12,
        abilities: 2,
    },
    Beam {
        id: 10,
        tech: [0, 12, 0, 0, 0, 0],
        name: "Mini Blaster",
        mass: 1,
        resource_cost: 9,
        ore_cost: [0, 10, 0],
        range_max: 1,
        dp: 66,
        initiative: 9,
        abilities: 0,
    },
    Beam {
        id: 11,
        tech: [0, 13, 0, 0, 0, 0],
        name: "Bludgeon",
        mass: 10,
        resource_cost: 9,
        ore_cost: [0, 22, 0],
        range_max: 0,
        dp: 231,
        initiative: 10,
        abilities: 0,
    },
    Beam {
        id: 12,
        tech: [0, 14, 0, 0, 0, 0],
        name: "Mark IV Blaster",
        mass: 2,
        resource_cost: 15,
        ore_cost: [0, 12, 0],
        range_max: 2,
        dp: 66,
        initiative: 7,
        abilities: 0,
    },
    Beam {
        id: 13,
        tech: [8, 15, 0, 0, 0, 0],
        name: "Phased Sapper",
        mass: 1,
        resource_cost: 16,
        ore_cost: [0, 0, 6],
        range_max: 3,
        dp: 211,
        initiative: 14,
        abilities: 1,
    },
    Beam {
        id: 14,
        tech: [0, 16, 0, 0, 0, 0],
        name: "Heavy Blaster",
        mass: 2,
        resource_cost: 25,
        ore_cost: [0, 20, 0],
        range_max: 3,
        dp: 66,
        initiative: 5,
        abilities: 0,
    },
    Beam {
        id: 15,
        tech: [0, 17, 0, 0, 0, 0],
        name: "Gatling Neutrino Cannon",
        mass: 3,
        resource_cost: 17,
        ore_cost: [0, 28, 0],
        range_max: 2,
        dp: 80,
        initiative: 13,
        abilities: 2,
    },
    Beam {
        id: 16,
        tech: [0, 18, 0, 0, 0, 0],
        name: "Myopic Disruptor",
        mass: 1,
        resource_cost: 12,
        ore_cost: [0, 14, 0],
        range_max: 1,
        dp: 169,
        initiative: 9,
        abilities: 0,
    },
    Beam {
        id: 17,
        tech: [0, 19, 0, 0, 0, 0],
        name: "Blunderbuss",
        mass: 10,
        resource_cost: 13,
        ore_cost: [0, 30, 0],
        range_max: 0,
        dp: 592,
        initiative: 11,
        abilities: 0,
    },
    Beam {
        id: 18,
        tech: [0, 20, 0, 0, 0, 0],
        name: "Disruptor",
        mass: 2,
        resource_cost: 20,
        ore_cost: [0, 16, 0],
        range_max: 2,
        dp: 169,
        initiative: 8,
        abilities: 0,
    },
    Beam {
        id: 19,
        tech: [21, 21, 0, 0, 16, 12],
        name: "Multi Contained Munition",
        mass: 8,
        resource_cost: 40,
        ore_cost: [6, 40, 6],
        range_max: 3,
        dp: 140,
        initiative: 6,
        abilities: 0,
    },
    Beam {
        id: 20,
        tech: [11, 21, 0, 0, 0, 0],
        name: "Syncro Sapper",
        mass: 1,
        resource_cost: 21,
        ore_cost: [0, 0, 8],
        range_max: 3,
        dp: 541,
        initiative: 14,
        abilities: 1,
    },
    Beam {
        id: 21,
        tech: [0, 22, 0, 0, 0, 0],
        name: "Mega Disruptor",
        mass: 2,
        resource_cost: 33,
        ore_cost: [0, 30, 0],
        range_max: 3,
        dp: 169,
        initiative: 6,
        abilities: 0,
    },
    Beam {
        id: 22,
        tech: [0, 23, 0, 0, 0, 0],
        name: "Big Mutha Cannon",
        mass: 3,
        resource_cost: 23,
        ore_cost: [0, 36, 0],
        range_max: 2,
        dp: 204,
        initiative: 13,
        abilities: 2,
    },
    Beam {
        id: 23,
        tech: [0, 24, 0, 0, 0, 0],
        name: "Streaming Pulverizer",
        mass: 1,
        resource_cost: 16,
        ore_cost: [0, 20, 0],
        range_max: 1,
        dp: 433,
        initiative: 9,
        abilities: 0,
    },
    Beam {
        id: 24,
        tech: [0, 26, 0, 0, 0, 0],
        name: "Anti-Matter Pulverizer",
        mass: 2,
        resource_cost: 27,
        ore_cost: [0, 22, 0],
        range_max: 2,
        dp: 433,
        initiative: 8,
        abilities: 0,
    },
];

// --- rgtorp (12 entries) ---
pub const TORPEDOES: [Torpedo; 12] = [
    Torpedo {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Alpha Torpedo",
        mass: 25,
        resource_cost: 5,
        ore_cost: [9, 3, 3],
        range_max: 4,
        dp: 5,
        initiative: 0,
        hit_chance: 35,
    },
    Torpedo {
        id: 2,
        tech: [0, 5, 1, 0, 0, 0],
        name: "Beta Torpedo",
        mass: 25,
        resource_cost: 6,
        ore_cost: [18, 6, 4],
        range_max: 4,
        dp: 12,
        initiative: 1,
        hit_chance: 45,
    },
    Torpedo {
        id: 3,
        tech: [0, 10, 2, 0, 0, 0],
        name: "Delta Torpedo",
        mass: 25,
        resource_cost: 8,
        ore_cost: [22, 8, 5],
        range_max: 4,
        dp: 26,
        initiative: 1,
        hit_chance: 60,
    },
    Torpedo {
        id: 4,
        tech: [0, 14, 3, 0, 0, 0],
        name: "Epsilon Torpedo",
        mass: 25,
        resource_cost: 10,
        ore_cost: [30, 10, 6],
        range_max: 5,
        dp: 48,
        initiative: 2,
        hit_chance: 65,
    },
    Torpedo {
        id: 5,
        tech: [0, 18, 4, 0, 0, 0],
        name: "Rho Torpedo",
        mass: 25,
        resource_cost: 12,
        ore_cost: [34, 12, 8],
        range_max: 5,
        dp: 90,
        initiative: 2,
        hit_chance: 75,
    },
    Torpedo {
        id: 6,
        tech: [0, 22, 5, 0, 0, 0],
        name: "Upsilon Torpedo",
        mass: 25,
        resource_cost: 15,
        ore_cost: [40, 14, 9],
        range_max: 5,
        dp: 169,
        initiative: 3,
        hit_chance: 75,
    },
    Torpedo {
        id: 7,
        tech: [0, 26, 6, 0, 0, 0],
        name: "Omega Torpedo",
        mass: 25,
        resource_cost: 18,
        ore_cost: [52, 18, 12],
        range_max: 5,
        dp: 316,
        initiative: 4,
        hit_chance: 80,
    },
    Torpedo {
        id: 8,
        tech: [0, 11, 12, 0, 0, 21],
        name: "Anti Matter Torpedo",
        mass: 8,
        resource_cost: 50,
        ore_cost: [3, 8, 1],
        range_max: 6,
        dp: 60,
        initiative: 0,
        hit_chance: 85,
    },
    Torpedo {
        id: 9,
        tech: [0, 12, 6, 0, 0, 0],
        name: "Jihad Missile",
        mass: 35,
        resource_cost: 13,
        ore_cost: [37, 13, 9],
        range_max: 5,
        dp: 85,
        initiative: 0,
        hit_chance: 20,
    },
    Torpedo {
        id: 10,
        tech: [0, 16, 8, 0, 0, 0],
        name: "Juggernaut Missile",
        mass: 35,
        resource_cost: 16,
        ore_cost: [48, 16, 11],
        range_max: 5,
        dp: 150,
        initiative: 1,
        hit_chance: 20,
    },
    Torpedo {
        id: 11,
        tech: [0, 20, 10, 0, 0, 0],
        name: "Doomsday Missile",
        mass: 35,
        resource_cost: 20,
        ore_cost: [60, 20, 13],
        range_max: 6,
        dp: 280,
        initiative: 2,
        hit_chance: 25,
    },
    Torpedo {
        id: 12,
        tech: [0, 24, 10, 0, 0, 0],
        name: "Armageddon Missile",
        mass: 35,
        resource_cost: 24,
        ore_cost: [67, 23, 16],
        range_max: 6,
        dp: 525,
        initiative: 3,
        hit_chance: 30,
    },
];

// --- rgspecialE (17 entries) ---
pub const SPECIALS_E: [Special; 17] = [
    Special {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Transport Cloaking",
        mass: 1,
        resource_cost: 3,
        ore_cost: [2, 0, 2],
        ability: 300,
    },
    Special {
        id: 2,
        tech: [2, 0, 0, 0, 5, 0],
        name: "Stealth Cloak",
        mass: 2,
        resource_cost: 5,
        ore_cost: [2, 0, 2],
        ability: 70,
    },
    Special {
        id: 3,
        tech: [4, 0, 0, 0, 10, 0],
        name: "Super-Stealth Cloak",
        mass: 3,
        resource_cost: 15,
        ore_cost: [8, 0, 8],
        ability: 140,
    },
    Special {
        id: 4,
        tech: [10, 0, 0, 0, 12, 0],
        name: "Ultra-Stealth Cloak",
        mass: 5,
        resource_cost: 25,
        ore_cost: [10, 0, 10],
        ability: 540,
    },
    Special {
        id: 5,
        tech: [11, 0, 11, 0, 11, 0],
        name: "Multi Function Pod",
        mass: 2,
        resource_cost: 15,
        ore_cost: [5, 0, 5],
        ability: 60,
    },
    Special {
        id: 6,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Battle Computer",
        mass: 1,
        resource_cost: 6,
        ore_cost: [0, 0, 15],
        ability: 20,
    },
    Special {
        id: 7,
        tech: [5, 0, 0, 0, 11, 0],
        name: "Battle Super Computer",
        mass: 1,
        resource_cost: 14,
        ore_cost: [0, 0, 25],
        ability: 30,
    },
    Special {
        id: 8,
        tech: [10, 0, 0, 0, 19, 0],
        name: "Battle Nexus",
        mass: 1,
        resource_cost: 15,
        ore_cost: [0, 0, 30],
        ability: 50,
    },
    Special {
        id: 9,
        tech: [2, 0, 0, 0, 6, 0],
        name: "Jammer 10",
        mass: 1,
        resource_cost: 6,
        ore_cost: [0, 0, 2],
        ability: 10,
    },
    Special {
        id: 10,
        tech: [4, 0, 0, 0, 10, 0],
        name: "Jammer 20",
        mass: 1,
        resource_cost: 20,
        ore_cost: [1, 0, 5],
        ability: 20,
    },
    Special {
        id: 11,
        tech: [8, 0, 0, 0, 16, 0],
        name: "Jammer 30",
        mass: 1,
        resource_cost: 20,
        ore_cost: [1, 0, 6],
        ability: 30,
    },
    Special {
        id: 12,
        tech: [16, 0, 0, 0, 22, 0],
        name: "Jammer 50",
        mass: 1,
        resource_cost: 20,
        ore_cost: [2, 0, 7],
        ability: 50,
    },
    Special {
        id: 13,
        tech: [7, 0, 0, 0, 4, 0],
        name: "Energy Capacitor",
        mass: 1,
        resource_cost: 5,
        ore_cost: [0, 0, 8],
        ability: 10,
    },
    Special {
        id: 14,
        tech: [14, 0, 0, 0, 8, 0],
        name: "Flux Capacitor",
        mass: 1,
        resource_cost: 5,
        ore_cost: [0, 0, 8],
        ability: 20,
    },
    Special {
        id: 15,
        tech: [14, 0, 8, 0, 0, 0],
        name: "Energy Dampener",
        mass: 2,
        resource_cost: 50,
        ore_cost: [5, 10, 0],
        ability: 1,
    },
    Special {
        id: 16,
        tech: [8, 0, 0, 0, 14, 0],
        name: "Tachyon Detector",
        mass: 1,
        resource_cost: 70,
        ore_cost: [1, 5, 0],
        ability: 2,
    },
    Special {
        id: 17,
        tech: [0, 12, 0, 0, 0, 7],
        name: "Anti-matter Generator",
        mass: 10,
        resource_cost: 10,
        ore_cost: [8, 3, 3],
        ability: 3,
    },
];

// --- rgspecialM (11 entries) ---
pub const SPECIALS_M: [Special; 11] = [
    Special {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Colonization Module",
        mass: 32,
        resource_cost: 10,
        ore_cost: [12, 10, 10],
        ability: 1,
    },
    Special {
        id: 2,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Orbital Construction Module",
        mass: 50,
        resource_cost: 20,
        ore_cost: [20, 15, 15],
        ability: 1,
    },
    Special {
        id: 3,
        tech: [0, 0, 0, 3, 0, 0],
        name: "Cargo Pod",
        mass: 5,
        resource_cost: 10,
        ore_cost: [5, 0, 2],
        ability: 2,
    },
    Special {
        id: 4,
        tech: [3, 0, 0, 9, 0, 0],
        name: "Super Cargo Pod",
        mass: 7,
        resource_cost: 15,
        ore_cost: [8, 0, 2],
        ability: 2,
    },
    Special {
        id: 5,
        tech: [5, 0, 0, 11, 5, 0],
        name: "Multi Cargo Pod",
        mass: 9,
        resource_cost: 25,
        ore_cost: [12, 0, 3],
        ability: 2,
    },
    Special {
        id: 6,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Fuel Tank",
        mass: 3,
        resource_cost: 4,
        ore_cost: [6, 0, 0],
        ability: 4,
    },
    Special {
        id: 7,
        tech: [6, 0, 4, 14, 0, 0],
        name: "Super Fuel Tank",
        mass: 8,
        resource_cost: 8,
        ore_cost: [8, 0, 0],
        ability: 4,
    },
    Special {
        id: 8,
        tech: [2, 0, 3, 0, 0, 0],
        name: "Maneuvering Jet",
        mass: 5,
        resource_cost: 10,
        ore_cost: [5, 0, 5],
        ability: 1,
    },
    Special {
        id: 9,
        tech: [5, 0, 12, 0, 0, 0],
        name: "Overthruster",
        mass: 5,
        resource_cost: 20,
        ore_cost: [10, 0, 8],
        ability: 2,
    },
    Special {
        id: 10,
        tech: [16, 0, 20, 20, 16, 0],
        name: "Jump Gate",
        mass: 10,
        resource_cost: 40,
        ore_cost: [0, 0, 50],
        ability: 0,
    },
    Special {
        id: 11,
        tech: [6, 6, 0, 6, 6, 0],
        name: "Beam Deflector",
        mass: 1,
        resource_cost: 8,
        ore_cost: [0, 0, 10],
        ability: 10,
    },
];

// --- rgspecialSB (16 entries) ---
pub const SPECIALS_SB: [Special; 16] = [
    Special {
        id: 1,
        tech: [0, 0, 5, 5, 0, 0],
        name: "Stargate 100/250",
        mass: 0,
        resource_cost: 400,
        ore_cost: [100, 40, 40],
        ability: 100,
    },
    Special {
        id: 2,
        tech: [0, 0, 6, 10, 0, 0],
        name: "Stargate any/300",
        mass: 0,
        resource_cost: 500,
        ore_cost: [100, 40, 40],
        ability: -1,
    },
    Special {
        id: 3,
        tech: [0, 0, 11, 7, 0, 0],
        name: "Stargate 150/600",
        mass: 0,
        resource_cost: 1000,
        ore_cost: [100, 40, 40],
        ability: 150,
    },
    Special {
        id: 4,
        tech: [0, 0, 9, 13, 0, 0],
        name: "Stargate 300/500",
        mass: 0,
        resource_cost: 1200,
        ore_cost: [100, 40, 40],
        ability: 300,
    },
    Special {
        id: 5,
        tech: [0, 0, 16, 12, 0, 0],
        name: "Stargate 100/any",
        mass: 0,
        resource_cost: 1400,
        ore_cost: [100, 40, 40],
        ability: 100,
    },
    Special {
        id: 6,
        tech: [0, 0, 12, 18, 0, 0],
        name: "Stargate any/800",
        mass: 0,
        resource_cost: 1400,
        ore_cost: [100, 40, 40],
        ability: -1,
    },
    Special {
        id: 7,
        tech: [0, 0, 19, 24, 0, 0],
        name: "Stargate any/any",
        mass: 0,
        resource_cost: 1600,
        ore_cost: [100, 40, 40],
        ability: -1,
    },
    Special {
        id: 8,
        tech: [4, 0, 0, 0, 0, 0],
        name: "Mass Driver 5",
        mass: 0,
        resource_cost: 140,
        ore_cost: [48, 40, 40],
        ability: 5,
    },
    Special {
        id: 9,
        tech: [7, 0, 0, 0, 0, 0],
        name: "Mass Driver 6",
        mass: 0,
        resource_cost: 288,
        ore_cost: [48, 40, 40],
        ability: 6,
    },
    Special {
        id: 10,
        tech: [9, 0, 0, 0, 0, 0],
        name: "Mass Driver 7",
        mass: 0,
        resource_cost: 1024,
        ore_cost: [200, 200, 200],
        ability: 7,
    },
    Special {
        id: 11,
        tech: [11, 0, 0, 0, 0, 0],
        name: "Super Driver 8",
        mass: 0,
        resource_cost: 512,
        ore_cost: [48, 40, 40],
        ability: 8,
    },
    Special {
        id: 12,
        tech: [13, 0, 0, 0, 0, 0],
        name: "Super Driver 9",
        mass: 0,
        resource_cost: 648,
        ore_cost: [48, 40, 40],
        ability: 9,
    },
    Special {
        id: 13,
        tech: [15, 0, 0, 0, 0, 0],
        name: "Ultra Driver 10",
        mass: 0,
        resource_cost: 1936,
        ore_cost: [200, 200, 200],
        ability: 10,
    },
    Special {
        id: 14,
        tech: [17, 0, 0, 0, 0, 0],
        name: "Ultra Driver 11",
        mass: 0,
        resource_cost: 968,
        ore_cost: [48, 40, 40],
        ability: 11,
    },
    Special {
        id: 15,
        tech: [20, 0, 0, 0, 0, 0],
        name: "Ultra Driver 12",
        mass: 0,
        resource_cost: 1152,
        ore_cost: [48, 40, 40],
        ability: 12,
    },
    Special {
        id: 16,
        tech: [24, 0, 0, 0, 0, 0],
        name: "Ultra Driver 13",
        mass: 0,
        resource_cost: 1352,
        ore_cost: [48, 40, 40],
        ability: 13,
    },
];

// --- rgmining (8 entries) ---
pub const MINING: [Special; 8] = [
    Special {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Robo-Midget Miner",
        mass: 80,
        resource_cost: 50,
        ore_cost: [14, 0, 4],
        ability: 5,
    },
    Special {
        id: 2,
        tech: [0, 0, 0, 2, 1, 0],
        name: "Robo-Mini-Miner",
        mass: 240,
        resource_cost: 100,
        ore_cost: [30, 0, 7],
        ability: 4,
    },
    Special {
        id: 3,
        tech: [0, 0, 0, 4, 2, 0],
        name: "Robo-Miner",
        mass: 240,
        resource_cost: 100,
        ore_cost: [30, 0, 7],
        ability: 12,
    },
    Special {
        id: 4,
        tech: [0, 0, 0, 7, 4, 0],
        name: "Robo-Maxi-Miner",
        mass: 240,
        resource_cost: 100,
        ore_cost: [30, 0, 7],
        ability: 18,
    },
    Special {
        id: 5,
        tech: [0, 0, 0, 12, 6, 0],
        name: "Robo-Super-Miner",
        mass: 240,
        resource_cost: 100,
        ore_cost: [30, 0, 7],
        ability: 27,
    },
    Special {
        id: 6,
        tech: [0, 0, 0, 15, 8, 0],
        name: "Robo-Ultra-Miner",
        mass: 80,
        resource_cost: 50,
        ore_cost: [14, 0, 4],
        ability: 25,
    },
    Special {
        id: 7,
        tech: [5, 0, 0, 10, 5, 5],
        name: "Alien Miner",
        mass: 20,
        resource_cost: 20,
        ore_cost: [8, 0, 2],
        ability: 10,
    },
    Special {
        id: 8,
        tech: [0, 0, 0, 0, 0, 6],
        name: "Orbital Adjuster",
        mass: 80,
        resource_cost: 50,
        ore_cost: [25, 25, 25],
        ability: 0,
    },
];

// --- rgmines (10 entries) ---
pub const MINE_LAYERS: [Special; 10] = [
    Special {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Mine Dispenser 40",
        mass: 25,
        resource_cost: 45,
        ore_cost: [2, 10, 8],
        ability: 4,
    },
    Special {
        id: 2,
        tech: [2, 0, 0, 0, 0, 4],
        name: "Mine Dispenser 50",
        mass: 30,
        resource_cost: 55,
        ore_cost: [2, 12, 10],
        ability: 5,
    },
    Special {
        id: 3,
        tech: [3, 0, 0, 0, 0, 7],
        name: "Mine Dispenser 80",
        mass: 30,
        resource_cost: 65,
        ore_cost: [2, 14, 10],
        ability: 8,
    },
    Special {
        id: 4,
        tech: [6, 0, 0, 0, 0, 12],
        name: "Mine Dispenser 130",
        mass: 30,
        resource_cost: 80,
        ore_cost: [2, 18, 10],
        ability: 13,
    },
    Special {
        id: 5,
        tech: [5, 0, 0, 0, 0, 3],
        name: "Heavy Dispenser 50",
        mass: 10,
        resource_cost: 50,
        ore_cost: [2, 20, 5],
        ability: 5,
    },
    Special {
        id: 6,
        tech: [9, 0, 0, 0, 0, 5],
        name: "Heavy Dispenser 110",
        mass: 15,
        resource_cost: 70,
        ore_cost: [2, 30, 5],
        ability: 11,
    },
    Special {
        id: 7,
        tech: [14, 0, 0, 0, 0, 7],
        name: "Heavy Dispenser 200",
        mass: 20,
        resource_cost: 90,
        ore_cost: [2, 45, 5],
        ability: 20,
    },
    Special {
        id: 8,
        tech: [0, 0, 2, 0, 0, 2],
        name: "Speed Trap 20",
        mass: 100,
        resource_cost: 60,
        ore_cost: [30, 0, 12],
        ability: 2,
    },
    Special {
        id: 9,
        tech: [0, 0, 3, 0, 0, 6],
        name: "Speed Trap 30",
        mass: 135,
        resource_cost: 72,
        ore_cost: [32, 0, 14],
        ability: 3,
    },
    Special {
        id: 10,
        tech: [0, 0, 5, 0, 0, 11],
        name: "Speed Trap 50",
        mass: 140,
        resource_cost: 80,
        ore_cost: [40, 0, 15],
        ability: 5,
    },
];

// --- rgterra (20 entries) ---
pub const TERRAFORMING: [Special; 20] = [
    Special {
        id: 1,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Total Terraform 3",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 3,
    },
    Special {
        id: 2,
        tech: [0, 0, 0, 0, 0, 3],
        name: "Total Terraform 5",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 5,
    },
    Special {
        id: 3,
        tech: [0, 0, 0, 0, 0, 6],
        name: "Total Terraform 7",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 7,
    },
    Special {
        id: 4,
        tech: [0, 0, 0, 0, 0, 9],
        name: "Total Terraform 10",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 10,
    },
    Special {
        id: 5,
        tech: [0, 0, 0, 0, 0, 13],
        name: "Total Terraform 15",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 15,
    },
    Special {
        id: 6,
        tech: [0, 0, 0, 0, 0, 17],
        name: "Total Terraform 20",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 20,
    },
    Special {
        id: 7,
        tech: [0, 0, 0, 0, 0, 22],
        name: "Total Terraform 25",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 25,
    },
    Special {
        id: 8,
        tech: [0, 0, 0, 0, 0, 25],
        name: "Total Terraform 30",
        mass: 0,
        resource_cost: 70,
        ore_cost: [0, 0, 0],
        ability: 30,
    },
    Special {
        id: 9,
        tech: [0, 0, 1, 0, 0, 1],
        name: "Gravity Terraform 3",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 3,
    },
    Special {
        id: 10,
        tech: [0, 0, 5, 0, 0, 2],
        name: "Gravity Terraform 7",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 7,
    },
    Special {
        id: 11,
        tech: [0, 0, 10, 0, 0, 3],
        name: "Gravity Terraform 11",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 11,
    },
    Special {
        id: 12,
        tech: [0, 0, 16, 0, 0, 4],
        name: "Gravity Terraform 15",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 15,
    },
    Special {
        id: 13,
        tech: [1, 0, 0, 0, 0, 1],
        name: "Temp Terraform 3",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 3,
    },
    Special {
        id: 14,
        tech: [5, 0, 0, 0, 0, 2],
        name: "Temp Terraform 7",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 7,
    },
    Special {
        id: 15,
        tech: [10, 0, 0, 0, 0, 3],
        name: "Temp Terraform 11",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 11,
    },
    Special {
        id: 16,
        tech: [16, 0, 0, 0, 0, 4],
        name: "Temp Terraform 15",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 15,
    },
    Special {
        id: 17,
        tech: [0, 1, 0, 0, 0, 1],
        name: "Radiation Terraform 3",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 3,
    },
    Special {
        id: 18,
        tech: [0, 5, 0, 0, 0, 2],
        name: "Radiation Terraform 7",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 7,
    },
    Special {
        id: 19,
        tech: [0, 10, 0, 0, 0, 3],
        name: "Radiation Terraform 11",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 11,
    },
    Special {
        id: 20,
        tech: [0, 16, 0, 0, 0, 4],
        name: "Radiation Terraform 15",
        mass: 0,
        resource_cost: 100,
        ore_cost: [0, 0, 0],
        ability: 15,
    },
];

// --- rgbomb (15 entries) ---
pub const BOMBS: [Bomb; 15] = [
    Bomb {
        id: 1,
        tech: [0, 2, 0, 0, 0, 0],
        name: "Lady Finger Bomb",
        mass: 40,
        resource_cost: 5,
        ore_cost: [1, 20, 0],
        rounds: 1,
        colonist_damage: 6,
        building_damage: 2,
    },
    Bomb {
        id: 2,
        tech: [0, 5, 0, 0, 0, 0],
        name: "Black Cat Bomb",
        mass: 45,
        resource_cost: 7,
        ore_cost: [1, 22, 0],
        rounds: 1,
        colonist_damage: 9,
        building_damage: 4,
    },
    Bomb {
        id: 3,
        tech: [0, 8, 0, 0, 0, 0],
        name: "M-70 Bomb",
        mass: 50,
        resource_cost: 9,
        ore_cost: [1, 24, 0],
        rounds: 1,
        colonist_damage: 12,
        building_damage: 6,
    },
    Bomb {
        id: 4,
        tech: [0, 11, 0, 0, 0, 0],
        name: "M-80 Bomb",
        mass: 55,
        resource_cost: 12,
        ore_cost: [1, 25, 0],
        rounds: 1,
        colonist_damage: 17,
        building_damage: 7,
    },
    Bomb {
        id: 5,
        tech: [0, 14, 0, 0, 0, 0],
        name: "Cherry Bomb",
        mass: 52,
        resource_cost: 11,
        ore_cost: [1, 25, 0],
        rounds: 1,
        colonist_damage: 25,
        building_damage: 10,
    },
    Bomb {
        id: 6,
        tech: [0, 5, 0, 0, 8, 0],
        name: "LBU-17 Bomb",
        mass: 30,
        resource_cost: 7,
        ore_cost: [1, 15, 15],
        rounds: 1,
        colonist_damage: 2,
        building_damage: 16,
    },
    Bomb {
        id: 7,
        tech: [0, 10, 0, 0, 10, 0],
        name: "LBU-32 Bomb",
        mass: 35,
        resource_cost: 10,
        ore_cost: [1, 24, 15],
        rounds: 1,
        colonist_damage: 3,
        building_damage: 28,
    },
    Bomb {
        id: 8,
        tech: [0, 15, 0, 0, 12, 0],
        name: "LBU-74 Bomb",
        mass: 45,
        resource_cost: 14,
        ore_cost: [1, 33, 12],
        rounds: 1,
        colonist_damage: 4,
        building_damage: 45,
    },
    Bomb {
        id: 9,
        tech: [0, 12, 0, 0, 12, 12],
        name: "Hush-a-Boom",
        mass: 5,
        resource_cost: 5,
        ore_cost: [1, 5, 0],
        rounds: 1,
        colonist_damage: 30,
        building_damage: 2,
    },
    Bomb {
        id: 10,
        tech: [0, 10, 0, 0, 0, 12],
        name: "Retro Bomb",
        mass: 45,
        resource_cost: 50,
        ore_cost: [15, 15, 10],
        rounds: 1,
        colonist_damage: 0,
        building_damage: 0,
    },
    Bomb {
        id: 11,
        tech: [0, 5, 0, 0, 0, 7],
        name: "Smart Bomb",
        mass: 50,
        resource_cost: 27,
        ore_cost: [1, 22, 0],
        rounds: 1,
        colonist_damage: 13,
        building_damage: 0,
    },
    Bomb {
        id: 12,
        tech: [0, 10, 0, 0, 0, 10],
        name: "Neutron Bomb",
        mass: 57,
        resource_cost: 30,
        ore_cost: [1, 30, 0],
        rounds: 1,
        colonist_damage: 22,
        building_damage: 0,
    },
    Bomb {
        id: 13,
        tech: [0, 15, 0, 0, 0, 12],
        name: "Enriched Neutron Bomb",
        mass: 64,
        resource_cost: 25,
        ore_cost: [1, 36, 0],
        rounds: 1,
        colonist_damage: 35,
        building_damage: 0,
    },
    Bomb {
        id: 14,
        tech: [0, 22, 0, 0, 0, 15],
        name: "Peerless Bomb",
        mass: 55,
        resource_cost: 32,
        ore_cost: [1, 33, 0],
        rounds: 1,
        colonist_damage: 50,
        building_damage: 0,
    },
    Bomb {
        id: 15,
        tech: [0, 26, 0, 0, 0, 17],
        name: "Annihilator Bomb",
        mass: 50,
        resource_cost: 28,
        ore_cost: [1, 30, 0],
        rounds: 1,
        colonist_damage: 70,
        building_damage: 0,
    },
];

// --- rghuldef (32 ship hulls) ---
pub const HULLS: [Hull; 32] = [
    Hull {
        id: 0,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Small Freighter",
        empty_mass: 25,
        resource_cost: 20,
        ore_cost: [12, 0, 17],
        cargo_max: 70,
        fuel_max: 130,
        armor: 25,
        initiative: 0,
        category: 1,
        slot_count: 3,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 1,
        tech: [0, 0, 0, 3, 0, 0],
        name: "Medium Freighter",
        empty_mass: 60,
        resource_cost: 40,
        ore_cost: [20, 0, 19],
        cargo_max: 210,
        fuel_max: 450,
        armor: 50,
        initiative: 0,
        category: 1,
        slot_count: 3,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 2,
        tech: [0, 0, 0, 8, 0, 0],
        name: "Large Freighter",
        empty_mass: 125,
        resource_cost: 100,
        ore_cost: [35, 0, 21],
        cargo_max: 1200,
        fuel_max: 2600,
        armor: 150,
        initiative: 0,
        category: 1,
        slot_count: 3,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 2,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 3,
        tech: [0, 0, 0, 13, 0, 0],
        name: "Super Freighter",
        empty_mass: 175,
        resource_cost: 125,
        ore_cost: [45, 0, 21],
        cargo_max: 3000,
        fuel_max: 8000,
        armor: 400,
        initiative: 0,
        category: 1,
        slot_count: 4,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 3,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 5,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 4,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Scout",
        empty_mass: 8,
        resource_cost: 10,
        ore_cost: [4, 2, 4],
        cargo_max: 0,
        fuel_max: 50,
        armor: 20,
        initiative: 1,
        category: 2,
        slot_count: 3,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x2,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 5,
        tech: [0, 0, 0, 6, 0, 0],
        name: "Frigate",
        empty_mass: 8,
        resource_cost: 12,
        ore_cost: [4, 2, 4],
        cargo_max: 0,
        fuel_max: 125,
        armor: 45,
        initiative: 4,
        category: 2,
        slot_count: 4,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x2,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 6,
        tech: [0, 0, 0, 3, 0, 0],
        name: "Destroyer",
        empty_mass: 30,
        resource_cost: 35,
        ore_cost: [15, 3, 5],
        cargo_max: 0,
        fuel_max: 280,
        armor: 200,
        initiative: 3,
        category: 2,
        slot_count: 7,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x8,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1000,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 7,
        tech: [0, 0, 0, 9, 0, 0],
        name: "Cruiser",
        empty_mass: 90,
        resource_cost: 85,
        ore_cost: [40, 5, 8],
        cargo_max: 0,
        fuel_max: 600,
        armor: 700,
        initiative: 5,
        category: 3,
        slot_count: 7,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1804,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x1804,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 8,
        tech: [0, 0, 0, 10, 0, 0],
        name: "Battle Cruiser",
        empty_mass: 120,
        resource_cost: 120,
        ore_cost: [55, 8, 12],
        cargo_max: 0,
        fuel_max: 1400,
        armor: 1000,
        initiative: 5,
        category: 3,
        slot_count: 7,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1804,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1804,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 9,
        tech: [0, 0, 0, 13, 0, 0],
        name: "Battleship",
        empty_mass: 222,
        resource_cost: 225,
        ore_cost: [120, 25, 20],
        cargo_max: 0,
        fuel_max: 2800,
        armor: 2000,
        initiative: 10,
        category: 3,
        slot_count: 11,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 8,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 6,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 6,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x8,
                capacity: 6,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 10,
        tech: [0, 0, 0, 16, 0, 0],
        name: "Dreadnought",
        empty_mass: 250,
        resource_cost: 275,
        ore_cost: [140, 30, 25],
        cargo_max: 0,
        fuel_max: 4500,
        armor: 4500,
        initiative: 10,
        category: 3,
        slot_count: 13,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 5,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 4,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 6,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 6,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 8,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 8,
            },
            HullSlot {
                allowed: 0x8,
                capacity: 8,
            },
            HullSlot {
                allowed: 0x34,
                capacity: 5,
            },
            HullSlot {
                allowed: 0x34,
                capacity: 5,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 11,
        tech: [0, 0, 0, 4, 0, 0],
        name: "Privateer",
        empty_mass: 65,
        resource_cost: 50,
        ore_cost: [50, 3, 2],
        cargo_max: 250,
        fuel_max: 650,
        armor: 150,
        initiative: 3,
        category: 4,
        slot_count: 5,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 12,
        tech: [0, 0, 0, 8, 0, 0],
        name: "Rogue",
        empty_mass: 75,
        resource_cost: 60,
        ore_cost: [80, 5, 5],
        cargo_max: 500,
        fuel_max: 2250,
        armor: 450,
        initiative: 4,
        category: 4,
        slot_count: 9,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x1900,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x2,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1900,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 13,
        tech: [0, 0, 0, 11, 0, 0],
        name: "Galleon",
        empty_mass: 125,
        resource_cost: 105,
        ore_cost: [70, 5, 5],
        cargo_max: 1000,
        fuel_max: 2500,
        armor: 900,
        initiative: 4,
        category: 4,
        slot_count: 8,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 4,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 2,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x1900,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1800,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x2,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 14,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Mini-Colony Ship",
        empty_mass: 8,
        resource_cost: 3,
        ore_cost: [2, 0, 2],
        cargo_max: 10,
        fuel_max: 150,
        armor: 10,
        initiative: 0,
        category: 0,
        slot_count: 2,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x1000,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 15,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Colony Ship",
        empty_mass: 20,
        resource_cost: 20,
        ore_cost: [10, 0, 15],
        cargo_max: 25,
        fuel_max: 200,
        armor: 20,
        initiative: 0,
        category: 0,
        slot_count: 2,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x1000,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 16,
        tech: [0, 0, 0, 1, 0, 0],
        name: "Mini Bomber",
        empty_mass: 28,
        resource_cost: 35,
        ore_cost: [20, 5, 10],
        cargo_max: 0,
        fuel_max: 120,
        armor: 50,
        initiative: 0,
        category: 5,
        slot_count: 2,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 17,
        tech: [0, 0, 0, 6, 0, 0],
        name: "B-17 Bomber",
        empty_mass: 69,
        resource_cost: 150,
        ore_cost: [55, 10, 10],
        cargo_max: 0,
        fuel_max: 400,
        armor: 175,
        initiative: 0,
        category: 5,
        slot_count: 4,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 18,
        tech: [0, 0, 0, 8, 0, 0],
        name: "Stealth Bomber",
        empty_mass: 70,
        resource_cost: 175,
        ore_cost: [55, 10, 15],
        cargo_max: 0,
        fuel_max: 750,
        armor: 225,
        initiative: 0,
        category: 5,
        slot_count: 5,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 19,
        tech: [0, 0, 0, 15, 0, 0],
        name: "B-52 Bomber",
        empty_mass: 110,
        resource_cost: 280,
        ore_cost: [90, 15, 10],
        cargo_max: 0,
        fuel_max: 750,
        armor: 450,
        initiative: 0,
        category: 5,
        slot_count: 7,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x40,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 20,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Midget Miner",
        empty_mass: 10,
        resource_cost: 20,
        ore_cost: [10, 0, 3],
        cargo_max: 0,
        fuel_max: 210,
        armor: 100,
        initiative: 0,
        category: 6,
        slot_count: 2,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 21,
        tech: [0, 0, 0, 2, 0, 0],
        name: "Mini-Miner",
        empty_mass: 80,
        resource_cost: 50,
        ore_cost: [25, 0, 6],
        cargo_max: 0,
        fuel_max: 210,
        armor: 130,
        initiative: 0,
        category: 6,
        slot_count: 4,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 22,
        tech: [0, 0, 0, 6, 0, 0],
        name: "Miner",
        empty_mass: 110,
        resource_cost: 110,
        ore_cost: [32, 0, 6],
        cargo_max: 0,
        fuel_max: 500,
        armor: 475,
        initiative: 0,
        category: 6,
        slot_count: 6,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x180a,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 23,
        tech: [0, 0, 0, 11, 0, 0],
        name: "Maxi-Miner",
        empty_mass: 110,
        resource_cost: 140,
        ore_cost: [32, 0, 6],
        cargo_max: 0,
        fuel_max: 850,
        armor: 1400,
        initiative: 0,
        category: 6,
        slot_count: 6,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x180a,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 24,
        tech: [0, 0, 0, 14, 0, 0],
        name: "Ultra-Miner",
        empty_mass: 100,
        resource_cost: 130,
        ore_cost: [30, 0, 6],
        cargo_max: 0,
        fuel_max: 1300,
        armor: 1500,
        initiative: 0,
        category: 6,
        slot_count: 6,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x180a,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x80,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 25,
        tech: [0, 0, 0, 4, 0, 0],
        name: "Fuel Transport",
        empty_mass: 12,
        resource_cost: 50,
        ore_cost: [10, 0, 5],
        cargo_max: 0,
        fuel_max: 750,
        armor: 5,
        initiative: 0,
        category: 7,
        slot_count: 2,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 26,
        tech: [0, 0, 0, 7, 0, 0],
        name: "Super-Fuel Xport",
        empty_mass: 111,
        resource_cost: 70,
        ore_cost: [20, 0, 8],
        cargo_max: 0,
        fuel_max: 2250,
        armor: 12,
        initiative: 0,
        category: 7,
        slot_count: 3,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x2,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 27,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Mini Mine Layer",
        empty_mass: 10,
        resource_cost: 20,
        ore_cost: [8, 2, 5],
        cargo_max: 0,
        fuel_max: 400,
        armor: 60,
        initiative: 0,
        category: 4,
        slot_count: 4,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x100,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x100,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 28,
        tech: [0, 0, 0, 15, 0, 0],
        name: "Super Mine Layer",
        empty_mass: 30,
        resource_cost: 30,
        ore_cost: [20, 3, 9],
        cargo_max: 0,
        fuel_max: 2200,
        armor: 1200,
        initiative: 0,
        category: 4,
        slot_count: 6,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x100,
                capacity: 8,
            },
            HullSlot {
                allowed: 0x100,
                capacity: 8,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x1802,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x1900,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 29,
        tech: [0, 0, 0, 26, 0, 0],
        name: "Nubian",
        empty_mass: 100,
        resource_cost: 150,
        ore_cost: [75, 12, 12],
        cargo_max: 0,
        fuel_max: 5000,
        armor: 5000,
        initiative: 2,
        category: 4,
        slot_count: 13,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 30,
        tech: [0, 0, 0, 8, 0, 0],
        name: "Mini Morph",
        empty_mass: 70,
        resource_cost: 100,
        ore_cost: [30, 8, 8],
        cargo_max: 150,
        fuel_max: 400,
        armor: 250,
        initiative: 2,
        category: 4,
        slot_count: 7,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 31,
        tech: [0, 0, 0, 10, 0, 0],
        name: "Meta Morph",
        empty_mass: 85,
        resource_cost: 120,
        ore_cost: [50, 12, 12],
        cargo_max: 300,
        fuel_max: 700,
        armor: 500,
        initiative: 2,
        category: 4,
        slot_count: 7,
        slots: [
            HullSlot {
                allowed: 0x1,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 8,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x193e,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
];

// --- rghuldefSB (5 starbase hulls) ---
pub const STARBASE_HULLS: [Hull; 5] = [
    Hull {
        id: 32,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Orbital Fort",
        empty_mass: 0,
        resource_cost: 80,
        ore_cost: [24, 0, 34],
        cargo_max: 0,
        fuel_max: 0,
        armor: 100,
        initiative: 10,
        category: 0,
        slot_count: 5,
        slots: [
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 12,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 12,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 12,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 12,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 33,
        tech: [0, 0, 0, 4, 0, 0],
        name: "Space Dock",
        empty_mass: 0,
        resource_cost: 200,
        ore_cost: [40, 10, 50],
        cargo_max: 200,
        fuel_max: 0,
        armor: 250,
        initiative: 12,
        category: 0,
        slot_count: 8,
        slots: [
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 24,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 24,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 2,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 34,
        tech: [0, 0, 0, 0, 0, 0],
        name: "Space Station",
        empty_mass: 0,
        resource_cost: 1200,
        ore_cost: [240, 160, 500],
        cargo_max: 65535,
        fuel_max: 0,
        armor: 500,
        initiative: 14,
        category: 0,
        slot_count: 12,
        slots: [
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
            HullSlot {
                allowed: 0x0,
                capacity: 0,
            },
        ],
    },
    Hull {
        id: 35,
        tech: [0, 0, 0, 12, 0, 0],
        name: "Ultra Station",
        empty_mass: 0,
        resource_cost: 1200,
        ore_cost: [240, 160, 600],
        cargo_max: 65535,
        fuel_max: 0,
        armor: 1000,
        initiative: 16,
        category: 0,
        slot_count: 16,
        slots: [
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 20,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 20,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 20,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 20,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 3,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 16,
            },
        ],
    },
    Hull {
        id: 36,
        tech: [0, 0, 0, 17, 0, 0],
        name: "Death Star",
        empty_mass: 0,
        resource_cost: 1500,
        ore_cost: [240, 160, 700],
        cargo_max: 65535,
        fuel_max: 0,
        armor: 1500,
        initiative: 18,
        category: 0,
        slot_count: 16,
        slots: [
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 32,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 30,
            },
            HullSlot {
                allowed: 0x4,
                capacity: 30,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 32,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 32,
            },
            HullSlot {
                allowed: 0xa00,
                capacity: 1,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 20,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0xc,
                capacity: 20,
            },
            HullSlot {
                allowed: 0x800,
                capacity: 4,
            },
            HullSlot {
                allowed: 0x30,
                capacity: 32,
            },
        ],
    },
];

/// The best component in a table the given tech levels can build, by table
/// order (the tables are ordered so that later entries supersede earlier ones
/// for a given role).
fn best_by<'a, T: 'a>(
    table: &'a [T],
    levels: &[u8; TECH_FIELDS],
    tech: impl Fn(&T) -> TechRequirement,
) -> Option<&'a T> {
    table.iter().rfind(|item| {
        tech(item)
            .iter()
            .zip(levels.iter())
            .all(|(need, have)| i32::from(*need) <= i32::from(*have))
    })
}

/// The best planetary scanner the given tech levels allow.
///
/// Defences share the planetary table, so only the scanner entries (ids 1..=9)
/// are considered.
#[must_use]
pub fn best_planetary_scanner(levels: &[u8; TECH_FIELDS]) -> Option<&'static Planetary> {
    let scanners = &PLANETARY[..9];
    best_by(scanners, levels, |p| p.tech)
}

/// The best ship scanner the given tech levels allow.
#[must_use]
pub fn best_scanner(levels: &[u8; TECH_FIELDS]) -> Option<&'static Scanner> {
    best_by(&SCANNERS, levels, |s| s.tech)
}

/// Look up an engine by its component id.
#[must_use]
pub fn engine(id: i16) -> Option<&'static Engine> {
    ENGINES.iter().find(|e| e.id == id)
}

/// Look up a hull by its id. Ids 0..=31 are ship hulls, 32..=36 starbases.
#[must_use]
pub fn hull(id: i16) -> Option<&'static Hull> {
    HULLS
        .iter()
        .chain(STARBASE_HULLS.iter())
        .find(|h| h.id == id)
}
