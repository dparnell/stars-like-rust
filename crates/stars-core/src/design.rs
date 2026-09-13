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
use crate::scanning::ScannerRange;

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
    /// The name the player gave the design (`HUL.szClass`), e.g.
    /// `"Long Range Scout"`.
    ///
    /// Nothing in the simulation reads it — a design is identified by its slot
    /// — but it is stored in the file and shown everywhere in the game, so it
    /// travels with the design rather than being looked up.
    pub name: String,
    /// Picture index (`HUL.ibmp`): which of the game's ship icons it is drawn
    /// with. Cosmetic, and stored in the file, so it travels with the design.
    pub picture: u8,
    /// The armour figure **as stored** (`HUL.dp`), which is `0` for every ship
    /// and `1000` for a starbase.
    ///
    /// This is not the armour the simulation uses: [`ShipDesign::armor`]
    /// derives that from the hull and what is fitted, exactly as the game
    /// recomputes it. This field exists so a design written back to a file
    /// carries the same figure it was read with.
    pub stored_armor: u16,
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

    /// Combined scanner range, normal and penetrating, for a design that
    /// gets nothing from its race.
    ///
    /// See [`Self::scanner_range_for`].
    #[must_use]
    pub fn scanner_range(&self) -> ScannerRange {
        self.scanner_range_for(None, false)
    }

    /// Combined scanner range, normal and penetrating — `GetShdefScannerRange`
    /// (`1038:50d0`).
    ///
    /// Every scanner aboard adds the fourth power of its range to the
    /// normal total, and the fourth root of that is the design's range.
    /// **Penetration** is not stored with the part: it comes from the
    /// scanner's ability class — 50 for class 1, 100 for class 2, 200 for
    /// class 3 (the Ferret, Dolphin and Elephant) — with three named
    /// exceptions among the class-4 parts: the Chameleon penetrates 45, the
    /// Robber Baron 120 and the Pick Pocket not at all. Three parts that are
    /// not scanners scan too: armour item 9 at 80/40, beam item 18 at 150/75
    /// and shield item 6 at 50/25.
    ///
    /// A **Jack of All Trades** race's Scout, Destroyer and Frigate hulls
    /// carry a scanner of their own, summed in with the rest — pass it as
    /// `builtin`, normal and penetrating: `20 × Electronics` and
    /// `10 × Electronics` in an ordinary game, and a fixed **40 and 20** in
    /// the tutorial (`fTutorial`; the constants at `1120:1cd2` and
    /// `1120:1cda`). That is how the tutorial's Armed Probe, fitted with
    /// nothing but a Rhino, reads Hiho from seventeen light years out and
    /// not from Prune, forty-three away. No Advanced Scanners doubles the
    /// normal range.
    #[must_use]
    pub fn scanner_range_for(&self, builtin: Option<(i32, i32)>, nas: bool) -> ScannerRange {
        let mut normal: f64 = 0.0;
        let mut penetrating: f64 = 0.0;
        let mut any = false;
        if let Some((wide, deep)) = builtin {
            if (4..=6).contains(&self.hull_id) {
                penetrating += f64::from(deep).powi(4);
                normal += f64::from(wide).powi(4);
                any = true;
            }
        }
        for s in &self.slots {
            if s.count == 0 {
                continue;
            }
            let count = f64::from(s.count);
            let (range, deep) = if s.is(slot::SCANNER) {
                let Some(part) = SCANNERS.get(usize::from(s.item)) else {
                    continue;
                };
                any = true;
                let deep = match (s.item, part.abilities) {
                    (5, _) => 0,
                    (6, _) => 45,
                    (14, _) => 120,
                    (_, 1) => 50,
                    (_, 2) => 100,
                    (_, a) if a >= 3 => 200,
                    _ => 0,
                };
                (i32::from(part.range), deep)
            } else if s.is(slot::ARMOR) && s.item == 9 {
                (80, 40)
            } else if s.is(slot::BEAM) && s.item == 18 {
                (150, 75)
            } else if s.is(slot::SHIELD) && s.item == 6 {
                (50, 25)
            } else {
                continue;
            };
            normal += count * f64::from(range).powi(4);
            penetrating += count * f64::from(deep).powi(4);
        }
        if !any {
            return ScannerRange::default();
        }
        #[allow(clippy::cast_possible_truncation)]
        let mut out = ScannerRange {
            normal: normal.sqrt().sqrt() as i32,
            penetrating: penetrating.sqrt().sqrt() as i32,
        };
        if nas {
            out.normal *= 2;
        }
        out
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

    /// The weapons fitted to this design, flattened out of their slots.
    ///
    /// A starbase reaches one square further than a ship with the same
    /// weapon, which is applied here so callers do not have to remember it.
    #[must_use]
    pub fn weapons(&self) -> Vec<crate::battle::Weapon> {
        let reach = i32::from(self.is_starbase());
        let mut out = Vec::new();
        for s in &self.slots {
            if s.count == 0 {
                continue;
            }
            let count = i32::from(s.count);
            let item = usize::from(s.item);
            if s.category == slot::BEAM {
                if let Some(p) = BEAMS.get(item) {
                    out.push(crate::battle::Weapon {
                        torpedo: false,
                        dp: i32::from(p.dp),
                        count,
                        range: i32::from(p.range_max) + reach,
                        nominal_range: i32::from(p.range_max),
                        initiative: i32::from(p.initiative),
                        accuracy: 100,
                        abilities: i32::from(p.abilities),
                        missile: false,
                    });
                }
            } else if s.category == slot::TORPEDO {
                if let Some(p) = TORPEDOES.get(item) {
                    out.push(crate::battle::Weapon {
                        torpedo: true,
                        dp: i32::from(p.dp),
                        count,
                        range: i32::from(p.range_max) + reach,
                        nominal_range: i32::from(p.range_max),
                        initiative: i32::from(p.initiative),
                        accuracy: i32::from(p.hit_chance),
                        abilities: 0,
                        // Jihad, Juggernaut, Doomsday and Armageddon.
                        missile: item >= crate::battle::FIRST_MISSILE,
                    });
                }
            }
        }
        out
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

/// What one component actually costs this player, after miniaturisation.
///
/// Every technology level you hold **above** what a component needs makes it
/// cheaper: 4% a level, to a floor of 25% of the list price. Bleeding Edge
/// Technology trades a penalty for a steeper curve — 5% a level down to 20% —
/// and pays for it by charging **double** for anything whose requirements you
/// have only just met.
///
/// The excess counted is the *smallest* margin over any field the component
/// actually requires; a component that requires nothing at all is measured
/// against the player's weakest field instead. Nineteen levels of margin is as
/// far as it counts.
///
/// Source: `GetTruePartCost` (`1050:cd00`). `MANUAL.PDF` p. 20-14 states the
/// same two curves; p. 8-2 says 5% and 75% in passing, which matches neither
/// and is the page to distrust.
#[must_use]
pub fn true_part_cost(part: &crate::parts::Part, who: &crate::parts::Builder<'_>) -> Cost {
    use crate::components::slot;
    use crate::race::lrt;

    let mut cost = Cost {
        resources: part.resource_cost,
        minerals: part.ore_cost,
    };

    // Terraforming modules never get cheaper, and of the planetary items only
    // the five defences do — not the scanners, and not the Genesis Device.
    let discountable = part.category & slot::TERRA == 0
        && (part.category & slot::PLANETARY == 0 || (9..=13).contains(&part.item));

    let bet = who.race.has_lrt(lrt::BLEEDING_EDGE_TECH);
    let mut excess = 0;

    if discountable {
        // The tightest margin over a field the part asks for.
        excess = 100;
        for field in 0..crate::components::TECH_FIELDS {
            let want = i32::from(part.tech[field]);
            if want > 0 {
                excess = excess.min(i32::from(who.levels[field]) - want);
            }
        }
        // A part that asks for nothing is measured against the weakest field.
        if excess == 100 {
            excess = who.levels.iter().map(|l| i32::from(*l)).min().unwrap_or(0);
        }

        if excess > 0 {
            let pct = if bet {
                (excess.min(19) * 5).min(80)
            } else {
                (excess.min(19) * 4).min(75)
            };
            for value in cost.iter_mut() {
                if *value != 0 {
                    // MulDiv rounds to nearest, and nothing ever falls to free.
                    *value -= (*value * pct + 50) / 100;
                    *value = (*value).max(1);
                }
            }
        }
    }

    // Bleeding Edge Technology's price for that: until you are past *every*
    // requirement, a part that requires anything costs twice as much.
    if excess < 1 && bet && part.tech.iter().any(|t| *t >= 1) {
        for value in cost.iter_mut() {
            *value *= 2;
        }
    }

    cost
}

impl Cost {
    /// The four figures in the order the game keeps them: three minerals, then
    /// resources.
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut i32> {
        self.minerals
            .iter_mut()
            .chain(std::iter::once(&mut self.resources))
    }
}

impl ShipDesign {
    /// What this design costs the player who is building it.
    ///
    /// [`ShipDesign::cost`] adds up the list prices; this is the figure the
    /// designer shows and the planet pays. Three things separate them:
    ///
    /// * every component, and the bare hull, is miniaturised against the
    ///   player's own tech levels — see [`true_part_cost`];
    /// * a starbase costs a fifth less to a race with Improved Starbases, and
    ///   to Alternate Reality, which lives on them;
    /// * and a starbase's cost is then **halved**, because the tables store it
    ///   doubled. An Orbital Fort is listed at 80 resources and built for 40.
    ///
    /// Source: `UpdateShdefCost` (`1038:47b0`) for the sum and
    /// `GetProductionCosts` (`produce.c`) for the two starbase adjustments,
    /// which the designer's own panel repeats.
    ///
    /// Upgrading a starbase in place is cheaper again — the planet is credited
    /// for the parts it already has — but that is a production rule rather than
    /// a design one and is not applied here.
    #[must_use]
    pub fn true_cost(&self, who: &crate::parts::Builder<'_>) -> Option<Cost> {
        use crate::components::slot;

        let hull_category = if self.is_starbase() {
            slot::SB_HULL
        } else {
            slot::HULL
        };
        let hull_item = if self.is_starbase() {
            usize::try_from(self.hull_id - 32).ok()?
        } else {
            usize::try_from(self.hull_id).ok()?
        };

        let hull = crate::parts::part(hull_category, hull_item)?;
        let mut cost = true_part_cost(&hull, who);

        for s in &self.slots {
            if s.count == 0 {
                continue;
            }
            let Some(p) = slot_part(s) else { continue };
            let each = true_part_cost(&p, who);
            let n = i32::from(s.count);
            cost.resources += each.resources * n;
            for (total, add) in cost.minerals.iter_mut().zip(each.minerals.iter()) {
                *total += add * n;
            }
        }

        if self.is_starbase() {
            let cheap = who.race.has_lrt(crate::race::lrt::ISB)
                || who.race.prt() == Some(crate::race::Prt::Ar);
            for value in cost.iter_mut() {
                if cheap {
                    *value -= *value / 5;
                }
                // Round up, as `GetProductionCosts` does with `(x + 1) / 2`.
                *value = (*value + 1) / 2;
            }
        }

        Some(cost)
    }
}

/// The component a design slot holds, resolved the way the original resolves
/// one: by trying the tables the slot's category names, in order, and taking
/// the first that has an entry at the index.
#[must_use]
pub fn slot_part(s: &DesignSlot) -> Option<crate::parts::Part> {
    use crate::components::slot;
    for category in [
        slot::ENGINE,
        slot::ARMOR,
        slot::SHIELD,
        slot::SCANNER,
        slot::BEAM,
        slot::TORPEDO,
        slot::BOMB,
        slot::MINING,
        slot::MINES,
        slot::TERRA,
        slot::SPECIAL_E,
        slot::SPECIAL_M,
        slot::SPECIAL_SB,
    ] {
        if s.category & category != 0 {
            if let Some(p) = crate::parts::part(category, usize::from(s.item)) {
                return Some(p);
            }
        }
    }
    None
}

/// How many ship designs a player may have at once (`SlotDlg`; `MANUAL.PDF`
/// p. 9-6). Reaching it greys out **Copy Selected Design** until one is
/// deleted.
pub const MAX_SHIP_DESIGNS: usize = 16;

/// How many starbase designs a player may have at once (`FillBuildDD`;
/// `MANUAL.PDF` p. 9-7).
pub const MAX_STARBASE_DESIGNS: usize = 10;

/// The longest design name the game will take: `HUL.szClass` holds 32 bytes and
/// the name field is limited to 31 characters (`EM_LIMITTEXT`, `SlotDlg`).
pub const MAX_NAME: usize = 31;

/// The name a copied design gets (`MakeNewName`).
///
/// `Long Range Scout` becomes `Long Range Scout (2)`, and copying that again
/// gives `(3)`. Only the one digit is touched, so a ninth copy wraps to `(0)`
/// rather than reaching `(10)`, and a name already 28 characters long is left
/// exactly as it was — which is how two designs can end up sharing a name.
#[must_use]
pub fn copied_name(name: &str) -> String {
    if name.chars().count() >= 28 {
        return name.to_string();
    }
    let b = name.as_bytes();
    if b.len() >= 3
        && b[b.len() - 1] == b')'
        && b[b.len() - 2].is_ascii_digit()
        && b[b.len() - 3] == b'('
    {
        let digit = b[b.len() - 2];
        let next = if digit == b'9' { b'0' } else { digit + 1 };
        let mut out = name.to_string();
        out.replace_range(name.len() - 2..name.len() - 1, &(next as char).to_string());
        return out;
    }
    format!("{name} (2)")
}

/// The eight classes a hull belongs to.
///
/// Every hull carries one in the packed word at `+0x7B` of its `HULDEF`,
/// transcribed as [`crate::components::Hull::category`]. It is what the
/// scanner's **Enemy Ship Class filter** selects on — `CShipsScanVis`
/// (`1058:4bf4`) compares `(huldef.wFlags >> 10) & 0xf` against the bit the
/// player ticked — and the names are the game's own, from the eight
/// consecutive strings the filter's menu is built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShipClass {
    /// Colony ships.
    Colony = 0,
    /// Freighters.
    Freighter = 1,
    /// Scouts, frigates and destroyers.
    Scout = 2,
    /// Cruisers and up.
    Warship = 3,
    /// Privateers, mine layers, and the hulls that fit no other box.
    Utility = 4,
    /// Bombers.
    Bomber = 5,
    /// Mining ships.
    Miner = 6,
    /// The two tankers.
    FuelTransport = 7,
}

impl ShipClass {
    /// All eight, in the order the filter's menu lists them.
    pub const ALL: [ShipClass; 8] = [
        ShipClass::Colony,
        ShipClass::Freighter,
        ShipClass::Scout,
        ShipClass::Warship,
        ShipClass::Utility,
        ShipClass::Bomber,
        ShipClass::Miner,
        ShipClass::FuelTransport,
    ];

    /// The name the game gives it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ShipClass::Colony => "Colony",
            ShipClass::Freighter => "Freighter",
            ShipClass::Scout => "Scout",
            ShipClass::Warship => "Warship",
            ShipClass::Utility => "Utility",
            ShipClass::Bomber => "Bomber",
            ShipClass::Miner => "Miner",
            ShipClass::FuelTransport => "Fuel Transport",
        }
    }

    /// Its number, which is the bit the filter uses.
    #[must_use]
    pub fn index(self) -> u8 {
        self as u8
    }

    /// The class a hull's stored category names.
    #[must_use]
    pub fn from_category(category: u8) -> Option<ShipClass> {
        Self::ALL.get(usize::from(category)).copied()
    }
}

impl ShipDesign {
    /// Which class this design's hull puts it in.
    ///
    /// `None` for an empty design slot, and for a **starbase**: every starbase
    /// hull stores category 0, which would otherwise read as a colony ship.
    #[must_use]
    pub fn ship_class(&self) -> Option<ShipClass> {
        if self.hull_id < 0 || self.hull_id >= 32 {
            return None;
        }
        ShipClass::from_category(self.hull()?.category)
    }
}
