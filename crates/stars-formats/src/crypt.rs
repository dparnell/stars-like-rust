//! The Stars! payload obfuscation (stream cipher) and its seeded PRNG.
//!
//! Every Stars! file is a sequence of [`crate::block::Block`]s. The first block
//! (the [`crate::FILE_HEADER_BLOCK`], type 8) is stored in **plaintext** and
//! carries the values that seed a pseudo-random keystream. The
//! [`FILE_FOOTER_BLOCK`] (type 0) is likewise stored in plaintext. **Every
//! other block's payload is XOR-encrypted** with successive 32-bit words drawn
//! from the PRNG, and the keystream runs *continuously* across those blocks.
//!
//! Because the transform is a plain XOR against a keystream, decryption and
//! encryption are the **same operation** — see [`StarsRng::apply`].
//!
//! ## PRNG
//!
//! The generator is a pair of Lehmer/Park–Miller style linear congruential
//! sub-generators combined by subtraction (a subtractive combined LCG). Two
//! primes chosen from a fixed table (indexed by the header's encryption salt)
//! seed the two sub-generators; the generator is then advanced a
//! header-dependent number of "warm-up" rounds before it is used as a
//! keystream.
//!
//! This was reverse-engineered from the original binary and cross-checked
//! against the community `starsapi`/`TotalHost` implementations; it reproduces
//! real game files **byte-for-byte** (see the round-trip tests in
//! `tests/real_files.rs` and `docs/formats/blocks.md`).

/// The Stars! primes table used to seed the PRNG.
///
/// > Note the well-known anomaly: entry 55 is `279`, which is **not** prime
/// > (it should be `269`). The original `STARS!.EXE` ships this exact table, so
/// > we must reproduce the "bug" to stay bit-compatible.
pub const PRIMES: [u32; 128] = [
    3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 279, 271, 277, 281, 283, 293, 307,
    311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409, 419, 421,
    431, 433, 439, 443, 449, 457, 461, 463, 467, 479, 487, 491, 499, 503, 509, 521, 523, 541, 547,
    557, 563, 569, 571, 577, 587, 593, 599, 601, 607, 613, 617, 619, 631, 641, 643, 647, 653, 659,
    661, 673, 677, 683, 691, 701, 709, 719, 727,
];

/// The Stars! encryption PRNG and stream cipher.
///
/// Construct one from a parsed file header via
/// [`crate::header::FileHeader::init_rng`], then feed each encrypted block's
/// payload through [`StarsRng::apply`] **in file order** so the keystream stays
/// aligned with the original game.
#[derive(Debug, Clone)]
pub struct StarsRng {
    seed_a: i64,
    seed_b: i64,
}

impl StarsRng {
    /// Create a generator directly from two 32-bit seeds and a warm-up round
    /// count. The seeds are typically two entries of [`PRIMES`]; `rounds` is
    /// derived from the header (see [`crate::header::FileHeader::init_rng`]).
    #[must_use]
    pub fn new(seed_a: u32, seed_b: u32, rounds: u32) -> Self {
        let mut rng = Self {
            seed_a: i64::from(seed_a),
            seed_b: i64::from(seed_b),
        };
        for _ in 0..rounds {
            rng.next_u32();
        }
        rng
    }

    /// Advance the generator and return the next 32-bit keystream word.
    pub fn next_u32(&mut self) -> u32 {
        // Two Park–Miller style sub-generators (Schrage's method), combined by
        // subtraction. Constants recovered from the binary.
        let mut new_a = (self.seed_a % 53668) * 40014 - (self.seed_a / 53668) * 12211;
        let mut new_b = (self.seed_b % 52774) * 40692 - (self.seed_b / 52774) * 3791;
        if new_a < 0 {
            new_a += 0x7fff_ffab;
        }
        if new_b < 0 {
            new_b += 0x7fff_ff07;
        }
        let mut random = new_a - new_b;
        if new_a < new_b {
            random += 0x1_0000_0000; // 2^32
        }
        self.seed_a = new_a;
        self.seed_b = new_b;
        (random & 0xFFFF_FFFF) as u32
    }

    /// Apply the keystream to `data`, returning the transformed bytes.
    ///
    /// This performs both **decryption and encryption** (XOR is its own
    /// inverse). The data is processed in 4-byte little-endian chunks; a final
    /// partial chunk is zero-padded to consume a whole keystream word (matching
    /// the original), and the padding is dropped from the output. Advancing the
    /// generator this way keeps a later block's keystream correctly aligned.
    #[must_use]
    pub fn apply(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        let (chunks, rem) = data.as_chunks::<4>();
        for chunk in chunks {
            let word = u32::from_le_bytes(*chunk);
            let x = word ^ self.next_u32();
            out.extend_from_slice(&x.to_le_bytes());
        }
        if !rem.is_empty() {
            let mut buf = [0u8; 4];
            buf[..rem.len()].copy_from_slice(rem);
            let word = u32::from_le_bytes(buf);
            let x = word ^ self.next_u32();
            out.extend_from_slice(&x.to_le_bytes()[..rem.len()]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_is_its_own_inverse() {
        let plain = b"Hello, Stars! encryption round-trips.".to_vec();
        let mut enc = StarsRng::new(PRIMES[3], PRIMES[9], 7);
        let cipher = enc.apply(&plain);
        assert_ne!(cipher, plain);
        let mut dec = StarsRng::new(PRIMES[3], PRIMES[9], 7);
        assert_eq!(dec.apply(&cipher), plain);
    }

    #[test]
    fn partial_final_chunk_consumes_one_word() {
        // 5 bytes => one full word + a 1-byte remainder that still advances the
        // generator by a whole word.
        let mut a = StarsRng::new(3, 5, 1);
        let _ = a.apply(&[1, 2, 3, 4, 5]);
        let after_partial = a.next_u32();

        let mut b = StarsRng::new(3, 5, 1);
        b.next_u32(); // first word (for bytes 0..4)
        b.next_u32(); // second word (for the padded remainder)
        assert_eq!(after_partial, b.next_u32());
    }

    #[test]
    fn primes_table_has_the_279_anomaly() {
        assert_eq!(
            PRIMES[55], 279,
            "must reproduce the original non-prime entry"
        );
    }
}
