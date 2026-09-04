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
    let targets = terraform_targets(planet, race, tech, Intent::Help);
    let mut env = planet.env;
    for (v, slot) in env.iter_mut().enumerate() {
        if let Some(target) = targets[v] {
            *slot = target;
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

/// Whether a terraforming act is meant to help the planet's owner or harm them.
///
/// This is `FCanTerraformLppl`'s fifth parameter, `fHelp` in the NB09 symbols.
/// The decompiler loses it — it merges the flag with the preceding output array
/// — so every call site here is confirmed from the disassembly.
///
/// | caller | flag | site |
/// |--------|------|------|
/// | [`terraform_steps`] via `IpctCanTerraformLppl` | help | `PUSH 0x1` at `1048:7f5f` |
/// | [`auto_terraform`] via `AutoTerraform` | help | `PUSH 0x1` at `10b8:4b5f` |
/// | [`remote_terraform`] via `RemoteTerraforming` | either | `PUSH [BP-0x4]` at `10b8:4df8` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Move the environment toward the owner's ideal, stopping at the ideal or
    /// the edge of the reachable band.
    Help,
    /// Move it **away** from the owner's ideal, as far as the band allows.
    /// Only remote terraforming by an unfriendly fleet does this.
    Harm,
}

/// The value each variable would be driven to, or `None` where nothing moves.
///
/// This is the pair of bounds `FCanTerraformLppl` leaves usable, collapsed to
/// the single surviving one. With [`Intent::Help`] it is [`optimal_env`]; with
/// [`Intent::Harm`] it is the bound *furthest* from the ideal, and only when
/// that is further than where the planet already sits:
///
/// ```text
/// cur = |env - ideal|
/// dlo = low  usable ? |low  - ideal| : 0
/// dhi = high usable ? |high - ideal| : 0
/// if cur < dlo or cur < dhi   keep whichever of low/high is further out
/// else                        neither: the planet is already at its worst
/// ```
#[must_use]
pub fn terraform_targets(
    planet: &Planet,
    race: &Race,
    tech: [u8; 6],
    intent: Intent,
) -> [Option<i8>; VARIABLES] {
    let band = reachable_band(planet, race, tech);
    let reach = terraform_reach(race, tech);
    let mut out = [None; VARIABLES];
    for (v, slot) in out.iter_mut().enumerate() {
        if reach[v] == 0 || race.is_immune(v) {
            continue;
        }
        let env = planet.env[v];
        let (lo, hi) = band[v];
        // A bound is usable only if it lies strictly beyond the current value.
        let low = (lo < env).then_some(lo);
        let high = (env < hi).then_some(hi);
        let ideal = race.env_center[v];
        *slot = match intent {
            Intent::Help => match ideal.cmp(&env) {
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Less => low.map(|l| l.max(ideal)),
                std::cmp::Ordering::Greater => high.map(|h| h.min(ideal)),
            },
            Intent::Harm => {
                let dist = |x: i8| i32::from((i16::from(x) - i16::from(ideal)).abs());
                let cur = dist(env);
                let dlo = low.map_or(0, dist);
                let dhi = high.map_or(0, dist);
                if cur < dlo || cur < dhi {
                    // Keep the bound further from the ideal.
                    if dlo < dhi {
                        high
                    } else {
                        low
                    }
                } else {
                    None
                }
            }
        };
    }
    out
}

/// Which variable to move next, and which way — the game's `IBestTerraform`.
///
/// Source: `IBestTerraform` (`1048:5dd2`). It returns a signed one-based index,
/// `+(v + 1)` when the move is upward and `-(v + 1)` when downward, or `0` when
/// nothing should move. This returns the same decision as `(v, step)` with
/// `step` of `+1` or `-1`.
///
/// The score is unchanged by the intent, because it takes the **absolute**
/// change in habitability:
///
/// ```text
/// score[v] = |desirability(v at its target) - desirability(now)| * 100 / clicks + 1
/// ```
///
/// so a hostile adjuster picks the variable that does the most damage per click
/// by the same arithmetic a friendly one uses to do the most good. Ties keep the
/// lowest index (`if (score[best] < score[v]) best = v`).
#[must_use]
pub fn best_terraform_step(
    planet: &Planet,
    race: &Race,
    tech: [u8; 6],
    intent: Intent,
) -> Option<(usize, i8)> {
    let targets = terraform_targets(planet, race, tech, intent);
    let base = i32::from(crate::hab::pct_planet_desirability(planet, race));

    let mut best: Option<(i32, usize, i8)> = None;
    for (v, target) in targets.iter().enumerate() {
        let Some(target) = *target else { continue };
        let clicks = i32::from((i16::from(target) - i16::from(planet.env[v])).abs());
        if clicks == 0 {
            continue;
        }
        let mut probe = planet.clone();
        probe.env[v] = target;
        let moved = i32::from(crate::hab::pct_planet_desirability(&probe, race));
        let score = (moved - base).abs() * 100 / clicks + 1;
        let step = if target > planet.env[v] { 1 } else { -1 };
        if best.is_none_or(|(b, _, _)| score > b) {
            best = Some((score, v, step));
        }
    }
    best.map(|(_, v, step)| (v, step))
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
    best_terraform_step(planet, race, tech, Intent::Help).map(|(v, _)| v)
}

/// The primary racial trait `AutoTerraform` serves: Claim Adjuster.
///
/// `AutoTerraform` (`10b8:48f6`) opens by scanning every player for
/// `GetRaceStat(plr, rsMajorAdv) == 3` and does nothing at all if none matches.
pub const AUTO_TERRAFORM_PRT: crate::race::Prt = crate::race::Prt::Ca;

/// Population, in units of 100 colonists, at or above which the Claim Adjuster
/// drift is certain rather than a roll (`10b8:4a99` compares against `0x3e8`).
pub const DRIFT_CERTAIN_POP: i32 = 1000;

/// One turn of free Claim Adjuster terraforming.
///
/// Source: `AutoTerraform` (`10b8:48f6`), step 16 of the turn pipeline. It runs
/// only for players whose primary trait is [`AUTO_TERRAFORM_PRT`], and does two
/// separate things to each planet such a player owns.
///
/// **The drift.** One environment variable, chosen with `Random(3)`, is nudged a
/// single click toward the race's ideal — but the click is applied to
/// `rgEnvVarOrig`, not to the current environment. That is the Claim Adjuster's
/// permanent improvement: it moves the planet's *baseline*, and so shifts the
/// whole reachable band, rather than being overwritten by the terraforming that
/// follows. The gates, in the order the routine rolls them:
///
/// ```text
/// v = Random(3)
/// skip unless the race cares about v, and orig[v] is not already the ideal
/// skip unless Random(10) == 0
/// skip unless pop >= 1000, or Random(1000) < pop
/// orig[v] += 1 toward the ideal
/// ```
///
/// **The terraforming.** The planet's environment is then set straight to the
/// edge of its reachable band — not one click, the whole way:
///
/// ```c
/// if (FCanTerraformLppl(planet, low, high, items, 1)) {
///   for (v = 0; v < 3; v++)
///     if (low[v] == -1) { if (high[v] != -1) env[v] = high[v]; }
///     else                                   env[v] = low[v];
/// }
/// ```
///
/// The fifth argument is `PUSH 0x1` at `10b8:4b5f` — the same flag
/// `IpctCanTerraformLppl` passes — so only the direction moving toward the
/// ideal survives and each bound is clamped at the ideal. That makes the result
/// exactly [`optimal_env`].
///
/// This is why a Claim Adjuster never queues terraforming: its planets are
/// already at their optimum every turn, so `IpctCanTerraformLppl` returns zero
/// and `InitProduction` puts no terraform item in the production catalogue at
/// all. See `docs/formulas/terraforming.md`.
///
/// Returns whether anything moved. The generator is stepped exactly as the
/// original steps it, so a future RNG-exact replay lines up.
pub fn auto_terraform(
    planet: &mut Planet,
    race: &Race,
    tech: [u8; VARIABLES + 3],
    rng: &mut crate::rng::Rng,
) -> bool {
    if race.prt() != Some(AUTO_TERRAFORM_PRT) {
        return false;
    }
    let mut moved = false;

    // The drift, on the original environment.
    let v = usize::from(rng.random(3).unsigned_abs());
    if let Some(orig) = planet.env_orig.as_mut() {
        // An immune variable stores -1 as its ideal, which the routine skips.
        // Written as one chain so the generator is stepped in the original's
        // order: Random(10) is only rolled once the variable qualifies, and
        // Random(1000) only when the population does not already settle it.
        if v < VARIABLES
            && !race.is_immune(v)
            && orig[v] != race.env_center[v]
            && rng.random(10) == 0
            && (planet.pop >= DRIFT_CERTAIN_POP || i32::from(rng.random(1000)) < planet.pop)
        {
            if race.env_center[v] < orig[v] {
                orig[v] -= 1;
            } else {
                orig[v] += 1;
            }
            moved = true;
        }
    }

    // The terraforming, all the way to the reachable bound.
    let target = optimal_env(planet, race, tech);
    if target != planet.env {
        planet.env = target;
        moved = true;
    }
    moved
}

/// Index of the Orbital Adjuster in [`crate::components::MINING`].
///
/// `PctTerraFromLpfl` (`1080:275c`) counts design slots whose category is
/// `hstMining` (`0x80`) and whose item is `7`. `FLookupPart`'s `hstMining` arm
/// gates that item on `GetRaceStat(plr, rsMajorAdv) != 3`, so like
/// [`auto_terraform`] this is Claim Adjuster equipment.
pub const ORBITAL_ADJUSTER: u8 = 7;

/// How many clicks of remote terraforming a fleet can apply in one turn.
///
/// Source: `PctTerraFromLpfl` (`1080:275c`), which walks the fleet's sixteen
/// design slots and, for each with ships aboard, sums the Orbital Adjusters
/// fitted to that design and multiplies by the number of ships:
///
/// ```text
/// total = sum over designs of (adjusters per ship * ships of that design)
/// ```
///
/// The part carries no ability value — its entry in `MINING` has `ability: 0` —
/// so each Orbital Adjuster is worth exactly one click.
#[must_use]
pub fn orbital_adjusters(stacks: &[(&crate::design::ShipDesign, i32)]) -> i32 {
    stacks
        .iter()
        .map(|(design, count)| {
            let per_ship: i32 = design
                .slots
                .iter()
                .filter(|s| s.is(crate::components::slot::MINING) && s.item == ORBITAL_ADJUSTER)
                .map(|s| i32::from(s.count))
                .sum();
            per_ship * count.max(&0)
        })
        .sum()
}

/// The race `IBestRemoteTerra` builds to decide a remote terraforming step.
///
/// Source: `IBestRemoteTerra` (`10b8:8b70`). It copies the **fleet owner's**
/// whole `PLAYER` record over the planet owner's, then puts back just the
/// planet owner's three habitability arrays (`+0x10` centre, `+0x13` and
/// `+0x16`), calls the ordinary `IBestTerraform`, and restores the record
/// afterwards.
///
/// The effect is worth stating plainly: **the fleet owner's technology decides
/// how far the planet can be moved, and the planet owner's race decides which
/// way is better.** A friendly adjuster therefore improves an ally's planet for
/// *that ally's* biology using its own modules, and a hostile one makes the
/// planet worse by the same measure.
#[must_use]
pub fn remote_race(fleet_owner: &Race, planet_owner: &Race) -> Race {
    let mut race = fleet_owner.clone();
    race.env_center = planet_owner.env_center;
    race.env_min = planet_owner.env_min;
    race.env_max = planet_owner.env_max;
    race
}

/// Apply up to `clicks` steps of remote terraforming to a planet.
///
/// Source: the inner loop of `RemoteTerraforming` (`10b8:4c56`). Each click
/// re-runs [`best_terraform_step`], moves that variable one, and clamps the
/// result to `1..=99`; it stops early when nothing more should move.
///
/// `race` must be the blend [`remote_race`] produces, and `tech` the fleet
/// owner's levels. Returns how many clicks were actually applied.
///
/// Note the environment moves but `env_orig` does not, so remote terraforming
/// is bounded by the same band as any other terraforming — it cannot walk a
/// planet away indefinitely.
pub fn remote_terraform(
    planet: &mut Planet,
    race: &Race,
    tech: [u8; 6],
    clicks: i32,
    intent: Intent,
) -> i32 {
    let mut done = 0;
    for _ in 0..clicks {
        let Some((v, step)) = best_terraform_step(planet, race, tech, intent) else {
            break;
        };
        planet.env[v] = (planet.env[v] + step).clamp(ENV_MIN, ENV_MAX);
        done += 1;
    }
    done
}

/// Whether a fleet may remote-terraform the planet it orbits, and how.
///
/// Source: `RemoteTerraforming` (`10b8:4c56`), gates at `10b8:4d35`-`4db5`.
///
/// `fHelp` is set when the fleet's owner owns the planet, or when the **fleet
/// owner's** relations table marks the planet's owner a friend (value `1` — see
/// `docs/formats/player.md`). Otherwise the act is hostile, and is refused
/// outright if the planet has a starbase:
///
/// ```text
/// fHelp = (fleet.owner == planet.owner) or relation[fleet.owner][planet.owner] == 1
/// proceed if fHelp or not planet.starbase
/// ```
///
/// Returns `None` when the fleet may not act at all.
#[must_use]
pub fn remote_intent(
    same_owner: bool,
    friendly: bool,
    planet_has_starbase: bool,
) -> Option<Intent> {
    if same_owner || friendly {
        Some(Intent::Help)
    } else if planet_has_starbase {
        None
    } else {
        Some(Intent::Harm)
    }
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

    /// A Claim Adjuster's planets are terraformed to their optimum for free,
    /// all the way to the band edge rather than one click.
    #[test]
    fn a_claim_adjuster_terraforms_to_the_band_edge_for_free() {
        let mut race = tt_race();
        race.attrs[crate::race::RaceStat::MajorAdv as usize] = AUTO_TERRAFORM_PRT as i16;

        let mut planet = planet_at([40, 50, 50]);
        planet.env_orig = Some([40, 50, 50]);
        let mut rng = crate::rng::Rng::randomize(1);

        assert!(auto_terraform(&mut planet, &race, [0; 6], &mut rng));
        // Total Terraform 3 from an origin of 40 reaches 43, short of the
        // ideal of 50, and the whole three clicks are applied at once.
        assert_eq!(planet.env[0], 43);
    }

    /// Every other race gets nothing from it, and the generator is not stepped.
    #[test]
    fn auto_terraforming_is_claim_adjuster_only() {
        let race = tt_race(); // JOAT
        let mut planet = planet_at([40, 50, 50]);
        planet.env_orig = Some([40, 50, 50]);
        let mut rng = crate::rng::Rng::randomize(1);
        let untouched = rng.clone();

        assert!(!auto_terraform(&mut planet, &race, [0; 6], &mut rng));
        assert_eq!(planet.env, [40, 50, 50]);
        assert_eq!(rng, untouched, "a non-CA race must not consume a draw");
    }

    /// A hostile adjuster drives the environment away from the owner's ideal,
    /// where a friendly one drives it toward.
    #[test]
    fn a_hostile_adjuster_terraforms_the_wrong_way() {
        let race = tt_race(); // ideal 50/50/50
        let mut planet = planet_at([50, 50, 50]);
        planet.env_orig = Some([50, 50, 50]);

        // Nothing to gain: helping a planet already at its ideal does nothing.
        assert_eq!(
            terraform_targets(&planet, &race, [0; 6], Intent::Help),
            [None, None, None]
        );
        // Harming it can push every variable three clicks off, and the band is
        // symmetric so the low bound wins the tie.
        assert_eq!(
            terraform_targets(&planet, &race, [0; 6], Intent::Harm),
            [Some(47), Some(47), Some(47)]
        );

        let done = remote_terraform(&mut planet, &race, [0; 6], 10, Intent::Harm);
        assert_eq!(done, 9, "three clicks on each of three variables");
        assert_eq!(planet.env, [47, 47, 47]);
        // And it cannot go further: the band is measured from env_orig.
        assert_eq!(
            remote_terraform(&mut planet, &race, [0; 6], 10, Intent::Harm),
            0
        );
    }

    /// The gate: a starbase refuses a hostile adjuster but not a friendly one.
    #[test]
    fn a_starbase_blocks_only_hostile_remote_terraforming() {
        assert_eq!(remote_intent(true, false, true), Some(Intent::Help));
        assert_eq!(remote_intent(false, true, true), Some(Intent::Help));
        assert_eq!(remote_intent(false, false, false), Some(Intent::Harm));
        assert_eq!(remote_intent(false, false, true), None);
    }

    /// The blend: the fleet owner's technology, the planet owner's biology.
    #[test]
    fn remote_terraforming_blends_the_two_races() {
        let mut fleet_owner = Race::humanoid();
        fleet_owner.lrt_bits |= 1 << lrt::TT;
        let mut planet_owner = Race::humanoid();
        planet_owner.env_center = [20, 20, 20];
        planet_owner.env_min = [1, 1, 1];
        planet_owner.env_max = [40, 40, 40];

        let blend = remote_race(&fleet_owner, &planet_owner);
        // The reach comes from the fleet owner, which has Total Terraforming.
        assert_eq!(terraform_reach(&blend, [0; 6]), [3, 3, 3]);
        // The ideal comes from the planet owner.
        assert_eq!(blend.env_center, [20, 20, 20]);

        // A planet at 30 is helped downward, toward 20, not toward 50.
        let mut planet = planet_at([30, 30, 30]);
        planet.env_orig = Some([30, 30, 30]);
        assert_eq!(
            terraform_targets(&planet, &blend, [0; 6], Intent::Help),
            [Some(27), Some(27), Some(27)]
        );
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
