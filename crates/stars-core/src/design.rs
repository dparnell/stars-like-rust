//! Ship and starbase designs: turning a hull plus a list of fitted components
//! into the numbers the rest of the simulation needs.
//!
//! A design is a hull and up to sixteen slots, each holding some number of one
//! component. Everything else — mass, armour, shields, fuel and cargo
//! capacity, scanner range, cost — is derived from those, which is why the
//! game stores only the slots and recomputes the rest.
//!
//! Source: the NB09 `HUL`/`HS`/`SHDEF` structs and `battle.c`'s
//! `InitFromHuldef`; the hull tables are transcribed in [`crate::components`].
//! `stars_formats::DesignRecord` decodes designs from `.hst`/`.mN` files — see
//! `docs/formats/design.md`. Full derivation in `docs/formulas/design.md`.

use crate::components::{
    hull, slot, Hull, ARMORS, BEAMS, BOMBS, ENGINES, MINE_LAYERS, MINING, SCANNERS, SHIELDS,
    SPECIALS_E, SPECIALS_M, SPECIALS_SB, TERRAFORMING, TORPEDOES,
};
use crate::scanning::{combine_ranges, ScannerRange};

/// Index of the Croby Sharmor in [`SHIELDS`]; it also provides 65 armour points.
const SHIELD_CROBY_SHARMOR: usize = 3;
/// Index of the Langston Shell in [`SHIELDS`]; it also provides 65 armour points.
const SHIELD_LANGSTON_SHELL: usize = 6;
/// Index of Fielded Kelarium in [`ARMORS`]; it also provides 50 shield points.
const ARMOR_FIELDED_KELARIUM: usize = 6;
/// Index of the Mega Poly Shell in [`ARMORS`]; it also provides 100 shield points.
const ARMOR_MEGA_POLY_SHELL: usize = 9;
/// Index of the Cargo Pod in the mechanical specials.
const SPECIAL_M_CARGO_POD: usize = 2;
/// Index of the Super Cargo Pod.
const SPECIAL_M_SUPER_CARGO_POD: usize = 3;
/// Index of the Multi Cargo Pod.
const SPECIAL_M_MULTI_CARGO_POD: usize = 4;
/// Index of the Fuel Tank.
const SPECIAL_M_FUEL_TANK: usize = 5;
/// Index of the Super Fuel Tank.
const SPECIAL_M_SUPER_FUEL_TANK: usize = 6;
/// Index of the Anti-Matter Generator in the electrical specials.
const SPECIAL_E_ANTIMATTER_GENERATOR: usize = 16;

/// One equipment slot of a design.
///
/// `item` is a **zero-based index** into the table its category names — the
/// game's own lookups index the component arrays directly rather than
/// searching by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignSlot {
    /// Category bitmask (see [`crate::components::slot`]).
    pub category: u16,
    /// Index into that category's component table.
    pub item: u8,
    /// How many are fitted; `0` means the slot is empty.
    pub count: u8,
}

impl DesignSlot {
    /// Whether this slot holds a component of the given category.
    #[must_use]
    pub fn is(&self, category: u16) -> bool {
        self.count > 0 && self.category & category != 0
    }
}

/// A ship or starbase design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipDesign {
    /// Hull id: 0..=31 for ships, 32..=36 for starbases.
    pub hull_id: i16,
    /// The fitted slots, in hull-slot order.
    pub slots: Vec<DesignSlot>,
}

/// A design's derived cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cost {
    /// Resources to build one.
    pub resources: i32,
    /// Minerals to build one: ironium, boranium, germanium.
    pub minerals: [i32; 3],
}

impl ShipDesign {
    /// The hull this design is built on.
    #[must_use]
    pub fn hull(&self) -> Option<&'static Hull> {
        hull(self.hull_id)
    }

    /// Whether this is a starbase design.
    #[must_use]
    pub fn is_starbase(&self) -> bool {
        self.hull_id >= 32
    }

    /// Total mass of one ship, in kT: the bare hull plus everything fitted.
    ///
    /// Cargo is **not** included; it is added per fleet, because two ships of
    /// the same design can carry different loads.
    #[must_use]
    pub fn mass(&self) -> Option<i32> {
        let hull = self.hull()?;
        let mut mass = i32::from(hull.empty_mass);
        for s in &self.slots {
            mass += self.component_mass(s) * i32::from(s.count);
        }
        Some(mass)
    }

    /// Armour of one ship, in damage points.
    ///
    /// The hull's own armour plus three contributions, exactly as
    /// `UpdateShdefCost` computes them:
    ///
    /// * fitted armour, **halved for a race with Regenerating Shields** — the
    ///   price that trait pays for its shields;
    /// * 65 per Croby Sharmor or Langston Shell, two shields that also armour;
    /// * 50 per Multi Cargo Pod.
    ///
    /// The slot categories are matched **exactly**, not as a bitmask, which is
    /// what the original does.
    #[must_use]
    pub fn armor(&self, regenerating_shields: bool) -> Option<i32> {
        let hull = self.hull()?;
        let mut dp = i32::from(hull.armor);

        for s in &self.slots {
            if s.count == 0 {
                continue;
            }
            let count = i32::from(s.count);
            let item = usize::from(s.item);

            if s.category == slot::SHIELD {
                if item == SHIELD_CROBY_SHARMOR || item == SHIELD_LANGSTON_SHELL {
                    dp += count * 65;
                }
            } else if s.category == slot::ARMOR {
                let mut fitted = count * ARMORS.get(item).map_or(0, |p| i32::from(p.dp));
                if regenerating_shields {
                    fitted /= 2;
                }
                dp += fitted;
            } else if s.category == slot::SPECIAL_M && item == SPECIAL_M_MULTI_CARGO_POD {
                dp += count * 50;
            }
        }
        Some(dp)
    }

    /// Shield points of one ship.
    ///
    /// Shields pool across a whole token in battle, so a stack's shield total
    /// is this multiplied by the number of ships.
    ///
    /// Two armours also carry shielding: Fielded Kelarium adds 50 a piece and
    /// the Mega Poly Shell 100. A race with Regenerating Shields gets a
    /// further 40% (`DpShieldOfShdef`).
    #[must_use]
    pub fn shields(&self, regenerating: bool) -> i32 {
        let mut dp = 0;
        for s in &self.slots {
            if s.count == 0 {
                continue;
            }
            let count = i32::from(s.count);
            // Categories are compared exactly, as `DpShieldOfShdef` does.
            if s.category == slot::SHIELD {
                if let Some(p) = SHIELDS.get(usize::from(s.item)) {
                    dp += i32::from(p.dp) * count;
                }
            } else if s.category == slot::ARMOR {
                match usize::from(s.item) {
                    ARMOR_FIELDED_KELARIUM => dp += count * 50,
                    ARMOR_MEGA_POLY_SHELL => dp += count * 100,
                    _ => {}
                }
            }
        }
        if regenerating {
            dp += dp * 2 / 5;
        }
        dp.min(0xffff)
    }

    /// Fuel capacity, in mg: the hull's tank plus any fitted fuel tanks.
    #[must_use]
    pub fn fuel_capacity(&self) -> Option<i32> {
        let mut fuel = i32::from(self.hull()?.fuel_max);
        for s in &self.slots {
            let count = i32::from(s.count);
            if s.category & slot::SPECIAL_M != 0 {
                match usize::from(s.item) {
                    SPECIAL_M_FUEL_TANK => fuel += count * 250,
                    SPECIAL_M_SUPER_FUEL_TANK => fuel += count * 500,
                    _ => {}
                }
            } else if s.category & slot::SPECIAL_E != 0
                && usize::from(s.item) == SPECIAL_E_ANTIMATTER_GENERATOR
            {
                fuel += count * 200;
            }
        }
        Some(fuel)
    }

    /// Cargo capacity, in kT: the hull's hold plus any fitted cargo pods.
    #[must_use]
    pub fn cargo_capacity(&self) -> Option<i32> {
        let mut cargo = i32::from(self.hull()?.cargo_max);
        for s in self
            .slots
            .iter()
            .filter(|s| s.category & slot::SPECIAL_M != 0)
        {
            let count = i32::from(s.count);
            match usize::from(s.item) {
                SPECIAL_M_CARGO_POD => cargo += count * 50,
                SPECIAL_M_SUPER_CARGO_POD => cargo += count * 100,
                SPECIAL_M_MULTI_CARGO_POD => cargo += count * 250,
                _ => {}
            }
        }
        Some(cargo)
    }

    /// The engine fitted, if any.
    #[must_use]
    pub fn engine(&self) -> Option<&'static crate::components::Engine> {
        self.slots
            .iter()
            .find(|s| s.is(slot::ENGINE))
            .and_then(|s| ENGINES.get(usize::from(s.item)))
    }

    /// Combined scanner range, normal and penetrating.
    ///
    /// Multiple scanners combine as the fourth root of the sum of fourth
    /// powers, and a scanner stored with a negative range also penetrates, at
    /// half its magnitude — see [`crate::scanning`].
    #[must_use]
    pub fn scanner_range(&self) -> ScannerRange {
        let mut normal = Vec::new();
        let mut penetrating = Vec::new();
        for s in self.slots.iter().filter(|s| s.is(slot::SCANNER)) {
            let Some(part) = SCANNERS.get(usize::from(s.item)) else {
                continue;
            };
            for _ in 0..s.count {
                normal.push(i32::from(part.range.abs()));
                if part.range < 0 {
                    penetrating.push(i32::from(-part.range) / 2);
                }
            }
        }
        ScannerRange {
            normal: combine_ranges(&normal),
            penetrating: combine_ranges(&penetrating),
        }
    }

    /// Resource and mineral cost of one ship.
    #[must_use]
    pub fn cost(&self) -> Option<Cost> {
        let hull = self.hull()?;
        let mut cost = Cost {
            resources: i32::from(hull.resource_cost),
            minerals: [
                i32::from(hull.ore_cost[0]),
                i32::from(hull.ore_cost[1]),
                i32::from(hull.ore_cost[2]),
            ],
        };
        for s in &self.slots {
            let Some((resources, ore)) = self.component_cost(s) else {
                continue;
            };
            let n = i32::from(s.count);
            cost.resources += resources * n;
            for (total, add) in cost.minerals.iter_mut().zip(ore.iter()) {
                *total += add * n;
            }
        }
        Some(cost)
    }

    /// Whether the design carries any weapon.
    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.slots
            .iter()
            .any(|s| s.is(slot::BEAM) || s.is(slot::TORPEDO))
    }

    /// Mass of one of the components in a slot.
    fn component_mass(&self, s: &DesignSlot) -> i32 {
        component_stats(s).map_or(0, |(mass, _, _)| mass)
    }

    /// Resource and mineral cost of one of the components in a slot.
    fn component_cost(&self, s: &DesignSlot) -> Option<(i32, [i32; 3])> {
        component_stats(s).map(|(_, resources, ore)| (resources, ore))
    }
}

/// Mass, resource cost and mineral cost of the component a slot holds.
///
/// A hull slot's category can name several tables at once — most accept either
/// a shield or armour, and many accept any of the special kinds — so the
/// tables are tried in order and the first that both matches the category and
/// has an entry at the index wins. That mirrors how the original resolves a
/// slot, which is by category bit rather than by searching for an id.
fn component_stats(s: &DesignSlot) -> Option<(i32, i32, [i32; 3])> {
    if s.count == 0 {
        return None;
    }
    let i = usize::from(s.item);

    macro_rules! try_table {
        ($flag:expr, $table:expr) => {
            if s.category & $flag != 0 {
                if let Some(p) = $table.get(i) {
                    return Some((
                        i32::from(p.mass),
                        i32::from(p.resource_cost),
                        [
                            i32::from(p.ore_cost[0]),
                            i32::from(p.ore_cost[1]),
                            i32::from(p.ore_cost[2]),
                        ],
                    ));
                }
            }
        };
    }

    try_table!(slot::ENGINE, ENGINES);
    try_table!(slot::ARMOR, ARMORS);
    try_table!(slot::SHIELD, SHIELDS);
    try_table!(slot::SCANNER, SCANNERS);
    try_table!(slot::BEAM, BEAMS);
    try_table!(slot::TORPEDO, TORPEDOES);
    try_table!(slot::BOMB, BOMBS);
    try_table!(slot::MINING, MINING);
    try_table!(slot::MINES, MINE_LAYERS);
    try_table!(slot::TERRA, TERRAFORMING);
    try_table!(slot::SPECIAL_E, SPECIALS_E);
    try_table!(slot::SPECIAL_M, SPECIALS_M);
    try_table!(slot::SPECIAL_SB, SPECIALS_SB);
    None
}
