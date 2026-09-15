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
    /// `idmHasLoaded`: a Transport task took minerals aboard
    /// (`SatisfyOrders`). The fleet is the object; parameters
    /// `[fleet, amount lo, amount hi, kind, target class, target]`.
    pub const HAS_LOADED: u16 = 0x2b;
    /// `idmHasBeamed`: the same, for colonists coming aboard.
    pub const HAS_BEAMED_UP: u16 = 0x2c;
    /// `idmHasUnloaded`: a fleet put minerals down somewhere, in the same
    /// shape.
    ///
    /// The third message the tutorial teaches you to filter, once the
    /// freighter is shuttling and sends one every year.
    pub const HAS_UNLOADED: u16 = 45;
    /// `idmHasBeamed2`: colonists put down.
    pub const HAS_BEAMED_DOWN: u16 = 0x2e;
    /// `idmHasLoadedMiningRobotsWorking`: a Transport load at an unowned
    /// planet where the player's own remote miner sits took what the
    /// robots had dug (`SatisfyOrders`, `turn3.c`, the `fMining` path);
    /// `[fleet, amount lo, amount hi, kind, miner, planet]`.
    ///
    /// The fourth message the tutorial teaches you to filter.
    pub const MINING_ROBOTS_LOADED: u16 = 125;
    /// `idmScientistsHaveCompletedResearchTechLevelWill`: a level gained,
    /// `[level, field, the field research goes on in]`, with the object
    /// `-2` so Goto opens the Research dialog (`DoResearch`, `turn2.c`).
    pub const TECH_LEVEL_GAINED: u16 = 0x50;
    /// `idmScientistsHaveCompletedResearchTechLevelPrimary`: the same for a
    /// race with Generalized Research, whose primary field it names.
    pub const TECH_LEVEL_GAINED_GENERAL: u16 = 0x136;
    /// `idmRecentBreakthroughHasAlsoGivenBenefit`: a level gained has
    /// brought a component within reach — `UpdateResearchStatus`
    /// (`10b8:80fe`) walks every category after each level and reports
    /// each part now buildable whose requirement in that field is exactly
    /// the new level; `[field, category bits, item]`, with the part's
    /// browser word (`0xc000 | category index << 8 | item`) as the object,
    /// so Goto opens the Technology Browser on it.
    pub const BREAKTHROUGH_PART: u16 = 0x5f;
    /// `idmRecentBreakthroughHasAlsoGivenHullType`: the same for a ship
    /// hull, with the Ship Design dialog (`-3`) for its Goto.
    pub const BREAKTHROUGH_HULL: u16 = 0x78;
    /// `idmRecentBreakthroughHasAlsoGivenHullDesign`: a starbase hull.
    pub const BREAKTHROUGH_STARBASE_HULL: u16 = 0xd0;
    /// `idmRecentBreakthroughHasAlsoTaughtHowBuild`: planetary items 9
    /// to 13, the defences, which upgrade every planet's at once.
    pub const BREAKTHROUGH_DEFENSE: u16 = 0x145;
    /// `idmRecentBreakthroughHasAlsoTaughtHowBuild2`: planetary items 0 to
    /// 8, the scanners, likewise.
    pub const BREAKTHROUGH_SCANNER: u16 = 0x157;
    /// `idmColonistsHaveDiedOffLongerControlPlanet`: a planet's people are
    /// gone and the planet with them — `UpdatePopulations` (`10b8:50a0`),
    /// when the year's change took the count to nothing; `[planet]`, the
    /// planet the object. An Alternate Reality race gets the next id.
    pub const COLONISTS_DIED_OFF: u16 = 0x23;
    /// `idmColonistsHaveJumpedShipLongerControlPlanet`: the same when the
    /// count was already nothing — the colonists were taken off, not lost.
    pub const COLONISTS_JUMPED_SHIP: u16 = 0x40;
    /// `idmHasDismantledKtMineralsWhichHaveDeposited`: a colony ship broke
    /// itself up on arrival. The fifth message the tutorial filters.
    pub const FLEET_DISMANTLED: u16 = 89;
    /// `idmHasCompletedAssignedOrders`: a fleet has arrived at the last of
    /// its waypoints with nothing there to do — `KillUsedWaypoints`
    /// (`1080:189a`) at the end of the movement pass, and `SatisfyOrders`'
    /// cancel path. The fleet is the object and the first parameter. It is
    /// the message the tutorial's Goto buttons lean on from 2402 onward.
    pub const ORDERS_COMPLETE: u16 = 0x4e;
    /// `idmHasRunFuel`: a fleet has emptied its tank short of its waypoint
    /// and cannot move at any warp (`MoveFleets`, `10b0:42c3`). The fleet is
    /// the object and the first parameter.
    pub const OUT_OF_FUEL: u16 = 0x27;
    /// `idmHasRunFuelFleetsSpeedHasDecreased`: the same, but the engines run
    /// free at some warp, so the leg has been slowed to it — the second
    /// parameter.
    pub const OUT_OF_FUEL_SLOWED: u16 = 0x8b;
    /// `idmStarbaseHasBuiltNew`: one ship built, with the new fleet as the
    /// object and `[planet, (player << 5) | design]` as parameters
    /// (`FBuildObject`, `10b8:19b2`).
    pub const SHIP_BUILT: u16 = 0x2f;
    /// `idmStarbaseHasBuiltNewShips`: several, with the count between the
    /// planet and the design word.
    pub const SHIPS_BUILT: u16 = 0x30;
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
    /// A battle was fought with the player in it: object the battle id
    /// with bit 14 set (the Goto finds the place from the parameters, or
    /// the Battles report), parameters the place (`-1` and the planet,
    /// or the coordinates), then the player's ships, their losses, the
    /// enemy's ships and its losses. The original words the outcome a
    /// dozen ways (`SendBattleMessages`, ids `0x8d`–`0xa8` and
    /// `0x113`–`0x116`); this is its general case, `0xa8`.
    pub const BATTLE: u16 = 0xa8;
    /// A battle was fought within sight of one of the player's fleets or
    /// at a planet of theirs, without them (`0xfa`; the original uses
    /// `0xf9` for the planet's owner).
    pub const BATTLE_SEEN: u16 = 0xfa;
    /// The player's fleet bombed a planet (`0x60` and its neighbours in
    /// `DoBombing`, which words the result two dozen ways): parameters
    /// the fleet, the planet, the colonists killed (in hundreds), the
    /// installations destroyed, the defences' stopping share in
    /// hundredths of a percent, and whether several fleets bombed.
    pub const BOMBED: u16 = 0x60;
    /// A planet of the player's was bombed (`0x6a`), the same parameters.
    pub const BOMBED_YOU: u16 = 0x6a;
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
    /// `idmHaveFoundPlanetOccupiedSomeoneElseCurrently`: a planet seen for
    /// the first time turns out to be somebody's. The **client** sends the
    /// six "found a planet" messages to itself as it reads a planet record
    /// flagged first-year from its turn file (`file.c`), so they follow the
    /// host's messages, one per new planet in id order. Object and first
    /// parameter are the planet; the second is `owner | 0x30`.
    pub const FOUND_OCCUPIED: u16 = 0xaa;
    /// `idmHaveFoundNewPlanetWhichUnfortunatelyHabitable`: a new planet the
    /// race cannot live on. Parameters `[deaths, planet]`, the first being
    /// ten times the (negative) value, as a percentage lost a year.
    pub const FOUND_HOSTILE: u16 = 0xab;
    /// `idmHaveFoundNewHabitablePlanetColonistsWill`: a new planet the race
    /// can live on. Parameters `[growth, planet]`, the first the value times
    /// the race's true maximum growth.
    pub const FOUND_HABITABLE: u16 = 0xac;
    /// `idmHaveFoundNewPlanetDontKnowIf`: a new planet known only from a
    /// distance, at minimal detail.
    pub const FOUND_UNKNOWN: u16 = 0xad;
    /// `idmHaveFoundNewPlanetWhichHaveAbility`: a new planet the race could
    /// terraform into range. Parameters `[growth, planet]`, the growth
    /// figured from the terraformed value.
    pub const FOUND_TERRAFORMABLE: u16 = 0xae;
    /// `idmHaveInfoNewPlanetIfColonizeCan`: a Claim Adjuster's version of
    /// the same, `[planet, value]`.
    pub const FOUND_CLAIM_ADJUSTER: u16 = 0x15d;

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

    // Random events — see [`crate::events`]. `MeteorStrike` (`10b8:560e`)
    // sends `0x83 + size` to everyone but the (non-AR) owner, who gets
    // `0x87 + size`; the Exodus game's `.m6` holds `0x84` with the planet
    // as its one parameter.
    /// A meteor struck a planet; add the size (0 to 3).
    pub const METEOR: u16 = 0x83;
    /// A meteor struck *your* planet; add the size (0 to 3).
    pub const METEOR_YOURS: u16 = 0x87;
    /// Your planet's climate changed (`PlanetaryClimateChange`,
    /// `10b8:5c54`): the planet, then the variable.
    pub const CLIMATE_CHANGE: u16 = 0xfd;
    /// New minerals were found on your planet (`DiscoverNewMinerals`,
    /// `10b8:5e0c`): the planet, then the mineral.
    pub const NEW_MINERALS: u16 = 0xfe;
    /// A Mystery Trader has set out (`10b8:5efa`), told to everyone with
    /// [`super::THING_OBJECT`] and the Trader's full id.
    pub const TRADER_APPEARED: u16 = 0x12b;

    // A mineral packet landing — `MoveThings` from `10b0:1f03`, see
    // [`crate::packet::land`]. All to the planet's owner but the
    // terraforming four, which go to the thrower. The parameters are the
    // planet, then the mass as a long (low word, high word) or the thrower.
    /// The planet's driver caught the packet, or it did no harm; the planet,
    /// the thrower, the mass.
    pub const PACKET_CAUGHT: u16 = 0xd5;
    /// A packet did damage past a driver, colonists only; the planet, the
    /// mass, the thrower, the colonists killed in hundreds.
    pub const PACKET_DAMAGE: u16 = 0xd6;
    /// As [`PACKET_DAMAGE`], with defences destroyed as a fifth parameter.
    pub const PACKET_DAMAGE_DEFENCES: u16 = 0xd7;
    /// As [`PACKET_DAMAGE`], at a planet with no driver.
    pub const PACKET_DAMAGE_NO_DRIVER: u16 = 0xd8;
    /// As [`PACKET_DAMAGE_DEFENCES`], at a planet with no driver.
    pub const PACKET_DAMAGE_DEFENCES_NO_DRIVER: u16 = 0xd9;
    /// A packet killed everyone on the planet; the planet, the thrower.
    pub const PACKET_WIPED_OUT: u16 = 0xda;
    /// A packet landed on a planet with no driver and did no harm; as
    /// [`PACKET_CAUGHT`].
    pub const PACKET_HARMLESS: u16 = 0x146;
    /// A packet hit a planet of yours with nobody on it; the planet, the
    /// thrower.
    pub const PACKET_HIT_EMPTY: u16 = 0x181;
    /// Your packet moved a planet's original environment: whether upward,
    /// the variable, the planet, the clicks.
    pub const PACKET_TERRAFORMED_ORIG: u16 = 0x131;
    /// As [`PACKET_TERRAFORMED_ORIG`], on somebody else's planet.
    pub const PACKET_TERRAFORMED_ORIG_THEIRS: u16 = 0x132;
    /// Your packet terraformed a planet: whether upward, the variable, the
    /// planet, the variable in the high byte over the new value.
    pub const PACKET_TERRAFORMED: u16 = 0x133;
    /// As [`PACKET_TERRAFORMED`], on somebody else's planet.
    pub const PACKET_TERRAFORMED_THEIRS: u16 = 0x134;
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
    /// The Research dialog: object `-2`, which `MessageWndProc`'s Goto arm
    /// (`1030:6d8d`, mode 3) answers by posting the Commands menu's
    /// `&Research...` (`0x7e`). Research reports go here.
    Research,
    /// The Ship Design dialog: object `-3` (mode 5, menu `0x7d`).
    ShipDesign,
    /// The Score sheet: object `-4` (mode 8, menu `0x5f`).
    Score,
    /// The serial-number box: object `-5` (mode 9, dialog `0x56`).
    SerialNumber,
    /// The Battles report: object `-7` (mode `0xb`, menu `0x901`).
    BattleReport,
    /// Player Relations: object `0x4800` (mode 7, menu `0x7de`).
    PlayerRelations,
    /// A component in the Technology Browser: an object word with bits
    /// 14 and 15 set (mode 4), the category in bits 8..=11 and the item
    /// in the low byte.
    Part(u16),
}

/// The object word that sends a message's Goto to the Research dialog.
///
/// `DoResearch` (`turn2.c`) passes `-2` with every tech-level report.
pub const RESEARCH_OBJECT: i16 = -2;
/// The object word of a message about a `THING`, whose full id is the
/// first parameter.
pub const THING_OBJECT: i16 = -6;

/// How a summary names the things it mentions. The engine has only ids;
/// a frontend that knows the universe's planet names and its fleets'
/// designs supplies the rest.
pub trait Names {
    /// A planet, by id.
    fn planet(&self, id: i16) -> String;
    /// A fleet of the player's, by id.
    fn fleet(&self, id: u16) -> String;
}

/// The engine's own naming: the ids, spelt out.
pub struct PlainNames;

impl Names for PlainNames {
    fn planet(&self, id: i16) -> String {
        format!("planet {id}")
    }

    fn fleet(&self, id: u16) -> String {
        format!("fleet {id}")
    }
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
        self.summary_with(&PlainNames)
    }

    /// The summary, with planets and fleets named by `names` — the
    /// frontend's names for them, where it has them.
    #[must_use]
    pub fn summary_with(&self, names: &dyn Names) -> String {
        let param = |at: usize| self.params.get(at).copied().unwrap_or(0);
        let fleet_id = || u16::try_from(i32::from(param(0)) & 0x1ff).unwrap_or(0);
        let fleet = || names.fleet(fleet_id());
        let planet = |id: i16| names.planet(id);
        let long = |at: usize| {
            let low = i32::from(param(at)) & 0xFFFF;
            let high = i32::from(param(at + 1));
            (high << 16) | low
        };
        let cargo_kind = |kind: i16| match kind {
            0 => "ironium",
            1 => "boranium",
            2 => "germanium",
            3 => "colonists",
            _ => "fuel",
        };
        let place = || {
            let x = param(0);
            let y = param(1);
            if x == -1 {
                planet(y)
            } else {
                format!("({x}, {y})")
            }
        };
        let object_planet = || planet(self.object);
        let object_fleet =
            || names.fleet(u16::try_from(i32::from(self.object) & 0x1ff).unwrap_or(0));
        match self.id {
            id::BATTLE => format!(
                "A battle took place at {}: {} of your ships fought {} of theirs; you lost {}, they lost {}.",
                place(),
                param(2),
                param(4),
                param(3),
                param(5)
            ),
            id::BATTLE_SEEN => format!("A battle took place at {}.", place()),
            id::BOMBED => format!(
                "{} has bombed {}, killing {} colonists and destroying {} installations.",
                fleet(),
                planet(param(1)),
                i32::from(param(2)) * 100,
                param(3)
            ),
            id::BOMBED_YOU => format!(
                "{} has been bombed by {}: {} colonists killed and {} installations destroyed.",
                planet(param(1)),
                fleet(),
                i32::from(param(2)) * 100,
                param(3)
            ),
            id::ORDERS_COMPLETE => format!("{} has finished its orders.", fleet()),
            id::HAS_LOADED | id::HAS_BEAMED_UP => format!(
                "{} has taken {}kT of {} aboard at {}.",
                fleet(),
                long(1),
                cargo_kind(param(3)),
                planet(param(5))
            ),
            id::MINING_ROBOTS_LOADED => format!(
                "{} has taken {}kT of {} aboard, dug at {} by the mining robots of {}.",
                fleet(),
                long(1),
                cargo_kind(param(3)),
                planet(param(5)),
                names.fleet(u16::try_from(i32::from(param(4)) & 0x1ff).unwrap_or(0))
            ),
            id::TECH_LEVEL_GAINED | id::TECH_LEVEL_GAINED_GENERAL => {
                let field = |at: usize| {
                    usize::try_from(param(at))
                        .ok()
                        .and_then(|i| crate::research::TechField::ALL.get(i))
                        .map_or("?", |f| f.name())
                };
                format!(
                    "Your scientists have reached level {} in {}; research goes on in {}.",
                    param(0),
                    field(1),
                    field(2)
                )
            }
            id::BREAKTHROUGH_PART
            | id::BREAKTHROUGH_HULL
            | id::BREAKTHROUGH_STARBASE_HULL
            | id::BREAKTHROUGH_DEFENSE
            | id::BREAKTHROUGH_SCANNER => {
                let field = usize::try_from(param(0))
                    .ok()
                    .and_then(|i| crate::research::TechField::ALL.get(i))
                    .map_or("?", |f| f.name());
                let part = crate::parts::part(
                    param(1) as u16,
                    usize::try_from(param(2)).unwrap_or(usize::MAX),
                )
                .map_or("a new part", |p| p.name);
                match self.id {
                    id::BREAKTHROUGH_HULL => format!(
                        "Your recent breakthrough in {field} has also given you the {part} hull type; copy it from the designer's available hulls to build ships with it."
                    ),
                    id::BREAKTHROUGH_STARBASE_HULL => format!(
                        "Your recent breakthrough in {field} has also given you the {part} hull; copy it from the designer's available starbase hulls to design starbases with it."
                    ),
                    id::BREAKTHROUGH_DEFENSE => format!(
                        "Your recent breakthrough in {field} has also taught you how to build {part} defences; every planetary defence you have is upgraded."
                    ),
                    id::BREAKTHROUGH_SCANNER => format!(
                        "Your recent breakthrough in {field} has also taught you how to build the {part} scanner; every planetary scanner you have is upgraded."
                    ),
                    _ => format!(
                        "Your recent breakthrough in {field} has also given you the {part}."
                    ),
                }
            }
            id::COLONISTS_DIED_OFF | 0x24 => format!(
                "All of your colonists on {} have died off; the planet is no longer yours.",
                planet(0)
            ),
            id::COLONISTS_JUMPED_SHIP | 0x41 => format!(
                "All of your colonists on {} have left; the planet is no longer yours.",
                planet(0)
            ),
            id::HAS_UNLOADED | id::HAS_BEAMED_DOWN => format!(
                "{} has put {}kT of {} down at {}.",
                fleet(),
                long(1),
                cargo_kind(param(3)),
                planet(param(5))
            ),
            id::OUT_OF_FUEL => format!("{} has no fuel left and cannot move.", fleet()),
            id::OUT_OF_FUEL_SLOWED => format!(
                "{} has no fuel left and has slowed to warp {}.",
                fleet(),
                param(1)
            ),
            id::YOUR_FIELD_SWEPT => format!(
                "Someone swept {} mines from one of your minefields.",
                long(1)
            ),
            id::FLEET_SWEPT => format!("{} swept {} mines.", fleet(), long(1)),
            id::MINES_LAID => format!("{} laid {} mines.", fleet(), long(1)),
            id::STARBASE_SWEPT => format!(
                "Your starbase at {} swept {} mines.",
                planet(param(0)),
                long(1)
            ),
            id::GIFT_HAS_COLONISTS => format!(
                "{} could not be given away: your colonists are aboard.",
                fleet()
            ),
            id::TRADER_REFUSED => {
                format!("The Mystery Trader refused {} an audience.", fleet())
            }
            id::TRADER_GAVE_TECH | id::TRADER_GAVE_TECH_AGAIN => format!(
                "The Mystery Trader absorbed a fleet and gave {} technology levels.",
                param(1)
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
                "The Mystery Trader has already traded with {}.",
                fleet()
            ),
            id::TRADER_ANOTHER_PASS => {
                "The Mystery Trader has decided to make another pass.".to_string()
            }
            id::TRADER_CHANGED_COURSE => {
                "The Mystery Trader has changed course, or speed, or both.".to_string()
            }
            id::TRADER_VANISHED => format!(
                "The Mystery Trader {} was following has gone; its orders now point at where it last was.",
                fleet()
            ),
            id::TRADER_TRIED_SHIP => {
                "The Mystery Trader meant to give a ship and could not.".to_string()
            }
            id::TRADER_APPEARED => {
                "A Mystery Trader has entered the galaxy. Send a fleet carrying at \
                 least 1,200kT of minerals to meet it and it may trade you something \
                 for them."
                    .to_string()
            }
            0x83..=0x86 => {
                let size = ["small", "medium-sized", "large", "huge"]
                    [usize::from(self.id - 0x83)];
                format!(
                    "A {size} meteor has struck {}. The planet's surface minerals and \
                     climate have changed.",
                    planet(param(0))
                )
            }
            0x87..=0x8a => {
                let size = ["small", "medium-sized", "large", "huge"]
                    [usize::from(self.id - 0x87)];
                format!(
                    "A {size} meteor has struck your planet {}: colonists were killed, \
                     the climate and the surface minerals have changed, and anything in \
                     the production queue that was not an auto-build item has been \
                     dropped.",
                    planet(param(0))
                )
            }
            id::PACKET_CAUGHT | id::PACKET_HARMLESS => format!(
                "A mineral packet of {}kT from player {} has arrived at {} and been \
                 recovered without harm.",
                long(2),
                param(1) + 1,
                planet(param(0))
            ),
            id::PACKET_DAMAGE
            | id::PACKET_DAMAGE_DEFENCES
            | id::PACKET_DAMAGE_NO_DRIVER
            | id::PACKET_DAMAGE_DEFENCES_NO_DRIVER => {
                let driver = matches!(self.id, id::PACKET_DAMAGE | id::PACKET_DAMAGE_DEFENCES);
                let mut text = format!(
                    "A mineral packet of {}kT from player {} has struck {} {}, killing {} \
                     colonists",
                    long(1),
                    param(3) + 1,
                    planet(param(0)),
                    if driver {
                        "faster than its mass driver could catch"
                    } else {
                        "which has no mass driver to catch it"
                    },
                    i32::from(param(4)) * 100
                );
                if self.params.len() > 5 {
                    text.push_str(&format!(" and destroying {} defences", param(5)));
                }
                text.push('.');
                text
            }
            id::PACKET_WIPED_OUT => format!(
                "A mineral packet from player {} has struck {} and killed every last \
                 colonist there; the planet is no longer yours.",
                param(1) + 1,
                planet(param(0))
            ),
            id::PACKET_HIT_EMPTY => format!(
                "A mineral packet from player {} has struck {}, where nobody lives; its \
                 defences are destroyed.",
                param(1) + 1,
                planet(param(0))
            ),
            id::PACKET_TERRAFORMED | id::PACKET_TERRAFORMED_THEIRS => format!(
                "Your mineral packet has {} the {} of {} to {}.",
                if param(0) != 0 { "raised" } else { "lowered" },
                ["gravity", "temperature", "radiation"]
                    .get(usize::try_from(param(1)).unwrap_or(usize::MAX))
                    .copied()
                    .unwrap_or("climate"),
                planet(param(2)),
                i32::from(param(3)) & 0xff
            ),
            id::PACKET_TERRAFORMED_ORIG | id::PACKET_TERRAFORMED_ORIG_THEIRS => format!(
                "Your mineral packet has permanently {} the {} of {} by {} click(s).",
                if param(0) != 0 { "raised" } else { "lowered" },
                ["gravity", "temperature", "radiation"]
                    .get(usize::try_from(param(1)).unwrap_or(usize::MAX))
                    .copied()
                    .unwrap_or("climate"),
                planet(param(2)),
                param(3)
            ),
            id::CLIMATE_CHANGE => format!(
                "The {} of {} has shifted; check its habitability, and its \
                 production queue has been reduced to auto-build items.",
                ["gravity", "temperature", "radiation"]
                    .get(usize::try_from(param(1)).unwrap_or(usize::MAX))
                    .copied()
                    .unwrap_or("climate"),
                planet(param(0))
            ),
            id::NEW_MINERALS => format!(
                "Your miners have found a new vein of {} on {}, and its \
                 concentration has risen.",
                cargo_kind(param(1)),
                planet(param(0))
            ),
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
            id::BUILT_FACTORY => format!("A factory has been built on {}.", object_planet()),
            id::BUILT_FACTORIES => format!(
                "{} factories have been built on {}.",
                param(0),
                object_planet()
            ),
            id::BUILT_MINE => format!("A mine has been built on {}.", object_planet()),
            id::BUILT_MINES => format!(
                "{} mines have been built on {}.",
                param(0),
                object_planet()
            ),
            id::SHIP_BUILT => format!(
                "{} has built a new ship, {}.",
                planet(param(0)),
                object_fleet()
            ),
            id::SHIPS_BUILT => format!(
                "{} has built {} new ships, {}.",
                planet(param(0)),
                param(1),
                object_fleet()
            ),
            id::QUEUE_EMPTY => format!(
                "{} has finished everything in its production queue, which is now \
                 empty.",
                object_planet()
            ),
            id::FLEET_DISMANTLED => format!(
                "{} has been dismantled and its {}kT of minerals put down on {}.",
                fleet(),
                long(1),
                object_planet()
            ),
            id::COLONISTS_CONTROL | id::COLONISTS_CONTROL_AR => format!(
                "Your colonists have settled {} and it is yours.",
                object_planet()
            ),
            id::FOUND_OCCUPIED => format!(
                "You have come across {}, and it is somebody else's.",
                planet(self.params.first().copied().unwrap_or(self.object))
            ),
            id::FOUND_HOSTILE => format!(
                "You have found {}, and it is no place for your people: \
                 {}% of any colonists there would die each year.",
                planet(self.params.get(1).copied().unwrap_or(self.object)),
                f64::from(param(0)) / 10.0
            ),
            id::FOUND_HABITABLE => format!(
                "You have found {}, and your people could live there, \
                 growing by up to {}% a year.",
                planet(self.params.get(1).copied().unwrap_or(self.object)),
                param(0)
            ),
            id::FOUND_UNKNOWN => format!(
                "You have found {}, but only from afar: whether your people \
                 could live there will stay a mystery until a fleet with better \
                 scanners visits.",
                planet(self.params.first().copied().unwrap_or(self.object))
            ),
            id::FOUND_TERRAFORMABLE => format!(
                "You have found {}, which terraforming could make liveable: \
                 your people could then grow there by up to {}% a year.",
                planet(self.params.get(1).copied().unwrap_or(self.object)),
                param(0)
            ),
            id::FOUND_CLAIM_ADJUSTER => format!(
                "You have the measure of {}: settled, it could be brought to {}%.",
                planet(self.params.first().copied().unwrap_or(self.object)),
                param(1)
            ),
            id::HOME_PLANET => format!(
                "{} is your home world. Your people have grown restless and are \
                 ready to leave the nest: explore the stars around you, find worlds to \
                 settle, and build the ships to take you there.",
                planet(self.params.first().copied().unwrap_or(self.object))
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
            -2 => Goto::Research,
            -3 => Goto::ShipDesign,
            -4 => Goto::Score,
            -5 => Goto::SerialNumber,
            -7 => Goto::BattleReport,
            _ => {
                let bits = word as u16;
                if bits & 0xC000 == 0xC000 {
                    // A component, shown in the part browser.
                    Goto::Part(bits & 0x3FFF)
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
                    Goto::PlayerRelations
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
