//! # stars-core
//!
//! The deterministic, platform-agnostic heart of the Stars! reimplementation.
//!
//! This crate owns the game model ([`GameState`]) and the simulation systems
//! that advance it (production, minerals/resources, population growth, fleet
//! movement, scanning, combat, research) together with the turn/order
//! processing pipeline and the AI.
//!
//! ## Non-negotiable constraints
//!
//! * **Deterministic** — identical inputs and seed must produce identical
//!   outputs on every platform (native and wasm). All randomness flows through
//!   [`Rng`], a reproduction of the original engine's PRNG.
//! * **Headless** — no filesystem, no rendering, no platform APIs. File bytes
//!   enter and leave through `stars-formats`; presentation lives in the UI
//!   crates.
//!
//! The concrete data model and formulas are recovered in Steps 3–4 of the
//! delivery plan. This file currently establishes the public surface and the
//! deterministic RNG seam that everything else will build on.

#![forbid(unsafe_code)]

/// A deterministic pseudo-random number generator seam.
///
/// The real implementation must reproduce the original Stars! PRNG exactly (to
/// be recovered from Ghidra in Step 3) so that combat, mineral, and event
/// outcomes match the original engine bit-for-bit. Until then this is a simple
/// linear congruential placeholder with a stable, documented sequence so that
/// higher layers can be written and tested against a fixed seed.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    /// Create a generator from an explicit 32-bit seed.
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Return the next pseudo-random `u32` and advance the state.
    ///
    /// NOTE: placeholder algorithm — replaced by the reverse-engineered Stars!
    /// PRNG in a later step. The public signature is expected to remain stable.
    pub fn next_u32(&mut self) -> u32 {
        // Numerical Recipes LCG constants; deterministic and dependency-free.
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.state
    }
}

/// The complete, serializable state of a game at a single turn boundary.
///
/// Fields (universe, planets, fleets, players, tech, …) are added in Step 3 as
/// the data model is reverse-engineered. `turn` and `seed` are present now
/// because they anchor determinism and are needed by the RNG seam.
#[derive(Debug, Clone)]
pub struct GameState {
    /// The current game year / turn number.
    pub turn: u32,
    /// The master RNG seed for this game.
    pub seed: u32,
}

impl GameState {
    /// Construct an (empty) game state for a given seed at turn zero.
    pub fn new(seed: u32) -> Self {
        Self { turn: 0, seed }
    }
}

/// Player orders for a single turn.
///
/// Populated in Step 4 alongside the turn-generation pipeline.
#[derive(Debug, Clone, Default)]
pub struct PlayerOrders;

/// Advance the game by one full turn given every player's orders.
///
/// This is the central deterministic entry point of the engine: for a fixed
/// input state (including its seed) and a fixed set of orders it must always
/// produce the same next state. The body is filled in during Step 4; for now
/// it deterministically bumps the turn counter so the seam is exercisable.
pub fn generate_turn(state: &GameState, _orders: &[PlayerOrders]) -> GameState {
    GameState {
        turn: state.turn + 1,
        seed: state.seed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_for_a_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..8 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn generate_turn_advances_deterministically() {
        let state = GameState::new(7);
        let next = generate_turn(&state, &[]);
        assert_eq!(next.turn, 1);
        assert_eq!(next.seed, 7);
    }
}
