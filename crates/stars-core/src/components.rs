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
