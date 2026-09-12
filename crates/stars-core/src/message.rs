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

use stars_formats::{MessageFilter, MessageRecord};

/// Message ids this engine sends, each read from the routine that sends it.
///
/// The names are the ones the community reconstruction gives them, which agree
/// with what the binary does at each of these call sites — a useful check on
/// having read the right routine.
pub mod id {
    /// `idmHaveBuiltFactory`: one factory went up on a planet.
    pub const BUILT_FACTORY: u16 = 53;
    /// `idmHaveBuiltFactories`: several did.
    ///
    /// The commonest message in the game, and the one the tutorial teaches
    /// you to filter: *"Your first message is quite common and we don't need
    /// to look at it every year."*
    pub const BUILT_FACTORIES: u16 = 54;
    /// `idmHaveBuiltMine` and `idmHaveBuiltMines`, its two neighbours.
    pub const BUILT_MINE: u16 = 55;
    /// Several mines.
    pub const BUILT_MINES: u16 = 56;
    /// `idmHasUnloaded`: a fleet put its cargo down somewhere.
    ///
    /// The third message the tutorial teaches you to filter, once the
    /// freighter is shuttling and sends one every year.
    pub const HAS_UNLOADED: u16 = 45;
    /// `idmHasLoadedMiningRobotsWorking`: the remote miner reports its haul.
    ///
    /// The fourth message the tutorial teaches you to filter.
    pub const MINING_ROBOTS_LOADED: u16 = 125;
    /// `idmHasDismantledKtMineralsWhichHaveDeposited`: a colony ship broke
    /// itself up on arrival. The fifth message the tutorial filters.
    pub const FLEET_DISMANTLED: u16 = 89;
    /// `idmHasCompletedAssignedOrders`: a fleet has run out of orders.
    /// `SatisfyOrders`' cancel path.
    pub const ORDERS_COMPLETE: u16 = 0x4e;
    /// `idmHasCompletedOrdersProductionQueueEmpty`: a planet's queue has
    /// been worked through and is empty. The turn-3 tutorial file carries
    /// one with the planet as object and parameter.
    pub const QUEUE_EMPTY: u16 = 0x3e;
    /// `idmColonistsControl`: the colonists dropped on an unowned planet
    /// have taken it (`DropColonists`, `10b8:34e2`, which sends `10`, or
    /// `11` for an Alternate Reality race). Object and parameter are the
    /// planet.
    pub const COLONISTS_CONTROL: u16 = 0x0a;
    /// `idmColonistsHaveDeployedOrbitalConstructionModuleHa`: the same, for
    /// an Alternate Reality race, whose colonists live on the starbase.
    pub const COLONISTS_CONTROL_AR: u16 = 0x0b;
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

    // What a brand-new game says, all from `GenerateWorld` (`1078:0136`):
    // four playing tips to every player, with no object, and then one about
    // the home planet. The turn-0 fixture's `Game.m1` holds exactly these
    // five, in this order, and the tutorial's first page counts them.

    /// `idmTipCanHideUnimportantMessagesClickingCheckmark`: the first of the
    /// four tips — that a message you do not want to see again can be
    /// filtered with the check mark.
    pub const TIP_FILTERING: u16 = 0x7f;
    /// `idmTipAddWaypointsSelectShipClickDesired`: how to give a fleet a
    /// waypoint.
    pub const TIP_WAYPOINTS: u16 = 0x80;
    /// `idmTipDesignOwnShipsPressF4Select`: that F4 opens the ship designer.
    pub const TIP_DESIGNER: u16 = 0x81;
    /// `idmTipPopupHelpAvailableManyDisplayedStatistics`: that many figures
    /// on the screen explain themselves when clicked.
    pub const TIP_POPUPS: u16 = 0x82;
    /// `idmHomePlanetPeopleReadyLeaveNestExplore`: about the home planet,
    /// whose id is both the object and the one parameter.
    pub const HOME_PLANET: u16 = 0xa9;

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

/// The families of message ids the filter treats as one thing.
///
/// `SetFilteringGroups` (`1030:a018`) does not silence one id: it silences
/// every other **wording of the same event** with it. The game has several
/// sentences for one happening — singular and plural, minerals and colonists,
/// the five ways a bombing run can go — and a player who does not want to read
/// one does not want to read any of them.
///
/// Each entry is an inclusive range of ids, read out of the routine's own
/// comparisons:
///
/// | ids | what they say |
/// |-----|---------------|
/// | `0x2b..=0x2e` | a fleet loaded, beamed, or unloaded cargo at a planet |
/// | `0x2f..=0x30` | your starbase built a ship, or several |
/// | `0x35..=0x36` | you built a factory, or several |
/// | `0x37..=0x38` | you built a mine, or several |
/// | `0x39..=0x3a` | you built a defence, or several |
/// | `0x42..=0x43` | you transferred cargo to another player |
/// | `0x44..=0x45` | you received cargo from another player |
/// | `0x46..=0x47` | a transfer arrived short |
/// | `0x48..=0x49` | a delivery arrived short |
/// | `0x4a..=0x4b` | a transfer arrived not at all |
/// | `0x4c..=0x4d` | a delivery arrived not at all |
/// | `0x60..=0x64` | your bombers hit a planet, five ways |
/// | `0x6a..=0x6e` | somebody bombed one of yours, the same five |
/// | `0x79..=0x7a` | a fleet loaded or beamed cargo from another fleet |
/// | `0x91..=0xa8` | a battle report, in any of its two dozen forms |
///
/// The pairs among these are adjacent ids, so a pair and a range are the same
/// rule; the original writes the pairs as `id ^ a ^ b`, which for two adjacent
/// ids comes to the same thing.
pub const FILTER_GROUPS: [(u16, u16); 15] = [
    (0x2b, 0x2e),
    (0x2f, 0x30),
    (0x35, 0x36),
    (0x37, 0x38),
    (0x39, 0x3a),
    (0x42, 0x43),
    (0x44, 0x45),
    (0x46, 0x47),
    (0x48, 0x49),
    (0x4a, 0x4b),
    (0x4c, 0x4d),
    (0x60, 0x64),
    (0x6a, 0x6e),
    (0x79, 0x7a),
    (0x91, 0xa8),
];

/// Every id that is filtered along with this one, itself included.
#[must_use]
pub fn filter_group(id: u16) -> std::ops::RangeInclusive<u16> {
    FILTER_GROUPS
        .iter()
        .find(|(lo, hi)| (*lo..=*hi).contains(&id))
        .map_or(id..=id, |(lo, hi)| *lo..=*hi)
}

/// Silence a message, or stop silencing it — and its whole family with it.
///
/// This is what the filter checkbox does: `SetFilteringGroups` (`1030:a018`).
pub fn set_filtered(filter: &mut MessageFilter, id: u16, hidden: bool) {
    for member in filter_group(id) {
        filter.set(member, hidden);
    }
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

/// What a message points at, and so what its **Goto** button does.
///
/// `SetMsgTitle` (`1030:7218`) classifies the message's object word into a
/// `mdMsgObj`, and the Goto button is enabled only when that comes out
/// non-zero. The word is not a plain id: negative values name a fleet or one of
/// several dialogs, and the top two bits pick between a planet, a component in
/// the browser and a place on the map.
///
/// ```text
/// -1                 nothing to go to
/// -2 -3 -4 -5 -7     one of the game's own windows: a report, the score
///                    sheet, the serial-number box
/// -6                 a THING, whose id is the first parameter
/// 0xc000 set         a component, shown in the browser
/// 0x4000 clear:
///     negative       a fleet, id in the low 15 bits
///     positive       a planet, id as it stands
/// 0x4000 set         a place on the map, from the first two parameters —
///                    which is how a battle report finds its battle
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goto {
    /// Nothing: the button is dead.
    None,
    /// A planet, by id.
    Planet(i16),
    /// A fleet, by id.
    Fleet(u16),
    /// A space object — a minefield, a wormhole, the Mystery Trader.
    Thing(u16),
    /// A place on the map, which is where a battle happened.
    Position(i16, i16),
    /// One of the original's own windows, which this engine has no equivalent
    /// for. The original enables the button; here it does nothing, so the
    /// button is left dead rather than lying about what it will do.
    Elsewhere,
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
            // The four playing tips and the home-planet greeting every new
            // game opens with. This wording is this project's, not the game's.
            id::TIP_FILTERING => {
                "Tip: a kind of message you would rather not see again can be switched \
                 off. Click the check mark at the top left of the Messages pane while \
                 one is showing and that kind stays out of the way from then on."
                    .to_string()
            }
            id::TIP_WAYPOINTS => {
                "Tip: to send a fleet somewhere, select it and then hold shift while you \
                 click its destination on the map. Each shift-click adds another stop \
                 to the route."
                    .to_string()
            }
            id::TIP_DESIGNER => {
                "Tip: the ships you start with are only a beginning. Press F4 to open \
                 the Ship Designer and put together designs of your own as your \
                 technology improves."
                    .to_string()
            }
            id::TIP_POPUPS => {
                "Tip: many of the figures on the screen will explain themselves. Click \
                 on a number in the Command or Selection Summary panes and a small \
                 window tells you where it comes from."
                    .to_string()
            }
            // One built carries only the planet; several carry the count
            // first and then the planet.
            id::BUILT_FACTORY => format!("A factory has been built on planet {}.", self.object),
            id::BUILT_FACTORIES => format!(
                "{} factories have been built on planet {}.",
                self.params.first().copied().unwrap_or(0),
                self.object
            ),
            id::BUILT_MINE => format!("A mine has been built on planet {}.", self.object),
            id::BUILT_MINES => format!(
                "{} mines have been built on planet {}.",
                self.params.first().copied().unwrap_or(0),
                self.object
            ),
            id::QUEUE_EMPTY => format!(
                "Planet {} has finished everything in its production queue, which is now \
                 empty.",
                self.object
            ),
            id::FLEET_DISMANTLED => format!(
                "Fleet {} has been dismantled and its {}kT of minerals put down on planet {}.",
                i32::from(self.params.first().copied().unwrap_or(0)) & 0x1ff,
                long(1),
                self.object
            ),
            id::COLONISTS_CONTROL | id::COLONISTS_CONTROL_AR => format!(
                "Your colonists have settled planet {} and it is yours.",
                self.object
            ),
            id::HOME_PLANET => format!(
                "Planet {} is your home world. Your people have grown restless and are \
                 ready to leave the nest: explore the stars around you, find worlds to \
                 settle, and build the ships to take you there.",
                self.params.first().copied().unwrap_or(self.object)
            ),
            other => format!("Message {other}."),
        }
    }

    /// What this message points at.
    ///
    /// See [`Goto`]. A message about a fleet that no longer exists points at
    /// nothing, which is why this needs to know which fleets there are.
    #[must_use]
    pub fn goto(&self, fleets: &[u16]) -> Goto {
        let word = self.object;
        match word {
            -1 => Goto::None,
            -6 => self
                .params
                .first()
                .map_or(Goto::None, |id| Goto::Thing(*id as u16)),
            -2 | -3 | -4 | -5 | -7 => Goto::Elsewhere,
            _ => {
                let bits = word as u16;
                if bits & 0xC000 == 0xC000 {
                    // A component, shown in the part browser.
                    Goto::Elsewhere
                } else if bits & 0x4000 == 0 {
                    if word < 0 {
                        let id = bits & 0x7FFF;
                        if fleets.contains(&id) {
                            Goto::Fleet(id)
                        } else {
                            Goto::None
                        }
                    } else {
                        Goto::Planet(word)
                    }
                } else if bits & 0x3FFF == 0x800 {
                    Goto::Elsewhere
                } else {
                    let x = self.params.first().copied().unwrap_or(0);
                    let y = self.params.get(1).copied().unwrap_or(0);
                    Goto::Position(x, y)
                }
            }
        }
    }

    /// Whether a player's filter hides this message.
    ///
    /// The filter is a **reading** choice, not a rule of the game: the message
    /// is still sent, still written to the file, and still counted. All it
    /// changes is whether the list steps over it.
    #[must_use]
    pub fn hidden_by(&self, filter: &MessageFilter) -> bool {
        filter.hidden(self.id)
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

    /// A filter checkbox silences a family of messages, not one sentence.
    #[test]
    fn filtering_one_wording_filters_them_all() {
        let mut filter = MessageFilter::new();
        // "You have built a factory" and "You have built 3 factories".
        set_filtered(&mut filter, 0x35, true);
        assert!(filter.hidden(0x35));
        assert!(filter.hidden(0x36));
        // The mine messages next door are untouched.
        assert!(!filter.hidden(0x37));

        // A battle report silences every form of battle report.
        set_filtered(&mut filter, 0xa0, true);
        for id in 0x91..=0xa8 {
            assert!(filter.hidden(id), "battle report {id:#04x}");
        }
        assert!(!filter.hidden(0x90));
        assert!(!filter.hidden(0xa9));

        // And clearing one clears the family.
        set_filtered(&mut filter, 0x91, false);
        assert!((0x91..=0xa8).all(|id| !filter.hidden(id)));
    }

    /// A message with no family is filtered on its own.
    #[test]
    fn an_ungrouped_message_stands_alone() {
        let mut filter = MessageFilter::new();
        set_filtered(&mut filter, id::MINES_LAID, true);
        assert_eq!(
            filter_group(id::MINES_LAID),
            id::MINES_LAID..=id::MINES_LAID
        );
        assert!(filter.hidden(id::MINES_LAID));
        assert!(!filter.hidden(id::MINES_LAID + 1));

        let message = Message {
            player: 0,
            id: id::MINES_LAID,
            object: fleet_object(1),
            params: vec![1],
        };
        assert!(message.hidden_by(&filter));
        assert!(!Message {
            id: id::FLEET_SWEPT,
            ..message
        }
        .hidden_by(&filter));
    }
}
