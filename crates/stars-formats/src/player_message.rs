//! Messages players write to one another — `rtPlrMsg`, block type 40.
//!
//! The record is the game's `MSGPLR` written verbatim (`WritePlayerMessages`,
//! `1030:97c0`, `WriteRt(rtPlrMsg, cb + 12, lpmsgplr)`; NB09 `cbMSGPLR = 12`),
//! and read back the same way (`ReadPlayerMessages`, `1030:9a5a`, straight
//! into a fresh `MSGPLR` whose first four bytes are then overwritten):
//!
//! | offset | size | field |
//! |--------|------|-------|
//! | 0 | 4 | `lpmsgplrNext` — the in-memory list link, meaningless on disk |
//! | 4 | 2 | `iPlrFrom`, the sender |
//! | 6 | 2 | `iPlrTo`: `0` for everybody, else the recipient plus one |
//! | 8 | 2 | `iInRe`: the index of the message this replies to |
//! | 10 | 2 | `cLen`: the text's length — negative for text stored as it was typed |
//! | 12 | … | the text |
//!
//! `FFinishPlrMsgEntry` (`1030:9bd6`) packs the text with
//! `FCompressUserString` — the nibble code of [`crate::strings`] — and keeps
//! the packed form only when it is no longer than the text (the routine is
//! given the text's own length as the room it has), else it stores the text
//! raw with `cLen = -1 - length`. A message is at most 1,000 characters
//! (`GetWindowText(hwndMsgEdit, lpb2k, 1000)`).
//!
//! In a player's `.x` file the record is one of that player's outgoing
//! messages; in a `.m` file it is one the host delivered — every message to
//! everybody from somebody else, and every one addressed to that player
//! (`WritePlayerMessages`).

use crate::strings::{decode_packed, encode_packed};
use crate::{FormatError, Result};

/// Block type of a player message (`rtPlrMsg`).
pub const PLAYER_MESSAGE_BLOCK: u8 = 40;

/// The most characters a message may hold.
pub const MAX_MESSAGE_LEN: usize = 1000;

/// One message from a player to another, or to everybody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerMessage {
    /// The sender (`iPlrFrom`).
    pub from: i16,
    /// The recipient plus one, or `0` for everybody (`iPlrTo`).
    pub to: i16,
    /// The index of the message this replies to (`iInRe`).
    pub in_re: i16,
    /// The text.
    pub text: String,
}

impl PlayerMessage {
    /// Everybody, as the recipient field spells it.
    pub const EVERYBODY: i16 = 0;

    /// The recipient as a player index, or `None` for everybody.
    #[must_use]
    pub fn recipient(&self) -> Option<usize> {
        (self.to > 0).then(|| usize::try_from(self.to - 1).unwrap_or(usize::MAX))
    }

    /// Whether the host delivers this message to `player`
    /// (`WritePlayerMessages`): to everybody but the sender, or to the one
    /// it names.
    #[must_use]
    pub fn is_for(&self, player: usize) -> bool {
        match self.recipient() {
            None => usize::try_from(self.from).ok() != Some(player),
            Some(to) => to == player,
        }
    }

    /// Decode a block's payload.
    ///
    /// # Errors
    /// [`FormatError::Malformed`] when the payload is shorter than its
    /// header or its text.
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(FormatError::Malformed(format!(
                "player message of {} bytes, under the 12 of its header",
                data.len()
            )));
        }
        let word = |o: usize| i16::from_le_bytes([data[o], data[o + 1]]);
        let from = word(4);
        let to = word(6);
        let in_re = word(8);
        let len = word(10);
        let body = &data[12..];
        let text = if len < 0 {
            let count = usize::try_from(-i32::from(len) - 1).unwrap_or(0);
            if body.len() < count {
                return Err(FormatError::Malformed(format!(
                    "player message text of {count} bytes in a block with {}",
                    body.len()
                )));
            }
            body[..count].iter().map(|b| char::from(*b)).collect()
        } else {
            let count = usize::try_from(len).unwrap_or(0);
            if body.len() < count {
                return Err(FormatError::Malformed(format!(
                    "player message packed text of {count} bytes in a block with {}",
                    body.len()
                )));
            }
            decode_packed(&body[..count])
        };
        Ok(Self {
            from,
            to,
            in_re,
            text,
        })
    }

    /// Encode as a block payload: the twelve-byte header — the link field
    /// zero — and the text packed when that is no longer than the text
    /// itself, else raw.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let text: String = self.text.chars().take(MAX_MESSAGE_LEN).collect();
        let raw: Vec<u8> = text
            .chars()
            .map(|c| {
                if (c as u32) < 256 && c != '\0' {
                    c as u8
                } else {
                    b'?'
                }
            })
            .collect();
        let packed = encode_packed(&text);
        let (len, body): (i16, Vec<u8>) = if packed.len() <= raw.len() {
            (i16::try_from(packed.len()).unwrap_or(i16::MAX), packed)
        } else {
            (-1 - i16::try_from(raw.len()).unwrap_or(i16::MAX - 1), raw)
        };
        let mut out = Vec::with_capacity(12 + body.len());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&self.from.to_le_bytes());
        out.extend_from_slice(&self.to.to_le_bytes());
        out.extend_from_slice(&self.in_re.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&body);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_round_trips_packed_and_raw() {
        let message = PlayerMessage {
            from: 2,
            to: 0,
            in_re: 5,
            text: "the northern lanes are ours; keep your freighters south".to_string(),
        };
        let bytes = message.encode();
        assert_eq!(&bytes[..4], &[0, 0, 0, 0]);
        assert!(i16::from_le_bytes([bytes[10], bytes[11]]) > 0, "packed");
        assert_eq!(PlayerMessage::decode(&bytes).unwrap(), message);

        // Text the nibble code cannot shorten is kept as typed.
        let odd = PlayerMessage {
            from: 0,
            to: 3,
            in_re: 0,
            text: "#@!%^&*{}[]".to_string(),
        };
        let bytes = odd.encode();
        assert!(i16::from_le_bytes([bytes[10], bytes[11]]) < 0, "raw");
        assert_eq!(PlayerMessage::decode(&bytes).unwrap(), odd);
        assert_eq!(odd.recipient(), Some(2));
        assert!(odd.is_for(2) && !odd.is_for(1));
        assert!(message.is_for(0) && message.is_for(1) && !message.is_for(2));
    }

    #[test]
    fn a_short_block_is_refused() {
        assert!(PlayerMessage::decode(&[0; 11]).is_err());
        let mut bytes = PlayerMessage {
            from: 0,
            to: 0,
            in_re: 0,
            text: "hello there".to_string(),
        }
        .encode();
        bytes.truncate(14);
        assert!(PlayerMessage::decode(&bytes).is_err());
    }
}
