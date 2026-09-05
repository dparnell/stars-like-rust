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
//! [`encode_field`] is the inverse. Each character has exactly one table
//! entry — the five tables partition the printable set without overlap — so
//! the only choice an encoder makes is what to do with a leftover nibble when
//! the stream is an odd length. Padding with `0xF` is what the real files do:
//! the escape reads its two following nibbles, finds the block has ended, and
//! emits nothing, so a `0xF` tail is invisible to the decoder. Every packed
//! string in the fixtures re-encodes byte for byte under that rule.

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

/// The nibbles one character encodes to.
///
/// Returns the single-nibble form when the character is one of the eleven in
/// [`ENCODES_ONE`], the two-nibble form when it is in one of the four
/// secondary tables, and the three-nibble `0xF` escape otherwise.
fn char_nibbles(c: char) -> [Option<u8>; 3] {
    let byte = if (c as u32) < 256 {
        c as u8
    } else {
        b'?' // Nothing outside Latin-1 can be stored; keep it printable.
    };
    if let Some(i) = ENCODES_ONE.iter().position(|t| *t == byte) {
        return [Some(i as u8), None, None];
    }
    for (lead, table) in [
        (0x0Bu8, ENCODES_B),
        (0x0C, ENCODES_C),
        (0x0D, ENCODES_D),
        (0x0E, ENCODES_E),
    ] {
        if let Some(i) = table.iter().position(|t| *t == byte) {
            return [Some(lead), Some(i as u8), None];
        }
    }
    [Some(0x0F), Some(byte & 0x0F), Some(byte >> 4)]
}

/// Encode a string into packed data bytes (without the length prefix).
///
/// Exact inverse of [`decode_packed`] for every string the tables can express.
#[must_use]
pub fn encode_packed(text: &str) -> Vec<u8> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(text.len() * 2);
    for c in text.chars() {
        for nibble in char_nibbles(c).into_iter().flatten() {
            nibbles.push(nibble);
        }
    }
    // An odd stream is padded with the escape nibble, which the decoder drops
    // because the two nibbles it would read are past the end.
    if nibbles.len() % 2 == 1 {
        nibbles.push(0x0F);
    }
    nibbles
        .chunks(2)
        .map(|pair| (pair[0] << 4) | pair[1])
        .collect()
}

/// The largest packed form the game will write (`cOut = 0x1f` in
/// `WriteRtString`); a longer one falls back to a literal string.
pub const MAX_PACKED_LEN: usize = 31;

/// Decode a **user string** field, the form the game writes for names the
/// player typed: a fleet's name, and the name in a rename order.
///
/// It is the packed field with one escape. A length byte of `0` means the
/// packed form did not fit, and what follows is the string itself as
/// NUL-terminated bytes:
///
/// ```c
/// // WriteRtString, save.c / 1070:87b4
/// cOut = 0x1f;
/// if (FCompressUserString(lpsz, rgb + 1, &cOut) == 0) {
///     strcpy(rgb + 1, lpsz);       // the packed form did not fit
///     rgb[0] = 0;
///     cOut = strlen(lpsz) + 1;     // the NUL is written too
/// } else {
///     rgb[0] = cOut;
/// }
/// WriteRt(rtString, cOut + 1, rgb);
/// ```
#[must_use]
pub fn decode_user_string(field: &[u8]) -> String {
    match field.split_first() {
        None => String::new(),
        // The escape: a literal, NUL-terminated string.
        Some((0, literal)) => literal
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| char::from(*b))
            .collect(),
        Some((length, packed)) => {
            let end = usize::from(*length).min(packed.len());
            decode_packed(&packed[..end])
        }
    }
}

/// Encode a **user string** field, the inverse of [`decode_user_string`].
///
/// The packed form is used when it fits in [`MAX_PACKED_LEN`] bytes, and the
/// literal escape when it does not — which is the choice `WriteRtString` makes.
/// The literal path can hold 31 characters, so longer names are truncated
/// there rather than overrunning the 33-byte buffer the game reads into.
#[must_use]
pub fn encode_user_string(text: &str) -> Vec<u8> {
    let packed = encode_packed(text);
    if packed.len() <= MAX_PACKED_LEN {
        let mut out = Vec::with_capacity(packed.len() + 1);
        #[allow(clippy::cast_possible_truncation)]
        out.push(packed.len() as u8);
        out.extend_from_slice(&packed);
        return out;
    }
    let mut out = vec![0u8];
    for c in text.chars().take(MAX_PACKED_LEN) {
        out.push(if (c as u32) < 256 && c != '\0' {
            c as u8
        } else {
            b'?'
        });
    }
    out.push(0);
    out
}

/// Encode a string as a packed string **field**: a length byte followed by the
/// packed data.
///
/// # Errors
/// [`crate::FormatError::Malformed`] if the packed form is longer than the
/// 255 bytes a length byte can count.
pub fn encode_field(text: &str) -> crate::Result<Vec<u8>> {
    let packed = encode_packed(text);
    let len = u8::try_from(packed.len()).map_err(|_| {
        crate::FormatError::Malformed(format!(
            "packed string is {} bytes, more than a length byte can count",
            packed.len()
        ))
    })?;
    let mut out = Vec::with_capacity(packed.len() + 1);
    out.push(len);
    out.extend_from_slice(&packed);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_round_trip_through_the_tables() {
        for text in [
            "",
            "a",
            "Armed Probe",
            "Santa Maria",
            "Long Range Scout",
            "M.T. Lifeboat",
            "Smaugarian Peeping Tom",
            "Humanoid",
            "0123456789",
            "punctuation +-,!.?:;'*%$",
            "MiXeD CaSe 42",
        ] {
            let field = encode_field(text).expect("short enough");
            assert_eq!(decode_field(&field), text, "{text:?}");
        }
    }

    #[test]
    fn an_odd_nibble_count_pads_invisibly() {
        // "a" is one nibble; the pad must not add a character.
        let field = encode_field("a").expect("short enough");
        assert_eq!(field.len(), 2, "one length byte plus one packed byte");
        assert_eq!(decode_field(&field), "a");
    }

    #[test]
    fn user_strings_round_trip() {
        for text in [
            "",
            "Bold Endeavour",
            "Fleet #7",
            "The Second Expeditionary Force",
            "!!! *** !!!",
        ] {
            let field = encode_user_string(text);
            assert_eq!(decode_user_string(&field), text, "{text:?}");
        }
    }

    /// A name too long to pack falls back to the literal escape.
    #[test]
    fn a_name_that_will_not_pack_is_written_literally() {
        // Every character escapes to three nibbles, so 21 of them pack to 32
        // bytes — one over the budget.
        let awkward: String = std::iter::repeat_n('#', 21).collect();
        assert!(encode_packed(&awkward).len() > MAX_PACKED_LEN);
        let field = encode_user_string(&awkward);
        assert_eq!(field[0], 0, "the literal escape");
        assert_eq!(field.last(), Some(&0), "and it is NUL-terminated");
        assert_eq!(decode_user_string(&field), awkward);
    }

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
