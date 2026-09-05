//! What the host tells a player.
//!
//! The original narrates a turn: a fleet stopped by mines, an order it could
//! not carry out, a colony founded. Each of those is a **message** — a numeric
//! id choosing a line of text, an object it is about, and up to seven
//! parameters — queued for one player and written into their turn file. The
//! record is decoded in [`stars_formats::message`].
//!
//! The text lives in the executable's resources, which this project does not
//! read and would not copy; the wording in [`Message::summary`] is this
//! project's own. The **ids** are the game's, and only ids read out of
//! `stars.2.7j.exe` itself are used, each cited where it is defined.

use stars_formats::MessageRecord;

/// Message ids this engine sends, each read from the routine that sends it.
///
/// The names are the ones the community reconstruction gives them, which agree
/// with what the binary does at each of these call sites — a useful check on
/// having read the right routine.
pub mod id {
    /// `idmHasCompletedAssignedOrders`: a fleet has run out of orders.
    /// `SatisfyOrders`' cancel path.
    pub const ORDERS_COMPLETE: u16 = 0x4e;
    /// `idmSomeoneHasSweptMinesMineField`: somebody cleared mines from a field
    /// of yours (`SweepForMines`, `10b8:76a4`).
    pub const YOUR_FIELD_SWEPT: u16 = 0xbe;
    /// `idmHasSweptMinesMineField`: your fleet cleared somebody's mines.
    pub const FLEET_SWEPT: u16 = 0xc2;
    /// `idmHasDispersedMines`: your fleet laid mines (`10b0:999e`).
    pub const MINES_LAID: u16 = 0xc3;
    /// `idmStarbaseHasSweptMinesMineField`: a starbase of yours cleared mines.
    pub const STARBASE_SWEPT: u16 = 0xf4;
    /// `idmCouldntGiveAwayBecauseThereColonistsBoard`: a fleet with colonists
    /// aboard cannot be given away (`10b0:9436`).
    pub const GIFT_HAS_COLONISTS: u16 = 0x149;
}

/// An object id as a message carries it: a fleet has bit 15 set.
#[must_use]
pub fn fleet_object(id: u16) -> i16 {
    (id | 0x8000) as i16
}

/// One message, for one player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Who it is for.
    pub player: usize,
    /// Which message: see [`id`].
    pub id: u16,
    /// What it is about.
    pub object: i16,
    /// Its arguments, in the order the original passes them.
    pub params: Vec<i16>,
}

impl Message {
    /// The two halves of a 32-bit parameter, low word first, which is how the
    /// original passes a count that will not fit in one.
    #[must_use]
    pub fn long(value: i32) -> [i16; 2] {
        #[allow(clippy::cast_possible_truncation)]
        [value as i16, (value >> 16) as i16]
    }

    /// This project's own wording for the messages it sends.
    ///
    /// Deliberately not the original's text, which belongs to the game; the
    /// id is what matters for a file, and a frontend is free to say it however
    /// it likes.
    #[must_use]
    pub fn summary(&self) -> String {
        let fleet = || i32::from(self.params.first().copied().unwrap_or(0));
        let long = |at: usize| {
            let low = i32::from(self.params.get(at).copied().unwrap_or(0)) & 0xFFFF;
            let high = i32::from(self.params.get(at + 1).copied().unwrap_or(0));
            (high << 16) | low
        };
        match self.id {
            id::ORDERS_COMPLETE => format!("Fleet {} has finished its orders.", fleet()),
            id::YOUR_FIELD_SWEPT => format!(
                "Someone swept {} mines from one of your minefields.",
                long(1)
            ),
            id::FLEET_SWEPT => format!("Fleet {} swept {} mines.", fleet(), long(1)),
            id::MINES_LAID => format!("Fleet {} laid {} mines.", fleet(), long(1)),
            id::STARBASE_SWEPT => format!(
                "Your starbase at planet {} swept {} mines.",
                fleet(),
                long(1)
            ),
            id::GIFT_HAS_COLONISTS => format!(
                "Fleet {} could not be given away: your colonists are aboard.",
                fleet()
            ),
            other => format!("Message {other}."),
        }
    }

    /// The record this message writes into a file.
    #[must_use]
    pub fn record(&self) -> MessageRecord {
        MessageRecord {
            id: self.id,
            object: self.object,
            params: self.params.clone(),
        }
    }

    /// Rebuild a message from a file record, for a known recipient.
    #[must_use]
    pub fn from_record(player: usize, record: &MessageRecord) -> Self {
        Self {
            player,
            id: record.id,
            object: record.object,
            params: record.params.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_parameter_splits_and_rejoins() {
        let message = Message {
            player: 0,
            id: id::FLEET_SWEPT,
            object: fleet_object(3),
            params: {
                let mut p = vec![3];
                p.extend_from_slice(&Message::long(70_000));
                p.extend_from_slice(&[1, 0, 1000, 1000]);
                p
            },
        };
        assert!(message.summary().contains("70000"), "{}", message.summary());
        // And it survives the file format.
        let record = message.record();
        let bytes = record.encode();
        let back = MessageRecord::decode(&bytes).expect("decodes");
        assert_eq!(back, record);
    }

    #[test]
    fn a_fleet_object_carries_its_flag() {
        assert_eq!(fleet_object(3), -32765);
        assert_eq!(fleet_object(3) as u16 & 0x1ff, 3);
    }
}
