//! Mineral packets: the mass driver's payload, in flight.
//!
//! A packet is a `THING` (see `docs/formats/thing.md`) thrown from one planet's
//! mass driver at another, carrying minerals across the galaxy faster than any
//! ship. It loses part of itself every year to decay, and what happens when it
//! lands depends on whether the receiving planet can **catch** it: a packet
//! arriving faster than the driver that meets it delivers less and does damage.
//!
//! They are the most common object in a real game by a wide margin — 95,798 of
//! them across this repository's fixtures, against 24,193 minefields.
//!
//! See `docs/formulas/packets.md`.

use crate::movement::Point;

/// How the packet's stored warp relates to the real one.
///
/// The `iWarp` field is four bits and packets fly between warp 5 and 13, so the
/// stored value is the warp less four: every routine that uses it says
/// `iWarp + 4` (`10b0:1d10`, `10b0:1f22`).
pub const WARP_BIAS: u8 = 4;

/// A mineral packet in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Object id.
    pub id: u16,
    /// Who threw it.
    pub owner: i16,
    /// Where it is now.
    pub position: Point,
    /// The planet it is aimed at.
    pub target: u16,
    /// Its speed, **as stored**: the real warp is this plus [`WARP_BIAS`].
    pub warp: u8,
    /// Ironium, boranium and germanium aboard, in kilotons.
    pub minerals: [i16; 3],
    /// The game's decay setting for packets: `0` none, `1` a tenth a year,
    /// `2` a quarter, `3` a half.
    pub decay_rate: u8,
    /// Whether it has already moved this year.
    pub moved: bool,
    /// Whether the player's view includes it, carried through untouched.
    pub include: bool,
    /// The turn stamp the record carries.
    pub turn: u16,
}

impl Packet {
    /// The warp it is actually flying at.
    #[must_use]
    pub fn speed(&self) -> i32 {
        i32::from(self.warp) + i32::from(WARP_BIAS)
    }

    /// How far it travels in a full year: the square of its warp.
    #[must_use]
    pub fn range(&self) -> i32 {
        self.speed() * self.speed()
    }

    /// What it is carrying altogether.
    #[must_use]
    pub fn mass(&self) -> i32 {
        self.minerals.iter().map(|m| i32::from(*m)).sum()
    }
}

/// What one year's decay takes out of a packet, per mineral.
///
/// `FPacketDecay` (`10b8:6e9c`). The game's setting picks a rate — 10, 25 or 50
/// per cent — which a **Packet Physics** player halves, and `part` scales it for
/// a move that covered less than a full year. Whatever the arithmetic says, a
/// packet loses at least ten kilotons of each mineral it carries — five for
/// Packet Physics — and never more than it has.
#[must_use]
pub fn decay_loss(amount: i16, decay_rate: u8, packet_physics: bool, part_percent: i32) -> i16 {
    if amount == 0 {
        return 0;
    }
    let rate = match decay_rate {
        1 => 10,
        2 => 25,
        3 => 50,
        _ => return 0,
    };
    let (rate, floor) = if packet_physics {
        (rate / 2, 5)
    } else {
        (rate, 10)
    };
    let loss = i32::from(amount) * rate * part_percent / 10_000;
    let loss = loss.max(floor).min(i32::from(amount));
    i16::try_from(loss).unwrap_or(amount)
}

/// Decay a packet, and say whether it has run out.
///
/// `part_percent` is how much of a year the move covered: 100 for a whole one.
#[must_use]
pub fn decay(packet: &mut Packet, packet_physics: bool, part_percent: i32) -> bool {
    for mineral in &mut packet.minerals {
        *mineral -= decay_loss(*mineral, packet.decay_rate, packet_physics, part_percent);
    }
    packet.mass() == 0
}

/// How much of a packet a planet catches, in parts per thousand.
///
/// `10b0:2052`. A driver at least as fast as the packet catches all of it;
/// otherwise it catches the square of its own warp over the square of the
/// packet's. An **Inner Tech** receiver is worse at it — the original halves the
/// driver's squared warp before the comparison — and a planet with no driver
/// catches nothing.
#[must_use]
pub fn caught_per_mille(packet_warp: i32, driver_warp: i32, receiver_inner_tech: bool) -> i32 {
    let packet2 = packet_warp * packet_warp;
    let mut driver2 = driver_warp * driver_warp;
    if receiver_inner_tech {
        driver2 /= 2;
    }
    if driver2 >= packet2 {
        1000
    } else if driver_warp > 0 {
        driver2 * 1000 / packet2
    } else {
        0
    }
}

/// What the planet keeps of each mineral, in parts per thousand.
///
/// Everything caught, plus a **ninth** of what was not: `10b0:20fd`. So even a
/// planet with no mass driver at all keeps about a ninth of what hits it.
#[must_use]
pub fn kept_per_mille(caught: i32) -> i32 {
    caught + (1000 - caught) / 9
}

/// The damage an uncaught packet does before defences are taken into account.
///
/// `10b0:21e0`: the difference of the squared warps, times the mass, over 160.
#[must_use]
pub fn damage(packet_warp: i32, driver_warp: i32, receiver_inner_tech: bool, mass: i32) -> i32 {
    let packet2 = packet_warp * packet_warp;
    let mut driver2 = driver_warp * driver_warp;
    if receiver_inner_tech {
        driver2 /= 2;
    }
    if driver2 >= packet2 {
        return 0;
    }
    (packet2 - driver2) * mass / 160
}

/// Move a packet toward its target, and say whether it arrived.
///
/// `MoveThings` (`10b0:18f4`). A packet covers the square of its warp in a
/// full year and half that in the year it is launched, which is the second of
/// the two passes the original makes. Arriving is a matter of the remaining
/// distance fitting inside the move.
#[must_use]
pub fn advance(packet: &mut Packet, target: Point, range: i32) -> bool {
    let distance = crate::movement::distance(packet.position, target);
    if distance <= f64::from(range) {
        packet.position = target;
        return true;
    }
    packet.position = crate::movement::advance(packet.position, target, range);
    packet.position == target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(minerals: [i16; 3], decay_rate: u8) -> Packet {
        Packet {
            id: 1,
            owner: 0,
            position: Point::new(1000, 1000),
            target: 7,
            warp: 6, // warp 10
            minerals,
            decay_rate,
            moved: false,
            include: true,
            turn: 0,
        }
    }

    /// The stored warp is four less than the real one, and a packet covers the
    /// square of the real one in a year.
    #[test]
    fn a_packet_flies_at_the_square_of_its_warp() {
        let p = packet([100, 0, 0], 1);
        assert_eq!(p.speed(), 10);
        assert_eq!(p.range(), 100);
    }

    /// The three decay settings are a tenth, a quarter and a half a year, and
    /// Packet Physics halves whichever is in force.
    #[test]
    fn decay_follows_the_games_setting() {
        assert_eq!(decay_loss(1000, 0, false, 100), 0, "no decay set");
        assert_eq!(decay_loss(1000, 1, false, 100), 100);
        assert_eq!(decay_loss(1000, 2, false, 100), 250);
        assert_eq!(decay_loss(1000, 3, false, 100), 500);
        assert_eq!(decay_loss(1000, 2, true, 100), 120, "Packet Physics: 12.5%");
        // Half a year's travel is half the decay.
        assert_eq!(decay_loss(1000, 2, false, 50), 125);
    }

    /// However small the packet, it loses ten kilotons of each mineral a year —
    /// five for Packet Physics — but never more than it has.
    #[test]
    fn decay_has_a_floor_and_a_ceiling() {
        assert_eq!(
            decay_loss(50, 1, false, 100),
            10,
            "5 rounds up to the floor"
        );
        assert_eq!(decay_loss(50, 1, true, 100), 5);
        assert_eq!(decay_loss(3, 1, false, 100), 3, "never more than it has");

        let mut p = packet([8, 0, 0], 1);
        assert!(decay(&mut p, false, 100), "and then it is gone");
        assert_eq!(p.minerals, [0, 0, 0]);
    }

    /// A driver as fast as the packet catches all of it; a slower one catches
    /// the ratio of the squares; and what is not caught still leaves a ninth
    /// behind.
    #[test]
    fn catching_is_the_ratio_of_the_squares() {
        assert_eq!(caught_per_mille(10, 10, false), 1000);
        assert_eq!(
            caught_per_mille(10, 13, false),
            1000,
            "faster still catches"
        );
        assert_eq!(caught_per_mille(10, 5, false), 250, "25 / 100");
        assert_eq!(caught_per_mille(10, 0, false), 0, "no driver, no catch");
        // Inner Tech is worse at receiving: the driver counts as half.
        assert_eq!(caught_per_mille(10, 10, true), 500);

        assert_eq!(kept_per_mille(1000), 1000);
        assert_eq!(kept_per_mille(0), 111, "a ninth of it lands anyway");
        assert_eq!(kept_per_mille(250), 333);
    }

    /// Damage is the excess speed times the mass, and none at all if the
    /// packet was caught.
    #[test]
    fn damage_is_the_excess_speed_times_the_mass() {
        assert_eq!(damage(10, 10, false, 1000), 0, "caught");
        assert_eq!(damage(10, 5, false, 1000), (100 - 25) * 1000 / 160);
        assert_eq!(damage(10, 0, false, 1600), 1000);
    }

    /// A move that covers the remaining distance arrives; one that does not
    /// leaves the packet on the line.
    #[test]
    fn a_packet_arrives_when_the_move_reaches() {
        let mut p = packet([100, 0, 0], 0);
        let target = Point::new(1000, 1050);
        assert!(advance(&mut p, target, 100), "50 light years inside 100");
        assert_eq!(p.position, target);

        let mut p = packet([100, 0, 0], 0);
        let target = Point::new(1000, 1300);
        assert!(!advance(&mut p, target, 100));
        assert_eq!(p.position, Point::new(1000, 1100));
    }
}
