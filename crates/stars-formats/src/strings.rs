//! Decoder for the **Stars! packed string** encoding used for user-supplied
//! text such as race singular/plural names.
//!
//! Stars! stores these strings as a length-prefixed, nibble-packed blob rather
//! than plain ASCII. The scheme was recovered from Rick Steeves' TotalHost
//! `StarsBlock.pm` (`decodeBytesForStarsString`) and is reproduced here.
//!
//! ## Layout
//!
//! A field is `[len: u8][len bytes of packed data]`. The `len` byte is **not**
//! part of the text (it counts the packed data bytes that follow). Each packed
//! byte contributes two 4-bit **nibbles**, high nibble first, forming a nibble
//! stream that is decoded left-to-right:
//!
//! | leading nibble | meaning                                                     |
//! |----------------|-------------------------------------------------------------|
//! | `0x0..=0xA`    | index into [`ENCODES_ONE`] (`" aehilnorst"`) — one char      |
//! | `0xB`          | next nibble indexes [`ENCODES_B`]                            |
//! | `0xC`          | next nibble indexes [`ENCODES_C`]                            |
//! | `0xD`          | next nibble indexes [`ENCODES_D`]                            |
//! | `0xE`          | next nibble indexes [`ENCODES_E`]                            |
//! | `0xF`          | the next two nibbles are a literal byte (low nibble first)   |
//!
//! The tables pack the most common lowercase letters into a single nibble
//! (`" aehilnorst"`), the rest of the printable set into two nibbles, and fall
//! back to a raw byte (`0xF` escape) for anything else.
//!
//! This decoder is **read-only**: like [`crate::race::RaceRecord`], it is a
//! typed *view* over verified fields. Re-encoding a file still goes through the
//! byte-exact cipher container in [`crate::file`], so a string encoder is not
//! needed for round-tripping and is intentionally omitted (the original
//! `charToNibble` in `StarsBlock.pm` is likewise marked untested).

/// Single-nibble table: nibble values `0x0..=0xA` map directly to one of these
/// eleven characters (space plus the ten most common lowercase letters).
pub const ENCODES_ONE: &[u8; 11] = b" aehilnorst";
/// Two-nibble table selected by a leading `0xB` nibble.
pub const ENCODES_B: &[u8; 16] = b"ABCDEFGHIJKLMNOP";
/// Two-nibble table selected by a leading `0xC` nibble.
pub const ENCODES_C: &[u8; 16] = b"QRSTUVWXYZ012345";
/// Two-nibble table selected by a leading `0xD` nibble.
pub const ENCODES_D: &[u8; 16] = b"6789bcdfgjkmpquv";
/// Two-nibble table selected by a leading `0xE` nibble.
pub const ENCODES_E: &[u8; 16] = b"wxyz+-,!.?:;'*%$";

/// Decode a Stars! packed string **field**, whose first byte is the length
/// prefix (the count of packed bytes that follow).
///
/// The length byte is skipped and the remaining bytes are decoded as the nibble
/// stream described in the [module docs](self). Bytes produced by the `0xF`
/// escape are interpreted as Latin-1 (each byte becomes the `char` of that code
/// point), matching the original's single-byte character handling.
#[must_use]
pub fn decode_field(field: &[u8]) -> String {
    if field.len() <= 1 {
        return String::new();
    }
    decode_packed(&field[1..])
}

/// Decode a Stars! packed string from its **packed data bytes** (i.e. with the
/// length prefix already stripped).
#[must_use]
pub fn decode_packed(packed: &[u8]) -> String {
    let mut nibbles = Vec::with_capacity(packed.len() * 2);
    for &b in packed {
        nibbles.push(b >> 4);
        nibbles.push(b & 0x0F);
    }
    decode_nibbles(&nibbles)
}

fn nibble(nibbles: &[u8], i: usize) -> u8 {
    // Out-of-range reads decode as 0, matching the reference's `substr` beyond
    // end -> "" -> hex(0) behaviour at odd stream boundaries.
    nibbles.get(i).copied().unwrap_or(0)
}

fn decode_nibbles(nibbles: &[u8]) -> String {
    let n = nibbles.len();
    let mut out = String::new();
    let mut t = 0usize;
    while t < n {
        match nibbles[t] {
            c @ 0x0..=0x0A => {
                out.push(ENCODES_ONE[c as usize] as char);
                t += 1;
            }
            0x0B => {
                out.push(ENCODES_B[nibble(nibbles, t + 1) as usize] as char);
                t += 2;
            }
            0x0C => {
                out.push(ENCODES_C[nibble(nibbles, t + 1) as usize] as char);
                t += 2;
            }
            0x0D => {
                out.push(ENCODES_D[nibble(nibbles, t + 1) as usize] as char);
                t += 2;
            }
            0x0E => {
                out.push(ENCODES_E[nibble(nibbles, t + 1) as usize] as char);
                t += 2;
            }
            _ => {
                // 0xF: the next two nibbles form a literal byte (low nibble
                // first). Only emit when at least the following two nibbles are
                // available, matching the reference's boundary check.
                if t + 2 <= n {
                    let low = nibble(nibbles, t + 1);
                    let high = nibble(nibbles, t + 2);
                    out.push(char::from((high << 4) | low));
                }
                t += 3;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_length_only_fields_decode_to_empty() {
        assert_eq!(decode_field(&[]), "");
        assert_eq!(decode_field(&[0]), "");
    }

    #[test]
    fn single_nibble_table_letters() {
        // nibbles: 0=' ',1='a',2='e',... 10='t'
        // byte 0x12 -> nibbles 1,2 -> "ae"
        assert_eq!(decode_packed(&[0x12]), "ae");
        // byte 0x0A -> nibbles 0,10 -> " t"
        assert_eq!(decode_packed(&[0x0A]), " t");
    }

    #[test]
    fn b_table_uppercase() {
        // nibble B (0x0B) then index 0 -> 'A'
        assert_eq!(decode_packed(&[0xB0]), "A");
        // nibble B then index 1 -> 'B'
        assert_eq!(decode_packed(&[0xB1]), "B");
    }

    #[test]
    fn f_escape_literal_byte() {
        // Bytes 0xF1,0x31 -> nibble stream F,1,3,1.
        // F consumes the next two nibbles (low=1, high=3) -> byte 0x31 = '1'.
        // The remaining nibble 1 -> ENCODES_ONE[1] = 'a'.
        assert_eq!(decode_packed(&[0xF1, 0x31]), "1a");
    }
}
