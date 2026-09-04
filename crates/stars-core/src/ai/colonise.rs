//! Which planet a computer player sends its colonists to.
//!
//! Source: `IdNearestColonizablePlanet` (`1090:0e3e`), with the move itself in
//! `FColonizeAiFleet` (`1090:0c78`).
//!
//! The routine works in two passes. First it writes a one-byte mark for every
//! planet into its scratch array, saying what the planet is to this player.
//! Then it walks the marks and returns the **nearest** planet still marked
//! [`Mark::Colonisable`], measured by squared distance from the colony fleet.
//! Nothing is weighed but distance: a merely adequate planet close by beats an
//! ideal one further out.

use crate::planet::Planet;

use super::AiPersonality;

/// What a planet is to the AI doing the marking.
///
/// The values are the bytes `IdNearestColonizablePlanet` writes into
/// `vlpbAiPlanet[id * 16 + 15]`. Only [`Mark::Colonisable`] is a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Mark {
    /// Unowned, and worth settling. The only value the search accepts.
    Colonisable = 0x00,
    /// Already ours.
    Ours = 0x01,
    /// Held by somebody else.
    Foreign = 0x02,
    /// Unowned, but another of our colony fleets is already on its way.
    Claimed = 0x04,
    /// Unowned, and not worth settling — `PctPlanetOptValue` is negative.
    Hostile = 0x08,
    /// The default for a planet this player knows nothing about. Robotoid and
    /// Macinti instead default such planets to [`Mark::Colonisable`], so they
    /// will strike out at unexplored space where the others will not.
    Unknown = 0x10,
}

/// Whether this personality settles anything it can reach.
///
/// Robotoid and Macinti skip the habitability test and treat unexplored
/// planets as candidates (`1090:0e5c` and `1090:0ea6` both test the mode word
/// against 0 and 5). For Macinti that follows from what it is: the Macinti
/// players in the corpus are Alternate Reality races, which live on their
/// starbases and barely care what is underneath.
#[must_use]
pub fn settles_anywhere(personality: Option<AiPersonality>) -> bool {
    matches!(
        personality,
        Some(AiPersonality::Robotoid) | Some(AiPersonality::Macinti)
    )
}

/// Mark one planet the AI knows about.
///
/// `opt_value` is [`super::colonise`]'s reading of `PctPlanetOptValue` — the
/// habitability the planet would have **after** terraforming, not as it stands.
/// See [`pct_planet_opt_value`].
#[must_use]
pub fn mark_planet(
    planet: &Planet,
    us: i16,
    personality: Option<AiPersonality>,
    opt_value: i16,
) -> Mark {
    match planet.owner {
        Some(owner) if owner == us => Mark::Ours,
        Some(_) => Mark::Foreign,
        None => {
            if settles_anywhere(personality) || opt_value >= 0 {
                Mark::Colonisable
            } else {
                Mark::Hostile
            }
        }
    }
}

/// The default mark for a planet this player has never seen.
#[must_use]
pub fn unknown_mark(personality: Option<AiPersonality>) -> Mark {
    if settles_anywhere(personality) {
        Mark::Colonisable
    } else {
        Mark::Unknown
    }
}

/// The nearest planet still marked colonisable, by squared distance.
///
/// `candidates` pairs each planet id with its position. The comparison is on
/// squared distance and nothing else — `IdNearestColonizablePlanet` keeps a
/// running best and takes the first planet that beats it, so ties go to the
/// **lowest planet id**, which is the order the original walks its array in.
#[must_use]
pub fn nearest_colonisable(
    from: (i32, i32),
    candidates: &[(i16, (i32, i32), Mark)],
) -> Option<i16> {
    let mut best: Option<(i64, i16)> = None;
    for &(id, (x, y), mark) in candidates {
        if mark != Mark::Colonisable {
            continue;
        }
        let dx = i64::from(x - from.0);
        let dy = i64::from(y - from.1);
        let d2 = dx * dx + dy * dy;
        if best.is_none_or(|(bd, _)| d2 < bd) {
            best = Some((d2, id));
        }
    }
    best.map(|(_, id)| id)
}

/// How habitable a planet would be once terraformed as far as the race's
/// technology allows.
///
/// Source: `PctPlanetOptValue` (`1048:6b88`). It asks `FCanTerraformLppl` what
/// each environment variable could be moved to, temporarily writes those values
/// into the planet, takes [`crate::hab::pct_planet_desirability`], and then
/// **puts the original values back**. When nothing can be terraformed it is
/// just the plain desirability.
///
/// This distinction matters: it is `PctPlanetOptValue`, not the plain
/// desirability, that decides whether the AI will settle a planet. A world
/// that would kill colonists today but can be terraformed into something
/// habitable is a legitimate target, which is why computer players in the
/// corpus settle planets whose *current* value is negative.
///
/// `reachable` is what `FCanTerraformLppl` returns: the value each of gravity,
/// temperature and radiation could be brought to. Terraforming is not modelled
/// yet, so the caller supplies it; passing the planet's current environment
/// gives the untreated value.
#[must_use]
pub fn pct_planet_opt_value(planet: &Planet, race: &crate::race::Race, reachable: [i8; 3]) -> i16 {
    let mut probe = planet.clone();
    probe.env = reachable;
    crate::hab::pct_planet_desirability(&probe, race)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race::Race;

    fn unowned(id: i16) -> Planet {
        Planet::unowned(id)
    }

    #[test]
    fn marks_follow_ownership() {
        let mut ours = unowned(1);
        ours.owner = Some(3);
        assert_eq!(mark_planet(&ours, 3, None, 50), Mark::Ours);

        let mut theirs = unowned(2);
        theirs.owner = Some(4);
        assert_eq!(mark_planet(&theirs, 3, None, 50), Mark::Foreign);

        assert_eq!(mark_planet(&unowned(5), 3, None, 50), Mark::Colonisable);
        assert_eq!(mark_planet(&unowned(5), 3, None, -1), Mark::Hostile);
    }

    /// Robotoid and Macinti settle a planet the others would refuse, and will
    /// head for one they have never seen.
    #[test]
    fn two_personalities_ignore_habitability() {
        for p in [AiPersonality::Robotoid, AiPersonality::Macinti] {
            assert_eq!(mark_planet(&unowned(1), 0, Some(p), -30), Mark::Colonisable);
            assert_eq!(unknown_mark(Some(p)), Mark::Colonisable);
        }
        for p in [
            AiPersonality::TurinDrone,
            AiPersonality::Automitron,
            AiPersonality::Rototill,
            AiPersonality::Cyber,
            AiPersonality::Maid,
        ] {
            assert_eq!(mark_planet(&unowned(1), 0, Some(p), -30), Mark::Hostile);
            assert_eq!(unknown_mark(Some(p)), Mark::Unknown);
        }
    }

    /// Distance decides, and only distance.
    #[test]
    fn the_nearest_colonisable_planet_wins() {
        let cands = [
            (7i16, (100, 0), Mark::Colonisable),
            (8, (10, 0), Mark::Hostile), // closer, but refused
            (9, (30, 0), Mark::Colonisable),
            (10, (5, 0), Mark::Ours),
            (11, (20, 0), Mark::Claimed), // another fleet has it
        ];
        assert_eq!(nearest_colonisable((0, 0), &cands), Some(9));

        // With nothing to settle there is no answer.
        let none = [(1i16, (1, 1), Mark::Foreign)];
        assert_eq!(nearest_colonisable((0, 0), &none), None);
    }

    /// A tie goes to the planet the original meets first, which is the lowest
    /// id.
    #[test]
    fn ties_go_to_the_lower_planet_id() {
        let cands = [
            (4i16, (0, 10), Mark::Colonisable),
            (2, (10, 0), Mark::Colonisable),
        ];
        // Both are 100 away; the array is walked in id order.
        let mut sorted = cands;
        sorted.sort_by_key(|c| c.0);
        assert_eq!(nearest_colonisable((0, 0), &sorted), Some(2));
    }

    /// Terraforming a planet toward the race's ideal can only improve it, and
    /// the planet itself is left untouched.
    #[test]
    fn the_optimal_value_terraforms_without_altering_the_planet() {
        let race = Race::humanoid();
        let mut planet = Planet::unowned(0);
        planet.env = [10, 50, 50];

        let now = crate::hab::pct_planet_desirability(&planet, &race);
        let after = pct_planet_opt_value(&planet, &race, [30, 50, 50]);
        assert!(after > now, "terraforming should help: {now} -> {after}");
        assert_eq!(planet.env, [10, 50, 50], "the planet must be unchanged");
    }
}
