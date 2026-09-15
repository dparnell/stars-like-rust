//! Random events: the last step of a turn, `RandomEvents` (`10b8:3064`),
//! run from `Produce` (`10b8:0c87`) unless the game's "no random events"
//! flag (`GAME` word `+0x10`, bit 7 — `game_flag::NO_RANDOM`) is set.
//!
//! Four things, in this order, each rolling for itself:
//!
//! * a **meteor strike** (`MeteorStrike`, `10b8:560e`), one chance in
//!   twenty a year;
//! * a **climate change** (`PlanetaryClimateChange`, `10b8:5c54`), one in
//!   twenty;
//! * a **mineral discovery** (`DiscoverNewMinerals`, `10b8:5e0c`), one in
//!   `15 − size` for a galaxy size of 0 (tiny) to 4 (huge);
//! * a **Mystery Trader** setting out (`MysteryTrader`, `10b8:5efa`), on
//!   a schedule of its own from turn 40.
//!
//! `MANUAL.PDF` p. 7-12 ("Disaster Strikes Planet X!") says a comet
//! resets the production queue and usually brings minerals, and p. 19-2
//! introduces the Trader; every number below is the binary's, and the
//! spec is `docs/formulas/random-events.md`.

use crate::message::{self, id, Message};
use crate::movement::Point;
use crate::rng::Rng;
use crate::wormhole::MysteryTrader;
use crate::GameState;

/// What the year's random events did, for the turn report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventReport {
    /// A meteor struck: the planet and the strike's size, 0 to 3.
    pub meteor: Option<(i16, u8)>,
    /// A planet's climate shifted: the planet and the variable, 0 gravity,
    /// 1 temperature, 2 radiation.
    pub climate: Option<(i16, u8)>,
    /// New minerals were found: the planet and the mineral.
    pub minerals: Option<(i16, u8)>,
    /// A Mystery Trader set out: its id.
    pub trader: Option<u16>,
}

/// Population under which a young colony is fair game for a strike in the
/// first twenty years: `rgwtMin[3] < 0x33`, in hundreds.
const YOUNG_COLONY: i32 = 0x33;

/// The year's random events.
pub fn random_events(state: &mut GameState, rng: &mut Rng) -> EventReport {
    let meteor = meteor_strike(state, rng);
    let climate = climate_change(state, rng);
    let minerals = discover_minerals(state, rng);
    let trader = mystery_trader(state, rng);
    EventReport {
        meteor,
        climate,
        minerals,
        trader,
    }
}

/// Clamp a habitability click to the original's `1..=99`.
fn clamp_env(value: i32) -> i8 {
    i8::try_from(value.clamp(1, 99)).unwrap_or(1)
}

/// Whether a planet may be struck this year: an unowned one, a colony still
/// under 5,100 people, or anything once twenty years have passed
/// (`game.turn > 0x13`).
fn strikeable(state: &GameState, index: usize) -> bool {
    let planet = &state.planets[index];
    planet.owner.is_none() || planet.pop < YOUNG_COLONY || state.turn > 19
}

/// `MeteorStrike` (`10b8:560e`).
///
/// One in twenty a year, on a planet drawn at random, from turn 10 — and
/// before turn 20 only an unowned planet or a young colony. The strike's
/// size is a draw of four. Every player is told; the owner, unless AR,
/// loses `25 + 20 × size` percent of the population. Each mineral gets 50
/// to 299 kT on the surface; then `size + 1` of them (at most three, in a
/// shuffled order) get 3,000 to 19,999 kT more and their concentration
/// raised by 50 to 99 (a size-3 strike adds 15 to 29 on top), capped at
/// 200; and `size + 1` of the habitability variables, in order, move by 3
/// to 5 clicks (6 to 10 for size 3) either way, the original values with
/// them. Whatever the planet was building that is not an auto-build item
/// is thrown out (`TossNonAutoBuildItems`, `10b8:5aec`).
fn meteor_strike(state: &mut GameState, rng: &mut Rng) -> Option<(i16, u8)> {
    if rng.random(20) != 0 || state.planets.is_empty() {
        return None;
    }
    let index = usize::try_from(rng.random(planet_count(state))).unwrap_or(0);
    if !(strikeable(state, index) && state.turn > 9) {
        return None;
    }
    let size = u8::try_from(rng.random(4)).unwrap_or(0);
    // The order the minerals are named in, shuffled three swaps deep. The
    // routine passes it with the message, but the record the file keeps
    // for one of these carries the planet alone (the Exodus game's
    // `0x84`), so it is drawn and not kept.
    let mut named = [0i16, 1, 2];
    for i in 0..3 {
        let j = usize::try_from(rng.random(3)).unwrap_or(0);
        named.swap(i, j);
    }
    // What lands on each: 50 to 299 kT to begin with.
    let mut order = [0usize, 1, 2];
    let mut amounts = [0i32; 3];
    for (m, amount) in amounts.iter_mut().enumerate() {
        order[m] = m;
        *amount = i32::from(rng.random(250)) + 50;
    }
    for i in 0..2 {
        let j = usize::try_from(rng.random(3 - i16::try_from(i).unwrap_or(0))).unwrap_or(0);
        order.swap(i, i + j);
    }

    let planet_id = state.planets[index].id;
    let owner = state.planets[index].owner;
    let owner_ar = owner
        .and_then(|o| usize::try_from(o).ok())
        .and_then(|o| state.players.get(o))
        .is_some_and(|p| p.race.is_ar());
    for player in 0..state.players.len() {
        let mine = owner.is_some_and(|o| usize::try_from(o).ok() == Some(player)) && !owner_ar;
        state.messages.push(Message {
            player,
            id: if mine { id::METEOR_YOURS } else { id::METEOR } + u16::from(size),
            object: planet_id,
            params: vec![planet_id],
        });
    }

    let planet = &mut state.planets[index];
    if owner.is_some() && !owner_ar {
        let pct = i64::from(size) * 20 + 25;
        let killed = i64::from(planet.pop) * pct / 100;
        planet.pop -= i32::try_from(killed).unwrap_or(0);
    }
    for &which in &order[..=usize::from(size).min(2)] {
        amounts[which] += i32::from(rng.random(17000)) + 3000;
        let mut conc = i32::from(planet.min_conc[which]) + i32::from(rng.random(50)) + 50;
        if size == 3 {
            conc += i32::from(rng.random(15)) + 15;
        }
        planet.min_conc[which] = u8::try_from(conc.min(200)).unwrap_or(200);
    }
    for (have, more) in planet.surface_min.iter_mut().zip(amounts) {
        *have = have.saturating_add(more);
    }
    for v in 0..=usize::from(size).min(2) {
        let mut delta = i32::from(rng.random(3)) + 3;
        if size == 3 {
            delta += i32::from(rng.random(3)) + 3;
        }
        if rng.random(2) != 0 {
            delta = -delta;
        }
        planet.env[v] = clamp_env(i32::from(planet.env[v]) + delta);
        if let Some(orig) = planet.env_orig.as_mut() {
            orig[v] = clamp_env(i32::from(orig[v]) + delta);
        }
    }
    planet.queue.retain(crate::production::QueueItem::is_auto);
    Some((planet_id, size))
}

/// `PlanetaryClimateChange` (`10b8:5c54`).
///
/// One in twenty a year, on a planet drawn at random — an unowned one, a
/// young colony, or anything from turn 20. One variable, drawn from three,
/// moves by 3 to 5 clicks, or 6 to 8 when the first draw came out at 3,
/// either way, the original value with it; the owner is told. Non-auto
/// items leave the queue.
fn climate_change(state: &mut GameState, rng: &mut Rng) -> Option<(i16, u8)> {
    if rng.random(20) != 0 || state.planets.is_empty() {
        return None;
    }
    let index = usize::try_from(rng.random(planet_count(state))).unwrap_or(0);
    if !strikeable(state, index) {
        return None;
    }
    let variable = usize::try_from(rng.random(3)).unwrap_or(0);
    let planet_id = state.planets[index].id;
    if let Some(owner) = state.planets[index]
        .owner
        .and_then(|o| usize::try_from(o).ok())
    {
        state.messages.push(Message {
            player: owner,
            id: id::CLIMATE_CHANGE,
            object: planet_id,
            params: vec![planet_id, i16::try_from(variable).unwrap_or(0)],
        });
    }
    let mut delta = i32::from(rng.random(3)) + 3;
    if delta == 3 {
        delta = i32::from(rng.random(3)) + 6;
    }
    if rng.random(2) != 0 {
        delta = -delta;
    }
    let planet = &mut state.planets[index];
    planet.env[variable] = clamp_env(i32::from(planet.env[variable]) + delta);
    if let Some(orig) = planet.env_orig.as_mut() {
        orig[variable] = clamp_env(i32::from(orig[variable]) + delta);
    }
    planet.queue.retain(crate::production::QueueItem::is_auto);
    Some((planet_id, u8::try_from(variable).unwrap_or(0)))
}

/// `DiscoverNewMinerals` (`10b8:5e0c`).
///
/// One in `15 − size` a year, on a planet drawn at random, from turn 10:
/// one mineral, drawn from three, has its concentration raised by 5 to 19
/// while it is under 180; the owner is told either way.
fn discover_minerals(state: &mut GameState, rng: &mut Rng) -> Option<(i16, u8)> {
    let odds = (15 - state.galaxy_size).max(1);
    if rng.random(odds) != 0 || state.planets.is_empty() {
        return None;
    }
    let index = usize::try_from(rng.random(planet_count(state))).unwrap_or(0);
    if state.turn <= 9 {
        return None;
    }
    let mineral = usize::try_from(rng.random(3)).unwrap_or(0);
    let planet_id = state.planets[index].id;
    if let Some(owner) = state.planets[index]
        .owner
        .and_then(|o| usize::try_from(o).ok())
    {
        state.messages.push(Message {
            player: owner,
            id: id::NEW_MINERALS,
            object: planet_id,
            params: vec![planet_id, i16::try_from(mineral).unwrap_or(0)],
        });
    }
    let planet = &mut state.planets[index];
    if planet.min_conc[mineral] < 180 {
        let more = u8::try_from(rng.random(15) + 5).unwrap_or(5);
        planet.min_conc[mineral] = planet.min_conc[mineral].saturating_add(more);
    }
    Some((planet_id, u8::try_from(mineral).unwrap_or(0)))
}

/// `MysteryTrader` (`10b8:5efa`): a Trader setting out.
///
/// Nothing before turn 40. Then the odds by the year: one in two when the
/// year ends in 71, one in three at 33, one in four when `turn & 0x7f` is
/// 49, and otherwise one in seven on even years and never on odd ones. A
/// Trader that sets out has warp 8 to 12 and crosses the galaxy from one
/// edge to the opposite — twenty light years inside the edge, at a random
/// place along it — starting on a top-or-bottom edge or a left-or-right
/// one at a coin's toss. What it carries: with `5` chances in ten before
/// turn 100, `3` before 250 and `2` after (one more for a slow Trader,
/// one fewer for a fast one) it carries nothing in particular — or, one
/// time in six of those, the lifeboat (`0x1000`); otherwise one part bit
/// of thirteen, the four rarer ones (`0x40`, `0x80`, `0x400`, `0x800`)
/// re-rolled once, and the three that come late — `0x80` before turn 120,
/// `0x400` before 150, `0x800` before 180 — dropped half the time. Every
/// player hears of it.
fn mystery_trader(state: &mut GameState, rng: &mut Rng) -> Option<u16> {
    let turn = i32::from(state.turn);
    if turn <= 0x27 {
        return None;
    }
    let odds = if turn % 100 == 0x47 {
        2
    } else if turn % 100 == 0x21 {
        3
    } else if turn & 0x7f == 0x31 {
        4
    } else if turn & 1 != 0 {
        return None;
    } else {
        7
    };
    if rng.random(odds) != 0 {
        return None;
    }
    let warp = u8::try_from(rng.random(5) + 8).unwrap_or(8);
    // Two random places along an edge, and the two edges.
    let span = state.galaxy_size * 400 + 361;
    let along = [rng.random(span) + 0x3fc, rng.random(span) + 0x3fc];
    let far = state.galaxy_size * 400 + 0x564;
    let edges = if rng.random(2) == 0 {
        [0x3fc, far]
    } else {
        [far, 0x3fc]
    };
    let (position, destination) = if rng.random(2) == 0 {
        // Along the top or the bottom, crossing to the other.
        (
            Point::new(along[0], edges[0]),
            Point::new(along[1], edges[1]),
        )
    } else {
        (
            Point::new(edges[0], along[0]),
            Point::new(edges[1], along[1]),
        )
    };
    let mut chances = if turn < 100 {
        5
    } else if turn < 0xfa {
        3
    } else {
        2
    };
    if warp < 10 {
        chances += 1;
    } else if warp > 10 {
        chances -= 1;
    }
    let part = if rng.random(10) < chances {
        if rng.random(6) == 0 {
            crate::wormhole::part::LIFEBOAT
        } else {
            0
        }
    } else {
        let mut bit = 1u16 << rng.random(13);
        if matches!(bit, 0x40 | 0x80 | 0x400 | 0x800) {
            bit = 1u16 << rng.random(13);
            let early = (turn < 0x78 && bit == 0x80)
                || (turn < 0x96 && bit == 0x400)
                || (turn < 0xb4 && bit == 0x800);
            if early && rng.random(2) != 0 {
                bit = 0;
            }
        }
        bit
    };
    let id = state
        .traders
        .iter()
        .map(|t| t.id)
        .max()
        .map_or(0, |id| id + 1);
    state.traders.push(MysteryTrader {
        id,
        position,
        destination,
        warp,
        include: true,
        detected_by: 0,
        part,
        turn: u16::try_from(state.turn).unwrap_or(0),
    });
    let id_full = i16::from_le_bytes((0x6000 | id).to_le_bytes());
    for player in 0..state.players.len() {
        state.messages.push(Message {
            player,
            id: id::TRADER_APPEARED,
            object: message::THING_OBJECT,
            params: vec![id_full],
        });
    }
    Some(id)
}

/// `cPlanet`, as a draw's range.
fn planet_count(state: &GameState) -> i16 {
    i16::try_from(state.planets.len()).unwrap_or(i16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planet::Planet;
    use crate::production::{item, QueueItem};

    fn a_state(planets: usize, turn: i16) -> GameState {
        let mut state = GameState::new(7);
        state.turn = turn;
        state.random_events = true;
        state
            .players
            .push(crate::Player::new(crate::Race::humanoid()));
        for i in 0..planets {
            let mut planet = Planet::unowned(i16::try_from(i).unwrap_or(0));
            planet.env = [50, 50, 50];
            planet.env_orig = Some([50, 50, 50]);
            planet.min_conc = [40, 40, 40];
            planet.position = Some(Point::new(1100 + 10 * i16::try_from(i).unwrap_or(0), 1100));
            state.planets.push(planet);
        }
        state
    }

    /// A seed that makes the first draw land where a strike wants it.
    fn seed_for(mut wanted: impl FnMut(&mut Rng) -> bool) -> u32 {
        (1..100_000u32)
            .find(|seed| wanted(&mut Rng::randomize(*seed)))
            .expect("some seed rolls it")
    }

    #[test]
    fn a_meteor_strike_does_what_the_routine_does() {
        let seed = seed_for(|rng| rng.random(20) == 0);
        let mut state = a_state(3, 30);
        // The owner's colony: people, a queue with a ship in it.
        for planet in &mut state.planets {
            planet.owner = Some(0);
            planet.pop = 1000;
            planet.queue = vec![
                QueueItem {
                    count: 1,
                    item: 2,
                    ship: true,
                    completion: 0,
                },
                QueueItem {
                    count: 1,
                    item: item::AUTO_FACTORY,
                    ship: false,
                    completion: 0,
                },
            ];
        }
        let mut rng = Rng::randomize(seed);
        let report = random_events(&mut state, &mut rng);
        let (id, size) = report.meteor.expect("the seed strikes");
        let planet = state
            .planets
            .iter()
            .find(|p| p.id == id)
            .expect("the struck planet");
        // The population lost its share, to the unit.
        let pct = i64::from(size) * 20 + 25;
        assert_eq!(i64::from(planet.pop), 1000 - 1000 * pct / 100);
        // Every mineral gained at least fifty, `size + 1` of them thousands.
        assert!(planet.surface_min.iter().all(|&m| m >= 50));
        let big = planet.surface_min.iter().filter(|&&m| m >= 3050).count();
        assert_eq!(big, usize::from(size).min(2) + 1);
        // Concentrations rose on the same ones, capped at 200.
        let raised = planet.min_conc.iter().filter(|&&c| c > 40).count();
        assert_eq!(raised, usize::from(size).min(2) + 1);
        assert!(planet.min_conc.iter().all(|&c| c <= 200));
        // The first `size + 1` variables moved, originals with them, within
        // the clamp; the rest stayed.
        let orig = planet.env_orig.expect("kept");
        for (v, (now, orig)) in planet.env.iter().zip(orig).enumerate() {
            if v <= usize::from(size).min(2) {
                assert_ne!(*now, 50, "variable {v} moved");
                assert_eq!(*now, orig);
            } else {
                assert_eq!(*now, 50);
            }
        }
        // The ship left the queue; the auto-build item stayed.
        assert_eq!(planet.queue.len(), 1);
        assert!(planet.queue[0].is_auto());
        // Everyone was told; the owner in the second family of wordings.
        let mine: Vec<&Message> = state
            .messages
            .iter()
            .filter(|m| m.id >= id::METEOR && m.id < id::METEOR_YOURS + 4)
            .collect();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].id, id::METEOR_YOURS + u16::from(size));
        assert_eq!(mine[0].object, id);
    }

    #[test]
    fn a_strike_waits_for_turn_ten_and_spares_grown_colonies_before_twenty() {
        let seed = seed_for(|rng| rng.random(20) == 0);
        let mut early = a_state(3, 5);
        assert!(meteor_strike(&mut early, &mut Rng::randomize(seed)).is_none());
        let mut settled = a_state(3, 15);
        for planet in &mut settled.planets {
            planet.owner = Some(0);
            planet.pop = 1000;
        }
        assert!(meteor_strike(&mut settled, &mut Rng::randomize(seed)).is_none());
        let mut young = a_state(3, 15);
        for planet in &mut young.planets {
            planet.owner = Some(0);
            planet.pop = 40;
        }
        assert!(meteor_strike(&mut young, &mut Rng::randomize(seed)).is_some());
    }

    #[test]
    fn an_ar_owner_keeps_its_people() {
        let seed = seed_for(|rng| rng.random(20) == 0);
        let mut state = a_state(3, 30);
        let mut race = crate::Race::humanoid();
        race.attrs[crate::race::RaceStat::MajorAdv as usize] = 8;
        state.players[0] = crate::Player::new(race);
        for planet in &mut state.planets {
            planet.owner = Some(0);
            planet.pop = 1000;
        }
        let (id, _) = meteor_strike(&mut state, &mut Rng::randomize(seed)).expect("strikes");
        let planet = state.planets.iter().find(|p| p.id == id).expect("struck");
        assert_eq!(planet.pop, 1000, "an AR race lives in orbit");
        assert!(state
            .messages
            .iter()
            .all(|m| m.id < id::METEOR_YOURS || m.id >= id::METEOR_YOURS + 4));
    }

    #[test]
    fn a_climate_change_moves_one_variable_and_its_original() {
        let seed = seed_for(|rng| rng.random(20) == 0);
        let mut state = a_state(3, 30);
        let (id, variable) =
            climate_change(&mut state, &mut Rng::randomize(seed)).expect("changes");
        let planet = state.planets.iter().find(|p| p.id == id).expect("changed");
        let v = usize::from(variable);
        let moved = i32::from(planet.env[v]) - 50;
        assert!(matches!(moved.abs(), 3..=8), "moved {moved}");
        assert_eq!(planet.env[v], planet.env_orig.expect("kept")[v]);
        for other in (0..3).filter(|o| *o != v) {
            assert_eq!(planet.env[other], 50);
        }
    }

    #[test]
    fn a_discovery_raises_one_concentration_from_turn_ten() {
        let seed = seed_for(|rng| rng.random(15) == 0);
        let mut early = a_state(3, 5);
        assert!(discover_minerals(&mut early, &mut Rng::randomize(seed)).is_none());
        let mut state = a_state(3, 30);
        state.planets[0].owner = Some(0);
        let (id, mineral) =
            discover_minerals(&mut state, &mut Rng::randomize(seed)).expect("finds");
        let planet = state.planets.iter().find(|p| p.id == id).expect("found");
        let more = i32::from(planet.min_conc[usize::from(mineral)]) - 40;
        assert!((5..=19).contains(&more), "raised by {more}");
        // Rich ground is left alone.
        let mut rich = a_state(3, 30);
        for planet in &mut rich.planets {
            planet.min_conc = [190, 190, 190];
        }
        discover_minerals(&mut rich, &mut Rng::randomize(seed));
        assert!(rich.planets.iter().all(|p| p.min_conc == [190, 190, 190]));
    }

    #[test]
    fn a_trader_sets_out_from_an_edge_for_the_opposite_one() {
        // An even year from 40 rolls one in seven.
        let seed = seed_for(|rng| rng.random(7) == 0);
        let mut state = a_state(3, 50);
        state.galaxy_size = 1;
        let id = mystery_trader(&mut state, &mut Rng::randomize(seed)).expect("sets out");
        let trader = state.traders.iter().find(|t| t.id == id).expect("there");
        let (near, far) = (0x3fc, 400 + 0x564);
        let on_edge = |p: Point| p.x == near || p.x == far || p.y == near || p.y == far;
        assert!(on_edge(trader.position) && on_edge(trader.destination));
        assert!((8..=12).contains(&trader.warp));
        assert!(trader.part == 0 || trader.part.count_ones() == 1);
        assert_eq!(
            state
                .messages
                .iter()
                .filter(|m| m.id == id::TRADER_APPEARED)
                .count(),
            state.players.len()
        );
        // Odd years off the schedule never roll.
        let mut odd = a_state(3, 51);
        assert!(mystery_trader(&mut odd, &mut Rng::randomize(seed)).is_none());
        let mut early = a_state(3, 30);
        assert!(mystery_trader(&mut early, &mut Rng::randomize(seed)).is_none());
    }
}
