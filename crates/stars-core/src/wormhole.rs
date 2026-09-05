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
    /// Who has seen it.
    pub detected_by: u16,
    /// Who has traded with it.
    pub met_by: u16,
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
            met_by: 0,
            turn: 0,
        };
        assert_eq!(trader.range(), 81);
    }
}
