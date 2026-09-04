//! Landing colonists: settling an empty planet, and invading a held one.
//!
//! Source: `DropColonists` (`10b8:34e2`), called from `DoOrders`. One routine
//! does both jobs, because they are the same act: colonists are put down on a
//! planet, and what happens next depends on whether anyone was already there.
//!
//! Every drop aimed at a planet in the same turn is resolved together, so two
//! players who both send colonists to the same empty world contest it, and an
//! invasion by several players at once is a three-way fight.

use crate::race::Prt;

/// How much an attacker's colonists are worth, as a percentage.
///
/// Source: `10b8:3766`. War Mongers fight at 165%, Alternate Reality races
/// cannot take a planet by force at all, and everyone else attacks at 110%.
#[must_use]
pub fn attack_weight(prt: Option<Prt>) -> i32 {
    match prt {
        Some(Prt::Wm) => 165,
        Some(Prt::Ar) => 0,
        _ => 110,
    }
}

/// How much a defender's colonists are worth, as a percentage.
///
/// Source: `10b8:3820`. Inner Strength defends at 200%; everyone else at 100%.
#[must_use]
pub fn defence_weight(prt: Option<Prt>) -> i32 {
    match prt {
        Some(Prt::Is) => 200,
        _ => 100,
    }
}

/// Whether a race will take an item from its default production queue when a
/// new planet inherits one.
///
/// Source: the template loop in `DropColonists` (`10b8:3d1a`). A newly settled
/// planet is given a queue built from its owner's default template — the "zip"
/// production queue, `PLAYER.zpq1` — with two races filtering it:
///
/// - **Alternate Reality** drops every item at or below
///   [`crate::production::item::DEFENSE`], because it has no planetary
///   installations to build.
/// - **Claim Adjuster** drops the two terraforming items, because it terraforms
///   from orbit for free.
///
/// The item is taken from the low six bits of the template word.
#[must_use]
pub fn template_allows(prt: Option<Prt>, item: u16) -> bool {
    use crate::production::item;
    match prt {
        Some(Prt::Ar) => item > item::DEFENSE,
        Some(Prt::Ca) => item != item::MIN_TERRAFORM && item != item::MAX_TERRAFORM,
        _ => true,
    }
}

/// One player's colonists arriving at a planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Landing {
    /// Whose colonists these are.
    pub player: i16,
    /// How many, in units of 100 as the planet stores them.
    pub colonists: i32,
    /// Their race's primary trait, which sets their weight.
    pub prt: Option<Prt>,
}

impl Landing {
    /// The weighted strength of this landing as an attacker.
    #[must_use]
    pub fn strength(self) -> i32 {
        self.colonists.saturating_mul(attack_weight(self.prt)) / 100
    }
}

/// What a set of landings did to a planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nobody landed anything that counts.
    Nothing,
    /// The planet was empty and is now held by this player.
    Settled {
        /// The new owner.
        player: i16,
        /// Colonists left standing, in units of 100.
        colonists: i32,
    },
    /// The planet was held and the defender kept it.
    Held {
        /// Defenders left standing, in units of 100.
        colonists: i32,
    },
    /// The planet was held and changed hands.
    Taken {
        /// The new owner.
        player: i16,
        /// Colonists left standing, in units of 100.
        colonists: i32,
    },
}

/// Resolve every landing on one planet.
///
/// `defender` is the population already there and its race's trait, or `None`
/// for an empty planet.
///
/// The strongest landing takes an empty planet, weighted as above; on a held
/// planet the attackers' combined strength is set against the defenders'.
///
/// # What is not modelled
///
/// The survivor counts the original computes after a *contested* landing are
/// not transcribed. `DropColonists` scales them through several 32-bit terms
/// that the decompiler has flattened past the point of confident reading, and
/// no fixture separates a contested landing from an uncontested one — the 513
/// colonisations in `fixtures/games/all-computer-players` are all onto empty
/// planets with a single claimant. What is returned here for those cases is
/// the uncontested answer, and [`Outcome::Taken`] reports the attacker's
/// surplus rather than the original's formula.
#[must_use]
pub fn resolve_landings(defender: Option<(i32, Option<Prt>)>, landings: &[Landing]) -> Outcome {
    let strongest = landings
        .iter()
        .filter(|l| l.strength() > 0)
        .max_by_key(|l| (l.strength(), std::cmp::Reverse(l.player)));
    let Some(winner) = strongest.copied() else {
        return Outcome::Nothing;
    };
    let attack: i32 = landings.iter().map(|l| l.strength()).sum();

    let Some((held, prt)) = defender else {
        // An empty planet: the strongest claim simply takes it, and its
        // colonists land intact. What a *losing* claimant leaves behind after a
        // contested settling is part of what is not transcribed.
        return Outcome::Settled {
            player: winner.player,
            colonists: winner.colonists,
        };
    };

    let defence = held.saturating_mul(defence_weight(prt)) / 100;
    if attack < defence {
        // The defenders hold, losing ground in proportion to what hit them.
        let lost = if defence > 0 {
            held.saturating_mul(attack) / defence
        } else {
            0
        };
        return Outcome::Held {
            colonists: (held - lost).max(1),
        };
    }

    // The planet falls. `UninhabitPlanet` clears it and the strongest claim
    // settles what is left.
    let surplus = attack - defence;
    let colonists = surplus.saturating_mul(100) / attack_weight(winner.prt).max(1);
    Outcome::Taken {
        player: winner.player,
        colonists: colonists.max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn landing(player: i16, colonists: i32, prt: Prt) -> Landing {
        Landing {
            player,
            colonists,
            prt: Some(prt),
        }
    }

    #[test]
    fn the_weights_are_the_ones_the_routine_uses() {
        assert_eq!(attack_weight(Some(Prt::Wm)), 165);
        assert_eq!(attack_weight(Some(Prt::Ar)), 0);
        assert_eq!(attack_weight(Some(Prt::Joat)), 110);
        assert_eq!(defence_weight(Some(Prt::Is)), 200);
        assert_eq!(defence_weight(Some(Prt::Joat)), 100);
    }

    #[test]
    fn an_empty_planet_goes_to_the_only_claimant() {
        let out = resolve_landings(None, &[landing(3, 250, Prt::Joat)]);
        assert_eq!(
            out,
            Outcome::Settled {
                player: 3,
                colonists: 250
            }
        );
    }

    /// An Alternate Reality race cannot take a planet with colonists.
    #[test]
    fn alternate_reality_cannot_claim_a_planet() {
        assert_eq!(
            resolve_landings(None, &[landing(1, 5_000, Prt::Ar)]),
            Outcome::Nothing
        );
    }

    /// Contested: the heavier weighted claim wins, not the larger one.
    #[test]
    fn a_war_monger_outweighs_a_larger_landing() {
        let out = resolve_landings(
            None,
            &[landing(1, 100, Prt::Wm), landing(2, 140, Prt::Joat)],
        );
        // 100 * 165 = 16500 against 140 * 110 = 15400.
        assert_eq!(
            out,
            Outcome::Settled {
                player: 1,
                colonists: 100
            }
        );
    }

    /// Inner Strength doubles the defence, which turns a losing fight.
    #[test]
    fn inner_strength_holds_a_planet_a_weaker_race_would_lose() {
        let attackers = [landing(2, 150, Prt::Joat)]; // 165 strength

        let joat = resolve_landings(Some((160, Some(Prt::Joat))), &attackers);
        assert!(
            matches!(joat, Outcome::Taken { player: 2, .. }),
            "160 defenders at 100% should fall to 165: {joat:?}"
        );

        let is = resolve_landings(Some((160, Some(Prt::Is))), &attackers);
        assert!(
            matches!(is, Outcome::Held { .. }),
            "160 defenders at 200% should hold against 165: {is:?}"
        );
    }

    #[test]
    fn a_repelled_invasion_still_costs_the_defender() {
        let out = resolve_landings(
            Some((1_000, Some(Prt::Joat))),
            &[landing(2, 100, Prt::Joat)],
        );
        match out {
            Outcome::Held { colonists } => {
                assert!(colonists < 1_000, "the defender should lose ground");
                assert!(colonists > 800, "but not be gutted: {colonists}");
            }
            other => panic!("expected the defender to hold, got {other:?}"),
        }
    }

    /// The two races that filter their inherited queue, and what they drop.
    #[test]
    fn the_queue_template_filter_matches_the_routine() {
        use crate::production::item;

        // Alternate Reality builds no planetary installation at all.
        for i in [item::MINE, item::FACTORY, item::DEFENSE] {
            assert!(
                !template_allows(Some(Prt::Ar), i),
                "AR should drop item {i}"
            );
        }
        assert!(template_allows(Some(Prt::Ar), item::ALCHEMY));

        // Claim Adjuster terraforms for free, so it queues no terraforming.
        for i in [item::MIN_TERRAFORM, item::MAX_TERRAFORM] {
            assert!(
                !template_allows(Some(Prt::Ca), i),
                "CA should drop item {i}"
            );
        }
        assert!(template_allows(Some(Prt::Ca), item::FACTORY));

        // Everyone else takes the template as it stands.
        for i in 0..=12u16 {
            assert!(template_allows(Some(Prt::Joat), i));
        }
    }

    #[test]
    fn nothing_lands_nothing_happens() {
        assert_eq!(resolve_landings(None, &[]), Outcome::Nothing);
        assert_eq!(resolve_landings(Some((500, None)), &[]), Outcome::Nothing);
    }
}
