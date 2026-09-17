//! Stargate jumps.
//!
//! A leg ordered at [`WARP`] ("Use Stargate") is not flown: the fleet is
//! put through the gate on its starbase and comes out of the gate at the
//! other end the same year, however far that is, burning nothing. What it
//! risks instead is the gates' **limits** — each gate is rated for a mass
//! per ship and a range, and a jump past either is a gamble on every ship.
//!
//! Sources: the stargate branch of `MoveFleets` (`10b0:354f`–`3d0d`),
//! `FStargateJump` (`1080:0cfe`), `MdCalcStargateDamage` (`1080:152e`),
//! `IStargateFromLppl` (`1038:…`, `util.c`) and `FFleetCanJumpgate`
//! (`util.c`). The player's guide's *Stargate Navigation* topic says the
//! same in its own words: the range is the **source** gate's, the mass
//! limit is the tighter of the two, and either may be exceeded up to
//! fivefold "and possibly still arrive", always with damage.
//!
//! See `docs/formulas/stargates.md`.

use crate::components::{slot, SPECIALS_SB};
use crate::design::ShipDesign;
use crate::fleet::Fleet;
use crate::planet::Planet;
use crate::rng::Rng;

/// The waypoint warp that means "use the stargate" (`iWarpStargate`, 11).
pub const WARP: u8 = 11;

/// How many entries at the front of [`SPECIALS_SB`] are stargates; the rest
/// are mass drivers (`IStargateFromLppl` takes `iItem < 7`).
pub const GATES: usize = 7;

/// The Jump Gate's index in [`crate::components::SPECIALS_M`]
/// (`ispecialMJumpGate`, 9): a ship carrying one needs no gate to leave
/// from.
pub const JUMP_GATE: u8 = 9;

/// Each stargate's safe range in light years, `-1` for unlimited —
/// `SPECIALSB.grAbility2`, the word at `+0x36` of each `rgspecialSB` entry
/// (`1008:4d8a`), which [`SPECIALS_SB`] does not carry because the other
/// specials have no second rating. The mass limit is the entry's
/// `ability`.
pub const RANGE: [i16; GATES] = [250, 300, 600, 500, -1, 800, -1];

/// The range stood in for an unlimited gate (`MdCalcStargateDamage`, the
/// `10000` a `-1` rating becomes).
const UNLIMITED_RANGE: i32 = 10_000;

/// The index into [`SPECIALS_SB`] of the stargate on a planet's starbase,
/// if it has one (`IStargateFromLppl`): the first orbital slot holding a
/// gate.
#[must_use]
pub fn gate_of(planet: &Planet, designs: &[ShipDesign]) -> Option<usize> {
    if !planet.starbase || planet.owner.is_none() {
        return None;
    }
    let base = planet
        .starbase_design
        .map(usize::from)
        .map(|s| usize::from(crate::startup::FIRST_STARBASE_SLOT) + s)
        .and_then(|s| designs.get(s))?;
    base.slots
        .iter()
        .find(|s| s.count != 0 && s.category == slot::SPECIAL_SB && usize::from(s.item) < GATES)
        .map(|s| usize::from(s.item))
}

/// Whether every design in the fleet carries a Jump Gate
/// (`FFleetCanJumpgate`), so it can jump from open space.
#[must_use]
pub fn can_jumpgate(fleet: &Fleet, designs: &[ShipDesign]) -> bool {
    fleet.stacks.iter().filter(|s| s.count > 0).all(|s| {
        designs.get(usize::from(s.design)).is_some_and(|d| {
            d.slots
                .iter()
                .any(|h| h.count != 0 && h.category == slot::SPECIAL_M && h.item == JUMP_GATE)
        })
    })
}

/// Whether the client should offer "Use Stargate" for a leg from `src` to
/// `dst` — the `1` answer of `FCanFleetUseStargates` (`1038:75e2`) that
/// `IWarpBestForWaypoint` (`1058:7a18`) takes: both planets the fleet's
/// owner's own with a gate on their starbase (a friend's gate is an
/// *uncertain* answer there, and is not offered), nothing in the hold
/// unless the owner is an Interstellar Traveler, and every design's jump
/// judged undamaged.
#[must_use]
pub fn is_safe_leg(
    fleet: &Fleet,
    designs: &[ShipDesign],
    interstellar_traveler: bool,
    src: &Planet,
    dst: &Planet,
) -> bool {
    if src.owner != Some(fleet.owner) || dst.owner != Some(fleet.owner) {
        return false;
    }
    let (Some(from), Some(to)) = (gate_of(src, designs), gate_of(dst, designs)) else {
        return false;
    };
    if !interstellar_traveler
        && (fleet.cargo.minerals.iter().any(|m| *m != 0) || fleet.cargo.colonists != 0)
    {
        return false;
    }
    let (Some(a), Some(b)) = (src.position, dst.position) else {
        return false;
    };
    #[allow(clippy::cast_possible_truncation)]
    let span = crate::movement::distance(a, b) as i32;
    fleet.stacks.iter().filter(|s| s.count != 0).all(|s| {
        let mass = designs
            .get(usize::from(s.design))
            .and_then(ShipDesign::mass)
            .unwrap_or(0);
        verdict(from, to, span, mass) == Verdict::Damage(0)
    })
}

/// What a jump does to ships of one mass (`MdCalcStargateDamage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Beyond five times the source gate's range: the jump is refused.
    TooFar,
    /// Beyond five times either gate's mass limit: the jump is refused.
    TooMassive,
    /// Within the limits, or past them by less than fivefold: the ships go
    /// through with this much damage, `0` for a safe jump.
    Damage(i32),
    /// The odds came out at nothing: every ship of the design is lost.
    Lost,
}

/// The damage a jump inflicts on a ship of `mass` kT sent `distance` light
/// years from gate `src` to gate `dst` (both indices into [`SPECIALS_SB`]).
///
/// Survival starts at 100% and is cut for each limit exceeded, by
/// `(limit × 5 − actual) × 25% ÷ limit` — linear from whole at the limit
/// to nothing at five times it — and the cuts multiply. The damage is the
/// rest, in whole percent.
#[must_use]
pub fn verdict(src: usize, dst: usize, distance: i32, mass: i32) -> Verdict {
    let range = match RANGE.get(src).copied().unwrap_or(-1) {
        -1 => UNLIMITED_RANGE,
        r => i32::from(r),
    };
    if distance > range * 5 {
        return Verdict::TooFar;
    }
    let limit = |gate: usize| i32::from(SPECIALS_SB.get(gate).map_or(-1, |s| s.ability));
    let (src_limit, dst_limit) = (limit(src), limit(dst));
    if (src_limit > 0 && mass > src_limit * 5) || (dst_limit > 0 && mass > dst_limit * 5) {
        return Verdict::TooMassive;
    }
    let mut survive: i64 = 10_000;
    if distance > range {
        survive = (i64::from(range) * 5 - i64::from(distance)) * 2500 / i64::from(range);
        if survive < 1 {
            return Verdict::Lost;
        }
    }
    for limit in [src_limit, dst_limit] {
        if limit > 0 && limit < mass {
            let part = (i64::from(limit) * 5 - i64::from(mass)) * 2500 / i64::from(limit);
            if part < 1 {
                return Verdict::Lost;
            }
            survive = survive * part / 10_000;
        }
    }
    #[allow(clippy::cast_possible_truncation)]
    Verdict::Damage(((10_000 - survive) / 100) as i32)
}

/// Why a jump did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A design too far for the source gate (`idm` `0xe3`).
    TooFar,
    /// A design too massive for a gate (`idm` `0xe4`), with its slot.
    TooMassive(u8),
    /// No design could survive the jump: the fleet is gone (`idm` `0xe7`).
    Annihilated,
}

/// What became of a fleet that jumped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jump {
    /// Ships lost on the way.
    pub lost: i32,
    /// Ships that set out.
    pub sent: i32,
    /// Whether any design came through damaged — the fleet does not heal
    /// this year (`fNoHeal`).
    pub damaged: bool,
    /// The ships that did not arrive, by design slot, for balancing the
    /// cargo.
    pub dead: Vec<crate::fleet::ShipStack>,
}

/// Put a fleet through a pair of gates (`FStargateJump`), taking the
/// losses and the damage out of its stacks.
///
/// Each design is judged by [`verdict`] on its **empty** mass — cargo does
/// not count, which is the Interstellar Traveler's advantage since every
/// other race has to unload first. A refused design refuses the whole
/// fleet. Past the limits, every ship rolls against a third of the damage
/// figure to be lost outright — not for an Interstellar Traveler, whose
/// ships only take the damage — and the survivors carry the damage as
/// armour lost, added to whatever they carried in: a ship already so
/// damaged that the new damage would finish it is lost too, and the
/// stack's damage is then the average over its ships, all of them counted
/// as damaged.
///
/// # Errors
///
/// [`Refused`] when the jump does not happen; the fleet is untouched then,
/// except that an annihilated one has lost every ship.
pub fn jump(
    fleet: &mut Fleet,
    designs: &[ShipDesign],
    interstellar_traveler: bool,
    src: usize,
    dst: usize,
    distance: i32,
    rng: &mut Rng,
) -> Result<Jump, Refused> {
    let mut damage = vec![0i32; fleet.stacks.len()];
    let mut eligible = 0usize;
    let mut sent = 0i32;
    let mut damaged = false;
    for (at, stack) in fleet.stacks.iter().enumerate() {
        if stack.count <= 0 {
            continue;
        }
        sent += stack.count;
        let mass = designs
            .get(usize::from(stack.design))
            .and_then(ShipDesign::mass)
            .unwrap_or(0);
        match verdict(src, dst, distance, mass) {
            Verdict::TooFar => return Err(Refused::TooFar),
            Verdict::TooMassive => return Err(Refused::TooMassive(stack.design)),
            Verdict::Lost => damage[at] = 100,
            Verdict::Damage(pct) => {
                eligible += 1;
                damage[at] = pct;
                damaged = true;
            }
        }
    }
    if eligible == 0 {
        for stack in &mut fleet.stacks {
            stack.count = 0;
        }
        return Err(Refused::Annihilated);
    }

    let before = fleet.stacks.clone();
    let mut lost = 0i32;
    for (at, stack) in fleet.stacks.iter_mut().enumerate() {
        if stack.count <= 0 || damage[at] == 0 {
            continue;
        }
        if damage[at] == 100 {
            lost += stack.count;
            stack.count = 0;
            stack.damaged_pct = 0;
            stack.damage_pct = 0;
            continue;
        }
        let armour = designs
            .get(usize::from(stack.design))
            .and_then(|d| d.armor(false))
            .unwrap_or(0);
        let kill_chance = if interstellar_traveler {
            0
        } else {
            damage[at] / 3
        };
        // How many of the stack were damaged coming in: `pctSh` of the
        // count, at least one when any was.
        let mut damaged_before = if stack.damaged_pct == 0 {
            0
        } else {
            (stack.count * stack.damaged_pct / 100).max(1)
        };
        let mut left = stack.count;
        if kill_chance != 0 {
            for _ in 0..stack.count {
                if i32::from(rng.random(100)) < kill_chance {
                    left -= 1;
                    // A damaged ship is as likely to be among the lost as
                    // its damage makes it.
                    if damaged_before != 0 && i32::from(rng.random(500)) < stack.damage_pct {
                        damaged_before -= 1;
                    }
                }
            }
            lost += stack.count - left;
        }
        if left != 0 {
            let old_per_ship = if stack.damage_pct == 0 {
                0
            } else {
                (armour * stack.damage_pct / 500).max(1)
            };
            let new_per_ship = (armour * damage[at] / 100).max(1);
            if damaged_before != 0 && armour <= new_per_ship + old_per_ship {
                lost += damaged_before;
                left -= damaged_before;
            }
            if left != 0 {
                let total = i64::from(old_per_ship) * i64::from(damaged_before)
                    + i64::from(new_per_ship) * i64::from(left);
                let pct = if armour == 0 {
                    1
                } else {
                    (total / i64::from(left) * 500 / i64::from(armour)).max(1)
                };
                #[allow(clippy::cast_possible_truncation)]
                {
                    stack.damage_pct = pct.min(511) as i32;
                }
                stack.damaged_pct = 100;
            }
        }
        stack.count = left.max(0);
        if stack.count == 0 {
            stack.damaged_pct = 0;
            stack.damage_pct = 0;
        }
    }
    if fleet.stacks.iter().all(|s| s.count <= 0) {
        return Err(Refused::Annihilated);
    }
    let dead = before
        .iter()
        .zip(&fleet.stacks)
        .filter(|(was, now)| was.count > now.count)
        .map(|(was, now)| crate::fleet::ShipStack {
            design: was.design,
            count: was.count - now.count,
            damaged_pct: 0,
            damage_pct: 0,
        })
        .collect();
    Ok(Jump {
        lost,
        sent,
        damaged,
        dead,
    })
}

/// Which of the three "lost ships" messages a jump earns: `0xe8` for fewer
/// than a quarter of the ships, `0xe9` up to half, `0xea` past that
/// (`FStargateJump`, `1080:139a`).
#[must_use]
pub fn loss_message(sent: i32, lost: i32) -> u16 {
    if lost < sent >> 2 {
        crate::message::id::STARGATE_LOST_FEW
    } else if lost <= sent >> 1 {
        crate::message::id::STARGATE_LOST_SOME
    } else {
        crate::message::id::STARGATE_LOST_MANY
    }
}
