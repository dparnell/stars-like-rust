//! Scanner ranges.
//!
//! Source: `GetPlanetScannerRange` (`1038:4c02`) and `GetShdefScannerRange`
//! (`1038:50d0`) in `stars.2.7j.exe`, with the combination rule stated
//! directly in `MANUAL.PDF` p. 17-2. Full derivation in
//! `docs/formulas/scanning.md`.
//!
//! Stars! distinguishes two ranges. A **normal** range detects fleets in deep
//! space; a **penetrating** range additionally sees through planets, revealing
//! orbiting fleets and sub-surface mineral concentrations.

use crate::planet::Planet;
use crate::race::{lrt, Race};

/// A scanner's two ranges, in light years.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScannerRange {
    /// Normal (non-penetrating) range.
    pub normal: i32,
    /// Planet-penetrating range; `0` for a scanner that cannot penetrate.
    pub penetrating: i32,
}

/// Combine several scanner ranges into one, the way a ship with multiple
/// scanners does: the **fourth root of the sum of fourth powers**.
///
/// Two 100 ly scanners and one 60 ly scanner give
/// `(100^4 + 100^4 + 60^4)^(1/4) = 120` ly (`MANUAL.PDF` p. 17-2).
#[must_use]
pub fn combine_ranges(ranges: &[i32]) -> i32 {
    let sum: f64 = ranges
        .iter()
        .filter(|r| **r > 0)
        .map(|r| {
            let x = f64::from(*r);
            x * x * x * x
        })
        .sum();
    if sum <= 0.0 {
        return 0;
    }
    sum.powf(0.25) as i32
}

/// The scanning range of a planet, given the best planetary scanner its owner
/// has researched.
///
/// `best_scanner` is that part's stored range: a **negative** value marks a
/// penetrating scanner, whose penetrating range is half its magnitude. Pass
/// `None` when the planet has no scanner at all.
///
/// No Advanced Scanners doubles conventional range and gives up penetration
/// entirely (`MANUAL.PDF` p. 20-13).
#[must_use]
pub fn planet_scanner_range(
    planet: &Planet,
    race: &Race,
    best_scanner: Option<i32>,
) -> ScannerRange {
    if race.is_ar() {
        // Alternate Reality planets scan from their starbase, by population.
        let range = (f64::from(planet.pop) * 10.0).sqrt() as i32;
        if race.has_lrt(lrt::NO_ADV_SCANNER) {
            // The original scales by 1412/1000 — the fourth root of 4, i.e.
            // the same "doubled range" rule applied under the 4th-power law.
            return ScannerRange {
                normal: range * 1412 / 1000,
                penetrating: 0,
            };
        }
        // A sufficiently advanced starbase penetrates at half range; that
        // depends on the starbase hull, which the ship-design layer owns, so
        // only the normal range is reported here.
        return ScannerRange {
            normal: range,
            penetrating: 0,
        };
    }

    let Some(raw) = best_scanner else {
        return ScannerRange::default();
    };

    let mut normal = raw.abs();
    let mut penetrating = if raw < 0 { -raw / 2 } else { 0 };

    if race.has_lrt(lrt::NO_ADV_SCANNER) {
        normal *= 2;
        penetrating = 0;
    }

    ScannerRange {
        normal,
        penetrating,
    }
}
