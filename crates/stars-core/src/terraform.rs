//! Terraforming: how far a planet's environment can be moved, and how many
//! steps that is worth.
//!
//! Source: `FCanTerraformLppl` (`1048:6...`), read through its callers
//! `PctPlanetOptValue` (`1048:6b88`) and the production queue's terraform
//! item. Cross-checked against `MANUAL.PDF` pp. 6-14..6-15.
//!
//! Two rules carry everything:
//!
//! 1. **The reach is measured from the planet's original environment**, not its
//!    current one. Terraforming a planet ten clicks and then researching a
//!    wider module does not let you go ten further; the band is always
//!    `original ± reach`, clamped to `1..=99`. This is why the planet record
//!    keeps `rgEnvVarOrig` alongside the current values.
//! 2. The reach for each variable is the **best single module** that applies to
//!    it: the widest Total Terraform the player can build, or the widest
//!    variable-specific one, whichever goes further.
//!
//! "Can build" is not a pure tech test. `FLookupPart`'s `hstTerra` arm gates
//! the eight Total Terraform modules behind the **Total Terraforming** lesser
//! racial trait:
//!
//! ```text
//! else if (HVar1 == hstTerra) {
//!   if (0x13 < iItem) return 0;
//!   ppart->pcom = (COMPART *)(iItem * 0x36 + 0x19e2);
//!   if (idPlayer != -1 && iItem < 8 &&
//!       GetRaceGrbit(rgplr + idPlayer, ibitRaceTT) == 0)
//!     return -1;
//! }
//! ```
//!
//! This matters because Total Terraform 3 costs no research at all. Without the
//! gate every race would start the game able to move all three variables three
//! clicks, when in fact a race without the trait can do nothing until it has
//! researched a variable-specific module (the cheapest, Gravity Terraform 3,
//! needs Propulsion 1 and Biotechnology 1).

use crate::components::TERRAFORMING;
use crate::planet::Planet;
use crate::race::{lrt, Race};

/// The three environment variables, in their stored order.
pub const VARIABLES: usize = 3;

/// The lowest and highest a terraformed variable may reach
/// (`1048:6...` clamps to 1 and 99).
pub const ENV_MIN: i8 = 1;
/// See [`ENV_MIN`].
pub const ENV_MAX: i8 = 99;

/// Index of the first Total Terraform module in [`TERRAFORMING`].
///
/// The table is laid out as the binary indexes it: eight Total Terraform
/// modules, then four each of Gravity, Temperature and Radiation.
const TOTAL_FIRST: usize = 0;
/// How many Total Terraform modules there are.
const TOTAL_COUNT: usize = 8;
/// How many modules each single variable has.
const SINGLE_COUNT: usize = 4;

/// How far each environment variable can be moved, in clicks.
///
/// A zero means the player has no module that reaches that variable, or the
/// race is immune to it and has nothing to gain.
#[must_use]
pub fn terraform_reach(race: &Race, tech: [u8; 6]) -> [i8; VARIABLES] {
    let buildable = |part: &crate::components::Special| -> bool {
        part.tech
            .iter()
            .zip(tech.iter())
            .all(|(need, have)| i32::from(*need) <= i32::from(*have))
    };

    // The widest Total Terraform module reaches every variable — but only a
    // race with the Total Terraforming trait may build one at all
    // (`FLookupPart`, `hstTerra` arm).
    let total = if race.has_lrt(lrt::TT) {
        TERRAFORMING
            .iter()
            .skip(TOTAL_FIRST)
            .take(TOTAL_COUNT)
            .filter(|p| buildable(p))
            .map(|p| p.ability)
            .max()
            .unwrap_or(0)
    } else {
        0
    };

    let mut reach = [0i8; VARIABLES];
    for (v, slot) in reach.iter_mut().enumerate() {
        // A race immune to a variable never terraforms it: every value is
        // already ideal.
        if race.is_immune(v) {
            continue;
        }
        let start = TOTAL_COUNT + v * SINGLE_COUNT;
        let single = TERRAFORMING
            .iter()
            .skip(start)
            .take(SINGLE_COUNT)
            .filter(|p| buildable(p))
            .map(|p| p.ability)
            .max()
            .unwrap_or(0);
        *slot = i8::try_from(total.max(single)).unwrap_or(0);
    }
    reach
}

/// The band each variable can be brought to: `(lowest, highest)`.
///
/// Measured from the planet's **original** environment and clamped to
/// [`ENV_MIN`]`..=`[`ENV_MAX`].
#[must_use]
pub fn reachable_band(planet: &Planet, race: &Race, tech: [u8; 6]) -> [(i8, i8); VARIABLES] {
    let reach = terraform_reach(race, tech);
    let mut out = [(0i8, 0i8); VARIABLES];
    for (v, slot) in out.iter_mut().enumerate() {
        let origin = planet.env_orig.unwrap_or(planet.env)[v];
        let r = reach[v];
        *slot = (
            origin.saturating_sub(r).max(ENV_MIN),
            origin.saturating_add(r).min(ENV_MAX),
        );
    }
    out
}

/// The environment the planet would have if terraformed as far as it can be.
///
/// Each variable is moved toward the race's ideal, stopping at the ideal or at
/// the edge of the reachable band, whichever comes first. A variable already at
/// its ideal, or one the race is immune to, is left alone.
///
/// This is what `PctPlanetOptValue` measures habitability against — see
/// [`crate::ai::colonise::pct_planet_opt_value`].
#[must_use]
pub fn optimal_env(planet: &Planet, race: &Race, tech: [u8; 6]) -> [i8; VARIABLES] {
    let band = reachable_band(planet, race, tech);
    let mut env = planet.env;
    for (v, slot) in env.iter_mut().enumerate() {
        if race.is_immune(v) {
            continue;
        }
        let ideal = race.env_center[v];
        let (lo, hi) = band[v];
        if *slot < ideal {
            *slot = (*slot).max(ideal.min(hi));
        } else if *slot > ideal {
            *slot = (*slot).min(ideal.max(lo));
        }
    }
    env
}

/// How many one-percent terraforming steps the planet still has available.
///
/// `MANUAL.PDF` p. 6-14 states the rule directly: "If you can improve Gravity
/// by 3%, Temperature by 5% and Radiation by 2% the Production dialog will let
/// you add 10% Terraforming to the queue. Each 1% Terraforming task executed
/// will modify one of the environmental factors by 1%."
///
/// So it is the sum, over the three variables, of how far each can still be
/// moved toward the race's ideal. This is the count the AI's terraform decision
/// caps at four — see [`crate::ai::production::queue_ai_terraforming`].
///
/// # Where the count is used
///
/// `InitProduction` (`10d0:015e`) puts this figure in the production
/// catalogue's terraform entry, and `FQueueAiTerraforming` (`1090:8d28`) queues
/// `min(count, 4)` of it. The binary computes it in `IpctCanTerraformLppl`
/// (`1048:7f56`), which sums, over the three variables, `env - low` and
/// `high - env` for whichever bounds `FCanTerraformLppl` left usable. Called
/// with its fifth argument set — `PUSH 0x1` at `1048:7f5f`, which the
/// decompiler loses — that routine keeps only the direction moving toward the
/// race's ideal and clamps it there, so the sum is the improvement still
/// available. That is this function.
///
/// # How well this matches
///
/// Scored against the AI's recorded auto-terraform orders across both AI games,
/// `min(terraform_steps, 4)` matches **196 of 196 fresh orders (100%)**.
///
/// Recovering the decision from the file takes two corrections, both instances
/// of the same rule: **a queue entry is a running balance, not a record of what
/// was chosen.**
///
/// 1. An entry counts down over following turns as production builds it, so
///    only a planet that had no terraform order the turn before is a decision
///    being made. Scoring every planet-turn instead gives 24%.
/// 2. `Produce` runs later in the *same* turn the AI queues the item, so even a
///    fresh order is already short by what the planet built that year. Those
///    clicks are visible as environment movement, so the decision is
///    `recorded + |env(Y) - env(Y-1)|` summed over the three variables.
///    Omitting this second correction gives 70%, with a residual that looks
///    like over-prediction and is not.
///
#[must_use]
pub fn terraform_steps(planet: &Planet, race: &Race, tech: [u8; 6]) -> i32 {
    let target = optimal_env(planet, race, tech);
    planet
        .env
        .iter()
        .zip(target.iter())
        .map(|(now, want)| i32::from((i16::from(*want) - i16::from(*now)).abs()))
        .sum()
}

/// Which environment variable the next terraforming step should move.
///
/// Source: `IBestTerraform` (`1048:5dd2`), called from `FBuildObject` when a
/// terraforming item is built.
///
/// For each variable it moves that variable all the way to its reachable
/// bound, measures how much the planet's habitability changes, and scores the
/// variable by the **gain per click**:
///
/// ```text
/// score[v] = |desirability(v at its bound) - desirability(now)| * 100 / clicks + 1
/// ```
///
/// The highest score wins, and a tie goes to the lowest index. The trailing
/// `+ 1` matters: a variable that can move but gains nothing still scores 1 and
/// so beats one that cannot move at all, which scores 0.
///
/// Returns the variable index, or `None` when nothing can usefully move.
///
/// # This is not what the manual says
///
/// `MANUAL.PDF` p. 6-15 describes the task as always working on "the factor
/// that is the furthest out of range". That is not what the code does, and
/// implementing the manual's rule instead cost five points of whole-turn
/// population accuracy. Distance from the ideal does not decide it; efficiency
/// does — a variable two clicks from a large habitability gain beats one ten
/// clicks from a small one.
#[must_use]
pub fn best_terraform_factor(planet: &Planet, race: &Race, tech: [u8; 6]) -> Option<usize> {
    let target = optimal_env(planet, race, tech);
    let base = i32::from(crate::hab::pct_planet_desirability(planet, race));

    let mut best: Option<(i32, usize)> = None;
    for (v, want) in target.iter().enumerate() {
        let clicks = i32::from((i16::from(*want) - i16::from(planet.env[v])).abs());
        if clicks == 0 {
            continue; // scores zero: it cannot move
        }
        let mut probe = planet.clone();
        probe.env[v] = *want;
        let moved = i32::from(crate::hab::pct_planet_desirability(&probe, race));
        let score = (moved - base).abs() * 100 / clicks + 1;
        // Strictly greater, so a tie keeps the lower index.
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, v));
        }
    }
    best.map(|(_, v)| v)
}

/// Move a planet one click along the variable [`best_terraform_factor`] picks.
///
/// Returns whether anything moved.
pub fn terraform_one_step(planet: &mut Planet, race: &Race, tech: [u8; 6]) -> bool {
    let Some(v) = best_terraform_factor(planet, race, tech) else {
        return false;
    };
    let target = optimal_env(planet, race, tech);
    match planet.env[v].cmp(&target[v]) {
        std::cmp::Ordering::Less => planet.env[v] += 1,
        std::cmp::Ordering::Greater => planet.env[v] -= 1,
        std::cmp::Ordering::Equal => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Humanoid that has the Total Terraforming trait, and so can build the
    /// free Total Terraform 3 module the other tests assume.
    fn tt_race() -> Race {
        let mut race = Race::humanoid();
        race.lrt_bits |= 1 << lrt::TT;
        race
    }

    fn planet_at(env: [i8; 3]) -> Planet {
        let mut p = Planet::unowned(0);
        p.env = env;
        p
    }

    /// Total Terraform 3 costs nothing to research, so a race with the Total
    /// Terraforming trait can move all three variables from turn one — and a
    /// race without it can do nothing at all until it researches a
    /// variable-specific module.
    #[test]
    fn the_free_module_needs_the_total_terraforming_trait() {
        let mut race = Race::humanoid();
        assert_eq!(terraform_reach(&race, [0; 6]), [0, 0, 0]);

        race.lrt_bits |= 1 << lrt::TT;
        assert_eq!(terraform_reach(&race, [0; 6]), [3, 3, 3]);
    }

    /// Biotech widens the Total Terraform module, and a variable-specific
    /// module can beat it.
    #[test]
    fn better_technology_reaches_further() {
        let mut race = Race::humanoid();
        race.lrt_bits |= 1 << lrt::TT;
        let mut tech = [0u8; 6];
        tech[5] = 3; // Total Terraform 5
        assert_eq!(terraform_reach(&race, tech), [5, 5, 5]);

        // Gravity Terraform 7 needs propulsion and biotech; it beats Total 5.
        let wide = terraform_reach(&race, [0, 0, 10, 0, 0, 10]);
        assert!(wide[0] >= 7, "gravity should reach at least 7: {wide:?}");
    }

    /// The band is measured from the planet's original environment, so
    /// terraforming does not compound.
    #[test]
    fn the_band_is_measured_from_the_original_environment() {
        let race = tt_race();
        let mut planet = planet_at([53, 50, 50]);
        planet.env_orig = Some([50, 50, 50]);

        let band = reachable_band(&planet, &race, [0; 6]);
        // Total Terraform 3 from an origin of 50, not from the current 53.
        assert_eq!(band[0], (47, 53));
    }

    /// Steps are the total improvement available across the three variables,
    /// which is the manual's worked example.
    #[test]
    fn steps_sum_the_improvement_available() {
        let race = tt_race(); // ideal 50/50/50
        let planet = planet_at([47, 45, 50]);
        // Total Terraform 3 reaches 3 on each: gravity 47->50 is 3, temperature
        // 45->48 is 3 (capped by reach, not by the ideal), radiation is done.
        assert_eq!(terraform_steps(&planet, &race, [0; 6]), 6);
    }

    /// A planet already ideal has nothing to do.
    #[test]
    fn an_ideal_planet_needs_no_terraforming() {
        let race = Race::humanoid();
        let planet = planet_at(race.env_center);
        assert_eq!(terraform_steps(&planet, &race, [0; 6]), 0);
        assert_eq!(optimal_env(&planet, &race, [0; 6]), race.env_center);
    }

    /// Terraforming never overshoots the ideal.
    #[test]
    fn terraforming_stops_at_the_ideal() {
        let race = tt_race();
        let planet = planet_at([49, 51, 50]);
        assert_eq!(optimal_env(&planet, &race, [0; 6]), [50, 50, 50]);
        assert_eq!(terraform_steps(&planet, &race, [0; 6]), 2);
    }
}
