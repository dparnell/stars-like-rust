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

/// How many mines a hit costs the field itself.
///
/// `FTravelThroughMineFields` (`10b0:2097`): a twentieth of the field, but a
/// hundredth once that would exceed fifty, with floors of ten and fifty. A
/// small field loses about 5% of itself, a large one about 1%.
#[must_use]
pub fn hit_cost(mines: i32) -> i32 {
    let twentieth = mines / 20;
    if twentieth < 51 {
        twentieth.max(10)
    } else {
        (mines / 100).max(50)
    }
}

/// What a design sweeps in a year.
///
/// `CMineSweepFromLphul` (`1080:2bfa`): every beam slot sweeps `range² ×
/// damage` mines per weapon. A **starbase** reaches one square further, a
/// **gattling** sweeps as though its range were 4 whatever it really is, and a
/// **sapper** sweeps nothing at all.
#[must_use]
pub fn sweep_capacity(design: &ShipDesign) -> i32 {
    let starbase = design.is_starbase();
    let mut total: i64 = 0;
    for slot in &design.slots {
        if slot.category != crate::components::slot::BEAM || slot.count == 0 {
            continue;
        }
        let Some(beam) = crate::components::BEAMS.get(usize::from(slot.item)) else {
            continue;
        };
        if beam.abilities & 1 != 0 {
            continue; // a sapper sweeps nothing
        }
        let mut range = if beam.abilities & 2 != 0 {
            4
        } else {
            i64::from(beam.range_max)
        };
        if starbase {
            range += 1;
        }
        total += range * range * i64::from(slot.count) * i64::from(beam.dp);
    }
    i32::try_from(total.max(0)).unwrap_or(i32::MAX)
}

/// What a whole fleet sweeps in a year.
#[must_use]
pub fn fleet_sweep(fleet: &Fleet, designs: &[ShipDesign]) -> i32 {
    let mut total: i64 = 0;
    for stack in &fleet.stacks {
        if stack.count <= 0 {
            continue;
        }
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        total += i64::from(sweep_capacity(design)) * i64::from(stack.count);
    }
    i32::try_from(total).unwrap_or(i32::MAX)
}

/// Take `sweep` mines out of a field somebody is sitting in.
///
/// `SweepForMines` (`10b8:76a4`). A **speed bump** field only gives up a third
/// of what the sweeper can manage, and every sweep clears at least two mines.
/// The odd-looking last rule is the original's: a field is never left so large
/// that the sweeper is still inside it, so sweeping from the centre destroys
/// the field outright.
///
/// `distance_squared` is the sweeper's distance from the field's centre.
/// Returns the mines actually swept.
#[must_use]
pub fn swept(field: &Minefield, sweep: i32, distance_squared: i64) -> i32 {
    let mut take = i64::from(sweep);
    if field.kind == 2 {
        take /= 3;
    }
    take = take.max(2);
    if i64::from(field.mines) - take < distance_squared - 1 {
        take = i64::from(field.mines) - distance_squared + 1;
    }
    take = take.min(i64::from(field.mines)).max(0);
    i32::try_from(take).unwrap_or(i32::MAX)
}

/// How much of itself a field loses in a year, as a percentage.
///
/// `ThingDecay` (`10b8:70c6`): two percent, plus four for every planet inside
/// the field — one for a Space Demolition player, whose fields last four times
/// as long — capped at fifty, and twenty-five more if the field is armed to
/// detonate.
#[must_use]
pub fn decay_percent(planets_inside: i32, space_demolition: bool, detonating: bool) -> i32 {
    let per_planet = if space_demolition { 1 } else { 4 };
    let mut pct = (per_planet * planets_inside + 2).min(50);
    if detonating {
        pct += 25;
    }
    pct
}

/// How many mines a field loses in a year.
///
/// The percentage above, but never fewer mines than the percentage itself, and
/// never fewer than ten unless the field is a speed bump.
#[must_use]
pub fn decay_amount(field: &Minefield, planets_inside: i32, space_demolition: bool) -> i32 {
    let pct = decay_percent(planets_inside, space_demolition, field.detonating);
    let mut lost = (i64::from(field.mines) * i64::from(pct) / 100).max(i64::from(pct));
    if field.kind != 2 && lost < 10 {
        lost = 10;
    }
    i32::try_from(lost).unwrap_or(i32::MAX)
}

/// What a fleet ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MineHit {
    /// The field it hit.
    pub field: u16,
    /// That field's owner.
    pub field_owner: i16,
    /// Which kind it was.
    pub kind: u8,
    /// How far along its leg the fleet got, in light years.
    pub travelled: i32,
    /// Total damage dealt, before shields and armour, capped at `0x7ff8` as
    /// the messages carry it.
    pub damage: i32,
    /// Ships destroyed by it.
    pub ships_lost: i32,
    /// Whether nothing of the fleet was left.
    pub fleet_destroyed: bool,
    /// Whether the field went off under the fleet rather than being flown
    /// into.
    pub detonation: bool,
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
/// Each field the leg crosses gives an interval of light years, kept **per
/// kind** in a list of up to eight sorted by start (`10b0:5049`–`10b0:52a0`):
/// an interval overlapping or touching one already there — its end no
/// earlier than the other's start less one — is merged into it, swallowing
/// any later intervals it now reaches, so two fields of a kind on top of each
/// other are rolled once; a ninth disjoint interval of a kind is dropped.
/// The intervals are then walked in order of start across the kinds, and
/// each light year inside one is a separate roll of
/// `Random(1000) < (warp - safe - expertise) × chance` (`10b0:5713`). The
/// first hit stops the fleet there, and the field hit is the **deepest**
/// enemy field of that kind at the hit point — the least
/// `distance² − radius²` (`10b0:5cc0`). Returns the index of that field and
/// how far along the leg the hit was; [`apply_damage`] does the rest.
#[must_use]
pub fn traverse(
    minefields: &[Minefield],
    fleet: &Fleet,
    leg: Leg,
    expertise: i32,
    friendly: &dyn Fn(i16) -> bool,
    rng: &mut Rng,
) -> Option<(usize, i32)> {
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

    let enemy = |field: &Minefield| field.owner != fleet.owner && !friendly(field.owner);
    let mut spans: [Vec<(i32, i32)>; MINE_KINDS] = Default::default();
    for field in minefields.iter().filter(|f| enemy(f)) {
        if let Some(span) = crossing(from, to, field, travelled) {
            let kind = usize::from(field.kind).min(MINE_KINDS - 1);
            merge_span(&mut spans[kind], span);
        }
    }
    if spans.iter().all(Vec::is_empty) {
        return None;
    }

    // Walk the kinds' lists together, the earliest start first.
    let mut next = [0usize; MINE_KINDS];
    loop {
        let mut pick: Option<(i32, usize)> = None;
        for (kind, list) in spans.iter().enumerate() {
            if let Some((start, _)) = list.get(next[kind]) {
                if pick.is_none_or(|(best, _)| *start < best) {
                    pick = Some((*start, kind));
                }
            }
        }
        let (start, kind) = pick?;
        let (_, end) = spans[kind][next[kind]];
        next[kind] += 1;
        let over = warp - SAFE_WARP[kind] - expertise;
        if over <= 0 {
            continue;
        }
        let chance = over * HIT_PER_MILLE[kind];
        for step in 0..(end - start).max(0) {
            if i32::from(rng.random(1000)) < chance {
                let at = start + step;
                // Where the fleet is when it goes off, and which field of
                // the kind it is deepest inside.
                let hit = leg_point(from, to, at);
                let field = minefields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| enemy(f) && usize::from(f.kind).min(MINE_KINDS - 1) == kind)
                    .min_by_key(|(_, f)| {
                        let dx = i64::from(f.position.x) - i64::from(hit.x);
                        let dy = i64::from(f.position.y) - i64::from(hit.y);
                        dx * dx + dy * dy - i64::from(f.mines.max(0))
                    })
                    .map(|(index, _)| index)?;
                return Some((field, at));
            }
        }
    }
}

/// Fold one field's crossing into a kind's sorted list of intervals, as
/// `FTravelThroughMineFields` keeps them: eight at most, merged where they
/// overlap or touch.
fn merge_span(list: &mut Vec<(i32, i32)>, (start, end): (i32, i32)) {
    // Past every interval that ends before this one starts.
    let mut at = 0;
    while at < list.len() && list[at].1 < start {
        at += 1;
    }
    if at == list.len() {
        if list.len() < 8 {
            list.push((start, end));
        }
        return;
    }
    if end < list[at].0 - 1 {
        if list.len() < 8 {
            list.insert(at, (start, end));
        }
        return;
    }
    // Overlapping: widen the interval at `at`, and swallow what follows.
    if start < list[at].0 {
        list[at].0 = start;
    }
    if list[at].1 < end {
        list[at].1 = end;
        let mut last = at + 1;
        while last < list.len() && list[last].0 <= end {
            last += 1;
        }
        if end < list[last - 1].1 {
            list[at].1 = list[last - 1].1;
        }
        list.drain(at + 1..last);
    }
}

/// The point `at` light years along a leg (`10b0:5a2e`: `MulDiv` of each
/// difference by the distance over the leg's length).
fn leg_point(from: Point, to: Point, at: i32) -> Point {
    let dx = i64::from(to.x) - i64::from(from.x);
    let dy = i64::from(to.y) - i64::from(from.y);
    let length = ((dx * dx + dy * dy) as f64).sqrt();
    #[allow(clippy::cast_possible_truncation)]
    let scale = |d: i64| -> i16 {
        if length <= 0.0 {
            return 0;
        }
        ((d as f64) * f64::from(at) / length).round() as i16
    };
    Point::new(
        from.x.saturating_add(scale(dx)),
        from.y.saturating_add(scale(dy)),
    )
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

/// Whether a fleet counts as flying on ram scoops for the mine tables.
///
/// `10b0:5613`: any design in the fleet whose engine's `rgcFuelUsed[4]` is
/// zero — an engine that runs free at warp 4. That is every scoop, and also
/// the Settler's Delight, the Fuel Mizer and the Enigma Pulsar.
#[must_use]
pub fn ram_scoop(fleet: &Fleet, designs: &[ShipDesign]) -> bool {
    fleet.stacks.iter().any(|s| {
        s.count > 0
            && designs
                .get(usize::from(s.design))
                .and_then(ShipDesign::engine)
                .is_some_and(|e| e.fuel_used.get(4).copied() == Some(0))
    })
}

/// The Mini Mine Layer and Super Mine Layer hulls, which a field's owner's
/// own detonation spares (`10b0:58f4`, hull ids `0x1b` and `0x1c`).
const MINE_LAYER_HULLS: [i16; 2] = [27, 28];

/// What a hit costs the fleet, and the fleet after it.
///
/// `FTravelThroughMineFields` from `10b0:57f1`. The kind's damage per ship,
/// times the ships — a fleet of four or fewer is topped up to the kind's
/// minimum, which is what makes a lone scout such an expensive way to find
/// a minefield — and every design's share is **multiplied by its engine
/// count** (`10b0:5a85`). Design by design:
///
/// 1. the design's shields, pooled across its ships, absorb what they can;
/// 2. the damage the ships already carried is added to what is left
///    (`armour × damaged ships × damage‰ / 500`, `10b0:5b99`);
/// 3. the total is spread evenly over the ships: if a ship's share exceeds
///    its armour every ship of the design is destroyed, otherwise all of
///    them are marked damaged with that share in 500ths of armour, at
///    least one (`10b0:5c1a`).
///
/// The top-up is paid once, by the first design with ships. In a
/// **detonation** the field's owner's own ships are hit too, all but the
/// mine-layer hulls, and a detonation that does nothing is not a hit at all.
///
/// Damage is dealt to `fleet`'s stacks; cargo and salvage are the caller's.
#[must_use]
pub fn apply_damage(
    fleet: &mut Fleet,
    designs: &[ShipDesign],
    regenerating_shields: bool,
    field: &Minefield,
    travelled: i32,
    detonation: bool,
) -> MineHit {
    let kind = usize::from(field.kind).min(MINE_KINDS - 1);
    let ships: i64 = fleet.stacks.iter().map(|s| i64::from(s.count)).sum();
    let scoop = usize::from(ram_scoop(fleet, designs));
    let per_ship = i64::from(DAMAGE_PER_SHIP[kind][scoop]);
    let mut top_up = i64::from(MIN_DAMAGE[kind][scoop]) - per_ship * ships;
    if ships > 4 || top_up < 1 {
        top_up = 0;
    }

    let mut total = 0i64;
    let mut lost = 0i64;
    for stack in &mut fleet.stacks {
        if stack.count <= 0 {
            continue;
        }
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        if detonation && field.owner == fleet.owner && MINE_LAYER_HULLS.contains(&design.hull_id) {
            continue;
        }
        let count = i64::from(stack.count);
        let engines = design
            .slots
            .iter()
            .find(|s| s.is(crate::components::slot::ENGINE))
            .map_or(1, |s| i64::from(s.count.max(1)));
        let shields = i64::from(design.shields(regenerating_shields)) * count;
        let armour = i64::from(design.stored_armor);

        let mut damage = (per_ship * count + top_up) * engines;
        top_up = 0;
        total += damage;
        let absorbed = damage.min(shields);
        damage -= absorbed;
        // What the ships already carried, back in damage points.
        let damaged_ships = count * i64::from(stack.damaged_pct) / 100;
        damage += armour * i64::from(stack.damage_pct) * damaged_ships / 500;

        let share = damage / count;
        if armour < share {
            lost += count;
            stack.count = 0;
            stack.damaged_pct = 0;
            stack.damage_pct = 0;
        } else {
            stack.damaged_pct = 100;
            let mut per_mille = if armour > 0 { share * 500 / armour } else { 0 };
            if per_mille == 0 {
                per_mille = 1;
            }
            stack.damage_pct = i32::try_from(per_mille & 0x1ff).unwrap_or(0);
        }
    }

    MineHit {
        field: field.id,
        field_owner: field.owner,
        kind: field.kind,
        travelled,
        damage: i32::try_from(total.min(0x7ff8)).unwrap_or(0x7ff8),
        ships_lost: i32::try_from(lost).unwrap_or(i32::MAX),
        fleet_destroyed: ships > 0 && lost == ships,
        detonation,
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
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: 0,
            slots,
        }
    }

    fn fleet(stacks: Vec<ShipStack>) -> Fleet {
        Fleet {
            name: None,
            repeat_orders: false,
            direction: None,
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
        let mut fleet = fleet(vec![stack(0, 10)]);
        let never = |_: i16| false;
        let (index, at) = traverse(
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
        assert_eq!(index, 0);
        assert!(at <= 81);
        let hit = apply_damage(
            &mut fleet,
            &[hull_with(1000)],
            false,
            &fields[index],
            at,
            false,
        );
        assert_eq!(hit.field, 7);
        assert_eq!(hit.kind, 0);
        assert_eq!(hit.damage, 100 * 10, "100 a ship, ten ships");
    }

    /// The per-kind interval list merges what overlaps or touches, keeps
    /// what is apart in order, and holds eight at most.
    #[test]
    fn crossings_of_a_kind_merge_where_they_overlap() {
        let mut list = Vec::new();
        merge_span(&mut list, (10, 20));
        merge_span(&mut list, (40, 50));
        assert_eq!(list, vec![(10, 20), (40, 50)]);
        // Overlapping the first: widened.
        merge_span(&mut list, (15, 30));
        assert_eq!(list, vec![(10, 30), (40, 50)]);
        // Touching the second's start less one: merged, not inserted.
        merge_span(&mut list, (33, 39));
        assert_eq!(list, vec![(10, 30), (33, 50)]);
        // Bridging both: one interval.
        merge_span(&mut list, (25, 45));
        assert_eq!(list, vec![(10, 50)]);
        // Wholly inside: nothing changes.
        merge_span(&mut list, (20, 30));
        assert_eq!(list, vec![(10, 50)]);
        // Ahead of everything: inserted in front.
        merge_span(&mut list, (0, 5));
        assert_eq!(list, vec![(0, 5), (10, 50)]);
        // Eight at most.
        for start in (100..).step_by(10).take(10) {
            merge_span(&mut list, (start, start + 3));
        }
        assert_eq!(list.len(), 8);
    }

    /// Two fields of one kind on top of each other are rolled once — the
    /// same draws hit at the same point as with one field — and the field
    /// hit is the deeper one at the point.
    #[test]
    fn overlapping_fields_of_a_kind_roll_once() {
        let field = |id: u16, x: i16, mines: i32| Minefield {
            id,
            owner: 1,
            position: Point::new(x, 1000),
            mines,
            kind: 0,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        };
        let fleet = fleet(vec![stack(0, 10)]);
        let never = |_: i16| false;
        let leg = Leg {
            from: Point::new(1000, 1000),
            to: Point::new(1081, 1000),
            travelled: 81,
        };
        let one = {
            let mut rng = crate::rng::Rng::from_seeds(3, 4);
            traverse(&[field(7, 1050, 10_000)], &fleet, leg, 0, &never, &mut rng)
        };
        let two = {
            let mut rng = crate::rng::Rng::from_seeds(3, 4);
            traverse(
                &[field(7, 1050, 10_000), field(8, 1060, 10_000)],
                &fleet,
                leg,
                0,
                &never,
                &mut rng,
            )
        };
        let (_, at_one) = one.expect("a hit");
        let (hit, at_two) = two.expect("a hit");
        assert_eq!(at_one, at_two, "one roll a light year, not two");
        // The deeper field at the hit point: distance² less radius².
        let hit_x = 1000 + at_two;
        let depth = |x: i32| (x - hit_x).pow(2) - 10_000;
        let deeper = if depth(1050) <= depth(1060) { 0 } else { 1 };
        assert_eq!(hit, deeper);
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

    /// Two percent a year, four more for every planet inside the field, capped
    /// at fifty — and a Space Demolition player's fields last four times as
    /// long against the same planets.
    #[test]
    fn decay_counts_the_planets_inside() {
        assert_eq!(decay_percent(0, false, false), 2);
        assert_eq!(decay_percent(1, false, false), 6);
        assert_eq!(decay_percent(3, false, false), 14);
        assert_eq!(decay_percent(1, true, false), 3, "Space Demolition");
        assert_eq!(decay_percent(20, false, false), 50, "capped");
        assert_eq!(decay_percent(0, false, true), 27, "armed to detonate");
    }

    /// A field never loses fewer mines than its decay percentage, nor fewer
    /// than ten unless it is a speed bump.
    #[test]
    fn decay_has_floors() {
        let field = |mines: i32, kind: u8| Minefield {
            id: 0,
            owner: 0,
            position: Point::new(0, 0),
            mines,
            kind,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        };
        // 6% of 100 is 6, but the floor of ten applies.
        assert_eq!(decay_amount(&field(100, 0), 1, false), 10);
        // A speed bump has no such floor, so it keeps its 6%.
        assert_eq!(decay_amount(&field(100, 2), 1, false), 6);
        // Big enough and the percentage takes over.
        assert_eq!(decay_amount(&field(10_000, 1), 1, false), 600);
    }

    /// A beam weapon sweeps `range² × damage` mines a year; a sapper sweeps
    /// nothing, and a starbase reaches one square further.
    #[test]
    fn beams_sweep_by_the_square_of_their_range() {
        let beam = |item: u8, count: u8| DesignSlot {
            category: slot::BEAM,
            item,
            count,
        };
        let laser = &crate::components::BEAMS[0];
        assert_eq!(laser.name, "Laser");
        let one = design(vec![beam(0, 1)]);
        let expected =
            i32::from(laser.range_max) * i32::from(laser.range_max) * i32::from(laser.dp);
        assert_eq!(sweep_capacity(&one), expected);
        // Two of them sweep twice as much.
        assert_eq!(sweep_capacity(&design(vec![beam(0, 2)])), expected * 2);
        // And a fleet sweeps per ship.
        let fleet = fleet(vec![stack(0, 3)]);
        assert_eq!(fleet_sweep(&fleet, &[one]), expected * 3);
    }

    /// Sweeping takes what the sweeper can manage, a third of that from a
    /// speed bump, and never leaves a field the sweeper is still inside.
    #[test]
    fn sweeping_shrinks_a_field_past_the_sweeper() {
        let field = |mines: i32, kind: u8| Minefield {
            id: 0,
            owner: 1,
            position: Point::new(0, 0),
            mines,
            kind,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        };
        // Well inside a big field: the sweep is what it is.
        assert_eq!(swept(&field(10_000, 0), 400, 8_100), 400);
        // A speed bump gives up a third of it.
        assert_eq!(swept(&field(10_000, 2), 400, 8_100), 133);
        // Every sweep clears at least two mines.
        assert_eq!(swept(&field(10_000, 2), 1, 8_100), 2);
        // A sweep big enough to wipe the field out instead stops just short:
        // the field is left at the sweeper's distance less one, so the sweeper
        // ends up outside it. 200 mines swept by 400 leaves 99, not nothing.
        assert_eq!(swept(&field(200, 0), 400, 100), 101);
        // Unless the sweeper is at the centre, where there is no room left to
        // stop short and the field goes altogether.
        assert_eq!(swept(&field(200, 0), 400, 0), 200);
    }

    /// A hit costs the field a twentieth of itself, or a hundredth once that
    /// would run past fifty.
    #[test]
    fn a_hit_costs_the_field() {
        assert_eq!(hit_cost(100), 10, "the floor of ten");
        assert_eq!(hit_cost(400), 20);
        assert_eq!(hit_cost(1_000), 50);
        assert_eq!(hit_cost(1_020), 50, "the floor of fifty takes over");
        assert_eq!(hit_cost(10_000), 100);
    }

    /// A hull with one engine, no shields and the armour asked for.
    fn hull_with(armor: u16) -> ShipDesign {
        ShipDesign {
            name: "Scout".to_string(),
            picture: 0,
            stored_armor: armor,
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: 0,
            slots: vec![DesignSlot {
                category: slot::ENGINE,
                item: 1,
                count: 1,
            }],
        }
    }

    fn a_field(kind: u8) -> Minefield {
        Minefield {
            id: 0,
            owner: 1,
            position: Point::new(1000, 1000),
            mines: 100,
            kind,
            detonating: false,
            detected_by: 0,
            visible_to: 0,
            turn: 0,
        }
    }

    /// A fleet of four or fewer takes the minimum instead of the per-ship
    /// figure, which is what makes a lone scout so expensive to lose.
    #[test]
    fn a_small_fleet_takes_the_minimum() {
        let field = a_field(0);
        let designs = [hull_with(1000)];
        let hit = |n: i32| {
            let mut f = fleet(vec![stack(0, n)]);
            apply_damage(&mut f, &designs, false, &field, 0, false)
        };
        // One ship: 100 damage per ship, but at least 500 in total.
        assert_eq!(hit(1).damage, 500);
        // Five ships: 500, the per-ship figure, and no top-up.
        assert_eq!(hit(5).damage, 500);
        // Six: 600.
        assert_eq!(hit(6).damage, 600);
    }

    /// A ship whose armour cannot take its share is destroyed; one that can
    /// is marked damaged by the share in 500ths.
    #[test]
    fn damage_is_shared_over_the_ships_and_kills_or_marks() {
        let field = a_field(0);
        // Ten scouts of 20 armour: 1,000 damage, 100 each, more than 20.
        let mut f = fleet(vec![stack(0, 10)]);
        let hit = apply_damage(&mut f, &[hull_with(20)], false, &field, 0, false);
        assert_eq!(hit.ships_lost, 10);
        assert!(hit.fleet_destroyed);
        assert_eq!(f.stacks[0].count, 0);

        // Ten of 400 armour: 100 each is a quarter — 125 of 500.
        let mut f = fleet(vec![stack(0, 10)]);
        let hit = apply_damage(&mut f, &[hull_with(400)], false, &field, 0, false);
        assert_eq!(hit.ships_lost, 0);
        assert_eq!(f.stacks[0].count, 10);
        assert_eq!(f.stacks[0].damaged_pct, 100);
        assert_eq!(f.stacks[0].damage_pct, 125);

        // Hit again: the 125/500 they carry is 100 each, plus another 100.
        let hit = apply_damage(&mut f, &[hull_with(400)], false, &field, 0, false);
        assert_eq!(hit.damage, 1000);
        assert_eq!(f.stacks[0].damage_pct, 250);
    }

    /// Shields absorb first, pooled over the design's ships, and the engine
    /// count multiplies the blow.
    #[test]
    fn shields_absorb_and_engines_multiply() {
        let field = a_field(0);
        let mut shielded = hull_with(400);
        shielded.slots.push(DesignSlot {
            category: slot::SHIELD,
            item: 0, // Mole-skin, 25 dp
            count: 4,
        });
        // Ten ships, 1,000 shield points between them: all of it absorbed.
        let mut f = fleet(vec![stack(0, 10)]);
        let hit = apply_damage(&mut f, &[shielded.clone()], false, &field, 0, false);
        assert_eq!(hit.damage, 1000);
        assert_eq!(f.stacks[0].damaged_pct, 100, "marked all the same");
        assert_eq!(f.stacks[0].damage_pct, 1, "at least one");

        // Two engines: 2,000 damage, 1,000 through the shields, 100 each.
        shielded.slots[0].count = 2;
        let mut f = fleet(vec![stack(0, 10)]);
        let hit = apply_damage(&mut f, &[shielded], false, &field, 0, false);
        assert_eq!(hit.damage, 2000);
        assert_eq!(f.stacks[0].damage_pct, 125);
    }

    /// A detonation spares the owner's own mine layers and nobody else.
    #[test]
    fn a_detonation_spares_the_owners_mine_layers() {
        let mut field = a_field(0);
        field.owner = 0;
        let mut layer = hull_with(20);
        layer.hull_id = 27;
        let designs = [layer, hull_with(20)];
        let mut f = fleet(vec![stack(0, 1), stack(1, 1)]);
        f.owner = 0;
        let hit = apply_damage(&mut f, &designs, false, &field, 0, true);
        assert_eq!(f.stacks[0].count, 1, "the layer is spared");
        assert_eq!(f.stacks[1].count, 0);
        assert_eq!(hit.ships_lost, 1);
        assert!(!hit.fleet_destroyed);
    }
}
