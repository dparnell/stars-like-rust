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
/// `10b0:21e0`: the packet's squared warp **less the driver's warp** — not
/// its square — times the mass, over 160. `MANUAL.PDF` p. 25-2 writes the
/// formula as `(spdPacket − spdReceiver) × wtPacket / 160` with both sides
/// squared, but the binary loads the plain warp (`[BP-0x42]`, the value
/// `IWarpMAFromLppl` returned plus one for a pair, at `10b0:21f4`) where
/// the squared, Inner-Tech-halved figure sits in `[BP-0x40]` beside it.
/// The squared figure decides only whether the packet was caught at all.
///
/// Nothing is done when the packet was caught in full, which is the only
/// case in which the routine does not reach this arithmetic.
#[must_use]
pub fn damage(packet_warp: i32, driver_warp: i32, receiver_inner_tech: bool, mass: i32) -> i32 {
    if caught_per_mille(packet_warp, driver_warp, receiver_inner_tech) == 1000 {
        return 0;
    }
    (packet_warp * packet_warp - driver_warp) * mass / 160
}

/// A packet thrown, as the turn reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flung {
    /// The planet that threw it.
    pub planet: i16,
    /// The packet it went into — new, or one already on the pad this year.
    pub packet: u16,
    /// Whether it joined a packet already there.
    pub joined: bool,
    /// What went up, in kT.
    pub minerals: [i16; 3],
    /// The warp it flies at.
    pub warp: u8,
}

/// Throw the packets a planet has just built — `FBuildObject`
/// (`10b8:19b2`), the mineral-packet arm from `10b8:2496`.
///
/// `item` is the queue item (14 to 17, or the auto packet, which is thrown
/// as a mixed one) and `count` how many were finished. Each is 100 kT of
/// its mineral — 70 for a Packet Physics race — or 40 of each of the three
/// (25 for Packet Physics), capped at 32,760 in all. The planet needs a
/// mass driver (else message `0xd1`) and a destination set (`0xd2`). The
/// warp is the planet's setting, or the driver's own — a pair counting one
/// faster — when the setting is under 5 or more than three over it; the
/// packet decays by how far over the driver it was thrown, an Interstellar
/// Traveller's as if one further. A packet of the planet's own, on the pad,
/// at the same warp and target and decay and under 1,630 kT takes the load
/// (`0xd4`); otherwise a new one is made (`0xd3`), or none when the game
/// has no room for another (`0x129`).
pub fn fling(
    state: &mut crate::GameState,
    planet_index: usize,
    item: u16,
    count: i32,
) -> Option<Flung> {
    use crate::message::{id, Message};
    use crate::production::item;
    use crate::race::Prt;

    let planet = &state.planets[planet_index];
    let planet_id = planet.id;
    let owner = usize::try_from(planet.owner?).ok()?;
    let prt = state.players.get(owner)?.race.prt();
    let designs = state.designs.get(owner)?;
    let driver = crate::production::mass_driver(planet, designs);
    let position = planet.position?;
    let mut tell = |id: u16, extra: Option<i16>| {
        let mut params = vec![planet_id];
        params.extend(extra);
        state.messages.push(Message {
            player: owner,
            id,
            object: planet_id,
            params,
        });
    };
    if driver.warp == 0 {
        tell(id::PACKET_NO_DRIVER, None);
        return None;
    }
    let planet = &state.planets[planet_index];
    let Some(target) = planet.fling_dest else {
        tell(id::PACKET_NO_DESTINATION, None);
        return None;
    };

    // What goes up.
    let item = if item == item::AUTO_PACKET {
        item::PACKET_MIXED
    } else {
        item
    };
    let per_unit = match (item == item::PACKET_MIXED, prt == Some(Prt::Pp)) {
        (true, true) => 25,
        (true, false) => 40,
        (false, true) => 70,
        (false, false) => 100,
    };
    let mut minerals = [0i16; 3];
    for (kind, slot) in minerals.iter_mut().enumerate() {
        let which = usize::from(item - item::PACKET_IRONIUM);
        if item == item::PACKET_MIXED || kind == which {
            let load = i64::from(per_unit) * i64::from(count);
            *slot = i16::try_from(load.min(0x7ff8)).unwrap_or(0x7ff8);
        }
    }

    // How fast, and how far over the driver.
    let driver_warp = driver.warp + i32::from(driver.paired);
    let mut warp = i32::from(planet.fling_warp) + 4;
    if warp < 5 || warp > driver.warp + 3 {
        warp = driver_warp;
    }
    let mut over = (warp - driver_warp).max(0);
    if prt == Some(Prt::It) && over < 3 {
        over += 1;
    }
    let stored_warp = u8::try_from(warp - 4).unwrap_or(0);
    let decay_rate = u8::try_from(over).unwrap_or(3);
    let owner_word = i16::try_from(owner).unwrap_or(0);

    // Onto a packet already on the pad, or a new one.
    let joined = state.packets.iter().position(|p| {
        p.owner == owner_word
            && p.position == position
            && p.warp == stored_warp
            && p.target == u16::try_from(target).unwrap_or(0)
            && p.decay_rate == decay_rate
            && p.mass() < 0x65e
    });
    let (packet_id, joined) = match joined {
        Some(index) => {
            let packet = &mut state.packets[index];
            for (have, more) in packet.minerals.iter_mut().zip(minerals) {
                *have = have.saturating_add(more);
            }
            (packet.id, true)
        }
        None => {
            if state.packets.len() >= 512 {
                tell(id::NO_ROOM_FOR_THING, None);
                return None;
            }
            let packet_id = state
                .packets
                .iter()
                .map(|p| p.id)
                .max()
                .map_or(0, |m| m + 1);
            state.packets.push(Packet {
                id: packet_id,
                owner: owner_word,
                position,
                target: u16::try_from(target).unwrap_or(0),
                warp: stored_warp,
                minerals,
                decay_rate,
                moved: false,
                include: true,
                turn: u16::try_from(state.turn).unwrap_or(0),
            });
            (packet_id, false)
        }
    };
    tell(
        if joined {
            id::PACKET_ADDED_TO
        } else {
            id::PACKET_FLUNG
        },
        Some(target),
    );
    Some(Flung {
        planet: planet_id,
        packet: packet_id,
        joined,
        minerals,
        warp: stored_warp + WARP_BIAS,
    })
}

/// What a packet did when it landed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Landing {
    /// The planet it hit.
    pub planet: i16,
    /// Who threw it.
    pub thrower: i16,
    /// How much of it the planet's driver caught, in parts per thousand.
    pub caught: i32,
    /// Minerals that reached the surface, in kT.
    pub delivered: [i32; 3],
    /// The damage that got past the defences.
    pub damage: i32,
    /// Colonists killed, in hundreds.
    pub killed: i32,
    /// Defences destroyed.
    pub defences_destroyed: i32,
    /// Whether the planet's people were wiped out.
    pub depopulated: bool,
    /// Clicks a Packet Physics thrower's packet moved each variable by.
    pub terraformed: [i32; 3],
    /// Clicks the same moved each variable's *original* value by.
    pub terraformed_orig: [i32; 3],
}

/// Land a packet on its target planet — `MoveThings` from `10b0:1f03`.
///
/// The receiving planet catches what its own mass driver can, keeps that
/// plus a ninth of the rest, and takes damage for whatever came in too fast
/// ([`caught_per_mille`], [`kept_per_mille`], [`damage`]). A pair of
/// drivers counts as one warp faster. The damage that gets past the
/// defences (`CalcPctSurvive`, the same share a bombing run meets) kills
/// `max(pop × damage / 1000, damage)` hundreds of colonists — a planet
/// left with none is uninhabited — and destroys `defences × damage /
/// 1000` defences, at least one with `Random(20) < damage` when that came
/// to nothing, at least `damage / 20`, and never more than there are. An
/// Alternate Reality owner takes no damage at all.
///
/// A **Packet Physics** thrower's packet terraforms as it lands
/// (`10b0:220c`): for each mineral, every hundred kilotons of the part
/// *not* caught (the last, smaller lot with its own smaller chance) has a
/// `Random(200) < kT` chance of moving the matching variable — ironium
/// gravity, boranium temperature, germanium radiation — one click toward
/// the thrower's ideal, and each such click a further one in ten chance of
/// moving the *original* value with it, both within the thrower's
/// terraforming reach. A thrower immune to the variable pushes it away
/// from the middle instead, half as far.
///
/// Not modelled: the thrower, when Packet Physics and the planet has a
/// driver, also learns the target's starbase design (`10b0:1f7f`), which
/// this state has no place for.
///
/// The messages are the original's ids with wordings of this project's:
/// `0xd5`/`0x146` caught or harmless, `0xd6`–`0xd9` damage with or
/// without a driver and with or without defences lost, `0xda` a planet
/// wiped out, `0x181` a planet with nobody on it, and `0x131`–`0x134` the
/// terraforming, to the thrower.
pub fn land(
    state: &mut crate::GameState,
    packet: &Packet,
    rng: &mut crate::rng::Rng,
) -> Option<Landing> {
    use crate::message::{id, Message};
    use crate::race::Prt;

    let target = i16::try_from(packet.target).ok()?;
    let index = state.planets.iter().position(|p| p.id == target)?;
    let owner = state.planets[index]
        .owner
        .and_then(|o| usize::try_from(o).ok())
        .filter(|o| *o < state.players.len());
    let thrower = usize::try_from(packet.owner)
        .ok()
        .filter(|t| *t < state.players.len());
    let thrower_pp = thrower.is_some_and(|t| state.players[t].race.prt() == Some(Prt::Pp));

    // The driver, a pair counting as one warp faster.
    let driver = owner.map_or(MassDriverWarp::default(), |o| {
        let d = crate::production::mass_driver(&state.planets[index], &state.designs[o]);
        MassDriverWarp {
            warp: d.warp + i32::from(d.paired),
        }
    });
    let inner_tech = owner.is_some_and(|o| state.players[o].race.prt() == Some(Prt::It));
    let packet_warp = packet.speed();
    let caught = caught_per_mille(packet_warp, driver.warp, inner_tech);
    let kept = kept_per_mille(caught);

    // What the surface gets, and what was not caught — the latter is what a
    // Packet Physics thrower terraforms with.
    let mut delivered = [0i32; 3];
    let mut uncaught = [0i32; 3];
    let mut mass = 0i32;
    for (kind, amount) in packet.minerals.iter().enumerate() {
        let amount = i32::from(*amount).max(0);
        uncaught[kind] = amount * (1000 - caught) / 1000;
        mass += amount;
        delivered[kind] = amount * kept / 1000;
        state.planets[index].surface_min[kind] += delivered[kind];
    }
    let planet_id = state.planets[index].id;
    let mut out = Landing {
        planet: planet_id,
        thrower: packet.owner,
        caught,
        delivered,
        ..Landing::default()
    };
    let mass_lo = i16::from_le_bytes((mass as u16).to_le_bytes());
    let mass_hi = i16::try_from(mass >> 16).unwrap_or(0);
    let harmless = |state: &mut crate::GameState| {
        if let Some(o) = owner {
            state.messages.push(Message {
                player: o,
                id: if driver.warp > 0 {
                    id::PACKET_CAUGHT
                } else {
                    id::PACKET_HARMLESS
                },
                object: planet_id,
                params: vec![planet_id, packet.owner, mass_lo, mass_hi],
            });
        }
    };
    if caught == 1000 {
        harmless(state);
        return Some(out);
    }

    let raw = damage(packet_warp, driver.warp, inner_tech, mass);

    if thrower_pp {
        if let Some(t) = thrower {
            let (moved, moved_orig) = packet_terraform(state, index, t, &uncaught, rng);
            out.terraformed = moved;
            out.terraformed_orig = moved_orig;
        }
    }

    let Some(o) = owner else {
        harmless(state);
        return Some(out);
    };
    let race = state.players[o].race.clone();
    let tech = state.players[o].research.levels;
    let (survive, _) = crate::bombing::pct_survive(&state.planets[index], &race, tech);
    #[allow(clippy::cast_possible_truncation)]
    let dmg = (f64::from(raw) * survive) as i32;
    if dmg == 0 || race.is_ar() {
        harmless(state);
        return Some(out);
    }
    out.damage = dmg;

    let planet = &mut state.planets[index];
    let defences = i32::from(planet.defenses);
    if planet.pop == 0 {
        // Nobody to kill: every defence goes.
        state.messages.push(Message {
            player: o,
            id: id::PACKET_HIT_EMPTY,
            object: planet_id,
            params: vec![planet_id, packet.owner],
        });
        out.defences_destroyed = defences;
        planet.defenses = 0;
        return Some(out);
    }
    let killed = (i64::from(planet.pop) * i64::from(dmg) / 1000).max(i64::from(dmg));
    if planet.pop > 0 && i64::from(planet.pop) <= killed {
        state.messages.push(Message {
            player: o,
            id: id::PACKET_WIPED_OUT,
            object: planet_id,
            params: vec![planet_id, packet.owner],
        });
        out.killed = planet.pop;
        out.depopulated = true;
        let ca = race.prt() == Some(Prt::Ca);
        crate::bombing::uninhabit(planet, ca);
        return Some(out);
    }
    let killed = i32::try_from(killed).unwrap_or(i32::MAX);
    let mut lost = defences * dmg / 1000;
    if lost == 0 && defences != 0 && i32::from(rng.random(20)) < dmg {
        lost = 1;
    }
    lost = lost.max(dmg / 20).min(defences);
    let id = match (driver.warp > 0, lost > 0) {
        (true, false) => id::PACKET_DAMAGE,
        (true, true) => id::PACKET_DAMAGE_DEFENCES,
        (false, false) => id::PACKET_DAMAGE_NO_DRIVER,
        (false, true) => id::PACKET_DAMAGE_DEFENCES_NO_DRIVER,
    };
    let mut params = vec![planet_id, mass_lo, mass_hi, packet.owner, n_i16(killed)];
    if lost > 0 {
        params.push(n_i16(lost));
        planet.defenses -= i16::try_from(lost).unwrap_or(0);
    }
    state.messages.push(Message {
        player: o,
        id,
        object: planet_id,
        params,
    });
    planet.pop -= killed;
    out.killed = killed;
    out.defences_destroyed = lost;
    Some(out)
}

/// The warp a landing is caught at.
#[derive(Debug, Clone, Copy, Default)]
struct MassDriverWarp {
    warp: i32,
}

/// A count as a message parameter word.
fn n_i16(n: i32) -> i16 {
    i16::try_from(n).unwrap_or(i16::MAX)
}

/// The Packet Physics terraforming a landing does (`10b0:220c`); see
/// [`land`]. Returns the clicks each variable moved and the clicks its
/// original moved.
fn packet_terraform(
    state: &mut crate::GameState,
    index: usize,
    thrower: usize,
    uncaught: &[i32; 3],
    rng: &mut crate::rng::Rng,
) -> ([i32; 3], [i32; 3]) {
    use crate::message::{id, Message};
    use crate::terraform::{terraform_targets, Intent, ENV_MAX, ENV_MIN};

    let race = state.players[thrower].race.clone();
    let tech = state.players[thrower].research.levels;
    let planet_id = state.planets[index].id;
    let owner = state.planets[index].owner;
    let theirs = owner.is_some_and(|o| usize::try_from(o).ok() != Some(thrower));
    let mut moved = [0i32; 3];
    let mut moved_orig = [0i32; 3];
    for v in 0..3 {
        // The rolls: one per hundred kilotons uncaught, the last lot at its
        // own size.
        let mut left = uncaught[v];
        let mut clicks = 0i32;
        let mut orig_clicks = 0i32;
        while left > 0 {
            let chance = left.min(100);
            if i32::from(rng.random(200)) < chance {
                clicks += 1;
                if rng.random(10) == 0 {
                    orig_clicks += 1;
                }
            }
            left -= 100;
        }

        // The original value, toward the ideal — or, for an immune thrower,
        // away from the middle.
        if orig_clicks > 0 {
            let planet = &mut state.planets[index];
            let orig_now = i32::from(planet.env_orig.unwrap_or(planet.env)[v]);
            let delta = if race.is_immune(v) {
                if orig_now < 50 {
                    -orig_clicks.min(orig_now - 1)
                } else {
                    orig_clicks.min(99 - orig_now)
                }
            } else {
                let centre = i32::from(race.env_center[v]);
                match orig_now.cmp(&centre) {
                    std::cmp::Ordering::Less => orig_clicks.min(centre - orig_now),
                    std::cmp::Ordering::Greater => -orig_clicks.min(orig_now - centre),
                    std::cmp::Ordering::Equal => 0,
                }
            };
            if delta != 0 {
                state.messages.push(Message {
                    player: thrower,
                    id: if theirs {
                        id::PACKET_TERRAFORMED_ORIG_THEIRS
                    } else {
                        id::PACKET_TERRAFORMED_ORIG
                    },
                    object: planet_id,
                    params: vec![
                        i16::from(delta > 0),
                        n_i16(v as i32),
                        planet_id,
                        n_i16(delta.abs()),
                    ],
                });
                let planet = &mut state.planets[index];
                let orig = planet.env_orig.get_or_insert(planet.env);
                orig[v] =
                    i8::try_from((orig_now + delta).clamp(i32::from(ENV_MIN), i32::from(ENV_MAX)))
                        .unwrap_or(orig[v]);
                moved_orig[v] = delta;
            }
        }

        // The value itself, within the thrower's reach.
        if clicks > 0 {
            let planet = &state.planets[index];
            let targets = terraform_targets(planet, &race, tech, Intent::Help);
            if targets.iter().all(Option::is_none) {
                continue;
            }
            let env = i32::from(planet.env[v]);
            let delta = if race.is_immune(v) {
                let half = clicks / 2;
                if env < 50 {
                    -half.min(env - 1)
                } else {
                    half.min(99 - env)
                }
            } else {
                match targets[v] {
                    None => 0,
                    Some(t) => {
                        let t = i32::from(t);
                        if t < env {
                            -clicks.min(env - t)
                        } else {
                            clicks.min(t - env)
                        }
                    }
                }
            };
            if delta != 0 {
                let planet = &mut state.planets[index];
                let now = (env + delta).clamp(i32::from(ENV_MIN), i32::from(ENV_MAX));
                planet.env[v] = i8::try_from(now).unwrap_or(planet.env[v]);
                moved[v] = delta;
                state.messages.push(Message {
                    player: thrower,
                    id: if theirs {
                        id::PACKET_TERRAFORMED_THEIRS
                    } else {
                        id::PACKET_TERRAFORMED
                    },
                    object: planet_id,
                    params: vec![
                        i16::from(delta > 0),
                        n_i16(v as i32),
                        planet_id,
                        n_i16((v as i32) << 8 | now),
                    ],
                });
            }
        }
    }
    (moved, moved_orig)
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
        // The manual's example (p. 25-3) has a warp 10 packet of 1,000 kT
        // against a warp 5 driver doing 469; the binary subtracts the warp
        // rather than its square and does 593.
        assert_eq!(damage(10, 5, false, 1000), (100 - 5) * 1000 / 160);
        assert_eq!(damage(10, 0, false, 1600), 1000);
        // An Inner Tech receiver's driver is halved for the catch, not here.
        assert_eq!(damage(10, 10, true, 1000), (100 - 10) * 1000 / 160);
    }

    /// A game with one player and one planet, id 7, for a packet to land on.
    fn a_state() -> crate::GameState {
        let mut state = crate::GameState::new(1);
        state.turn = 30;
        state
            .players
            .push(crate::Player::new(crate::Race::humanoid()));
        state.designs.push(Vec::new());
        let mut planet = crate::planet::Planet::unowned(7);
        planet.position = Some(Point::new(1000, 1000));
        planet.env = [40, 40, 40];
        planet.env_orig = Some([40, 40, 40]);
        state.planets.push(planet);
        state
    }

    /// The planet in [`a_state`] given a starbase with a Mass Driver 7
    /// (two of them when `paired`), aimed at planet 8 at the warp asked.
    fn with_driver(state: &mut crate::GameState, paired: bool, warp: u8) {
        use crate::components::slot;
        use crate::design::{DesignSlot, ShipDesign};
        let base = usize::from(crate::startup::FIRST_STARBASE_SLOT);
        let empty = ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: -1,
            slots: Vec::new(),
        };
        state.designs[0].resize(base + 1, empty);
        state.designs[0][base] = ShipDesign {
            name: "Dock".to_string(),
            picture: 0,
            stored_armor: 100,
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: 33,
            slots: vec![
                DesignSlot {
                    category: slot::SPECIAL_SB,
                    item: 9, // Mass Driver 7
                    count: 1,
                },
                DesignSlot {
                    category: slot::SPECIAL_SB,
                    item: 9,
                    count: u8::from(paired),
                },
            ],
        };
        let planet = &mut state.planets[0];
        planet.owner = Some(0);
        planet.pop = 1000;
        planet.starbase = true;
        planet.starbase_design = Some(0);
        planet.fling_dest = Some(8);
        planet.fling_warp = warp.saturating_sub(WARP_BIAS);
    }

    /// A packet built with no driver, or no destination, is not thrown, and
    /// the planet says why.
    #[test]
    fn a_packet_needs_a_driver_and_a_destination() {
        let mut state = a_state();
        state.planets[0].owner = Some(0);
        assert!(fling(&mut state, 0, crate::production::item::PACKET_IRONIUM, 1).is_none());
        assert_eq!(
            state.messages.last().map(|m| m.id),
            Some(crate::message::id::PACKET_NO_DRIVER)
        );
        with_driver(&mut state, false, 7);
        state.planets[0].fling_dest = None;
        assert!(fling(&mut state, 0, crate::production::item::PACKET_IRONIUM, 1).is_none());
        assert_eq!(
            state.messages.last().map(|m| m.id),
            Some(crate::message::id::PACKET_NO_DESTINATION)
        );
        assert!(state.packets.is_empty());
    }

    /// A hundred kilotons a packet, at the planet's warp, and a second lot
    /// the same year joins the first.
    #[test]
    fn a_packet_is_thrown_at_the_planets_warp_and_joined_by_the_next() {
        use crate::production::item;
        let mut state = a_state();
        with_driver(&mut state, false, 9);
        let flung = fling(&mut state, 0, item::PACKET_IRONIUM, 2).expect("thrown");
        assert!(!flung.joined);
        assert_eq!(flung.minerals, [200, 0, 0]);
        assert_eq!(
            flung.warp, 9,
            "the planet's setting, within three of the driver"
        );
        let packet = &state.packets[0];
        assert_eq!(packet.owner, 0);
        assert_eq!(packet.target, 8);
        assert_eq!(packet.speed(), 9);
        assert_eq!(packet.decay_rate, 2, "two over a warp 7 driver");
        assert_eq!(
            state.messages.last().map(|m| (m.id, m.params.clone())),
            Some((crate::message::id::PACKET_FLUNG, vec![7, 8]))
        );

        // A mixed packet of the same year goes into the same one.
        let again = fling(&mut state, 0, item::PACKET_MIXED, 1).expect("thrown");
        assert!(again.joined);
        assert_eq!(state.packets.len(), 1);
        assert_eq!(state.packets[0].minerals, [240, 40, 40]);
        assert_eq!(
            state.messages.last().map(|m| m.id),
            Some(crate::message::id::PACKET_ADDED_TO)
        );
    }

    /// A setting the driver cannot manage — under 5, or more than three
    /// over it — falls back to the driver's own warp, a pair counting one
    /// faster; and a Packet Physics race packs less into each.
    #[test]
    fn an_impossible_setting_falls_back_to_the_driver() {
        use crate::production::item;
        let mut state = a_state();
        with_driver(&mut state, true, 12);
        let flung = fling(&mut state, 0, item::PACKET_BORANIUM, 1).expect("thrown");
        assert_eq!(flung.warp, 8, "the pair of 7s counts as 8");
        assert_eq!(state.packets[0].decay_rate, 0);

        let mut state = a_state();
        state.players[0].race.attrs[crate::race::RaceStat::MajorAdv as usize] = 6;
        with_driver(&mut state, false, 7);
        let flung = fling(&mut state, 0, item::AUTO_PACKET, 3).expect("thrown");
        assert_eq!(flung.minerals, [75, 75, 75], "25 of each, three times");
    }

    /// With no driver, an unowned planet keeps a ninth and nothing else
    /// happens.
    #[test]
    fn an_unowned_planet_keeps_a_ninth() {
        let mut state = a_state();
        let mut rng = crate::rng::Rng::randomize(5);
        let landing = land(&mut state, &packet([900, 0, 0], 0), &mut rng).expect("landed");
        assert_eq!(landing.caught, 0);
        assert_eq!(landing.delivered, [900 * 111 / 1000, 0, 0]);
        assert_eq!(state.planets[0].surface_min[0], 99);
        assert_eq!(landing.damage, 0);
        assert!(state.messages.is_empty());
    }

    /// The manual's example (p. 25-3) with the binary's damage: a 1,000 kT
    /// warp 10 packet on a planet of 250,000 with no driver and no
    /// defences does 625, and kills `max(2500 × 625 / 1000, 625)` hundreds.
    #[test]
    fn an_uncaught_packet_kills_by_the_manuals_rule() {
        let mut state = a_state();
        state.planets[0].owner = Some(0);
        state.planets[0].pop = 2500;
        let mut rng = crate::rng::Rng::randomize(5);
        let landing = land(&mut state, &packet([1000, 0, 0], 0), &mut rng).expect("landed");
        assert_eq!(landing.damage, 625);
        assert_eq!(landing.killed, 1562);
        assert_eq!(state.planets[0].pop, 2500 - 1562);
        assert_eq!(landing.defences_destroyed, 0);
        assert!(!landing.depopulated);
        let msg = state.messages.last().expect("told");
        assert_eq!(msg.id, crate::message::id::PACKET_DAMAGE_NO_DRIVER);
        assert_eq!(msg.params, vec![7, 1000, 0, 0, 1562]);
    }

    /// Defences take their share of the damage first, and go down with the
    /// colonists: `defences × damage / 1000`, at least `damage / 20`.
    #[test]
    fn defences_soften_the_blow_and_fall_with_it() {
        let mut state = a_state();
        state.planets[0].owner = Some(0);
        state.planets[0].pop = 2500;
        state.planets[0].defenses = 50;
        let race = state.players[0].race.clone();
        let (survive, _) =
            crate::bombing::pct_survive(&state.planets[0], &race, state.players[0].research.levels);
        assert!(survive < 1.0, "the defences stop something");
        #[allow(clippy::cast_possible_truncation)]
        let dmg = (625.0 * survive) as i32;
        let mut rng = crate::rng::Rng::randomize(5);
        let landing = land(&mut state, &packet([1000, 0, 0], 0), &mut rng).expect("landed");
        assert_eq!(landing.damage, dmg);
        assert_eq!(landing.killed, (2500 * dmg / 1000).max(dmg));
        assert_eq!(
            landing.defences_destroyed,
            (50 * dmg / 1000).max(dmg / 20).min(50)
        );
        assert_eq!(
            i32::from(state.planets[0].defenses),
            50 - landing.defences_destroyed
        );
        let msg = state.messages.last().expect("told");
        assert_eq!(msg.id, crate::message::id::PACKET_DAMAGE_DEFENCES_NO_DRIVER);
        assert_eq!(msg.params.len(), 6);
    }

    /// A planet that loses everyone is uninhabited, and its owner told so.
    #[test]
    fn a_small_colony_is_wiped_out() {
        let mut state = a_state();
        state.planets[0].owner = Some(0);
        state.planets[0].pop = 300;
        let mut rng = crate::rng::Rng::randomize(5);
        let landing = land(&mut state, &packet([1000, 0, 0], 0), &mut rng).expect("landed");
        assert!(landing.depopulated);
        assert_eq!(landing.killed, 300);
        assert_eq!(state.planets[0].owner, None);
        assert_eq!(
            state.messages.last().map(|m| m.id),
            Some(crate::message::id::PACKET_WIPED_OUT)
        );
    }

    /// An Alternate Reality owner takes no damage and is told the packet
    /// was harmless.
    #[test]
    fn an_ar_owner_is_not_hurt() {
        let mut state = a_state();
        state.players[0].race.attrs[crate::race::RaceStat::MajorAdv as usize] = 8;
        state.planets[0].owner = Some(0);
        state.planets[0].pop = 2500;
        let mut rng = crate::rng::Rng::randomize(5);
        let landing = land(&mut state, &packet([1000, 0, 0], 0), &mut rng).expect("landed");
        assert_eq!(landing.damage, 0);
        assert_eq!(state.planets[0].pop, 2500);
        assert_eq!(
            state.messages.last().map(|m| m.id),
            Some(crate::message::id::PACKET_HARMLESS)
        );
    }

    /// A Packet Physics thrower's packet terraforms toward its ideal, within
    /// its reach — Total Terraform 3 here — and no further.
    #[test]
    fn a_packet_physics_packet_terraforms_within_reach() {
        let mut state = a_state();
        state.players[0].race.attrs[crate::race::RaceStat::MajorAdv as usize] = 6;
        state.players[0].race.lrt_bits |= 1 << crate::race::lrt::TT;
        let mut rng = crate::rng::Rng::randomize(5);
        // Ten lots of a hundred kilotons of ironium, each a coin's toss.
        let landing = land(&mut state, &packet([1000, 0, 0], 0), &mut rng).expect("landed");
        let env = i32::from(state.planets[0].env[0]);
        assert_eq!(landing.terraformed[0], env - 40);
        assert!((1..=3).contains(&landing.terraformed[0]), "reach is three");
        assert_eq!(state.planets[0].env[1], 40, "nothing was thrown for it");
        let orig = i32::from(state.planets[0].env_orig.expect("kept")[0]);
        assert_eq!(landing.terraformed_orig[0], orig - 40);
        assert!(landing.terraformed_orig[0] <= landing.terraformed[0].max(orig - 40));
        if landing.terraformed[0] > 0 {
            assert!(state
                .messages
                .iter()
                .any(|m| m.id == crate::message::id::PACKET_TERRAFORMED));
        }
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
