//! Wormholes and the Mystery Trader — the two wanderers.
//!
//! Both are `THING`s ([`thing.md`](../../docs/formats/thing.md)) that move on
//! their own rather than being ordered anywhere. A **wormhole** sits still for
//! years and then jumps, joining two ends of the galaxy for whoever finds it;
//! the **Mystery Trader** crosses the map at speed, occasionally changing its
//! mind, and hands out technology to a fleet that catches it.
//!
//! See `docs/formulas/wanderers.md`.

use crate::movement::Point;

/// One end of a wormhole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wormhole {
    /// Object id.
    pub id: u16,
    /// Where it is.
    pub position: Point,
    /// How settled it is, `0` (rickety) to `3` (rock solid).
    pub stability: u8,
    /// Years since it last jumped.
    pub years_still: u16,
    /// Whether the far end is known.
    pub dest_known: bool,
    /// Whether the player's view includes it.
    pub include: bool,
    /// Who has seen it.
    pub detected_by: u16,
    /// Who has been through it.
    pub traversed_by: u16,
    /// The `idFull` of the other end.
    pub partner: u16,
    /// The turn stamp the record carries.
    pub turn: u16,
}

/// The Mystery Trader, crossing the galaxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MysteryTrader {
    /// Object id.
    pub id: u16,
    /// Where it is.
    pub position: Point,
    /// Where it is going.
    pub destination: Point,
    /// Its warp; it covers the square of this in a year.
    pub warp: u8,
    /// Whether the player's view includes it.
    pub include: bool,
    /// Who has seen it — and, once they have traded, met it (`grbitPlr`).
    ///
    /// One mask serves for both: `DoThingInteractions` marks a player here the
    /// moment their fleet reaches the Trader, and the same bit is what stops
    /// them trading twice.
    pub detected_by: u16,
    /// Which single technology this Trader carries (`grbitTrader`), one of the
    /// [`part`] bits, or `0` for one that carries nothing in particular and
    /// gives research instead.
    pub part: u16,
    /// The turn stamp the record carries.
    pub turn: u16,
}

impl MysteryTrader {
    /// How far it travels in a year: the square of its warp.
    #[must_use]
    pub fn range(&self) -> i32 {
        i32::from(self.warp) * i32::from(self.warp)
    }
}

/// What became of a Mystery Trader in a year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// It changed course, or speed, or both — the one-in-twenty-five roll.
    ChangedCourse,
    /// It reached its destination and decided to make another pass: a fresh
    /// destination, a slower warp, and a year spent turning round.
    AnotherPass,
    /// It reached its destination and left the galaxy for good.
    Departed,
}

/// What came of one fleet meeting the Mystery Trader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gift {
    /// The fleet was not carrying enough to be worth talking to.
    Refused,
    /// This player has already traded with this Trader.
    AlreadyMet,
    /// The fleet was absorbed and the player given this many technology
    /// levels.
    Tech(i16),
    /// The fleet was absorbed and the player given this [`part`].
    Part(u16),
    /// The fleet was absorbed and there was nothing left to give.
    Nothing,
    /// The fleet was absorbed and the Trader gave ships of its own: one of
    /// its three designs ([`crate::startup::ship::MT_LIFEBOAT`] and the two
    /// after it), and how many.
    Ship {
        /// Which of the built-in templates, as an index into
        /// [`crate::startup::SHIPS`].
        design: usize,
        /// How many ships.
        ships: i32,
    },
    /// The Trader meant to give ships and could not: the player had no design
    /// slot free, or too many fleets already.
    ShipRefused,
}

/// The thirteen things the Mystery Trader has to give (`GrbitTrader`).
///
/// A Trader carries at most one of them, in [`MysteryTrader::part`], and a
/// player's own mask of the ones they have already been given lives in
/// [`crate::Player::trader_parts`]. The Trader never gives the same one twice.
pub mod part {
    /// A cargo pod.
    pub const CARGO: u16 = 0x0001;
    /// A special-purpose device.
    pub const SPECIAL: u16 = 0x0002;
    /// A shield.
    pub const SHIELD: u16 = 0x0004;
    /// Armour.
    pub const ARMOR: u16 = 0x0008;
    /// A mining robot.
    pub const MINER: u16 = 0x0010;
    /// A bomb.
    pub const BOMB: u16 = 0x0020;
    /// A torpedo.
    pub const TORP: u16 = 0x0040;
    /// A beam weapon.
    pub const BEAM: u16 = 0x0080;
    /// A hull.
    pub const HULL: u16 = 0x0100;
    /// An engine.
    pub const ENGINE: u16 = 0x0200;
    /// The Genesis Device.
    pub const GENESIS: u16 = 0x0400;
    /// A jump gate.
    pub const JUMPGATE: u16 = 0x0800;
    /// Not a part at all: the Trader gives a **ship** instead. Reached only
    /// when every part has already been given away.
    pub const LIFEBOAT: u16 = 0x1000;
    /// All thirteen bits, which is what "you have had everything" tests
    /// against.
    pub const ALL: u16 = 0x1FFF;
}

/// What handing over one part amounts to: a message and the item it unlocks.
///
/// `IdmGiveTraderPart` (`1110:1a96`) does two things — it sets the player's bit
/// and it picks the message. The second value is the item word the message
/// carries so the player can be shown *what* they were given: a category byte
/// (`0xC0` engine, `0xC2` shield, `0xC3` armour, `0xC4` beam, `0xC5` torpedo,
/// `0xC6` bomb, `0xC7` mining robot, `0xCB` special, `0xCC` cargo/jump gate,
/// `0xCE` hull, `0xCF` Genesis) and an index within it.
#[must_use]
pub fn part_gift(part: u16) -> (u16, u16) {
    match part {
        part::SPECIAL => (crate::message::id::TRADER_GAVE_PART, 0xCB04),
        part::SHIELD => (crate::message::id::TRADER_GAVE_PART, 0xC206),
        part::ARMOR => (crate::message::id::TRADER_GAVE_PART, 0xC309),
        part::MINER => (crate::message::id::TRADER_GAVE_PART, 0xC706),
        part::BOMB => (crate::message::id::TRADER_GAVE_PART, 0xC608),
        part::TORP => (crate::message::id::TRADER_GAVE_PART, 0xC507),
        part::BEAM => (crate::message::id::TRADER_GAVE_PART, 0xC412),
        part::ENGINE => (crate::message::id::TRADER_GAVE_PART, 0xC008),
        part::JUMPGATE => (crate::message::id::TRADER_GAVE_PART, 0xCC09),
        // A hull and the Genesis Device get their own wording.
        part::HULL => (crate::message::id::TRADER_GAVE_HULL, 0xCE1E),
        part::GENESIS => (crate::message::id::TRADER_GAVE_GENESIS, 0xCF0E),
        // Cargo pods, and anything unrecognised, fall through to the same
        // default the original ends on.
        _ => (crate::message::id::TRADER_GAVE_PART, 0xCC04),
    }
}

/// What a fleet must carry, in kilotons of minerals, to be worth talking to.
pub const TRADE_GOODS: i32 = 5_000;

/// How many technology levels the Trader gives for a load of minerals.
///
/// `DoThingInteractions` (`1110:0e63`): six levels for the five thousand
/// kilotons that buy an audience at all, and one more for every twelve hundred
/// on top, up to ten. Then the ladder takes most of it back off again from
/// anyone who is already advanced — a player with a hundred and eight levels
/// between the six fields gets exactly one, however much they brought.
///
/// `tech_total` is the sum of the player's six levels.
#[must_use]
pub fn tech_levels(cargo: i32, tech_total: i16) -> i16 {
    let mut levels = i16::try_from(((cargo - TRADE_GOODS) / 1_200).clamp(0, 4)).unwrap_or(4) + 6;
    if tech_total >= 108 {
        levels = 1;
    } else if tech_total >= 96 {
        levels = 2;
    } else if tech_total >= 84 {
        levels -= 3;
    } else if tech_total >= 72 {
        levels -= 2;
    } else if tech_total >= 60 {
        levels -= 1;
    }
    levels
}

/// The chance, in per cent, that a wormhole jumps this year.
///
/// `PctWormholeMoves` (`1110:0adc`): a fifth of a percentage point for every
/// year it has sat still, less two minus its stability, and never more than
/// **six**. So a rickety wormhole starts moving after ten quiet years and a
/// rock-solid one is restless from the first, but none of them jumps often.
#[must_use]
pub fn jump_chance(stability: u8, years_still: u16) -> i32 {
    let settled = i32::from(years_still) / 5;
    let penalty = 2 - i32::from(stability.min(3));
    (settled - penalty).clamp(0, 6)
}

/// How far from the edge of the galaxy a wormhole may sit.
const EDGE: i32 = 1000;

/// How badly a position suits a wormhole: `0` is ideal, `15` unusable.
///
/// `IValidateWormholePos` (`1110:064c`) scores a candidate rather than
/// accepting or refusing it, and the mover tries up to a hundred positions,
/// taking the first perfect one or the least bad. Sitting on top of anything —
/// a planet, a fleet, another object — or outside the galaxy is unusable; being
/// merely *near* a planet, the edge, or another wormhole costs a point or more.
///
/// Its own **partner** is kept furthest away of all: the penalty reaches out to
/// seventy light years rather than thirty, which is what stops a wormhole pair
/// collapsing into one corner of the galaxy — the point of a pair being that it
/// joins two distant places.
#[must_use]
pub fn position_score(
    at: Point,
    universe_size: i32,
    partner: u16,
    others: &[(u16, Point)],
    planets: &[Point],
    fleets: &[Point],
) -> u8 {
    let span = universe_size * 400;
    let (x, y) = (i32::from(at.x), i32::from(at.y));
    if x < EDGE || y < EDGE || x > span + 1400 || y > span + 1400 {
        return 15;
    }
    if others.iter().any(|(_, p)| *p == at) || planets.contains(&at) || fleets.contains(&at) {
        return 15;
    }

    let mut score = 0u8;
    // Within ten of the boundary is uncomfortable.
    if x < EDGE + 10 || y < EDGE + 10 || x > span + 1390 || y > span + 1390 {
        score |= 4;
    }
    let distance2 = |p: Point| -> i64 {
        let dx = i64::from(at.x) - i64::from(p.x);
        let dy = i64::from(at.y) - i64::from(p.y);
        dx * dx + dy * dy
    };
    for (id, other) in others {
        let d2 = distance2(*other);
        if *id == partner {
            // Its own far end, kept at arm's length: seventy light years.
            if d2 < 4900 {
                score |= match d2 {
                    d if d < 25 => 8,
                    d if d < 100 => 4,
                    d if d < 900 => 2,
                    _ => 1,
                };
            }
        } else if d2 < 900 {
            score |= match d2 {
                d if d < 16 => 8,
                d if d < 64 => 4,
                d if d < 225 => 2,
                _ => 1,
            };
        }
    }
    for planet in planets {
        let d2 = distance2(*planet);
        if d2 < 784 {
            score |= match d2 {
                d if d < 25 => 8,
                d if d < 100 => 4,
                d if d < 400 => 2,
                _ => 1,
            };
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder takes most of the Trader's generosity back off anyone who is
    /// already advanced.
    #[test]
    fn technology_is_worth_more_to_a_backward_race() {
        // Five thousand kilotons buys the audience and six levels with it;
        // every twelve hundred beyond that buys one more, to ten.
        assert_eq!(tech_levels(5_000, 0), 6);
        assert_eq!(tech_levels(6_200, 0), 7);
        assert_eq!(tech_levels(10_000, 0), 10);
        assert_eq!(tech_levels(100_000, 0), 10);
        // And then the ladder. Sixty levels between the six fields costs one,
        // seventy-two costs two, eighty-four costs three...
        assert_eq!(tech_levels(5_000, 59), 6);
        assert_eq!(tech_levels(5_000, 60), 5);
        assert_eq!(tech_levels(5_000, 72), 4);
        assert_eq!(tech_levels(5_000, 84), 3);
        // ...and beyond that the load of minerals stops mattering at all.
        assert_eq!(tech_levels(100_000, 96), 2);
        assert_eq!(tech_levels(100_000, 108), 1);
    }

    /// Each part carries its own message and names its own item.
    #[test]
    fn a_hull_and_a_genesis_device_are_announced_differently() {
        assert_eq!(
            part_gift(part::BEAM),
            (crate::message::id::TRADER_GAVE_PART, 0xC412)
        );
        assert_eq!(
            part_gift(part::HULL),
            (crate::message::id::TRADER_GAVE_HULL, 0xCE1E)
        );
        assert_eq!(
            part_gift(part::GENESIS),
            (crate::message::id::TRADER_GAVE_GENESIS, 0xCF0E)
        );
        // Cargo pods share the fall-through the original ends on.
        assert_eq!(
            part_gift(part::CARGO),
            (crate::message::id::TRADER_GAVE_PART, 0xCC04)
        );
    }

    /// A wormhole has to sit still for years before it is likely to move, and
    /// even then it is a one-in-sixteen sort of chance at best.
    #[test]
    fn a_wormhole_settles_before_it_jumps() {
        // Rickety: two percentage points of penalty to work off.
        assert_eq!(jump_chance(0, 0), 0);
        assert_eq!(jump_chance(0, 9), 0);
        assert_eq!(jump_chance(0, 10), 0);
        assert_eq!(jump_chance(0, 15), 1);
        // Rock solid: restless from the first year.
        assert_eq!(jump_chance(3, 0), 1);
        assert_eq!(jump_chance(3, 20), 5);
        // And never more than six.
        assert_eq!(jump_chance(3, 1000), 6);
    }

    /// Outside the galaxy, or on top of something, is no place for a wormhole.
    #[test]
    fn a_position_can_be_unusable() {
        let planets = [Point::new(2000, 2000)];
        let size = 1; // a small universe
        assert_eq!(
            position_score(Point::new(10, 10), size, 0, &[], &planets, &[]),
            15,
            "outside"
        );
        assert_eq!(
            position_score(Point::new(2000, 2000), size, 0, &[], &planets, &[]),
            15,
            "on a planet"
        );
    }

    /// Near a planet is merely uncomfortable, and how uncomfortable depends on
    /// how near.
    #[test]
    fn crowding_costs_points() {
        let size = 4;
        let planets = [Point::new(2000, 2000)];
        let near = position_score(Point::new(2003, 2000), size, 0, &[], &planets, &[]);
        let less = position_score(Point::new(2015, 2000), size, 0, &[], &planets, &[]);
        let clear = position_score(Point::new(2100, 2000), size, 0, &[], &planets, &[]);
        assert_eq!(near, 8);
        assert_eq!(less, 2);
        assert_eq!(clear, 0);
    }

    /// The two ends of one wormhole are pushed apart harder than two unrelated
    /// ends are: a pair that sat together would be no use to anybody.
    #[test]
    fn a_pair_is_kept_apart() {
        let size = 4;
        let at = Point::new(2000, 2000);
        // Fifty light years away: beyond a stranger's reach, inside its own
        // partner's.
        let far = Point::new(2050, 2000);
        assert_eq!(position_score(at, size, 7, &[(7, far)], &[], &[]), 1);
        assert_eq!(position_score(at, size, 99, &[(7, far)], &[], &[]), 0);
        // And right on top of its partner is worse than on top of a stranger.
        let close = Point::new(2004, 2000);
        assert_eq!(position_score(at, size, 7, &[(7, close)], &[], &[]), 8);
        assert_eq!(position_score(at, size, 99, &[(7, close)], &[], &[]), 4);
    }

    /// The trader covers the square of its warp.
    #[test]
    fn the_trader_flies_at_the_square_of_its_warp() {
        let trader = MysteryTrader {
            id: 0,
            position: Point::new(1000, 1000),
            destination: Point::new(2000, 1000),
            warp: 9,
            include: true,
            detected_by: 0,
            part: 0,
            turn: 0,
        };
        assert_eq!(trader.range(), 81);
    }
}
