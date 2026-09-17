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
    /// `idmHasBuiltNewPlanetaryScanner`: a planetary scanner went up
    /// (`FBuildObject`, `10b8:1a5c`, the scanner arm); the planet is the
    /// object and the parameters `[planet, -0x8000, scanner index]`.
    pub const BUILT_PLANETARY_SCANNER: u16 = 0x7c;
    /// `idmStrongFundamentalForcesHaveRebirthed`: a Genesis Device went
    /// off on the planet, which every player is told about
    /// (`FBuildObject`, the Genesis arm).
    pub const GENESIS_DEVICE: u16 = 0x11b;
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

    // The rest of `DropColonists` (`10b8:34e2`): the planet is the object,
    // a player word is `player | 0x30` (or `| 0x20`, `| 0xb0`), and a
    // count is a long in hundreds.
    /// `idmColonistsDroppedMassacredGroundTroops`: your landing was wiped
    /// out by the defenders (the count, the planet, the defender).
    pub const LANDING_MASSACRED: u16 = 0x00;
    /// `idmColonistsDroppedDestroyedPlanetaryDefensesRestMa`: the defences
    /// shot part of it down and the troops finished the rest (the count,
    /// the planet, the percent shot down, the defender).
    pub const LANDING_SHOT_DOWN: u16 = 0x01;
    /// `idmColonistsForcedTransportDiedBecauseDidColonize`: colonists put
    /// down on an empty planet by a cargo transfer rather than a Colonize
    /// order die (the count, the planet).
    pub const LANDING_NOT_COLONISED: u16 = 0x02;
    /// `idmGroundTroopsValiantlyDestroyedAttackingBarbarian`: your troops
    /// wiped out a landing (the planet, the count, the attacker).
    pub const LANDING_REPELLED: u16 = 0x03;
    /// `idmPlanetaryDefensesGroundTroopsDestroyedInvadingTr`: your defences
    /// and troops did (the planet, the count, the attacker).
    pub const LANDING_REPELLED_BY_DEFENCES: u16 = 0x04;
    /// `idmMultitudeEnemiesHaveMountedProngAttackResulting`: several
    /// enemies overran your planet and then killed each other off (the
    /// number of sides, the planet).
    pub const LANDING_PRONGED: u16 = 0x05;
    /// `idmInvolvedWayAssaultNobodysTroopsSurvivedBrutal`: you were one
    /// side of a fight nobody survived (the sides, the planet).
    pub const LANDING_FREE_FOR_ALL: u16 = 0x06;
    /// `idmHaveAttackedFirstRateStormTroopersThough`: your planet was
    /// taken (the attacker, the planet, the count).
    pub const LANDING_STORMED: u16 = 0x07;
    /// `idmInvolvedWayRaceUninhabitedPlanetForcesCrush`: you won a race
    /// for an empty planet (the sides, the planet).
    pub const LANDING_RACE_WON: u16 = 0x08;
    /// `idmColonistsDestroyedWayRaceUninhabitedPlanetContro`: you lost one
    /// (the sides, the planet, the winner).
    pub const LANDING_RACE_LOST: u16 = 0x09;
    /// `idmTroopsCrushSColonistsControlPlanet`: your troops took a held
    /// planet (the loser, the planet).
    pub const LANDING_CRUSHED_DEFENDERS: u16 = 0x0c;
    /// `idmColonistsDroppedDestroyedSpiritedFighting`: your landing was
    /// lost in a fight another attacker won (the planet).
    pub const LANDING_LOST_THE_FIGHT: u16 = 0x0d;
    /// `idmColonistsAttemptingSetShopReducedProtoplasmicBlo`: an Alternate
    /// Reality landing cannot live on a surface (the planet).
    pub const LANDING_AR_DIED: u16 = 0x57;
    /// `idmColonistsAssaultingHaveKilledForcesOrbitingStarb`: the starbase
    /// killed the landing (the planet).
    pub const LANDING_STARBASE: u16 = 0x58;
    /// `idmColonistsSettlingHaveFoundStrangeArtifactBoostin`: the planet
    /// settled held a Mystery Trader artifact (object `-2`; the planet,
    /// the field, the resources).
    pub const ARTIFACT_FOUND: u16 = 0x5e;

    // The Transport task (`SatisfyOrders`, `10b0:686a`), the fleet as the
    // object and first parameter; a place is `x, y`, `-1` and a planet, or
    // `-1` and a fleet word.
    /// `idmHasStolen`: a Pick Pocket or Robber Baron took from another
    /// player's fleet (a long amount, the kind, the fleet robbed).
    pub const HAS_STOLEN: u16 = 0x119;
    /// `idmHadOrdersTransferCargoFutilePursuit`: the object the task named
    /// is not a packet, or not here (the object kind).
    pub const TRANSFER_FUTILE: u16 = 0x11e;
    /// `idmAttemptedLoadPlanetDontControlOrderHas`: a load from a planet
    /// the player does not control, given up on the last pass (the kind).
    pub const LOAD_NOT_YOUR_PLANET: u16 = 0x11f;
    /// `idmAttemptedLoadFleetDontControlOrderHas`: the same, from a fleet.
    pub const LOAD_NOT_YOUR_FLEET: u16 = 0x120;
    /// `idmAttemptedSetAmountBoardUnfortunatelyCouldntProvi`: "set amount
    /// to" asked more than the far side had (the kind, the amount asked,
    /// then the place).
    pub const SET_AMOUNT_SHORT: u16 = 0x121;
    /// `idmAttemptedSetNumberBoardUnfortunatelyCouldntProvi`: the same for
    /// colonists.
    pub const SET_NUMBER_SHORT: u16 = 0x122;
    /// `idmAttemptedLoadDeepSpaceAttemptUnsuccessful`: a load in deep space
    /// (the kind).
    pub const LOAD_FROM_SPACE: u16 = 0x123;
    /// `idmFailedLoadFuel`: the optimal fuel could not be found (the place).
    pub const FUEL_LOAD_FAILED: u16 = 0x126;
    /// `idmThereIsntEnoughFuelAvailableAllowGet`: the far side has too
    /// little fuel for the next leg (the place, the fleet, the shortfall).
    pub const FUEL_NOT_AVAILABLE: u16 = 0x3c;
    /// `idmWillNeverMakeWaypointFuelCapacityMg`: the tank is too small for
    /// the next leg however full (the capacity, the need).
    pub const FUEL_NEVER_ENOUGH: u16 = 0x3d;
    /// `idmHasTriedBeamColonistsPlanetUninhabitedMust`: colonists cannot
    /// be unloaded onto an empty planet; it must be colonised (the planet).
    pub const BEAM_DOWN_UNINHABITED: u16 = 0x55;
    /// `idmCaptainHasAttemptedBeamColonistsOverruledBridge`: an Alternate
    /// Reality captain's landing overruled (the planet).
    pub const BEAM_DOWN_OVERRULED: u16 = 0x56;
    /// `idmHasTriedBeamColonistsPlanetsStarbaseWould`: the planet's
    /// starbase would kill a landing (the planet).
    pub const BEAM_DOWN_STARBASE: u16 = 0x135;
    /// `idmAllowedTransferColonistsAnotherPlayer`: colonists cannot be
    /// given to another player's fleet.
    pub const COLONISTS_TO_ANOTHER: u16 = 0x155;
    /// `idmHasTriedBeamColonistsDeepSpaceOrder`: colonists cannot be put
    /// into space.
    pub const BEAM_DOWN_SPACE: u16 = 0x165;
    /// `idmWormholeHeadingForHasVanished` (`0xf8`): the wormhole a leg was
    /// aimed at is gone or has moved unseen; the leg now ends where it was.
    pub const WORMHOLE_VANISHED: u16 = 0xf8;
    // `UpdatePlayerScores` (`10b8:6258`), put at the front of the year's
    // news (`FSendPrependedPlrMsg`).
    /// `idmForcesHaveDeclaredWinnerGameAdvisedAccept` (`0xb5`): somebody
    /// else has won (the winners, as a player mask).
    pub const GAME_WON_BY_OTHERS: u16 = 0xb5;
    /// `idmHaveDeclaredWinnerGameMayContinuePlay` (`0xb6`): you have won
    /// alone.
    pub const GAME_WON: u16 = 0xb6;
    /// `idmAlongHaveDeclaredWinnersGameMayContinue` (`0xb7`): you have won
    /// with others (the others, as a player mask).
    pub const GAME_WON_SHARED: u16 = 0xb7;
    /// `idmDeadPlanetsHaveOverrunSpaceshipsDefeated` (`0xb8`): the game is
    /// over and you are dead.
    pub const GAME_OVER_DEAD: u16 = 0xb8;
    /// `idmTracesHaveEliminatedGalaxyMayRestPeace` (`0xbb`): a player has
    /// been wiped out this year (`player | 0x30`).
    pub const PLAYER_ELIMINATED: u16 = 0xbb;
    /// `idmTracesEveryOtherRivalHaveEliminatedGalaxy` (`0xbc`): everybody
    /// else is gone; you alone are left.
    pub const LAST_ONE_STANDING: u16 = 0xbc;
    /// `idmMineFieldHeadingHasVanishedOrdersHave` (`0x111`): the minefield
    /// a leg was aimed at is gone from the player's map; the leg now ends
    /// where it was.
    pub const MINEFIELD_VANISHED: u16 = 0x111;
    /// `idmSWaypointAppearsHaveDestroyedHasDisappeared` (`0x28`): the fleet
    /// a leg was chasing is gone (the chaser, the fleet chased); the leg
    /// now ends where it was last seen.
    pub const CHASED_FLEET_GONE: u16 = 0x28;
    /// `idmFleetTrackingAppearsHaveDuckedBehindOrders` (`0x29`): the fleet
    /// a leg was chasing is out of sight at a planet (the chaser, the
    /// planet); the leg now ends at the planet.
    pub const CHASED_FLEET_DUCKED: u16 = 0x29;
    /// `idmFleetTrackingAppearsHaveOutrunRangeScanners` (`0x2a`): the fleet
    /// a leg was chasing is out of scanner range, or went through a gate
    /// (the chaser); the leg now ends where it was last seen.
    pub const CHASED_FLEET_OUTRUN: u16 = 0x2a;
    /// `idmHasRerouted`: a fleet at a planet with a route has been sent on
    /// (the fleet, the planet, the route's end).
    pub const REROUTED: u16 = 0x127;
    /// `idmHasReroutedUnfortuentlyDoesHaveEnoughFuel`: the same, with too
    /// little fuel to get there.
    pub const REROUTED_SHORT_OF_FUEL: u16 = 0x128;
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
    /// or the coordinates), then the races involved, the player's losses,
    /// their ships, the enemy's losses and its ships. The original words
    /// the outcome a dozen ways (`SendBattleMessages`, ids `0x8d`–`0xa8`
    /// and `0x113`–`0x116`); this is its general case, `0xa8`.
    pub const BATTLE: u16 = 0xa8;
    /// `idmReportsBattleTookPlaceForcesInvolved` (`0xfa`): a fleet of the
    /// player's saw a battle it was not in (the fleet, the place).
    pub const BATTLE_SEEN: u16 = 0xfa;
    /// `idmColonyReportsBattleTookPlaceOrbitForces` (`0xf9`): a battle in
    /// orbit of a planet of the player's, without them (the planet).
    pub const BATTLE_SEEN_FROM_PLANET: u16 = 0xf9;
    // `DoBombing` (`10f0:b9d4`) words a bombing two dozen ways: the fleet
    // and the planet first, then the colonists killed (in hundreds) when
    // any were, the installations destroyed when any were, and the
    // defences' stopping share in hundredths of a percent when it says so.
    /// `idmHasBombedKillingColonists` (`0x60`): people killed, nothing
    /// destroyed (the fleet, the planet, the hundreds).
    pub const BOMBED: u16 = 0x60;
    /// `idmHasBombedKillingColonists2` (`0x6a`): the same, to the planet's
    /// owner.
    pub const BOMBED_YOU: u16 = 0x6a;
    /// `idmHasBombedKillingColonistsDestroyingOneInstallati` (`0x63`):
    /// people killed and one installation destroyed (the fleet, the planet,
    /// the hundreds, the count). One more for several installations; two
    /// less with nobody killed; five more with the stopping share.
    pub const BOMBED_KILLED_AND_ONE: u16 = 0x63;
    /// `0x6d`: the same, to the planet's owner.
    pub const BOMBED_YOU_KILLED_AND_ONE: u16 = 0x6d;
    /// `idmHasBombedKillingOffEnemyColonists` (`0x8f`): the planet is
    /// emptied (the fleet, the planet, the hundreds, the installations).
    pub const BOMBED_OUT: u16 = 0x8f;
    /// `idmHasBombedKillingColonists3` (`0x90`): the same, to the owner.
    pub const BOMBED_OUT_YOU: u16 = 0x90;
    /// `0x166`: [`BOMBED`] by several fleets together.
    pub const BOMBED_FLEETS: u16 = 0x166;
    /// `0x169`: [`BOMBED_KILLED_AND_ONE`] by several fleets.
    pub const BOMBED_KILLED_AND_ONE_FLEETS: u16 = 0x169;
    /// `0x170`: [`BOMBED_YOU`] by several fleets.
    pub const BOMBED_YOU_FLEETS: u16 = 0x170;
    /// `0x173`: [`BOMBED_YOU_KILLED_AND_ONE`] by several fleets.
    pub const BOMBED_YOU_KILLED_AND_ONE_FLEETS: u16 = 0x173;
    /// `0x17c`: [`BOMBED_OUT`] by several fleets.
    pub const BOMBED_OUT_FLEETS: u16 = 0x17c;
    /// `0x17d`: [`BOMBED_OUT_YOU`] by several fleets.
    pub const BOMBED_OUT_YOU_FLEETS: u16 = 0x17d;
    /// `idmHasRetroBombedUndoingTerraforming` (`0x12e`): Retro Bombs
    /// undid so many steps of terraforming (the fleet, the planet, the
    /// steps), sent to both sides.
    pub const RETRO_BOMBED: u16 = 0x12e;
    /// `0x17a`: the same by several fleets, to the bomber.
    pub const RETRO_BOMBED_FLEETS: u16 = 0x17a;
    /// `0x17b`: the same by several fleets, to the planet's owner.
    pub const RETRO_BOMBED_YOU_FLEETS: u16 = 0x17b;
    /// `idmCouldntGiveAwayBecauseThereColonistsBoard`: a fleet with colonists
    /// aboard cannot be given away (`10b0:9436`).
    pub const GIFT_HAS_COLONISTS: u16 = 0x149;
    /// `idmCouldntGiveAwayBecausePlayerDead` (`0x148`): the player named
    /// is dead, or not in the game (the fleet).
    pub const GIFT_PLAYER_DEAD: u16 = 0x148;
    /// `idmCouldntGiveAwayBecauseDidntHaveAdministrative` (`0x14a`): the
    /// recipient has no room for the designs or the fleet (the fleet, the
    /// recipient as `player | 0x30`).
    pub const GIFT_NO_ROOM: u16 = 0x14a;
    /// `idmAttemptedGiveFleetDontHaveEnoughExcess` (`0x14b`): the same, to
    /// the recipient (the giver as `player | 0x30`).
    pub const GIFT_NO_ROOM_THEIRS: u16 = 0x14b;
    /// `idmSnubAttemptedGiftRefuseFleet` (`0x14c`): the recipient — a
    /// computer player, or somebody who counts the giver an enemy — will
    /// not have it (the recipient as `player | 0x30`).
    pub const GIFT_SNUBBED: u16 = 0x14c;
    /// `idmHasSuccessfullyGiven` (`0x14d`): the fleet is theirs now (the
    /// fleet word, the recipient as `player | 0x30`).
    pub const GIFT_GIVEN: u16 = 0x14d;
    /// `idmHaveGiven` (`0x14e`): a fleet has been given to the player (the
    /// giver as `player | 0x30`, the fleet word).
    pub const GIFT_RECEIVED: u16 = 0x14e;

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

    // A fleet in a minefield — `FTravelThroughMineFields` from `10b0:6174`.
    // One wording to the fleet's owner and one to the field's, each with
    // a detonation variant. The fleet owner's carry the fleet, the field's
    // owner, the kind and the place; the field owner's the fleet, the kind
    // and the place; then the damage and the ships lost where there were.
    /// Your fleet hit a minefield and took no damage.
    pub const MINE_HIT_NO_DAMAGE: u16 = 0xc5;
    /// Your fleet hit a minefield and was damaged.
    pub const MINE_HIT: u16 = 0xc6;
    /// Your fleet hit a minefield and lost ships.
    pub const MINE_HIT_SHIPS_LOST: u16 = 0xc7;
    /// Your fleet was destroyed by a minefield and left salvage — the object
    /// is [`super::THING_OBJECT`], the salvage's full id first.
    pub const MINE_HIT_DESTROYED_SALVAGE: u16 = 0xc8;
    /// Your fleet was destroyed by a minefield: a name word for the fleet,
    /// the field's owner, the kind, the place.
    pub const MINE_HIT_DESTROYED: u16 = 0x15f;
    /// Your fleet was caught in a minefield going off, and damaged.
    pub const MINE_DETONATED_ON: u16 = 0x160;
    /// Your fleet was caught in a minefield going off, and lost ships.
    pub const MINE_DETONATED_ON_SHIPS_LOST: u16 = 0x161;
    /// Somebody's fleet hit your minefield and took no damage.
    pub const YOUR_FIELD_HIT_NO_DAMAGE: u16 = 0xc9;
    /// Somebody's fleet hit your minefield and was damaged.
    pub const YOUR_FIELD_HIT: u16 = 0xca;
    /// Somebody's fleet hit your minefield and lost ships.
    pub const YOUR_FIELD_HIT_SHIPS_LOST: u16 = 0xcb;
    /// Your minefield destroyed a fleet: [`super::THING_OBJECT`], the
    /// salvage's (or, with none, the field's) full id, the fleet id, the
    /// kind, the place.
    pub const YOUR_FIELD_DESTROYED_FLEET: u16 = 0xcc;
    /// Your own fleet was destroyed by your minefield going off.
    pub const YOUR_FIELD_DESTROYED_YOURS: u16 = 0x162;
    /// Your minefield went off and damaged a fleet.
    pub const YOUR_FIELD_DETONATED_ON: u16 = 0x163;
    /// Your minefield went off and destroyed ships.
    pub const YOUR_FIELD_DETONATED_ON_SHIPS_LOST: u16 = 0x164;

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

    // Throwing a packet — `FBuildObject` (`10b8:19b2`), to the planet's
    // owner, with the planet and then the target.
    /// A packet was built with no mass driver to throw it.
    pub const PACKET_NO_DRIVER: u16 = 0xd1;
    /// A packet was built with no destination set.
    pub const PACKET_NO_DESTINATION: u16 = 0xd2;
    /// A packet was thrown.
    pub const PACKET_FLUNG: u16 = 0xd3;
    /// The year's packet joined one already on the pad.
    pub const PACKET_ADDED_TO: u16 = 0xd4;
    /// The game has no room for another packet (`10b8:2716`).
    pub const NO_ROOM_FOR_THING: u16 = 0x129;

    // A starbase finished — `FBuildObject` (`10b8:1a5c`): the planet, the
    // design word (`owner << 5 | slot`), the dock's size in kT.
    /// A starbase has been built over the planet.
    pub const STARBASE_BUILT: u16 = 0xcd;
    /// A starbase with a dock that builds ships up to so many kT.
    pub const STARBASE_BUILT_WITH_DOCK: u16 = 0xce;
    /// A starbase whose dock builds ships of any size.
    pub const STARBASE_BUILT_ANY_SIZE: u16 = 0xcf;

    // Wreckage — `ITechLearnATech` (`10f0:9918`), object `-2`, the place
    // (`x, y`, or `-1` and the planet), the field, the resources as a long.
    /// Wreckage from a battle you fought boosted your research.
    pub const WRECKAGE_BOOSTED_RESEARCH: u16 = 0xef;
    /// Wreckage from a battle in orbit of your planet boosted your research.
    pub const WRECKAGE_IN_ORBIT_BOOSTED_RESEARCH: u16 = 0xf0;
    /// Your fleet found wreckage from a battle it watched.
    pub const FLEET_FOUND_WRECKAGE: u16 = 0xf1;
    /// Wreckage yielded the plans of a Mystery Trader part: the Trader's
    /// wording moved up by `0x2f`, the item word as the object.
    pub const WRECKAGE_PLANS_PART: u16 = 0x13a;
    /// Wreckage yielded the plans of a Mystery Trader hull.
    pub const WRECKAGE_PLANS_HULL: u16 = 0x13b;

    // Movement mishaps — `MoveFleets` (`10b0:32ce`), the fleet as the
    // object.
    /// `idmEngineRadiationHasKilledColonistsTraveling`: a Radiating
    /// Hydro-Ram Scoop's radiation killed colonists aboard (the count in
    /// hundreds, then the fleet).
    pub const ENGINE_RADIATION_KILLED: u16 = 0x74;
    /// `idmDueRigorsWarpAccelerationColonistsHaveDied`: an Alternate
    /// Reality fleet lost colonists to acceleration (a long count, then the
    /// fleet).
    pub const WARP_ACCELERATION_KILLED: u16 = 0xc1;
    /// `idmOneShipsDestroyedWhenEnginesReactedTrying`: one ship's engine
    /// failed at warp 10.
    pub const WARP_TEN_LOST_ONE: u16 = 0xdf;
    /// `idmShipsDestroyedDueEngineStrain`: several ships' engines failed at
    /// warp 10 (the count, then the fleet).
    pub const WARP_TEN_LOST_SHIPS: u16 = 0xe0;
    /// `idmDestroyedMassiveReactorAccidentDueUnsafeOperatin`: every ship's
    /// engine failed at warp 10.
    pub const WARP_TEN_LOST_FLEET: u16 = 0xe1;
    /// `idmUnableEngageEnginesDueBalkyEquipmentEngineers`: a Cheap Engines
    /// fleet's engines would not start this year.
    pub const BALKY_ENGINES: u16 = 0xf2;
    /// `idmSRamScoopsHaveProducedMgFuel`: the ramscoops made fuel on the
    /// way (the fleet, then the milligrams).
    pub const RAMSCOOP_FUEL: u16 = 0xf3;

    // Stargates — the stargate branch of `MoveFleets` (`10b0:354f`) and
    // `FStargateJump` (`1080:0cfe`). The fleet is the object and the first
    // parameter; the place is `(x, y)` or `-1` and the planet.
    /// `idmAttemptedUseStargateStargateExistsThere`: no gate where the
    /// fleet stands (the place).
    pub const STARGATE_NONE_HERE: u16 = 0xde;
    /// `idmAttemptedUseStargateReachCouldBecauseStargate`: no gate at the
    /// destination (the source planet, then the place).
    pub const STARGATE_NONE_THERE: u16 = 0xe2;
    /// `idmAttemptedUseStargateReachCouldBecauseDestination`: the
    /// destination is out of range (the source planet, the destination).
    pub const STARGATE_TOO_FAR: u16 = 0xe3;
    /// `idmAttemptedUseStargateReachCouldBecauseShips`: ships of a design
    /// are too massive (the source, the destination, the design slot).
    pub const STARGATE_TOO_MASSIVE: u16 = 0xe4;
    /// `idmAttemptedUseStargateReachCouldBecauseStarbase`: the destination
    /// gate is not a friend's (the source, the destination twice).
    pub const STARGATE_BLOCKED_THERE: u16 = 0xe5;
    /// `idmAttemptedUseStargateCouldBecauseStarbaseOwned`: the source gate
    /// is not a friend's (the planet twice).
    pub const STARGATE_BLOCKED_HERE: u16 = 0xe6;
    /// `idmHeedlessDangerAttemptedUseStargateReachFleet`: the fleet never
    /// arrived (the source, the destination).
    pub const STARGATE_ANNIHILATED: u16 = 0xe7;
    /// `idmUsedStargateReachLosingShipsTreacherousVoid`: arrived losing
    /// under a quarter of the ships (the source, the destination, the
    /// count).
    pub const STARGATE_LOST_FEW: u16 = 0xe8;
    /// `idmUsedStargateReachLosingShipsUnforgivingVoid`: arrived losing up
    /// to half.
    pub const STARGATE_LOST_SOME: u16 = 0xe9;
    /// `idmUsedStargateReachUnfortunatelyLosingShipsGreat`: arrived losing
    /// more than half.
    pub const STARGATE_LOST_MANY: u16 = 0xea;
    /// `idmUsedStargateReachLosingUnbelievableShipsJump`: arrived losing a
    /// count too big for a word (a long count).
    pub const STARGATE_LOST_UNBELIEVABLE: u16 = 0xeb;
    /// `idmHasUnloadedKtMineralsPreparationJumpingThrough`: the minerals
    /// were put down on the planet first (a long kT, the kT again, the
    /// planet).
    pub const STARGATE_UNLOADED_MINERALS: u16 = 0xec;
    /// `idmHasUnloadedColonistsPreparationJumpingThroughSta`: the
    /// colonists were put down first (a long count, the count again, the
    /// planet).
    pub const STARGATE_UNLOADED_COLONISTS: u16 = 0xed;
    /// `idmHasUnloadedColonistsKtMineralsPreparationJumping`: both were
    /// (a long count, a long kT, the planet).
    pub const STARGATE_UNLOADED_BOTH: u16 = 0xee;
    /// `idmUnableUseStargateBecauseHadColonistsBoard`: colonists aboard at
    /// a planet that is not yours (the planet).
    pub const STARGATE_COLONISTS_ABOARD: u16 = 0x15e;
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
        // The bombing wordings that say what the defences stopped are the
        // ones five above their plain form; the share is the parameter at
        // `at`, in hundredths of a percent.
        let stopped_share = |id: u16, at: usize| -> String {
            let says_so = matches!(
                id,
                0x66..=0x69 | 0x70..=0x73 | 0x16c..=0x16f | 0x176..=0x179
            );
            if says_so {
                let share = i32::from(param(at));
                format!(
                    "; the planet's defences stopped {}.{:02}% of the bombs",
                    share / 100,
                    share % 100
                )
            } else {
                String::new()
            }
        };
        // A player mask, spelt out.
        let players = |mask: i16| -> String {
            let bits = u16::from_ne_bytes(mask.to_ne_bytes());
            let list: Vec<String> = (0..16)
                .filter(|i| bits & (1 << i) != 0)
                .map(|i| format!("player {i}"))
                .collect();
            list.join(", ")
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
        let mine_kind = |kind: i16| match kind {
            0 => "standard",
            1 => "heavy",
            _ => "speed trap",
        };
        // A place given as two parameters from `at`: `x, y`, or `-1` and
        // the planet.
        let place_at = |at: usize| {
            let x = param(at);
            let y = param(at + 1);
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
                "A battle took place at {} between {} races: {} of your ships fought {} of theirs; you lost {}, they lost {}.",
                place(),
                param(2),
                param(4),
                param(6),
                param(3),
                param(5)
            ),
            id::BATTLE_SEEN => format!(
                "{} reports a battle at {}; your forces were not involved.",
                fleet(),
                if param(1) == -1 {
                    planet(param(2))
                } else {
                    format!("({}, {})", param(1), param(2))
                }
            ),
            id::BATTLE_SEEN_FROM_PLANET => format!(
                "Your colony on {} reports a battle in orbit; your forces were not involved.",
                planet(param(0))
            ),
            id::BOMBED | id::BOMBED_FLEETS => format!(
                "{} has bombed {}, killing {} colonists.",
                fleet(),
                planet(param(1)),
                i32::from(param(2)) * 100
            ),
            id::BOMBED_YOU | id::BOMBED_YOU_FLEETS => format!(
                "{} has been bombed by {}: {} colonists killed.",
                planet(param(1)),
                fleet(),
                i32::from(param(2)) * 100
            ),
            id::BOMBED_OUT | id::BOMBED_OUT_FLEETS => format!(
                "{} has bombed {} and wiped out everyone on it.",
                fleet(),
                planet(param(1))
            ),
            id::BOMBED_OUT_YOU | id::BOMBED_OUT_YOU_FLEETS => format!(
                "{} has been bombed by {}; nobody on it survived.",
                planet(param(1)),
                fleet()
            ),
            0x61 | 0x62 | 0x66 | 0x67 | 0x167 | 0x168 | 0x16c | 0x16d => format!(
                "{} has bombed {}, destroying {} installations{}.",
                fleet(),
                planet(param(1)),
                param(2),
                stopped_share(self.id, 3)
            ),
            0x6b | 0x6c | 0x70 | 0x71 | 0x171 | 0x172 | 0x176 | 0x177 => format!(
                "{} has been bombed by {}: {} installations destroyed{}.",
                planet(param(1)),
                fleet(),
                param(2),
                stopped_share(self.id, 3)
            ),
            0x63 | 0x64 | 0x68 | 0x69 | 0x169 | 0x16a | 0x16e | 0x16f => format!(
                "{} has bombed {}, killing {} colonists and destroying {} installations{}.",
                fleet(),
                planet(param(1)),
                i32::from(param(2)) * 100,
                param(3),
                stopped_share(self.id, 4)
            ),
            0x6d | 0x6e | 0x72 | 0x73 | 0x173 | 0x174 | 0x178 | 0x179 => format!(
                "{} has been bombed by {}: {} colonists killed and {} installations destroyed{}.",
                planet(param(1)),
                fleet(),
                i32::from(param(2)) * 100,
                param(3),
                stopped_share(self.id, 4)
            ),
            id::RETRO_BOMBED | id::RETRO_BOMBED_FLEETS | id::RETRO_BOMBED_YOU_FLEETS => format!(
                "{} has retro-bombed {}, undoing {} steps of its terraforming.",
                fleet(),
                planet(param(1)),
                param(2)
            ),
            id::ORDERS_COMPLETE => format!("{} has finished its orders.", fleet()),
            id::ENGINE_RADIATION_KILLED => format!(
                "Radiation from the engines has killed {} colonists travelling in {}.",
                i32::from(param(0)) * 100,
                names.fleet(u16::try_from(i32::from(param(1)) & 0x1ff).unwrap_or(0))
            ),
            id::WARP_ACCELERATION_KILLED => format!(
                "The strain of acceleration has killed {} of the colonists aboard {}.",
                i64::from(long(0)) * 100,
                names.fleet(u16::try_from(i32::from(param(2)) & 0x1ff).unwrap_or(0))
            ),
            id::WARP_TEN_LOST_ONE => format!(
                "A ship of {} was lost when its engine failed under the strain of warp 10.",
                fleet()
            ),
            id::WARP_TEN_LOST_SHIPS => format!(
                "{} ships of {} were lost when their engines failed under the strain of warp 10.",
                param(0),
                names.fleet(u16::try_from(i32::from(param(1)) & 0x1ff).unwrap_or(0))
            ),
            id::WARP_TEN_LOST_FLEET => format!(
                "{} was lost with all hands when its engines failed under the strain of warp 10.",
                fleet()
            ),
            id::BALKY_ENGINES => format!(
                "{} could not get its engines started this year; its engineers believe they have found the fault.",
                fleet()
            ),
            id::RAMSCOOP_FUEL => format!(
                "The ramscoops of {} gathered {}mg of fuel on the way.",
                fleet(),
                param(1)
            ),
            id::STARGATE_NONE_HERE => format!(
                "{} tried to use a stargate at {}, but there is none there.",
                fleet(),
                place_at(1)
            ),
            id::STARGATE_NONE_THERE => format!(
                "{} tried to jump from the stargate at {} to {}, but no stargate could be found at the destination.",
                fleet(),
                planet(param(1)),
                place_at(2)
            ),
            id::STARGATE_TOO_FAR => format!(
                "{} tried to jump from the stargate at {} to {}, but the destination is out of the gate's range.",
                fleet(),
                planet(param(1)),
                planet(param(2))
            ),
            id::STARGATE_TOO_MASSIVE => format!(
                "{} tried to jump from the stargate at {} to {}, but its ships of design {} are too massive for the gates.",
                fleet(),
                planet(param(1)),
                planet(param(2)),
                param(3) + 1
            ),
            id::STARGATE_BLOCKED_THERE => format!(
                "{} tried to jump from the stargate at {} to {}, but the starbase there is not yours or a friend's.",
                fleet(),
                planet(param(1)),
                planet(param(2))
            ),
            id::STARGATE_BLOCKED_HERE => format!(
                "{} could not use the stargate at {}: the starbase is not yours or a friend's.",
                fleet(),
                planet(param(1))
            ),
            id::STARGATE_ANNIHILATED => format!(
                "{} jumped from the stargate at {} toward {} and never arrived; the distance or the mass was too much for the gates.",
                fleet(),
                planet(param(1)),
                planet(param(2))
            ),
            id::STARGATE_LOST_FEW => format!(
                "{} jumped from the stargate at {} to {}, losing {} ships on the way; it was fortunate, having exceeded what the gates can take.",
                fleet(),
                planet(param(1)),
                planet(param(2)),
                param(3)
            ),
            id::STARGATE_LOST_SOME => format!(
                "{} jumped from the stargate at {} to {}, losing {} ships on the way; exceeding what the gates can take is not advised.",
                fleet(),
                planet(param(1)),
                planet(param(2)),
                param(3)
            ),
            id::STARGATE_LOST_MANY => format!(
                "{} jumped from the stargate at {} to {}, losing {} ships on the way; exceeding what the gates can take is dangerous.",
                fleet(),
                planet(param(1)),
                planet(param(2)),
                param(3)
            ),
            id::STARGATE_LOST_UNBELIEVABLE => format!(
                "{} jumped from the stargate at {} to {}, losing an unbelievable {} ships; the jump was far beyond the gates.",
                fleet(),
                planet(param(1)),
                planet(param(2)),
                long(3)
            ),
            id::STARGATE_UNLOADED_MINERALS => format!(
                "{} put {}kT of minerals down on {} before jumping through the stargate.",
                fleet(),
                long(1),
                planet(param(4))
            ),
            id::STARGATE_UNLOADED_COLONISTS => format!(
                "{} put {} colonists down on {} before jumping through the stargate.",
                fleet(),
                i64::from(long(1)) * 100,
                planet(param(4))
            ),
            id::STARGATE_UNLOADED_BOTH => format!(
                "{} put {} colonists and {}kT of minerals down on {} before jumping through the stargate.",
                fleet(),
                i64::from(long(1)) * 100,
                long(3),
                planet(param(5))
            ),
            id::STARGATE_COLONISTS_ABOARD => format!(
                "{} could not use the stargate at {}: it has colonists aboard and the planet is not yours.",
                fleet(),
                planet(param(1))
            ),
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
            id::GIFT_PLAYER_DEAD => format!(
                "{} could not be given away: that player is dead.",
                fleet()
            ),
            id::GIFT_NO_ROOM => format!(
                "{} could not be given away: player {} has no room in their books for it.",
                fleet(),
                param(1) & 0xf
            ),
            id::GIFT_NO_ROOM_THEIRS => format!(
                "Player {} tried to give you a fleet, but you have no spare design slots to take it in.",
                param(0) & 0xf
            ),
            id::GIFT_SNUBBED => format!(
                "Player {} snubs your gift and refuses the fleet.",
                param(0) & 0xf
            ),
            id::GIFT_GIVEN => format!(
                "{} has been given to player {}.",
                names.fleet(u16::try_from(i32::from(param(0)) & 0x1ff).unwrap_or(0)),
                param(1) & 0xf
            ),
            id::GIFT_RECEIVED => format!(
                "Player {} has given you {}.",
                param(0) & 0xf,
                names.fleet(u16::try_from(i32::from(param(1)) & 0x1ff).unwrap_or(0))
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
            id::MINE_HIT_NO_DAMAGE => format!(
                "{} has struck a {} minefield belonging to player {} at ({}, {}); \
                 it took no damage but has stopped there.",
                fleet(),
                mine_kind(param(2)),
                param(1) + 1,
                param(3),
                param(4)
            ),
            id::MINE_HIT | id::MINE_DETONATED_ON => format!(
                "{} has {} a {} minefield belonging to player {} at ({}, {}) and \
                 taken {} damage.",
                fleet(),
                if self.id == id::MINE_HIT {
                    "struck"
                } else {
                    "been caught in the detonation of"
                },
                mine_kind(param(2)),
                param(1) + 1,
                param(3),
                param(4),
                param(5)
            ),
            id::MINE_HIT_SHIPS_LOST | id::MINE_DETONATED_ON_SHIPS_LOST => format!(
                "{} has {} a {} minefield belonging to player {} at ({}, {}), taking \
                 {} damage and losing {} ships.",
                fleet(),
                if self.id == id::MINE_HIT_SHIPS_LOST {
                    "struck"
                } else {
                    "been caught in the detonation of"
                },
                mine_kind(param(2)),
                param(1) + 1,
                param(3),
                param(4),
                param(5),
                param(6)
            ),
            id::MINE_HIT_DESTROYED => format!(
                "Fleet #{} was destroyed by a {} minefield belonging to player {} at \
                 ({}, {}).",
                (i32::from(param(0)) & 0x1ff) + 1,
                mine_kind(param(2)),
                param(1) + 1,
                param(3),
                param(4)
            ),
            id::MINE_HIT_DESTROYED_SALVAGE => format!(
                "Fleet #{} was destroyed by a {} minefield belonging to player {} at \
                 ({}, {}); its cargo lies there as salvage.",
                (i32::from(param(1)) & 0x1ff) + 1,
                mine_kind(param(3)),
                param(2) + 1,
                param(4),
                param(5)
            ),
            id::YOUR_FIELD_HIT_NO_DAMAGE => format!(
                "A fleet has struck your {} minefield at ({}, {}) and taken no damage.",
                mine_kind(param(1)),
                param(2),
                param(3)
            ),
            id::YOUR_FIELD_HIT | id::YOUR_FIELD_DETONATED_ON => format!(
                "A fleet {} your {} minefield at ({}, {}) and took {} damage.",
                if self.id == id::YOUR_FIELD_HIT {
                    "struck"
                } else {
                    "was caught in the detonation of"
                },
                mine_kind(param(1)),
                param(2),
                param(3),
                param(4)
            ),
            id::YOUR_FIELD_HIT_SHIPS_LOST | id::YOUR_FIELD_DETONATED_ON_SHIPS_LOST => format!(
                "A fleet {} your {} minefield at ({}, {}), took {} damage and lost {} \
                 ships.",
                if self.id == id::YOUR_FIELD_HIT_SHIPS_LOST {
                    "struck"
                } else {
                    "was caught in the detonation of"
                },
                mine_kind(param(1)),
                param(2),
                param(3),
                param(4),
                param(5)
            ),
            id::YOUR_FIELD_DESTROYED_FLEET => format!(
                "Your {} minefield at ({}, {}) has destroyed a fleet.",
                mine_kind(param(2)),
                param(3),
                param(4)
            ),
            id::YOUR_FIELD_DESTROYED_YOURS => format!(
                "Your own fleet #{} was destroyed when your {} minefield at ({}, {}) \
                 went off.",
                (i32::from(param(0)) & 0x1ff) + 1,
                mine_kind(param(1)),
                param(2),
                param(3)
            ),
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
            id::STARBASE_BUILT => {
                format!("{} has built a new starbase.", planet(param(0)))
            }
            id::STARBASE_BUILT_WITH_DOCK => format!(
                "{} has built a new starbase; its dock can build ships of up to {}kT.",
                planet(param(0)),
                param(2)
            ),
            id::STARBASE_BUILT_ANY_SIZE => format!(
                "{} has built a new starbase; its dock can build ships of any size.",
                planet(param(0))
            ),
            id::WRECKAGE_BOOSTED_RESEARCH
            | id::WRECKAGE_IN_ORBIT_BOOSTED_RESEARCH
            | id::FLEET_FOUND_WRECKAGE => {
                let field = usize::try_from(param(2))
                    .ok()
                    .and_then(|i| crate::research::TechField::ALL.get(i))
                    .map_or("?", |f| f.name());
                format!(
                    "{} the wreckage of the battle at {} yielded {} resources' worth of \
                     research in {}.",
                    match self.id {
                        id::WRECKAGE_IN_ORBIT_BOOSTED_RESEARCH => "In orbit of your planet,",
                        id::FLEET_FOUND_WRECKAGE => "Your fleet looked over",
                        _ => "Picked through,",
                    },
                    place(),
                    long(3),
                    field
                )
            }
            id::WRECKAGE_PLANS_PART | id::WRECKAGE_PLANS_HULL => format!(
                "Among the wreckage of the battle at {} your engineers found the plans of \
                 {} you could not have researched.",
                place(),
                if self.id == id::WRECKAGE_PLANS_HULL {
                    "a hull"
                } else {
                    "a part"
                }
            ),
            id::PACKET_NO_DRIVER => format!(
                "{} has built a mineral packet but has no mass driver to throw it \
                 with.",
                planet(param(0))
            ),
            id::PACKET_NO_DESTINATION => format!(
                "{} has built a mineral packet but its mass driver has no destination \
                 set.",
                planet(param(0))
            ),
            id::PACKET_FLUNG => format!(
                "{} has thrown a mineral packet at {}.",
                planet(param(0)),
                planet(param(1))
            ),
            id::PACKET_ADDED_TO => format!(
                "{} has added this year's minerals to the packet bound for {}.",
                planet(param(0)),
                planet(param(1))
            ),
            id::NO_ROOM_FOR_THING => format!(
                "{} could not throw its mineral packet: the galaxy can hold no more \
                 objects.",
                planet(param(0))
            ),
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
            id::BUILT_PLANETARY_SCANNER => format!(
                "{} has a new planetary scanner, a {}.",
                object_planet(),
                crate::components::PLANETARY
                    .get(usize::try_from(param(2)).unwrap_or(usize::MAX))
                    .map_or("scanner", |p| p.name)
            ),
            id::GENESIS_DEVICE => format!(
                "A Genesis Device has remade {}: its climate and its minerals are new.",
                object_planet()
            ),
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
            id::LANDING_MASSACRED => format!(
                "The {} colonists you landed on {} were wiped out by its defenders.",
                i64::from(long(0)) * 100,
                planet(param(2))
            ),
            id::LANDING_SHOT_DOWN => format!(
                "Of the {} colonists you landed on {}, the planet's defences shot down {}% and its defenders finished the rest.",
                i64::from(long(0)) * 100,
                planet(param(2)),
                -i32::from(param(3)) / 100
            ),
            id::LANDING_NOT_COLONISED => format!(
                "The {} colonists sent down to {} died: the planet had not been colonised first.",
                i64::from(long(0)) * 100,
                planet(param(2))
            ),
            id::LANDING_REPELLED => format!(
                "Your troops on {} wiped out a landing of {} colonists.",
                planet(param(0)),
                i64::from(long(1)) * 100
            ),
            id::LANDING_REPELLED_BY_DEFENCES => format!(
                "Your defences and troops on {} destroyed a landing of {} colonists.",
                planet(param(0)),
                i64::from(long(1)) * 100
            ),
            id::LANDING_PRONGED => format!(
                "{} enemies landed on {} at once; they overran it and then killed each other off.",
                param(0),
                planet(param(1))
            ),
            id::LANDING_FREE_FOR_ALL => format!(
                "You were one of {} sides fighting for {}; nobody's troops survived.",
                param(0),
                planet(param(1))
            ),
            id::LANDING_STORMED => format!(
                "{} has been taken from you by a landing of {} colonists.",
                planet(param(1)),
                i64::from(long(2)) * 100
            ),
            id::LANDING_RACE_WON => format!(
                "{} sides raced to settle {}; your colonists prevailed.",
                param(0),
                planet(param(1))
            ),
            id::LANDING_RACE_LOST => format!(
                "{} sides raced to settle {}; your colonists were destroyed and somebody else holds it.",
                param(0),
                planet(param(1))
            ),
            id::LANDING_CRUSHED_DEFENDERS => format!(
                "Your troops have crushed the defenders of {}; it is yours.",
                planet(param(1))
            ),
            id::LANDING_LOST_THE_FIGHT => format!(
                "The colonists you landed on {} were lost in the fighting.",
                planet(param(0))
            ),
            id::HAS_STOLEN => format!(
                "{} has stolen {}kT of {} from {}.",
                fleet(),
                long(1),
                cargo_kind(param(3)),
                names.fleet(u16::try_from(i32::from(param(4)) & 0x1ff).unwrap_or(0))
            ),
            id::TRANSFER_FUTILE => format!(
                "{} had orders to transfer cargo with something that is not there; the order is dropped.",
                fleet()
            ),
            id::LOAD_NOT_YOUR_PLANET => format!(
                "{} tried to load {} from a planet you do not control; the order is cancelled.",
                fleet(),
                cargo_kind(param(1))
            ),
            id::LOAD_NOT_YOUR_FLEET => format!(
                "{} tried to load {} from a fleet you do not control; the order is cancelled.",
                fleet(),
                cargo_kind(param(1))
            ),
            id::SET_AMOUNT_SHORT | id::SET_NUMBER_SHORT => format!(
                "{} tried to set its {} aboard to {}, but {} could not provide that much.",
                fleet(),
                cargo_kind(param(1)),
                param(2),
                place_at(4)
            ),
            id::LOAD_FROM_SPACE => format!(
                "{} tried to load {} from deep space, without success.",
                fleet(),
                cargo_kind(param(1))
            ),
            id::FUEL_LOAD_FAILED => format!(
                "{} failed to load fuel at {}.",
                fleet(),
                place_at(1)
            ),
            id::FUEL_NOT_AVAILABLE => format!(
                "There is not enough fuel at {} for {} to reach its next waypoint; it is {}mg short.",
                place_at(0),
                names.fleet(u16::try_from(i32::from(param(2)) & 0x1ff).unwrap_or(0)),
                param(3)
            ),
            id::FUEL_NEVER_ENOUGH => format!(
                "{} can never reach its next waypoint: its tank holds {}mg and the leg needs about {}mg.",
                fleet(),
                param(1),
                param(2)
            ),
            id::BEAM_DOWN_UNINHABITED => format!(
                "{} tried to put colonists down on {}, but the planet is uninhabited: it has to be colonised first.",
                fleet(),
                planet(param(1))
            ),
            id::BEAM_DOWN_OVERRULED => format!(
                "The captain of {} was overruled in trying to put colonists down on {}: your people cannot live on a surface.",
                fleet(),
                planet(param(1))
            ),
            id::BEAM_DOWN_STARBASE => format!(
                "{} tried to put colonists down on {}, but the starbase there would kill them.",
                fleet(),
                planet(param(1))
            ),
            id::COLONISTS_TO_ANOTHER => format!(
                "{} may not hand colonists to another player.",
                fleet()
            ),
            id::BEAM_DOWN_SPACE => format!(
                "{} tried to put colonists out into deep space; the order is cancelled.",
                fleet()
            ),
            id::WORMHOLE_VANISHED => format!(
                "The wormhole {} was heading for is no longer there; it will go to where the wormhole was.",
                fleet()
            ),
            id::GAME_WON_BY_OTHERS => format!(
                "The game has been won by {}. You may play on, but the outcome is settled.",
                players(param(0))
            ),
            id::GAME_WON => {
                "You have been declared the winner of this game. You may play on if you wish."
                    .to_string()
            }
            id::GAME_WON_SHARED => format!(
                "You and {} have been declared the winners of this game. You may play on if you wish.",
                players(param(0))
            ),
            id::GAME_OVER_DEAD => {
                "You are out of the game: every planet of yours is overrun and every ship lost."
                    .to_string()
            }
            id::PLAYER_ELIMINATED => format!(
                "Nothing remains of player {} anywhere in the galaxy.",
                param(0) & 0xf
            ),
            id::LAST_ONE_STANDING => {
                "Every rival has been wiped from the galaxy; you alone are left to rule it."
                    .to_string()
            }
            id::MINEFIELD_VANISHED => format!(
                "The minefield {} was heading for is no longer there; it will go to where the field was.",
                fleet()
            ),
            id::CHASED_FLEET_GONE => format!(
                "The fleet {} was chasing, {}, has been destroyed or has gone; it will go to where that fleet was last seen.",
                fleet(),
                names.fleet(u16::try_from(i32::from(param(1)) & 0x1ff).unwrap_or(0))
            ),
            id::CHASED_FLEET_DUCKED => format!(
                "The fleet {} was chasing seems to have slipped behind {}; it will go to that planet instead.",
                fleet(),
                planet(param(1))
            ),
            id::CHASED_FLEET_OUTRUN => format!(
                "The fleet {} was chasing has outrun its scanners; it will go to where that fleet was last seen.",
                fleet()
            ),
            id::REROUTED => format!(
                "{} is at {} and has been routed on to {}.",
                fleet(),
                planet(param(1)),
                planet(param(2))
            ),
            id::REROUTED_SHORT_OF_FUEL => format!(
                "{} is at {} and has been routed on to {}, though it lacks the fuel to get there.",
                fleet(),
                planet(param(1)),
                planet(param(2))
            ),
            id::ARTIFACT_FOUND => {
                let field = usize::try_from(param(1))
                    .ok()
                    .and_then(|i| crate::research::TechField::ALL.get(i))
                    .map_or("?", |f| f.name());
                format!(
                    "Your colonists settling {} found a strange artifact: {} resources toward {}.",
                    planet(param(0)),
                    param(2),
                    field
                )
            }
            id::LANDING_AR_DIED => format!(
                "Your colonists landing on {} could not live on its surface and died.",
                planet(param(0))
            ),
            id::LANDING_STARBASE => format!(
                "Every colonist you landed on {} was killed by its starbase.",
                planet(param(0))
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

    /// The record this message writes into a file: as many parameters as
    /// the id's table entry says (`PackageUpMsg`, `1030:802a`, stores that
    /// many whatever it was handed), which is also what a reader takes
    /// back out.
    #[must_use]
    pub fn record(&self) -> MessageRecord {
        let mut params = self.params.clone();
        params.resize(stars_formats::message::parameter_count(self.id), 0);
        MessageRecord {
            id: self.id,
            object: self.object,
            params,
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
