//! Two checks that do not fit the manual-derived vectors.
//!
//! 1. The simulation RNG must ride the *same* state trajectory as the file
//!    cipher's generator, which is already verified byte-exact against real
//!    save files. That makes this an indirect check of the simulation RNG
//!    against the original binary.
//! 2. Mineral concentration decay must consume the mine-years the manual
//!    specifies.

use stars_core::mining::mine_minerals;
use stars_core::planet::Planet;
use stars_core::race::{Prt, Race, RaceStat};
use stars_core::rng::Rng;
use stars_formats::StarsRng;

/// `m1 - 1`, the offset the simulation mapping folds a non-positive difference
/// by.
const FOLD: i64 = 2_147_483_562;

#[test]
fn simulation_rng_tracks_the_verified_cipher_generator() {
    // Any two primes-table entries will do; these are the ones `Randomize(0)`
    // would pick.
    let seed_a = stars_formats::crypt::PRIMES[0];
    let seed_b = stars_formats::crypt::PRIMES[1];

    let mut cipher = StarsRng::new(seed_a, seed_b, 0);
    let mut sim = Rng::from_seeds(seed_a, seed_b);

    for step in 0..10_000 {
        let c = i64::from(cipher.next_u32());
        let z = i64::from(sim.next_raw());

        // Both start from `s1 - s2`. The cipher wraps a borrow into 32 bits;
        // the simulation folds a non-positive difference by `m1 - 1`. So the
        // two outputs must differ by exactly one of the two known offsets.
        let matches = z == c || z == c - (1 << 32) + FOLD;
        assert!(
            matches,
            "step {step}: cipher {c} and simulation {z} are not the same draw"
        );
        assert!(
            (1..=FOLD).contains(&z),
            "step {step}: simulation draw {z} is outside 1..=m1-1"
        );
    }
}

#[test]
fn random_is_bounded_and_steps_even_when_the_bound_is_zero() {
    let mut rng = Rng::randomize(0x4996_02d2);
    for _ in 0..1000 {
        let r = rng.random(100);
        assert!((0..100).contains(&r), "Random(100) returned {r}");
    }

    // `Random(c)` advances the generator before it checks the bound, so a
    // non-positive bound still consumes a draw. Getting this wrong would
    // desynchronise every later result.
    let mut a = Rng::randomize(1);
    let mut b = Rng::randomize(1);
    assert_eq!(a.random(0), 0);
    a.random(10);
    b.random(10);
    b.random(10);
    assert_eq!(
        a.random(1_000),
        b.random(1_000),
        "a zero bound must still step the generator"
    );
}

#[test]
fn randomize_never_seeds_both_generators_alike() {
    // Low six bits and next six bits equal would seed both sub-generators from
    // the same prime; the original nudges the second index instead.
    for v in 0..64u32 {
        let dw = v | (v << 6);
        let nudged = Rng::randomize(dw);
        let distinct = Rng::from_seeds(
            stars_formats::crypt::PRIMES[v as usize],
            stars_formats::crypt::PRIMES[((v + 1) & 0x3f) as usize],
        );
        assert_eq!(
            nudged, distinct,
            "Randomize({dw:#x}) should nudge the second index"
        );
    }
}

fn miner_race() -> Race {
    let mut race = Race::humanoid();
    race.attrs[RaceStat::MajorAdv as usize] = Prt::Is as i16;
    race
}

#[test]
fn concentration_falls_after_the_manual_s_mine_years() {
    // MANUAL.PDF p. 13-2: "To calculate approximately how many Mine years must
    // pass to reduce a mineral's concentration by one, divide 12,500 by the
    // current mineral concentration." At concentration 100 that is 125
    // mine-years.
    let race = miner_race();
    let mut planet = Planet::unowned(0);
    planet.owner = Some(0);
    planet.env = race.env_center;
    planet.pop = 10_000; // enough to staff the mines
    planet.mines = 100;
    planet.min_conc = [100, 100, 100];

    // A deterministic generator: the only randomness in mining is the
    // fractional-kT rounding, which does not affect depletion.
    let mut rng = Rng::randomize(12_345);

    let mut years = 0;
    while planet.min_conc[0] == 100 && years < 100 {
        mine_minerals(&mut planet, &race, None, &mut rng);
        years += 1;
    }

    // 100 mines * concentration 100 / 100 = 100 mine-years a year, so the
    // 125 mine-years needed take two years to accumulate.
    assert_eq!(
        years, 2,
        "125 mine-years at 100 mine-years a year should cost the first point after 2 years"
    );
    assert_eq!(planet.min_conc[0], 99);
}

#[test]
fn a_homeworld_never_mines_below_concentration_thirty() {
    // MANUAL.PDF p. 6-5: the mineral concentration on a home world never drops
    // below 30 for the purpose of mining.
    let race = miner_race();
    let mut planet = Planet::unowned(0);
    planet.owner = Some(0);
    planet.env = race.env_center;
    planet.pop = 10_000;
    planet.mines = 100;
    planet.min_conc = [3, 3, 3];
    planet.homeworld = true;

    let mined = stars_core::mining::minerals_mined(&planet, &race, None, None);
    // 100 mines * 30 * 10 / 10 / 100 = 30 kT, not the 3 kT the raw
    // concentration would give.
    assert_eq!(mined[0], 30);

    let mut depleted = planet.clone();
    depleted.homeworld = false;
    let mined = stars_core::mining::minerals_mined(&depleted, &race, None, None);
    assert_eq!(mined[0], 3, "an ordinary planet gets no floor");
}
