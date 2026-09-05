//! Messages the host writes to a player: `rtMsg` (type 12).
//!
//! The turn generator tells a player everything that happened to them — a
//! fleet stopped by mines, a colony founded, an order it could not carry out —
//! by queueing a **message**: a numeric id, an object it is about, and up to
//! seven parameters. The text itself is a format string in the executable's
//! resources, which this project does not read; what the file carries, and what
//! this module decodes, is the id and its arguments.
//!
//! ## Layout
//!
//! From `PackageUpMsg` (`1030:802a`), which builds the record, minus its first
//! byte: in memory the record begins with a byte packing the recipient and the
//! parameter length, and neither survives into the file, where the recipient is
//! the file's owner and the length is the block's.
//!
//! ```text
//! word 0: bits 0..=8   message id
//!         bits 9..=15  which parameters were stored as words, one bit each
//! word 2: the object the message is about (a fleet id with bit 15 set, a
//!         planet id, or a plain number)
//! byte 4..: the parameters, in order — one byte each unless the mask says two
//! ```
//!
//! **How many parameters** a message has is not in the record: it comes from a
//! table indexed by message id, which is why decoding one needs
//! [`PARAMETER_COUNT`]. A parameter that fits in a byte is stored as one, so the
//! same message id can produce records of different lengths.

use crate::block::BlockType;
use crate::file::StarsFile;

/// The block type id of a message.
pub const MESSAGE_BLOCK: u8 = 12;

/// How many parameters each message id carries, read out of the table at
/// `1030:5b0e` — one byte per id, ending where the next routine's padding
/// begins. An id past the end carries none.
pub const PARAMETER_COUNT: [u8; 387] = [
    4, 5, 3, 4, 4, 2, 2, 4, 2, 3, 1, 1, 2, 1, 4, 3, 3, 3, 2, 2, 4, 4, 3, 3, 4, 4, 4, 3, 3, 3, 3, 4,
    4, 3, 3, 1, 1, 5, 3, 1, 2, 2, 1, 6, 6, 6, 6, 2, 3, 3, 4, 3, 4, 1, 2, 1, 2, 1, 2, 4, 5, 5, 1, 1,
    1, 1, 5, 5, 5, 5, 7, 7, 7, 7, 5, 5, 5, 5, 1, 2, 3, 1, 3, 2, 3, 2, 2, 1, 1, 4, 4, 2, 6, 6, 3, 3,
    3, 2, 3, 4, 4, 4, 4, 4, 5, 5, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 2, 2, 2, 1, 3, 5, 5, 4, 3, 6, 2, 0,
    0, 0, 0, 1, 1, 1, 1, 2, 3, 4, 4, 2, 2, 5, 5, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 6, 4, 4, 5, 5, 7,
    5, 5, 6, 6, 4, 5, 5, 6, 7, 1, 2, 2, 2, 1, 2, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 3, 1, 0, 2, 6, 1,
    1, 3, 7, 3, 3, 5, 6, 7, 6, 4, 5, 6, 5, 2, 3, 2, 3, 1, 1, 2, 2, 4, 5, 6, 5, 6, 2, 4, 2, 4, 3, 1,
    2, 1, 4, 3, 4, 4, 3, 3, 4, 4, 4, 5, 4, 4, 6, 5, 5, 5, 1, 2, 7, 1, 1, 2, 1, 1, 3, 2, 1, 2, 2, 2,
    0, 1, 1, 0, 1, 2, 2, 3, 1, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 5, 5, 6, 6, 0, 1, 5, 1, 1, 2, 2, 2, 2,
    2, 6, 6, 2, 5, 5, 3, 3, 3, 1, 1, 1, 4, 3, 3, 1, 1, 4, 4, 4, 4, 2, 3, 1, 1, 4, 2, 2, 4, 5, 6, 7,
    4, 4, 6, 6, 5, 3, 4, 3, 1, 1, 2, 1, 1, 2, 2, 2, 1, 2, 1, 0, 1, 1, 2, 3, 4, 2, 4, 3, 2, 2, 2, 5,
    6, 7, 4, 5, 6, 1, 3, 2, 3, 4, 4, 4, 4, 4, 5, 5, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 3, 3, 2, 2, 1, 1,
    2, 4, 1,
];

/// How many parameters a message id carries.
#[must_use]
pub fn parameter_count(id: u16) -> usize {
    PARAMETER_COUNT
        .get(usize::from(id))
        .map_or(0, |n| usize::from(*n))
}

/// A message to one player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRecord {
    /// The message id, which chooses the text. Nine bits.
    pub id: u16,
    /// What the message is about: a fleet (its id with bit 15 set), a planet,
    /// or a bare number, depending on the message.
    pub object: i16,
    /// The message's arguments, in the order the sender passed them.
    pub params: Vec<i16>,
}

impl MessageRecord {
    /// Decode the **first** message in a decrypted type-12 payload.
    ///
    /// One block often holds several: the host appends every message for a
    /// player into one buffer and the file writer emits the buffer, so a block
    /// is a run of records rather than a single one. Use [`Self::decode_all`]
    /// for the whole block.
    ///
    /// Returns `None` if the payload is shorter than the four-byte header or
    /// than the parameters its id calls for.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        Self::decode_at(data).map(|(record, _)| record)
    }

    /// Every message in a block, in order.
    #[must_use]
    pub fn decode_all(data: &[u8]) -> Vec<Self> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < data.len() {
            let Some((record, length)) = Self::decode_at(&data[at..]) else {
                break;
            };
            at += length;
            out.push(record);
        }
        out
    }

    /// Encode a run of messages back into one block.
    #[must_use]
    pub fn encode_all(records: &[Self]) -> Vec<u8> {
        records.iter().flat_map(Self::encode).collect()
    }

    /// Decode one message and say how many bytes it took.
    #[must_use]
    fn decode_at(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 4 {
            return None;
        }
        let head = u16::from_le_bytes([data[0], data[1]]);
        let id = head & 0x01FF;
        let wide = head >> 9;
        let object = i16::from_le_bytes([data[2], data[3]]);

        let mut params = Vec::new();
        let mut at = 4;
        for index in 0..parameter_count(id) {
            if wide & (1 << index) == 0 {
                params.push(i16::from(*data.get(at)?));
                at += 1;
            } else {
                let (low, high) = (*data.get(at)?, *data.get(at + 1)?);
                params.push(i16::from_le_bytes([low, high]));
                at += 2;
            }
        }
        Some((Self { id, object, params }, at))
    }

    /// Re-encode this message as a type-12 payload.
    ///
    /// A parameter that fits in a byte is written as one, which is what the
    /// original does — so a record round-trips only if it was built the same
    /// way. The mask is rebuilt from the values rather than carried.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut wide = 0u16;
        let mut tail = Vec::new();
        for (index, value) in self.params.iter().enumerate() {
            // The original tests the high **byte**, so a negative value or
            // anything over 255 takes two bytes.
            if value.to_le_bytes()[1] == 0 {
                tail.push(value.to_le_bytes()[0]);
            } else {
                wide |= 1 << index;
                tail.extend_from_slice(&value.to_le_bytes());
            }
        }
        let head = (self.id & 0x01FF) | (wide << 9);
        let mut out = Vec::with_capacity(4 + tail.len());
        out.extend_from_slice(&head.to_le_bytes());
        out.extend_from_slice(&self.object.to_le_bytes());
        out.extend_from_slice(&tail);
        out
    }
}

/// Decode every message block in a decoded [`StarsFile`], in file order.
#[must_use]
pub fn message_records(file: &StarsFile) -> Vec<MessageRecord> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::Message)
        .flat_map(|b| MessageRecord::decode_all(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real message out of `fixtures/`: id 7 with four byte parameters.
    #[test]
    fn decodes_a_byte_only_message() {
        let data = [0x07, 0x00, 0xf3, 0x00, 0x39, 0xf3, 0xfa, 0x00];
        let m = MessageRecord::decode(&data).expect("decodes");
        assert_eq!(m.id, 7);
        assert_eq!(parameter_count(7), 4);
        assert_eq!(m.object, 0x00f3);
        assert_eq!(m.params, vec![0x39, 0xf3, 0xfa, 0x00]);
        assert_eq!(m.encode(), data);
    }

    /// And one whose first parameter did not fit in a byte, so the mask says
    /// it is a word.
    #[test]
    fn decodes_a_mixed_width_message() {
        let data = [0x8f, 0x02, 0x0a, 0x8e, 0x0a, 0x0e, 0xd4];
        let m = MessageRecord::decode(&data).expect("decodes");
        assert_eq!(m.id, 0x8f);
        assert_eq!(parameter_count(0x8f), 2);
        assert_eq!(m.params, vec![0x0e0a, 0xd4]);
        assert_eq!(m.encode(), data);
    }

    /// The count table is what the executable holds.
    #[test]
    fn the_parameter_table_is_the_games_own() {
        assert_eq!(parameter_count(0), 4);
        assert_eq!(parameter_count(7), 4);
        assert_eq!(parameter_count(0xc2), 7, "a minefield sweep report");
        assert_eq!(parameter_count(0xbe), 6);
        assert_eq!(parameter_count(9999), 0, "an id past the table");
    }
}
