//! Fleets: stacks of ships, what they carry, and where they are.
//!
//! A fleet is a group of ships of one player, made of *stacks* — some number
//! of one design each. Everything the simulation needs about it derives from
//! those stacks plus the designs they name.
//!
//! `stars_formats::FleetRecord` decodes fleets from `.hst`/`.mN` files; see
//! `docs/formats/fleet.md`.

use crate::design::ShipDesign;
use crate::movement::Point;

/// Some number of ships of one design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipStack {
    /// The owner's design slot this stack was built from.
    pub design: u8,
    /// How many ships.
    pub count: i32,
    /// Percentage of the stack that is damaged.
    pub damaged_pct: i32,
    /// Damage each of those carries, in 500ths of the design's armour.
    pub damage_pct: i32,
}

/// What a fleet is carrying, in kT except fuel (mg) and colonists (hundreds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cargo {
    /// Ironium, boranium, germanium.
    pub minerals: [i32; 3],
    /// Colonists, in units of 100.
    pub colonists: i32,
    /// Fuel.
    pub fuel: i32,
}

impl Cargo {
    /// Mass of the cargo, in kT. Fuel is massless in Stars!; colonists are
    /// not.
    #[must_use]
    pub fn mass(&self) -> i32 {
        self.minerals.iter().sum::<i32>() + self.colonists
    }
}

/// A fleet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fleet {
    /// Fleet id, unique per player.
    pub id: u16,
    /// Owning player.
    pub owner: i16,
    /// Where it is.
    pub position: Point,
    /// The planet it is orbiting, if any.
    pub orbiting: Option<u16>,
    /// The ships it is made of.
    pub stacks: Vec<ShipStack>,
    /// What it carries.
    pub cargo: Cargo,
    /// The battle plan it fights under.
    pub battle_plan: u8,
    /// Warp factor of its current leg, if it is moving.
    pub warp: Option<u8>,
}

impl Fleet {
    /// Total ships in the fleet.
    #[must_use]
    pub fn ships(&self) -> i32 {
        self.stacks.iter().map(|s| s.count).sum()
    }

    /// Whether the fleet still exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ships() == 0
    }

    /// Mass of the fleet in kT, given the designs its stacks name.
    ///
    /// This is the hulls and everything fitted, plus the cargo — which is what
    /// fuel use is charged against.
    #[must_use]
    pub fn mass(&self, designs: &[ShipDesign]) -> i32 {
        let hulls: i32 = self
            .stacks
            .iter()
            .map(|s| {
                designs
                    .get(usize::from(s.design))
                    .and_then(ShipDesign::mass)
                    .unwrap_or(0)
                    * s.count
            })
            .sum();
        hulls + self.cargo.mass()
    }

    /// Total cargo capacity, in kT.
    #[must_use]
    pub fn cargo_capacity(&self, designs: &[ShipDesign]) -> i32 {
        self.stacks
            .iter()
            .map(|s| {
                designs
                    .get(usize::from(s.design))
                    .and_then(ShipDesign::cargo_capacity)
                    .unwrap_or(0)
                    * s.count
            })
            .sum()
    }

    /// Total fuel capacity, in mg.
    #[must_use]
    pub fn fuel_capacity(&self, designs: &[ShipDesign]) -> i32 {
        self.stacks
            .iter()
            .map(|s| {
                designs
                    .get(usize::from(s.design))
                    .and_then(ShipDesign::fuel_capacity)
                    .unwrap_or(0)
                    * s.count
            })
            .sum()
    }

    /// Whether any ship in the fleet carries a weapon.
    #[must_use]
    pub fn is_armed(&self, designs: &[ShipDesign]) -> bool {
        self.stacks.iter().any(|s| {
            s.count > 0
                && designs
                    .get(usize::from(s.design))
                    .is_some_and(ShipDesign::is_armed)
        })
    }
}
