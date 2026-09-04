//! The original Stars! simulation PRNG.
//!
//! Stars! contains exactly **one** generator, used both for the file cipher and
//! for the simulation. It is L'Ecuyer's combined multiplicative LCG: two
//! Park–Miller sub-generators advanced by Schrage's method and combined by
//! subtraction. The constants are confirmed against `FUN_1038_8a58` in
//! `STARS!.EXE` (`1038:8a58`) — see `docs/rng/prng.md`.
//!
//! The **state advance** here is identical to
//! [`stars_formats::StarsRng`], which implements the same generator for the
//! file cipher, but the two differ in how the raw state is turned into a
//! result:
//!
//! | consumer | mapping | range |
//! |----------|---------|-------|
//! | file cipher (`stars_formats::StarsRng::next_u32`) | `s1 - s2`, `+ 2^32` when it borrows | full 32-bit keystream word |
//! | simulation (`Rng::next_raw`, this module) | `s1 - s2`, `+ (m1 - 1)` when `< 1` | `1 ..= m1 - 1` |
//!
//! Mixing the two mappings up would desynchronise every RNG-dependent
//! simulation result, so they are deliberately kept as separate types rather
//! than one shared `next_u32`.
//!
//! Source: `Random` / `Randomize` (`utilgen.c` in the reconstructed NB09
//! sources; `1040:14a2` in our binary).

/// First sub-generator modulus, `m1` (`0x7fff_ffab`).
const M1: i64 = 2_147_483_563;
/// Second sub-generator modulus, `m2` (`0x7fff_ff07`).
const M2: i64 = 2_147_483_399;

/// The Stars! simulation random number generator.
///
/// Seed it with [`Rng::randomize`] (the game's own seeding, from a 32-bit
/// value such as the game id) or [`Rng::from_seeds`] for an exact state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    seed_1: i64,
    seed_2: i64,
}

impl Rng {
    /// Seed exactly, from two sub-generator states.
    #[must_use]
    pub fn from_seeds(seed_1: u32, seed_2: u32) -> Self {
        Self {
            seed_1: i64::from(seed_1),
            seed_2: i64::from(seed_2),
        }
    }

    /// Seed the way the game's `Randomize(dw)` does: take the low six bits of
    /// `dw` and the next six as indices into the primes table, nudging the
    /// second index when the two collide so the sub-generators never start
    /// from the same value.
    /// The seed `FGenerateTurn` uses when a game is set to generate turns
    /// reproducibly.
    ///
    /// `FGenerateTurn` (`10b0:0000`) opens with
    ///
    /// ```text
    /// if ((gd.flags >> 0xb & 1) != 0) Randomize(0x499602d2);
    /// ```
    ///
    /// so a game with bit 11 of the game flags set restarts the generator from
    /// this constant at the head of **every** turn. That is the only way a turn
    /// is reproducible; see [`Self::randomize`] for why.
    pub const DETERMINISTIC_TURN_SEED: u32 = 0x4996_02d2;

    /// The generator as a reproducible game starts each turn.
    #[must_use]
    pub fn for_deterministic_turn() -> Self {
        Self::randomize(Self::DETERMINISTIC_TURN_SEED)
    }

    #[must_use]
    pub fn randomize(dw: u32) -> Self {
        let a = (dw & 0x3f) as usize;
        let mut b = ((dw >> 6) & 0x3f) as usize;
        if a == b {
            b = (b + 1) & 0x3f;
        }
        Self::from_seeds(
            stars_formats::crypt::PRIMES[a],
            stars_formats::crypt::PRIMES[b],
        )
    }

    /// Advance the generator and return the raw combined value, in `1 ..= m1-1`.
    ///
    /// Both sub-generator states are committed unconditionally, before the
    /// caller's range check — matching the original, where an early return for
    /// a non-positive bound still consumes a step.
    pub fn next_raw(&mut self) -> i32 {
        // Schrage: s = a*(s % q) - r*(s / q), folded back into range if negative.
        let mut s1 = (self.seed_1 % 53_668) * 40_014 - (self.seed_1 / 53_668) * 12_211;
        if s1 < 0 {
            s1 += M1;
        }
        let mut s2 = (self.seed_2 % 52_774) * 40_692 - (self.seed_2 / 52_774) * 3_791;
        if s2 < 0 {
            s2 += M2;
        }
        self.seed_1 = s1;
        self.seed_2 = s2;

        let mut z = s1 - s2;
        if z < 1 {
            z += M1 - 1;
        }
        z as i32
    }

    /// The game's `Random(c)`: a value in `0..c`, or `0` when `c <= 0`.
    ///
    /// Note that the generator is stepped even when `c <= 0`.
    pub fn random(&mut self, c: i16) -> i16 {
        let z = self.next_raw();
        if c <= 0 {
            return 0;
        }
        #[allow(clippy::cast_sign_loss)] // z is >= 1, c is > 0
        let r = (z as u32) % u32::from(c.unsigned_abs());
        r as i16
    }
}
