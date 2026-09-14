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

/// The `grobj` classes a waypoint's target can belong to.
///
/// These are the game's own `GrobjClass` bits, and the value stored in a
/// waypoint's high nibble is the class of whatever it landed on
/// (`FAddWayPoint`, `1058:7504`). They are also the bits
/// `FFindNearestObject` (`1038:4070`) is asked to search with.
pub mod grobj {
    /// A planet.
    pub const PLANET: u8 = 1;
    /// A fleet.
    pub const FLEET: u8 = 2;
    /// Nothing — a point in deep space, or one of the fleet's own waypoints.
    pub const POSITION: u8 = 4;
    /// A `THING`: a minefield, a packet, a wormhole or the Mystery Trader.
    pub const THING: u8 = 8;
}

/// A point a fleet is ordered to travel to, and how fast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waypoint {
    /// Where to go.
    pub position: Point,
    /// The planet or fleet it refers to, if any.
    pub target: Option<u16>,
    /// What kind of object [`Self::target`] names — one of [`grobj`]. The
    /// Merge task needs it: a bare id cannot say whether it means planet 7 or
    /// fleet 7.
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
    /// Which way a fleet seen at a distance is heading, when that is known.
    ///
    /// Only fleets described in **part** carry this — another player's, seen
    /// by scanner. A player's own fleets are described in full and their
    /// course is read from their waypoints instead, so this is `None` for
    /// them. See [`stars_formats::FleetRecord::direction`], which applies the
    /// bias and the validity flags.
    pub direction: Option<(i16, i16)>,
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

/// Which design a fleet is drawn as, and how many different ones it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Primary {
    /// The owner's design slot whose picture stands for the fleet.
    pub design: usize,
    /// How many of the sixteen slots the fleet has any ships in, which is what
    /// the original counts to decide how many extra marks to draw beside the
    /// picture.
    pub distinct: usize,
}

/// The design whose picture stands for a fleet.
///
/// `IshdefPrimaryFromLpfl` (`1038:3e1c`) walks the sixteen design slots in
/// order and keeps the one with the most ships, comparing strictly — so a tie
/// stays with the slot that got there first.
///
/// There is one twist. When the design it has just chosen is a **fuel
/// transport** — hull 25 or 26 — its count is docked by one, which lowers the
/// bar the *following* slots have to clear. The effect is narrow and worth
/// stating exactly: a tanker **loses a tie** it would otherwise have won, and
/// nothing more. A tanker that is genuinely the most numerous ship still holds
/// the picture; a tanker merely level with the warships beside it does not.
///
/// Returns `None` for a fleet with no ships in it, which is what the
/// original's out-of-range `16` means.
#[must_use]
pub fn primary_design(fleet: &Fleet, designs: &[crate::design::ShipDesign]) -> Option<Primary> {
    let mut best: Option<usize> = None;
    let mut beat = 0i32;
    let mut distinct = 0;
    for slot in 0..16u8 {
        let count: i32 = fleet
            .stacks
            .iter()
            .filter(|stack| stack.design == slot)
            .map(|stack| stack.count)
            .sum();
        if count <= 0 {
            continue;
        }
        distinct += 1;
        if beat < count {
            best = Some(usize::from(slot));
            beat = count;
            // A tanker holds the picture less firmly than anything else.
            if designs
                .get(usize::from(slot))
                .is_some_and(|design| matches!(design.hull_id, 25 | 26))
            {
                beat -= 1;
            }
        }
    }
    best.map(|design| Primary { design, distinct })
}

/// Share two fleets' cargo and fuel out after ships have passed between them.
///
/// `FleetTransferCargoBalance` (`1050:ae7d`), which the original runs on
/// every ship transfer — a split, a merge, a Split All, a stargate jump, a
/// minefield's toll — after the ship counts have changed and before the
/// cargo has. Each side gives up the share of what it carries that the
/// ships it **lost** made of its capacity: `fuel × lost tank ÷ tank` and
/// `cargo × lost hold ÷ hold`, the cargo spread over the four kinds in
/// proportion to what is aboard, with any rounding shortfall made up one
/// unit at a time from the first kind that still has some. A side that
/// gained ships gives nothing, so a split hands the new fleet its ships'
/// share and a merge pulls everything into the survivor.
///
/// `before` is each fleet's ship counts before the transfer, by design
/// slot. The damage rebalancing the same routine does is not modelled.
pub fn balance_cargo(fleets: [&mut Fleet; 2], before: [&[ShipStack]; 2], designs: &[ShipDesign]) {
    let [a, b] = fleets;
    let mut delta = [[0i64; 5]; 2];
    for (side, (fleet, was)) in [(&*a, before[0]), (&*b, before[1])].into_iter().enumerate() {
        let (mut fuel_cap, mut fuel_lost, mut cargo_cap, mut cargo_lost) = (0i64, 0i64, 0i64, 0i64);
        for stack in was {
            let design = designs.get(usize::from(stack.design));
            let tank = i64::from(design.and_then(ShipDesign::fuel_capacity).unwrap_or(0));
            let hold = i64::from(design.and_then(ShipDesign::cargo_capacity).unwrap_or(0));
            let now = fleet
                .stacks
                .iter()
                .find(|s| s.design == stack.design)
                .map_or(0, |s| s.count);
            let lost = i64::from(stack.count - now);
            fuel_cap += i64::from(stack.count) * tank;
            cargo_cap += i64::from(stack.count) * hold;
            if lost > 0 {
                fuel_lost += lost * tank;
                cargo_lost += lost * hold;
            }
        }
        if fuel_cap != 0 {
            delta[side][4] -= i64::from(fleet.cargo.fuel) * fuel_lost / fuel_cap;
        }
        if cargo_cap != 0 {
            let holds = [
                fleet.cargo.minerals[0],
                fleet.cargo.minerals[1],
                fleet.cargo.minerals[2],
                fleet.cargo.colonists,
            ];
            let total: i64 = holds.iter().map(|h| i64::from(*h)).sum();
            if total != 0 {
                let moving = total * cargo_lost / cargo_cap;
                let mut left = moving;
                for (kind, held) in holds.iter().enumerate() {
                    let part = (i64::from(*held) * moving / total).min(left);
                    delta[side][kind] -= part;
                    left -= part;
                }
                for (kind, held) in holds.iter().enumerate() {
                    if left <= 0 {
                        break;
                    }
                    if i64::from(*held) + delta[side][kind] > 0 {
                        delta[side][kind] -= 1;
                        left -= 1;
                    }
                }
            }
        }
    }
    // What one side sheds, the other takes.
    let net: Vec<i32> = (0..5)
        .map(|kind| i32::try_from(delta[0][kind] - delta[1][kind]).unwrap_or(0))
        .collect();
    for (fleet, sign) in [(a, 1), (b, -1)] {
        for (held, moved) in fleet.cargo.minerals.iter_mut().zip(&net) {
            *held += sign * moved;
        }
        fleet.cargo.colonists += sign * net[3];
        fleet.cargo.fuel += sign * net[4];
    }
}

#[cfg(test)]
mod primary_tests {
    use super::*;
    use crate::design::ShipDesign;

    fn design(hull_id: i16) -> ShipDesign {
        ShipDesign {
            hull_id,
            slots: Vec::new(),
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            designed: 0,
            built: 0,
        }
    }

    fn fleet(stacks: &[(u8, i32)]) -> Fleet {
        Fleet {
            id: 1,
            owner: 0,
            position: Point::new(0, 0),
            orbiting: None,
            stacks: stacks
                .iter()
                .map(|(design, count)| ShipStack {
                    design: *design,
                    count: *count,
                    damaged_pct: 0,
                    damage_pct: 0,
                })
                .collect(),
            cargo: Cargo::default(),
            battle_plan: 0,
            warp: None,
            waypoints: Vec::new(),
            name: None,
            repeat_orders: false,
            direction: None,
        }
    }

    /// The most numerous design holds the picture, and a tie goes to the
    /// earlier slot because the comparison is strict.
    #[test]
    fn the_commonest_design_holds_the_picture() {
        let designs = [design(4), design(6), design(9)];
        let f = fleet(&[(0, 3), (1, 7), (2, 2)]);
        let primary = primary_design(&f, &designs).expect("a design");
        assert_eq!(primary.design, 1);
        assert_eq!(primary.distinct, 3);

        let tie = fleet(&[(0, 5), (2, 5)]);
        assert_eq!(
            primary_design(&tie, &designs).map(|p| p.design),
            Some(0),
            "a tie stays with the first"
        );
    }

    /// A fuel transport loses a tie it would otherwise have won, because the
    /// original docks its count by one once it has been chosen. That is the
    /// whole of the rule: it is a tie-break, not a ban.
    #[test]
    fn a_tanker_loses_a_tie() {
        for tanker in [25, 26] {
            let designs = [design(tanker), design(10)];

            // Level with the dreadnoughts, the tanker gives way — where any
            // other design in slot 0 would have kept it.
            let level = fleet(&[(0, 6), (1, 6)]);
            assert_eq!(
                primary_design(&level, &designs).map(|p| p.design),
                Some(1),
                "hull {tanker} yields the tie"
            );
            let ordinary = [design(4), design(10)];
            assert_eq!(
                primary_design(&level, &ordinary).map(|p| p.design),
                Some(0),
                "where a freighter would have kept it"
            );

            // A single ship clear and it holds the picture: the decrement
            // costs it a tie and nothing more.
            let ahead = fleet(&[(0, 6), (1, 5)]);
            assert_eq!(
                primary_design(&ahead, &designs).map(|p| p.design),
                Some(0),
                "hull {tanker} keeps it when it really is the most numerous"
            );
        }
    }

    /// A fleet with nothing in it has no picture.
    #[test]
    fn an_empty_fleet_has_no_primary() {
        let designs = [design(4)];
        assert_eq!(primary_design(&fleet(&[]), &designs), None);
        assert_eq!(primary_design(&fleet(&[(0, 0)]), &designs), None);
    }
}
