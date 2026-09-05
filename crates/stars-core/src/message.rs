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

    // The Mystery Trader, all from `DoThingInteractions` (`1110:0b3a`) unless
    // noted. See [`crate::wormhole`].

    /// `idmMysteryTraderHasRefusedGiveCaptainAudience`: the fleet reached the
    /// Trader without the five thousand kilotons it wants (`1110:0cad`).
    pub const TRADER_REFUSED: u16 = 0x108;
    /// `idmHasAbsorbedMysteryTraderTraderHasGiven`: the Trader took the fleet
    /// and gave technology for it (`1110:0f57`).
    pub const TRADER_GAVE_TECH: u16 = 0x109;
    /// `idmHasAbsorbedMysteryTraderReturnTraderHas`: the same, for a player who
    /// already holds every one of the Trader's parts (`1110:0f5f`).
    pub const TRADER_GAVE_TECH_AGAIN: u16 = 0x10a;
    /// `IdmGiveTraderPart`'s usual message: a part changed hands
    /// (`1110:1a96`).
    pub const TRADER_GAVE_PART: u16 = 0x10b;
    /// The same, worded for a hull.
    pub const TRADER_GAVE_HULL: u16 = 0x10c;
    /// The Trader had nothing left to give: everything researched, every part
    /// already handed over (`1110:0e2a`).
    pub const TRADER_GAVE_NOTHING: u16 = 0x10e;
    /// The same, worded for the Genesis Device.
    pub const TRADER_GAVE_GENESIS: u16 = 0x10f;
    /// `idmMysteryTraderEyesCaptainSuspiciously...`: this player has already
    /// traded with this Trader (`1110:0d91`).
    pub const TRADER_ALREADY_MET: u16 = 0x118;
    /// `idmMysteryTraderHasDecidedMakeAnotherPass`: it reached its
    /// destination and turned round (`MoveThings`, `10b0:1e86`). Sent to every
    /// player.
    pub const TRADER_ANOTHER_PASS: u16 = 0xC0;
    /// `idmMysteryTraderHasUnexplicablyChangedHisCourse` (`10b0:1b40`). Sent to
    /// every player.
    pub const TRADER_CHANGED_COURSE: u16 = 0x130;
    /// `idmMysteryTraderHeadingHasVanishedOrdersHave`: the Trader a fleet was
    /// following has gone, and its orders now point at where it last was.
    pub const TRADER_VANISHED: u16 = 0x110;
    /// The Trader gave a ship (`1110:142e`).
    pub const TRADER_GAVE_SHIP: u16 = 0x14F;
    /// The Trader meant to give a ship and could not (`1110:133b`).
    pub const TRADER_TRIED_SHIP: u16 = 0x150;
}

/// An object id as a message carries it: a fleet has bit 15 set.
#[must_use]
pub fn fleet_object(id: u16) -> i16 {
    (id | 0x8000) as i16
}

/// How a message names a fleet that no longer exists (`WFromLpfl`,
/// `1038:2b10`).
///
/// A message about a live fleet points at it and lets the player click through;
/// one about a fleet that has just been destroyed cannot, so the game packs
/// enough to *name* it into a single word instead: the fleet number in the low
/// nine bits, its main design in the next four, and bit 13 set when the fleet
/// held more than one design — which is the difference between reporting
/// "Long Range Scout #7" and a plain "Fleet #7".
#[must_use]
pub fn fleet_name_word(fleet_id: u16, design: u8, mixed: bool) -> i16 {
    let word = (fleet_id & 0x01FF) | (u16::from(design) << 9) | if mixed { 0x2000 } else { 0 };
    word as i16
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
            id::TRADER_REFUSED => {
                format!("The Mystery Trader refused fleet {} an audience.", fleet())
            }
            id::TRADER_GAVE_TECH | id::TRADER_GAVE_TECH_AGAIN => format!(
                "The Mystery Trader absorbed a fleet and gave {} technology levels.",
                self.params.get(1).copied().unwrap_or(0)
            ),
            id::TRADER_GAVE_PART | id::TRADER_GAVE_HULL | id::TRADER_GAVE_GENESIS => {
                format!(
                    "The Mystery Trader absorbed a fleet and gave item {:#06x}.",
                    self.object
                )
            }
            id::TRADER_GAVE_NOTHING => {
                "The Mystery Trader absorbed a fleet and had nothing to give.".to_string()
            }
            id::TRADER_ALREADY_MET => format!(
                "The Mystery Trader has already traded with fleet {}.",
                fleet()
            ),
            id::TRADER_ANOTHER_PASS => {
                "The Mystery Trader has decided to make another pass.".to_string()
            }
            id::TRADER_CHANGED_COURSE => {
                "The Mystery Trader has changed course, or speed, or both.".to_string()
            }
            id::TRADER_VANISHED => format!(
                "The Mystery Trader fleet {} was following has gone; its orders now point at where it last was.",
                fleet()
            ),
            id::TRADER_TRIED_SHIP => {
                "The Mystery Trader meant to give a ship and could not.".to_string()
            }
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
