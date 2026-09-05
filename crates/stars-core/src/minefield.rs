//! Minefields: laying them, and what happens to a fleet that flies through one.
//!
//! A minefield is a `THING` in the file (see `docs/formats/thing.md`) and a
//! circle in the galaxy: its **radius in light years is the square root of its
//! mine count**, which is why the game stores nothing but a centre and a count.
//! The rule is visible in the laying code, which asks whether the fleet is
//! inside an existing field by comparing the *squared* distance against the
//! count (`10b0:9c2b`).
//!
//! There are three kinds, and every number that separates them is a table in
//! the executable, read out at the addresses below:
//!
//! | kind | safe warp | hit chance | damage/ship | minimum |
//! |------|-----------|------------|-------------|---------|
//! | 0 standard | 4 | 0.3% | 100 (125) | 500 (600) |
//! | 1 heavy | 6 | 1.0% | 500 (600) | 2000 (2500) |
//! | 2 speed bump | 5 | 3.5% | 0 | 0 |
//!
//! The bracketed figure applies to a fleet with a ram scoop. The hit chance is
//! per light year travelled, per warp factor over the safe speed.
//!
//! See `docs/formulas/minefields.md`.

use crate::design::ShipDesign;
use crate::fleet::Fleet;
use crate::movement::Point;
use crate::rng::Rng;

/// How many kinds of minefield there are.
pub const MINE_KINDS: usize = 3;

/// A field this big or bigger takes no more mines; the next lot starts a new
/// field (`10b0:9cbe`).
pub const MAX_MINES: i32 = 1_000_000;

/// Fastest warp that never trips a field, by kind (`rgiWarpSafe`,
/// `10b0:4f5a`).
pub const SAFE_WARP: [i32; MINE_KINDS] = [4, 6, 5];

/// Chance in a thousand of a hit, per light year, per warp over the safe speed
/// (`rgpctMineHit`, `10b0:4f54`).
pub const HIT_PER_MILLE: [i32; MINE_KINDS] = [3, 10, 35];

/// Damage a hit does per ship, by kind and by whether the fleet has a ram scoop
/// (`rgrgdmgMine`, `10b0:4f3c`).
pub const DAMAGE_PER_SHIP: [[i32; 2]; MINE_KINDS] = [[100, 125], [500, 600], [0, 0]];

/// Least damage a hit does in total, for a fleet of fewer than five ships
/// (`rgrgdmgMinMine`, `10b0:4f48`).
pub const MIN_DAMAGE: [[i32; 2]; MINE_KINDS] = [[500, 600], [2000, 2500], [0, 0]];

/// Which mine-layer items lay which kind of field.
///
/// `CLayMinesFromLpfl` (`1080:2886`) takes a kind and bounds the item index it
/// will count; the three bounds split the ten mine layers into the four Mine
/// Dispensers, the three Heavy Dispensers and the three Speed Traps.
pub const LAYERS_FOR_KIND: [(usize, usize); MINE_KINDS] = [(0, 3), (4, 6), (7, 9)];

/// A minefield in play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minefield {
    /// Object id, unique among the game's `THING`s for this owner.
    pub id: u16,
    /// The player who laid it.
    pub owner: i16,
    /// Centre of the field.
    pub position: Point,
    /// How many mines it holds. Its radius is the square root of this.
    pub mines: i32,
    /// 0 standard, 1 heavy, 2 speed bump.
    pub kind: u8,
    /// Armed to detonate (`THMINE.fDetonate`).
    pub detonating: bool,
    /// Players who have ever detected it (`THMINE.grbitPlr`), kept so the field
    /// writes back as it was read.
    pub detected_by: u16,
    /// Players who can see it now (`THMINE.grbitPlrNow`), likewise.
    pub visible_to: u16,
    /// The turn stamp the record carries.
    pub turn: u16,
}

impl Minefield {
    /// Radius in light years: the square root of the mine count.
    #[must_use]
    pub fn radius(&self) -> f64 {
        f64::from(self.mines.max(0)).sqrt()
    }

    /// Whether a point is inside the field.
    ///
    /// Compared squared, as the original does (`10b0:9c2b`): a point is inside
    /// when `dx² + dy² <= mines`.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        self.distance_squared(point) <= i64::from(self.mines)
    }

    /// Squared distance from the field's centre to a point.
    #[must_use]
    pub fn distance_squared(&self, point: Point) -> i64 {
        let dx = i64::from(point.x) - i64::from(self.position.x);
        let dy = i64::from(point.y) - i64::from(self.position.y);
        dx * dx + dy * dy
    }
}

/// How many mines of one kind a fleet lays in a year.
///
/// `CLayMinesFromLpfl` (`1080:2886`): for every design in the fleet, add up the
/// mine layers fitted — the slot's count times the part's `ability` — multiply
/// by the ships carrying them, and multiply the total by ten. The ten is what
/// turns a "Mine Dispenser 40"'s stored ability of 4 into the forty mines its
/// name promises.
///
/// Two cases in the original are **not** modelled: one beam-slot item that also
/// counts toward standard fields, and hulls 27 and 28, whose per-ship total is
/// replaced rather than added to.
#[must_use]
pub fn mines_laid(fleet: &Fleet, designs: &[ShipDesign], kind: u8) -> i32 {
    let Some((first, last)) = LAYERS_FOR_KIND.get(usize::from(kind)).copied() else {
        return 0;
    };
    let mut total: i64 = 0;
    for stack in &fleet.stacks {
        if stack.count <= 0 {
            continue;
        }
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        let mut per_ship: i64 = 0;
        for slot in &design.slots {
            if slot.category != crate::components::slot::MINES || slot.count == 0 {
                continue;
            }
            let item = usize::from(slot.item);
            if item < first || item > last {
                continue;
            }
            let ability = crate::components::MINE_LAYERS
                .get(item)
                .map_or(0, |p| i64::from(p.ability));
            per_ship += i64::from(slot.count) * ability;
        }
        total += per_ship * i64::from(stack.count);
    }
    i32::try_from(total * 10).unwrap_or(i32::MAX)
}

/// Lay a fleet's mines, of every kind it can.
///
/// The placement rule is the loop at `10b0:9aa9`. For each kind in turn:
///
/// * a fleet that **moved** this year lays half as many, which only arises for
///   a Space Demolition player, since nobody else may lay while moving;
/// * the fleet's own fields of that kind that **contain** it are considered,
///   nearest first, and the nearest one that is not already full takes the
///   mines. Its centre moves toward the fleet, weighted by the two mine counts;
/// * otherwise a new field starts where the fleet is.
///
/// Returns what was laid, as `(kind, mines)` per kind that laid anything.
pub fn lay(
    minefields: &mut Vec<Minefield>,
    fleet: &Fleet,
    designs: &[ShipDesign],
    moved: bool,
) -> Vec<(u8, i32)> {
    let mut laid = Vec::new();
    for kind in 0..MINE_KINDS {
        let kind = u8::try_from(kind).unwrap_or(0);
        let mut mines = mines_laid(fleet, designs, kind);
        if mines == 0 {
            continue;
        }
        if moved {
            mines /= 2;
        }
        if mines == 0 {
            continue;
        }
        add_mines(minefields, fleet.owner, fleet.position, kind, mines);
        laid.push((kind, mines));
    }
    laid
}

/// Put `mines` of one kind at a point, growing the nearest field that holds
/// them or starting a new one.
pub fn add_mines(
    minefields: &mut Vec<Minefield>,
    owner: i16,
    position: Point,
    kind: u8,
    mines: i32,
) {
    let target = minefields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.owner == owner && f.kind == kind && f.contains(position))
        .min_by_key(|(_, f)| f.distance_squared(position))
        .map(|(index, _)| index)
        .filter(|index| minefields[*index].mines <= MAX_MINES);

    match target {
        Some(index) => {
            let field = &mut minefields[index];
            // The centre is the mine-weighted average of the two: 10b0:9cc9.
            let total = i64::from(field.mines) + i64::from(mines);
            if total > 0 {
                let weigh = |old: i16, new: i16| -> i16 {
                    let moved = (i64::from(new) * i64::from(mines)
                        + i64::from(old) * i64::from(field.mines))
                        / total;
                    i16::try_from(moved).unwrap_or(old)
                };
                field.position = Point::new(
                    weigh(field.position.x, position.x),
                    weigh(field.position.y, position.y),
                );
            }
            field.mines = field.mines.saturating_add(mines);
        }
        None => {
            let id = minefields
                .iter()
                .filter(|f| f.owner == owner)
                .map(|f| f.id)
                .max()
                .map_or(0, |id| id + 1);
            minefields.push(Minefield {
                id,
                owner,
                position,
                mines,
                kind,
                detonating: false,
                detected_by: 0,
                visible_to: 0,
                turn: 0,
            });
        }
    }
}

/// One year's flying: where the fleet started, where it ended up, and how far
/// that was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    /// Where the leg began.
    pub from: Point,
    /// Where it ended.
    pub to: Point,
    /// Light years covered.
    pub travelled: i32,
}

/// What a fleet ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MineHit {
    /// The field it hit.
    pub field: u16,
    /// That field's owner.
    pub field_owner: i16,
    /// Which kind it was.
    pub kind: u8,
    /// How far along its leg the fleet got, in light years.
    pub travelled: i32,
    /// Total damage taken.
    pub damage: i32,
    /// Ships destroyed by it.
    pub ships_lost: i32,
}

/// The mine-expertise a race brings: Space Demolition counts as two warp
/// factors, Super Stealth as one (`10b0:4f94`).
#[must_use]
pub fn mine_expertise(race: &crate::race::Race) -> i32 {
    match race.prt() {
        Some(crate::race::Prt::Sd) => 2,
        Some(crate::race::Prt::Ss) => 1,
        _ => 0,
    }
}

/// Fly a leg and see whether it ends in a minefield.
///
/// `FTravelThroughMineFields` (`10b0:4f60`). The speed that matters is the one
/// recovered from the distance travelled — the original searches for the first
/// `iWarp` in `3..10` whose square reaches `travelled - 1` — and a fleet that
/// slow enough, or one whose expertise covers the difference, is never at risk.
/// Only fields belonging to **someone else who is not a friend** are tested.
///
/// Each field the leg crosses gives an interval of light years; the intervals
/// are walked in order and each light year inside one is a separate roll of
/// `Random(1000) < (warp - safe - expertise) × chance`. The first hit stops the
/// fleet there.
///
/// Two things the original does that this does not: it merges overlapping
/// intervals of the **same kind** so that two fields on top of each other are
/// rolled once, and it scales damage by the engine count and lets shields
/// absorb. Damage here is the fleet's total, applied as whole ships lost.
#[must_use]
pub fn traverse(
    minefields: &[Minefield],
    fleet: &Fleet,
    leg: Leg,
    expertise: i32,
    friendly: &dyn Fn(i16) -> bool,
    rng: &mut Rng,
) -> Option<MineHit> {
    let Leg {
        from,
        to,
        travelled,
    } = leg;
    if travelled <= 0 || from == to {
        return None;
    }
    let mut warp = 3;
    while warp < 10 && warp * warp < travelled - 1 {
        warp += 1;
    }
    if warp <= expertise + 3 {
        return None;
    }

    let mut crossings: Vec<(i32, i32, usize)> = Vec::new();
    for (index, field) in minefields.iter().enumerate() {
        if field.owner == fleet.owner || friendly(field.owner) {
            continue;
        }
        if let Some((start, end)) = crossing(from, to, field, travelled) {
            crossings.push((start, end, index));
        }
    }
    crossings.sort_unstable();

    for (start, end, index) in crossings {
        let field = &minefields[index];
        let kind = usize::from(field.kind).min(MINE_KINDS - 1);
        let over = warp - SAFE_WARP[kind] - expertise;
        if over <= 0 {
            continue;
        }
        let chance = over * HIT_PER_MILLE[kind];
        for step in 0..(end - start).max(0) {
            if i32::from(rng.random(1000)) < chance {
                return Some(hit(fleet, field, start + step));
            }
        }
    }
    None
}

/// Where a leg enters and leaves a field, in light years from its start.
///
/// The original is `FIntersectCircleLine`; this solves the same quadratic and
/// clamps to the part of the leg actually flown this year.
fn crossing(from: Point, to: Point, field: &Minefield, travelled: i32) -> Option<(i32, i32)> {
    let dx = f64::from(to.x) - f64::from(from.x);
    let dy = f64::from(to.y) - f64::from(from.y);
    let length = dx.hypot(dy);
    if length <= 0.0 {
        return None;
    }
    let (ux, uy) = (dx / length, dy / length);
    let cx = f64::from(field.position.x) - f64::from(from.x);
    let cy = f64::from(field.position.y) - f64::from(from.y);
    // Distance along the leg to the closest approach, and how far off it passes.
    let along = cx * ux + cy * uy;
    let off2 = (cx * cx + cy * cy) - along * along;
    let radius = field.radius();
    let half2 = radius * radius - off2;
    if half2 <= 0.0 {
        return None;
    }
    let half = half2.sqrt();
    let start = (along - half).max(0.0);
    let end = (along + half).min(f64::from(travelled));
    if end <= start {
        return None;
    }
    Some((start as i32, end as i32))
}

/// What a hit costs the fleet.
fn hit(fleet: &Fleet, field: &Minefield, travelled: i32) -> MineHit {
    let kind = usize::from(field.kind).min(MINE_KINDS - 1);
    let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
    let ram_scoop = 0; // Ram scoops are not modelled; the plain column applies.
    let per_ship = DAMAGE_PER_SHIP[kind][ram_scoop];
    let mut damage = per_ship * ships;
    if ships <= 4 {
        // A small fleet takes a minimum total instead.
        damage = damage.max(MIN_DAMAGE[kind][ram_scoop]);
    }
    MineHit {
        field: field.id,
        field_owner: field.owner,
        kind: field.kind,
        travelled,
        damage,
        ships_lost: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{slot, MINE_LAYERS};
    use crate::design::{DesignSlot, ShipDesign};
    use crate::fleet::{Cargo, Fleet, ShipStack};

    fn layer(item: u8, count: u8) -> DesignSlot {
        DesignSlot {
            category: slot::MINES,
            item,
            count,
        }
    }

    fn design(slots: Vec<DesignSlot>) -> ShipDesign {
        ShipDesign {
            name: "Layer".to_string(),
            picture: 0,
            stored_armor: 0,
            hull_id: 0,
            slots,
        }
    }

    fn fleet(stacks: Vec<ShipStack>) -> Fleet {
        Fleet {
            name: None,
            repeat_orders: false,
            id: 1,
            owner: 0,
            position: Point::new(1000, 1000),
            orbiting: None,
            stacks,
            cargo: Cargo::default(),
            battle_plan: 0,
            warp: None,
            waypoints: Vec::new(),
        }
    }

    fn stack(design: u8, count: i32) -> ShipStack {
        ShipStack {
            design,
            count,
            damaged_pct: 0,
            damage_pct: 0,
        }
    }

    /// A "Mine Dispenser 40" lays the forty mines its name promises, which is
    /// the stored ability of 4 times the ten in `CLayMinesFromLpfl`.
    #[test]
    fn a_dispenser_lays_what_its_name_says() {
        assert_eq!(MINE_LAYERS[0].name, "Mine Dispenser 40");
        let designs = vec![design(vec![layer(0, 1)])];
        let one = fleet(vec![stack(0, 1)]);
        assert_eq!(mines_laid(&one, &designs, 0), 40);
        // Three ships, two dispensers each.
        let more = fleet(vec![stack(0, 3)]);
        let designs = vec![design(vec![layer(0, 2)])];
        assert_eq!(mines_laid(&more, &designs, 0), 240);
    }

    /// The three kinds of layer lay three kinds of field, and never each
    /// other's: the item ranges are what `CLayMinesFromLpfl` bounds.
    #[test]
    fn each_layer_lays_its_own_kind() {
        let designs = vec![design(vec![layer(4, 1)])]; // Heavy Dispenser 50
        let heavy = fleet(vec![stack(0, 1)]);
        assert_eq!(mines_laid(&heavy, &designs, 0), 0, "not a standard field");
        assert_eq!(mines_laid(&heavy, &designs, 1), 50);
        assert_eq!(mines_laid(&heavy, &designs, 2), 0);
    }

    /// A field grows where it stands, and its centre drifts toward the fleet in
    /// proportion to what is laid.
    #[test]
    fn laying_into_a_field_moves_its_centre() {
        let mut fields = vec![Minefield {
            id: 0,
            owner: 0,
            position: Point::new(1000, 1000),
            mines: 300,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        }];
        // 10 light years away, well inside a field of radius sqrt(300) ≈ 17.3.
        add_mines(&mut fields, 0, Point::new(1010, 1000), 0, 100);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].mines, 400);
        // (1010×100 + 1000×300) / 400 = 1002.5, truncated.
        assert_eq!(fields[0].position, Point::new(1002, 1000));
    }

    /// Outside every field, mines start a new one; and a field belonging to
    /// somebody else is never grown.
    #[test]
    fn mines_outside_a_field_start_another() {
        let mut fields = vec![Minefield {
            id: 0,
            owner: 1,
            position: Point::new(1000, 1000),
            mines: 10_000,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        }];
        // Inside player 1's field, but player 0 is laying.
        add_mines(&mut fields, 0, Point::new(1010, 1000), 0, 100);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].owner, 0);
        assert_eq!(fields[1].position, Point::new(1010, 1000));

        // Far outside its own field: another new one.
        add_mines(&mut fields, 0, Point::new(2000, 2000), 0, 100);
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[2].id, 1, "ids count up per player");
    }

    /// A field's radius is the square root of its mine count.
    #[test]
    fn a_field_reaches_the_root_of_its_mines() {
        let field = Minefield {
            id: 0,
            owner: 0,
            position: Point::new(1000, 1000),
            mines: 400,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        };
        assert!((field.radius() - 20.0).abs() < f64::EPSILON);
        assert!(field.contains(Point::new(1020, 1000)), "on the edge");
        assert!(!field.contains(Point::new(1021, 1000)), "just outside");
    }

    /// Slow enough is safe: the original recovers the speed from the distance
    /// travelled and lets anything at or under the safe warp through.
    #[test]
    fn a_slow_fleet_is_never_caught() {
        let mut rng = crate::rng::Rng::from_seeds(1, 2);
        let fields = vec![Minefield {
            id: 0,
            owner: 1,
            position: Point::new(1050, 1000),
            mines: 10_000, // radius 100: the whole leg is inside it
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        }];
        let fleet = fleet(vec![stack(0, 1)]);
        let never = |_: i16| false;
        // 16 light years: the search stops at warp 4, the safe speed.
        assert!(traverse(
            &fields,
            &fleet,
            Leg {
                from: Point::new(1000, 1000),
                to: Point::new(1016, 1000),
                travelled: 16,
            },
            0,
            &never,
            &mut rng
        )
        .is_none());
    }

    /// Fast and far enough through a big field, something is going to happen.
    #[test]
    fn a_fast_fleet_crossing_a_field_is_hit() {
        let mut rng = crate::rng::Rng::from_seeds(3, 4);
        let fields = vec![Minefield {
            id: 7,
            owner: 1,
            position: Point::new(1050, 1000),
            mines: 10_000,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        }];
        let fleet = fleet(vec![stack(0, 10)]);
        let never = |_: i16| false;
        let hit = traverse(
            &fields,
            &fleet,
            Leg {
                from: Point::new(1000, 1000),
                to: Point::new(1081, 1000),
                travelled: 81, // warp 9
            },
            0,
            &never,
            &mut rng,
        )
        .expect("81 light years at warp 9 through a 100 ly field");
        assert_eq!(hit.field, 7);
        assert_eq!(hit.kind, 0);
        assert!(hit.travelled <= 81);
        assert_eq!(hit.damage, 100 * 10, "100 a ship, ten ships");
    }

    /// A friend's minefield is not a hazard, and neither is your own.
    #[test]
    fn friends_and_your_own_fields_are_safe() {
        let mut rng = crate::rng::Rng::from_seeds(5, 6);
        let mut fields = vec![Minefield {
            id: 0,
            owner: 0, // the fleet's own
            position: Point::new(1050, 1000),
            mines: 10_000,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        }];
        let fleet = fleet(vec![stack(0, 10)]);
        let never = |_: i16| false;
        let leg =
            |fields: &[Minefield], rng: &mut crate::rng::Rng, friendly: &dyn Fn(i16) -> bool| {
                traverse(
                    fields,
                    &fleet,
                    Leg {
                        from: Point::new(1000, 1000),
                        to: Point::new(1081, 1000),
                        travelled: 81,
                    },
                    0,
                    friendly,
                    rng,
                )
            };
        assert!(leg(&fields, &mut rng, &never).is_none(), "its own field");

        fields[0].owner = 1;
        let friend = |other: i16| other == 1;
        assert!(leg(&fields, &mut rng, &friend).is_none(), "a friend's");
    }

    /// A fleet of four or fewer takes the minimum instead of the per-ship
    /// figure, which is what makes a lone scout so expensive to lose.
    #[test]
    fn a_small_fleet_takes_the_minimum() {
        let field = Minefield {
            id: 0,
            owner: 1,
            position: Point::new(1000, 1000),
            mines: 100,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        };
        // One ship: 100 damage per ship, but at least 500 in total.
        assert_eq!(hit(&fleet(vec![stack(0, 1)]), &field, 0).damage, 500);
        // Five ships: 500, the per-ship figure, and no top-up.
        assert_eq!(hit(&fleet(vec![stack(0, 5)]), &field, 0).damage, 500);
        // Six: 600.
        assert_eq!(hit(&fleet(vec![stack(0, 6)]), &field, 0).damage, 600);
    }
}
