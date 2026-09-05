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

/// A point a fleet is ordered to travel to, and how fast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waypoint {
    /// Where to go.
    pub position: Point,
    /// The planet or fleet it refers to, if any.
    pub target: Option<u16>,
    /// What kind of object [`Self::target`] names (`grobj`): 1 a planet, 2 a
    /// fleet, 4 nothing, 8 a `THING`. The Merge task needs it — a bare id
    /// cannot say whether it means planet 7 or fleet 7.
    pub target_class: u8,
    /// Warp factor for the leg **into** this waypoint.
    pub warp: u8,
    /// The task to perform on arrival, as stored. See
    /// [`stars_formats::task`] for the ids.
    ///
    /// A task is **consumed when it executes**, which is why every waypoint in
    /// a saved game that has already been reached reads `0`.
    pub task: u8,
    /// A Transport task's per-cargo instructions, when the waypoint carries
    /// one.
    pub transport: Option<stars_formats::TransportTask>,
    /// The task's payload exactly as the file holds it — the ten bytes after
    /// the waypoint header, empty when there is no task.
    ///
    /// [`Self::transport`] is the decoded view of it for a Transport task. The
    /// Lay Minefield task uses the first word as a **countdown of years**, so
    /// the raw bytes have to survive a load and a save; every other task's
    /// payload is carried for the same reason.
    pub task_data: Vec<u8>,
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
    /// Ordered waypoints. The first is where the fleet is now; the second, if
    /// present, is where it is heading.
    pub waypoints: Vec<Waypoint>,
    /// The name the player gave the fleet, when they have renamed it.
    ///
    /// An unnamed fleet is shown as its design and number ("Long Range Scout
    /// #3"), which is why this is optional rather than always filled in.
    ///
    /// It is written to and read from a [`FLEET_NAME_BLOCK`] that follows the
    /// fleet's waypoints. No file in this repository's fixtures contains one —
    /// nobody renamed a fleet in any of the captured games — so the layout is
    /// recovered from the binary rather than fixture-verified; see that
    /// constant.
    pub name: Option<String>,
    /// Whether the fleet's waypoint orders repeat (`FLEET.fRepOrders`).
    ///
    /// A repeating fleet returns to its first waypoint once it reaches its
    /// last, which is how a freighter is set to shuttle back and forth
    /// indefinitely. The turn generator does not act on it yet; it is carried
    /// so that reading and writing a file, and replaying an order that changes
    /// it, do not lose it.
    pub repeat_orders: bool,
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

    /// Where the fleet is heading, and at what warp, if it has somewhere to go.
    #[must_use]
    pub fn next_leg(&self) -> Option<(Point, u8)> {
        let next = self.waypoints.get(1)?;
        (next.warp > 0).then_some((next.position, next.warp))
    }

    /// Fuel the fleet burns covering `distance` light years at `warp`.
    ///
    /// Source: `EstFuelUse` (`1050:9fe4`). The subtlety is the cargo: it is
    /// assigned to the **most fuel-efficient designs first**, filling each up
    /// to its capacity, so a fleet carries its load in whatever burns least to
    /// move it. Each design then burns
    /// `mass * engine figure * distance / 2000`, and the total is divided by
    /// ten, rounding up.
    ///
    /// Improved Fuel Efficiency cuts each engine's figure by 15% first.
    #[must_use]
    pub fn fuel_use(
        &self,
        designs: &[ShipDesign],
        warp: u8,
        distance: i32,
        improved_fuel_efficiency: bool,
    ) -> i32 {
        // Pair each stack with its engine's fuel figure at this warp; a stack
        // with no usable engine is treated as very thirsty, as the original
        // does.
        let mut stacks: Vec<(i32, &ShipStack, &ShipDesign)> = Vec::new();
        for stack in &self.stacks {
            let Some(design) = designs.get(usize::from(stack.design)) else {
                continue;
            };
            let efficiency = design
                .engine()
                .and_then(|e| e.fuel_used.get(usize::from(warp)).map(|f| i32::from(*f)))
                .unwrap_or(99_999);
            stacks.push((efficiency, stack, design));
        }
        // Cheapest to move first.
        stacks.sort_by_key(|(efficiency, _, _)| *efficiency);

        let mut cargo_left = self.cargo.mass();
        let mut total: i64 = 0;
        for (efficiency, stack, design) in stacks {
            let mut efficiency = i64::from(efficiency);
            if improved_fuel_efficiency {
                efficiency -= efficiency * 15 / 100;
            }
            let capacity = design.cargo_capacity().unwrap_or(0) * stack.count;
            let carried = cargo_left.min(capacity.max(0));
            cargo_left -= carried;

            let mass =
                i64::from(carried) + i64::from(stack.count) * i64::from(design.mass().unwrap_or(0));
            let scaled = efficiency * i64::from(distance);
            if mass <= 0 || scaled <= 0 {
                continue;
            }
            total += mass * scaled / 2000;
        }
        i32::try_from((total + 9) / 10).unwrap_or(i32::MAX)
    }

    /// How far the fleet could travel at `warp` on the fuel it has.
    #[must_use]
    pub fn fuel_range(
        &self,
        designs: &[ShipDesign],
        warp: u8,
        improved_fuel_efficiency: bool,
    ) -> i32 {
        // The original measures use over a nominal 1000 light years and scales.
        let per_1000 = self.fuel_use(designs, warp, 1000, improved_fuel_efficiency);
        if per_1000 <= 0 {
            return i32::MAX; // a ramscoop at a free warp
        }
        i32::try_from(i64::from(self.cargo.fuel) * 1000 / i64::from(per_1000)).unwrap_or(i32::MAX)
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
