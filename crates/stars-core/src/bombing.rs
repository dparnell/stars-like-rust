//! Bombing a planet from orbit.
//!
//! Source: `DoBombing` (`battle.c`), `CalcPctSurvive` (`util.c`) and
//! `FCalcFleetBombDamage`. The first two are recovered in full; the third is a
//! stub in the reconstructed sources and its decompilation shifts every
//! parameter by one, because the leading far `FLEET *` occupies two slots. What
//! could be read from it is marked below; what could not is not implemented.
//!
//! See `docs/formulas/bombing.md`.

use crate::components::{slot, Bomb, BOMBS, PLANETARY};
use crate::design::ShipDesign;
use crate::fleet::ShipStack;
use crate::planet::Planet;
use crate::race::Race;
use crate::rng::Rng;

/// Index of the Retro Bomb in [`BOMBS`], which un-terraforms rather than kills.
///
/// `FCalcFleetBombDamage` singles it out by `iItem & 0xFF == 9` before any other
/// test.
pub const RETRO_BOMB: usize = 9;

/// Bombs below this index also guarantee a minimum kill of
/// [`MINIMUM_KILL_PER_BOMB`] each — the five basic bombs, all of which damage
/// buildings as well as people.
pub const FIRST_SPECIALISED_BOMB: usize = 5;

/// The minimum population each basic bomb kills whatever the percentages say,
/// in units of 100 colonists.
pub const MINIMUM_KILL_PER_BOMB: i32 = 3;

/// Index of the first planetary **defence** in [`PLANETARY`]; the entries below
/// it are scanners.
pub const FIRST_DEFENCE: usize = 9;

/// What a fleet's bombs add up to against one planet.
///
/// The people figures are in **tenths of a percent**, matching the component
/// table: a Cherry Bomb's 25 is 2.5% of the population.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BombLoad {
    /// `dmgBombPeople` — ordinary bombs, summed.
    pub people: i32,
    /// `dmgPeopleMin` — a floor on the kill, in units of 100 colonists.
    pub floor: i32,
    /// `dmgPeopleSmart` — smart bombs, which stack multiplicatively.
    pub smart: i32,
    /// `dmgBombBldg` — installations destroyed, an absolute count.
    pub buildings: i32,
    /// `pctTerra` — Retro Bombs aboard, which reverse terraforming.
    pub retro: i32,
}

impl BombLoad {
    /// Whether this load does anything at all (`FCalcFleetBombDamage`'s return).
    #[must_use]
    pub fn any(self) -> bool {
        self.people != 0
            || self.floor != 0
            || self.smart != 0
            || self.buildings != 0
            || self.retro != 0
    }
}

/// Add up the bombs a fleet carries.
///
/// A **smart** bomb is one that does no building damage — the Smart, Neutron,
/// Enriched Neutron, Peerless and Annihilator. Those do not sum: the routine
/// keeps a running product of `(1 - damage/1000)`, one factor per bomb, so a
/// hundred of them approach but never reach wiping the planet. Ordinary bombs
/// sum outright.
///
/// Two further contributors were read but are **not** implemented, because what
/// they are could not be established: a beam-slot item and an Alternate Reality
/// mechanical special each add fixed amounts. Neither appears in this
/// repository's fixtures. See `docs/formulas/bombing.md`.
#[must_use]
pub fn bomb_load(designs: &[ShipDesign], stacks: &[ShipStack]) -> BombLoad {
    let mut load = BombLoad::default();
    // Smart bombs stack multiplicatively; 1.0 is "nobody killed".
    let mut smart_survival = 1.0f64;

    for stack in stacks {
        if stack.count <= 0 {
            continue;
        }
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        for fitted in &design.slots {
            if fitted.count == 0 || fitted.category != slot::BOMB {
                continue;
            }
            let item = usize::from(fitted.item);
            let Some(bomb) = BOMBS.get(item) else {
                continue;
            };
            let count = i32::from(fitted.count) * stack.count;

            if item == RETRO_BOMB {
                load.retro += count;
                continue;
            }
            if is_smart(bomb) {
                let factor = 1.0 - f64::from(bomb.colonist_damage) / 1000.0;
                smart_survival *= factor.powi(count.max(0));
                continue;
            }
            load.people += count * i32::from(bomb.colonist_damage);
            load.buildings += count * i32::from(bomb.building_damage);
            if item < FIRST_SPECIALISED_BOMB {
                load.floor += count * MINIMUM_KILL_PER_BOMB;
            }
        }
    }

    // The product is turned back into a tenths-of-a-percent figure and capped.
    // The conversion itself is the one step of `FCalcFleetBombDamage` that the
    // decompilation hides — it leaves the value on the FPU stack — so this is
    // the natural reading of a survival product, not a transcription.
    load.smart = (((1.0 - smart_survival) * 1000.0) as i32).clamp(0, 1000);
    load
}

/// Whether a bomb is a **smart** bomb: it kills people and leaves buildings.
#[must_use]
pub fn is_smart(bomb: &Bomb) -> bool {
    bomb.building_damage == 0 && bomb.colonist_damage > 0
}

/// The share of a bombing run that gets through the planet's defences.
///
/// Source: `CalcPctSurvive` (`util.c`), recovered in full:
///
/// ```c
/// if (planet unowned || cDefenses == 0) { *ppct = 1.0; return; }
/// cDefenses = min(cDefenses, CMaxOperableDefenses(lppl, owner, false));
/// pct      = pow(1 - dDmgCol / 1000.0, cDefenses);
/// pctSmart = pow(1 - dDmgCol / 2000.0, cDefenses);
/// ```
///
/// `dDmgCol` is the coverage rating of the **best defence the owner can
/// build**, and defences are counted only up to what the population can staff.
/// Smart bombs are stopped at half the rate, which is why they stay useful
/// against a defended world.
///
/// Returns `(ordinary, smart)`, each in `0.0..=1.0`.
#[must_use]
pub fn pct_survive(planet: &Planet, race: &Race, tech: [u8; 6]) -> (f64, f64) {
    if planet.owner.is_none() {
        return (1.0, 1.0);
    }
    let defences = i32::from(planet.defenses).min(i32::from(
        crate::resources::max_operable_defenses(planet, race),
    ));
    if defences <= 0 {
        return (1.0, 1.0);
    }
    let Some(coverage) = best_defence(tech) else {
        return (1.0, 1.0);
    };
    let coverage = f64::from(coverage);
    let n = defences;
    (
        (1.0 - coverage / 1000.0).powi(n),
        (1.0 - coverage / 2000.0).powi(n),
    )
}

/// The coverage rating of the best planetary defence the player can build.
///
/// Source: `FGetBestDefensePart`. The defences are the tail of the planetary
/// table and improve monotonically, so the best buildable one is the last whose
/// technology is met.
#[must_use]
pub fn best_defence(tech: [u8; 6]) -> Option<i16> {
    best_defence_part(tech).map(|p| p.ability)
}

/// The best planetary defence the player can build, whole.
///
/// [`best_defence`] wants only its coverage; the planet pane wants its name.
#[must_use]
pub fn best_defence_part(tech: [u8; 6]) -> Option<&'static crate::components::Planetary> {
    PLANETARY
        .iter()
        .skip(FIRST_DEFENCE)
        .filter(|p| p.ability > 0)
        .filter(|p| {
            p.tech
                .iter()
                .zip(tech.iter())
                .all(|(need, have)| i32::from(*need) <= i32::from(*have))
        })
        .max_by_key(|p| p.ability)
}

/// What one bombing run did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BombResult {
    /// Population killed, in units of 100 colonists.
    pub colonists: i32,
    /// Factories destroyed.
    pub factories: i32,
    /// Mines destroyed.
    pub mines: i32,
    /// Defences destroyed.
    pub defenses: i32,
}

/// Bomb a planet, applying the losses.
///
/// Source: `DoBombing` (`battle.c`), recovered in full.
///
/// Building damage is shared over the three installation kinds **in proportion
/// to how many of each the planet has**, with the leftover resolved by a roll
/// and the mines taking whatever is left over from the other two:
///
/// ```c
/// cPPE = cFactories + cMines + cDefenses;
/// q = count * dmgBombBldg / cPPE;  r = count * dmgBombBldg % cPPE;
/// if (r > 0 && Random(cPPE) < r) q++;
/// ```
///
/// Population is killed in two stages: the smart bombs take their share of the
/// whole population first, capped one short of wiping it out, and the ordinary
/// bombs then take their share of **what is left**. A run with any ordinary
/// bomb kills at least one unit, and never fewer than the load's floor.
pub fn bomb_planet(
    planet: &mut Planet,
    load: BombLoad,
    survive: (f64, f64),
    rng: &mut Rng,
) -> BombResult {
    let mut load = load;
    // Whatever the defences stop, stops. The original rounds each bucket with
    // a half added before truncating.
    let scale = |value: i32, pct: f64| -> i32 {
        if value > 0 && pct < 1.0 {
            (f64::from(value) * pct + 0.5) as i32
        } else {
            value
        }
    };
    load.people = scale(load.people, survive.0);
    load.floor = scale(load.floor, survive.0);
    load.buildings = scale(load.buildings, survive.0);
    load.smart = scale(load.smart, survive.1);

    let mut out = BombResult::default();

    let total = i32::from(planet.factories) + i32::from(planet.mines) + i32::from(planet.defenses);
    if load.buildings > 0 && total > 0 {
        let mut share = |count: i32| -> i32 {
            let product = count * load.buildings;
            let mut killed = product / total;
            let remainder = product % total;
            if remainder > 0
                && i32::from(rng.random(i16::try_from(total).unwrap_or(i16::MAX))) < remainder
            {
                killed += 1;
            }
            killed.min(count)
        };
        out.factories = share(i32::from(planet.factories));
        out.defenses = share(i32::from(planet.defenses));
        // Mines take the remainder rather than their own share.
        out.mines = (load.buildings - (out.factories + out.defenses))
            .max(0)
            .min(i32::from(planet.mines));
    }

    if load.people > 0 || load.floor > 0 || load.smart > 0 {
        let pop = planet.pop;
        if pop > 0 {
            // Smart bombs first, and never quite to zero.
            let smart = (pop * load.smart / 1000).min(pop - 1);
            let left = pop - smart;
            let product = left * load.people;
            let mut killed = product / 1000;
            let remainder = product % 1000;
            // The original's test is `Random(1000) <= remainder`, not `<`.
            if remainder > 0 && i32::from(rng.random(1000)) <= remainder {
                killed += 1;
            }
            killed += smart;
            if load.people > 0 && killed < 1 {
                killed = 1;
            }
            killed = killed.max(load.floor).min(pop);
            out.colonists = killed;
        }
    }

    planet.pop -= out.colonists;
    planet.factories -= i16::try_from(out.factories).unwrap_or(0);
    planet.mines -= i16::try_from(out.mines).unwrap_or(0);
    planet.defenses -= i16::try_from(out.defenses).unwrap_or(0);
    out
}

/// Whether a fleet may bomb the planet it orbits.
///
/// Source: the gates at the head of `DoBombing`. The planet must be **owned by
/// someone else and inhabited**, the bomber must be at war with them, and —
/// the rule that is easy to miss — the planet must have **no starbase**. A
/// starbase stops bombing outright, however small it is.
#[must_use]
pub fn may_bomb(planet: &Planet, bomber: i16, at_war: bool) -> bool {
    match planet.owner {
        None => false,
        Some(owner) => owner != bomber && planet.pop > 0 && at_war && !planet.starbase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::DesignSlot;

    fn bomber(item: u8, count: u8) -> ShipDesign {
        ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            hull_id: 0,
            slots: vec![DesignSlot {
                category: slot::BOMB,
                item,
                count,
            }],
        }
    }

    fn stack(count: i32) -> ShipStack {
        ShipStack {
            design: 0,
            count,
            damaged_pct: 0,
            damage_pct: 0,
        }
    }

    fn colony(pop: i32, factories: i16, mines: i16, defenses: i16) -> Planet {
        let mut p = Planet::unowned(1);
        p.owner = Some(0);
        p.pop = pop;
        p.factories = factories;
        p.mines = mines;
        p.defenses = defenses;
        p
    }

    /// Ordinary bombs sum; smart bombs do not.
    #[test]
    fn smart_bombs_stack_multiplicatively() {
        // Four Cherry Bombs: 4 x 25 tenths of a percent, 4 x 10 buildings.
        let load = bomb_load(&[bomber(4, 4)], &[stack(1)]);
        assert_eq!(load.people, 100);
        assert_eq!(load.buildings, 40);
        assert_eq!(load.smart, 0);
        // Cherry is one of the five basic bombs, so it carries a floor.
        assert_eq!(load.floor, 4 * MINIMUM_KILL_PER_BOMB);

        // Four Smart Bombs at 1.3% each. Summed they would be 5.2%; the
        // product gives 1 - 0.987^4 = 5.0995%, which truncates to 50.
        let smart = bomb_load(&[bomber(10, 4)], &[stack(1)]);
        assert_eq!(smart.people, 0, "a smart bomb is not an ordinary one");
        assert_eq!(smart.buildings, 0);
        assert_eq!(smart.smart, 50);
        assert_eq!(smart.floor, 0);

        // Enough of them do saturate the load at 1000 — a thousand tenths of
        // a percent is the whole population. What keeps a planet alive is the
        // cap in `bomb_planet`, which holds the smart kill one short of the
        // population however large the load.
        let many = bomb_load(&[bomber(14, 100)], &[stack(50)]);
        assert_eq!(many.smart, 1000);
        let mut planet = colony(5_000, 0, 0, 0);
        let mut rng = Rng::randomize(11);
        let out = bomb_planet(&mut planet, many, (1.0, 1.0), &mut rng);
        assert_eq!(out.colonists, 4_999, "one unit always survives smart bombs");
        assert_eq!(planet.pop, 1);
    }

    /// A Retro Bomb is counted apart from the killing ones.
    #[test]
    fn a_retro_bomb_kills_nobody() {
        let load = bomb_load(&[bomber(u8::try_from(RETRO_BOMB).unwrap(), 3)], &[stack(2)]);
        assert_eq!(load.retro, 6);
        assert_eq!(load.people, 0);
        assert_eq!(load.buildings, 0);
        assert!(load.any(), "it still does something");
    }

    /// Defences stop ordinary bombs faster than smart ones, and an undefended
    /// planet stops nothing.
    #[test]
    fn defences_stop_ordinary_bombs_faster_than_smart_ones() {
        let race = Race::humanoid();
        let tech = [26u8; 6]; // everything researched: the best defence
        let bare = colony(1000, 0, 0, 0);
        assert_eq!(pct_survive(&bare, &race, tech), (1.0, 1.0));

        let defended = colony(10_000, 0, 0, 50);
        let (ordinary, smart) = pct_survive(&defended, &race, tech);
        assert!(
            ordinary < smart,
            "smart bombs get through more: {ordinary} vs {smart}"
        );
        assert!(ordinary > 0.0 && smart < 1.0);
    }

    /// Building damage is shared out in proportion, and the mines take the
    /// remainder.
    #[test]
    fn building_damage_is_shared_in_proportion() {
        let mut planet = colony(10_000, 40, 40, 20);
        let mut rng = Rng::randomize(7);
        let load = BombLoad {
            buildings: 20,
            ..BombLoad::default()
        };
        let out = bomb_planet(&mut planet, load, (1.0, 1.0), &mut rng);
        assert_eq!(
            out.factories + out.mines + out.defenses,
            20,
            "every point lands somewhere: {out:?}"
        );
        // 40 of 100 installations are factories, so about 8 of the 20.
        assert!((7..=9).contains(&out.factories), "{out:?}");
        assert_eq!(planet.factories, 40 - i16::try_from(out.factories).unwrap());
    }

    /// A run with ordinary bombs always kills at least the load's floor.
    #[test]
    fn the_floor_holds_against_a_large_population() {
        let mut planet = colony(1_000_000, 0, 0, 0);
        let mut rng = Rng::randomize(3);
        // A single Lady Finger: 0.6% of the population, floor 3.
        let load = bomb_load(&[bomber(0, 1)], &[stack(1)]);
        let out = bomb_planet(&mut planet, load, (1.0, 1.0), &mut rng);
        assert!(out.colonists >= load.floor);
        assert_eq!(out.colonists, 1_000_000 * 6 / 1000);
    }

    /// A starbase refuses bombing outright, and so does an empty planet.
    #[test]
    fn a_starbase_stops_bombing() {
        let mut planet = colony(500, 0, 0, 0);
        assert!(may_bomb(&planet, 1, true));
        assert!(!may_bomb(&planet, 0, true), "not your own planet");
        assert!(!may_bomb(&planet, 1, false), "not at war");
        planet.starbase = true;
        assert!(!may_bomb(&planet, 1, true), "a starbase stops it");
        planet.starbase = false;
        planet.pop = 0;
        assert!(!may_bomb(&planet, 1, true), "nobody to kill");
    }
}
