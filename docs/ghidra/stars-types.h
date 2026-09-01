/*
 * stars-types.h — consolidated, self-contained C header of the Stars! 2.7j
 * game types (enums + structs) recovered from the game's own CodeView NB09
 * debug symbols via sirgwain's `stars-asm` project.
 *
 * This file is GENERATED. It is the concatenation of:
 *   1. this hand-written prelude (fixed-width + Win16 shim typedefs), then
 *   2. tmp/stars-asm/decompiled/enums.h   (the 74 game enums), then
 *   3. tmp/stars-asm/decompiled/structs.h (the 110 game structs),
 * with the `#include`/include-guard lines of structs.h stripped so the whole
 * thing parses cleanly on its own.
 *
 * It is meant to be fed to Ghidra's C parser (see apply_types.py / the
 * "Parse C Source" GUI action) to populate the data-type manager of the open
 * `stars.2.7j.exe` program. See docs/ghidra/README.md for the workflow and for
 * how to regenerate this file.
 *
 * Sizing note: the target program is x86:LE:16, where Ghidra's data
 * organisation uses char=1, short=2, int=2, long=4. The fixed-width shims below
 * therefore map int32_t/uint32_t onto `long`/`unsigned long` (4 bytes), not
 * `int`, and the Win16 object handles onto a 2-byte word.
 */

/* ---- fixed-width integer shims (stdint.h is not available to the parser) --- */
typedef signed char        int8_t;
typedef unsigned char      uint8_t;
typedef short              int16_t;
typedef unsigned short     uint16_t;
typedef long               int32_t;
typedef unsigned long      uint32_t;

/* ---- minimal Win16 shims used by the game structs/globals ------------------ */
/* Win16 GDI/USER object handles are 16-bit words. */
typedef uint16_t HANDLE;
typedef uint16_t HWND;
typedef uint16_t HDC;
typedef uint16_t HMENU;
typedef uint16_t HBRUSH;
typedef uint16_t HPEN;
typedef uint16_t HFONT;
typedef uint16_t HBITMAP;
typedef uint16_t HPALETTE;
typedef uint16_t HCURSOR;
typedef uint16_t HICON;
typedef uint16_t HRGN;
typedef uint16_t HINSTANCE;
typedef uint16_t HGLOBAL;
typedef uint16_t HRSRC;

typedef uint32_t COLORREF;

typedef struct tagPOINT {
    int16_t x;
    int16_t y;
} POINT;

typedef struct tagRECT {
    int16_t left;
    int16_t top;
    int16_t right;
    int16_t bottom;
} RECT;

/*
 * The 16-bit Stars! structures are byte-packed (their in-memory layout matches
 * the on-disk record layout — e.g. THING is exactly 18 bytes). Without this the
 * C parser inserts natural-alignment padding and structs come out too large
 * (THING -> 24, SHDEF -> 216, ...). Ghidra's CParser honours #pragma pack even
 * when fed a raw string (no preprocessor run), so declaring it here makes every
 * generated game struct pack to its true size.
 */
#pragma pack(1)

/* ---- forward declarations (make the header order-independent) ---- */

typedef struct _aipart AIPART;
typedef struct _aistarbase AISTARBASE;
typedef struct _aihist AIHIST;
typedef struct _armor ARMOR;
typedef struct _beam BEAM;
typedef struct _bomb BOMB;
typedef struct _btlplan BTLPLAN;
typedef struct _coldrop COLDROP;
typedef struct _compart COMPART;
typedef struct _cyberinfo CYBERINFO;
typedef struct _cyberinfotemp CYBERINFOTEMP;
typedef struct _dv DV;
typedef struct _engine ENGINE;
typedef struct _fleetid FLEETID;
typedef struct _framestuff FRAMESTUFF;
typedef struct _game GAME;
typedef struct _gdata GDATA;
typedef struct _hb HB;
typedef struct _hdr HDR;
typedef struct _hs HS;
typedef struct _hul HUL;
typedef struct _huldef HULDEF;
typedef struct _itemaction ITEMACTION;
typedef struct _kill KILL;
typedef struct _btlrec BTLREC;
typedef struct _btlrec26 BTLREC26;
typedef struct _logxfer LOGXFER;
typedef struct _logxferf LOGXFERF;
typedef struct _lsb LSB;
typedef struct _mdplr MDPLR;
typedef struct _mines MINES;
typedef struct _mining MINING;
typedef struct _msgbig MSGBIG;
typedef struct _msghdr MSGHDR;
typedef struct _msgplr MSGPLR;
typedef struct _msgturn MSGTURN;
typedef struct _obj OBJ;
typedef struct _part PART;
typedef struct _pl PL;
typedef struct _planet PLANET;
typedef struct _planetary PLANETARY;
typedef struct _planetminimal PLANETMINIMAL;
typedef struct _planetsome PLANETSOME;
typedef struct _fleet FLEET;
typedef struct _fleetsome FLEETSOME;
typedef struct _popupdata POPUPDATA;
typedef struct _prod PROD;
typedef struct PLPROD PLPROD;
typedef struct _prodq1 PRODQ1;
typedef struct _btn BTN;
typedef struct _btnt BTNT;
typedef struct _drawcir DRAWCIR;
typedef struct _rpt RPT;
typedef struct _rtbof RTBOF;
typedef struct _rtchgname RTCHGNAME;
typedef struct _rtChgPlanetLong RTCHGPLANETLONG;
typedef struct _rtChgProdQ RTCHGPRODQ;
typedef struct _rthisthdr RTHISTHDR;
typedef struct _rtloghdr RTLOGHDR;
typedef struct _rtlogthing RTLOGTHING;
typedef struct _rtplanet RTPLANET;
typedef struct _rtshdef RTSHDEF;
typedef struct _rtchgshdef RTCHGSHDEF;
typedef struct _rtshipint RTSHIPINT;
typedef struct _rtshipint2 RTSHIPINT2;
typedef struct _rtxfer RTXFER;
typedef struct _rtxferf RTXFERF;
typedef struct _rtxferl RTXFERL;
typedef struct _rtxferx RTXFERX;
typedef struct _sbar SBAR;
typedef struct _scan SCAN;
typedef struct _scanner SCANNER;
typedef struct _score SCORE;
typedef struct _scorex SCOREX;
typedef struct _selSome SELSOME;
typedef struct _shdef SHDEF;
typedef struct _shield SHIELD;
typedef struct _special SPECIAL;
typedef struct _specialsb SPECIALSB;
typedef struct _starpack STARPACK;
typedef struct _tasklaymines TASKLAYMINES;
typedef struct _taskpatrol TASKPATROL;
typedef struct _tasksell TASKSELL;
typedef struct _taskxport TASKXPORT;
typedef struct _order ORDER;
typedef struct PLORD PLORD;
typedef struct _rtwaypt RTWAYPT;
typedef struct _terra TERRA;
typedef struct _thmine THMINE;
typedef struct _thpack THPACK;
typedef struct _thtrader THTRADER;
typedef struct _thworm THWORM;
typedef struct _thing THING;
typedef struct _sel SEL;
typedef struct _tile TILE;
typedef struct _timer TIMER;
typedef struct _tok TOK;
typedef struct _btldata BTLDATA;
typedef struct _torp TORP;
typedef struct _turnserial TURNSERIAL;
typedef struct _vers VERS;
typedef struct _wn WN;
typedef struct _ini INI;
typedef struct _xfer XFER;
typedef struct _xferfull XFERFULL;
typedef struct _ziporder ZIPORDER;
typedef struct _zipprodq1 ZIPPRODQ1;
typedef struct _player PLAYER;
typedef struct _tutor TUTOR;
typedef struct _zipprodq ZIPPRODQ;

/* ---- enums (tmp/stars-asm/decompiled/enums.h, curated) ---- */

typedef enum HeapType { htOrd = 0, htString, htMsg, htPlanets, htLog, htFleets, htMisc, htShips, htPlrMsg, htPerm, htThings, htBattle, htCount } HeapType;

typedef enum GrPopupType {
    grPopupMineral        = 1,
    grPopupPlayer         = 2,
    grPopupFleet          = 3,
    grPopupUnknownObj     = 4,
    grPopupPlanetEnv      = 5,
    grPopupShipOrders     = 6,
    grPopupPlanet         = 7,
    grPopupPlanetIndustry = 8,
    grPopupComponent      = 9,
    grPopupString         = 10,
    grPopupShdef          = 11,
    grPopupResources      = 12,
    grPopupUnknown        = 13,
    grPopupShdefSB        = 14,
    grPopupShdefBuild     = 15,
} GrPopupType;

typedef enum HtMineType {
    htMineNone = 0,
    htMineMineralConc1 = 1,
    htMineMineralConc2 = 2,
    htMineMineralConc3 = 3,
    htMineUnused4 = 4,
    htMineScale = 5,
    htMineEnvVar0 = 6,
    htMineEnvVar1 = 7,
    htMineEnvVar2 = 8,
    htMineScanSel = 9,
    htMineOwner = 10,
    htMineShipOrFleet = 11,
    htMinePlanet = 12,
    htMineStarbase = 13,
    htMineMinefieldType = 14,
} HtMineType;

typedef enum HtMsgType {
    htMsgNone = 0,
    htMsgCurrent = 1,
    htMsgZoom = 2,
    htMsgMode = 3,
} HtMsgType;


typedef enum DtFileType {
    dtXY = 0,
    dtLog = 1,
    dtHost = 2,
    dtTurn = 3,
    dtHist = 4,
} DtFileType;

typedef enum RaceGrbit {
    ibitRaceIFE = 0x00,
    ibitRaceTT = 0x01,
    ibitRaceARM = 0x02,
    ibitRaceISB = 0x03,
    ibitRaceGeneralizedResearch = 0x04,
    ibitRaceMineralAlchemy = 0x06,
    ibitRaceNoRamscoops = 0x07,
    ibitRaceCheapEngines = 0x08,
    ibitRaceOBRM = 0x09,
    ibitRaceNoAdvScanner = 0x0a,
    ibitRaceLowStartingPop = 0x0b,
    ibitRaceBleedingEdgeTech = 0x0c,
    ibitRaceRegeneratingShields = 0x0d,
    ibitRaceTech3 = 0x1d,
    ibitRaceAIPlayer = 0x1e,
    ibitRaceCheapFact = 0x1f,
    ibitRaceLast = 32,
} RaceGrbit;

typedef enum RaceStat {
    rsResGen = 0,
    rsFactProd = 1,
    rsFactBuild = 2,
    rsFactOperate = 3,
    rsMineProd = 4,
    rsMineBuild = 5,
    rsMineOperate = 6,
    rsUseLeftover = 7,
    rsTechBonus1 = 8,
    rsTechBonus2 = 9,
    rsTechBonus3 = 10,
    rsTechBonus4 = 11,
    rsTechBonus5 = 12,
    rsTechBonus6 = 13,
    rsMajorAdv = 14,
} RaceStat;

typedef enum RaceAttribute {
    raCheapCol = 0,
    raStealth = 1,
    raAttack = 2,
    raTerra = 3,
    raDefend = 4,
    raMines = 5,
    raMassAccel = 6,
    raStargate = 7,
    raMacintosh = 8,
    raNone = 9,
    raMax = 10,
} RaceAttribute;

typedef enum GrobjClass {
    grobjNone = 0x0,
    grobjPlanet = 0x1,
    grobjFleet = 0x2,
    grobjOther = 0x4,
    grobjThing = 0x8,
} GrobjClass;

typedef enum HullSlotType {
    hstNone = 0x0000,
    hstEngine = 0x0001,
    hstScanner = 0x0002,
    hstShield = 0x0004,
    hstArmor = 0x0008,
    hstBeam = 0x0010,
    hstTorp = 0x0020,
    hstBomb = 0x0040,
    hstMining = 0x0080,
    hstMines = 0x0100,
    hstSpecialSB = 0x0200,
    hstSBHull = 0x0400,
    hstSpecialE = 0x0800,
    hstSpecialM = 0x1000,
    hstTerra = 0x2000,
    hstHull = 0x4000,
    hstPlanetary = 0x8000,
    hstWeapon      = hstBeam | hstTorp,
    hstShArm       = hstShield | hstArmor,
    hstSpecialEM   = hstSpecialE | hstSpecialM,
    hstScanSpec    = hstScanner | hstSpecialE | hstSpecialM,
    hstShWeap      = hstShield | hstBeam | hstTorp,
    hstSomeSB      = hstSpecialSB | hstSpecialE,
    hstSpecMine    = hstSpecialE | hstMines,
    hstShSpec      = hstShield | hstSpecialE | hstSpecialM,
    hstScanSpecArm = hstScanner | hstArmor | hstSpecialE | hstSpecialM,
    hstEnabled     = 0x19FF,
    hstSome        = 0x193E,

} HullSlotType;

typedef enum HulDef {
    ihuldefSmallFreighter = 0,
    ihuldefMediumFreighter = 1,
    ihuldefLargeFreighter = 2,
    ihuldefSuperFreighter = 3,
    ihuldefScout = 4,
    ihuldefFrigate = 5,
    ihuldefDestroyer = 6,
    ihuldefCruiser = 7,
    ihuldefBattleCruiser = 8,
    ihuldefBattleship = 9,
    ihuldefDreadnought = 10,
    ihuldefPrivateer = 11,
    ihuldefRogue = 12,
    ihuldefGalleon = 13,
    ihuldefMiniColonyShip = 14,
    ihuldefColonyShip = 15,
    ihuldefMiniBomber = 16,
    ihuldefB17Bomber = 17,
    ihuldefStealthBomber = 18,
    ihuldefB52Bomber = 19,
    ihuldefMidgetMiner = 20,
    ihuldefMiniMiner = 21,
    ihuldefMiner = 22,
    ihuldefMaxiMiner = 23,
    ihuldefUltraMiner = 24,
    ihuldefFuelTransport = 25,
    ihuldefSuperFuelXport = 26,
    ihuldefMiniMineLayer = 27,
    ihuldefSuperMineLayer = 28,
    ihuldefNubian = 29,
    ihuldefMiniMorph = 30,
    ihuldefMetaMorph = 31,
    ihuldefOrbitalFort = 32,
    ihuldefSpaceDock = 33,
    ihuldefSpaceStation = 34,
    ihuldefUltraStation = 35,
    ihuldefDeathStart = 36,
} HulDef;

typedef enum StartingStarbase {
    Starbase = 0,
    AcceleratorPlatform = 1,
    PortholetoBeyond = 2,
    StarterColony = 3,
} StartingStarbase;

typedef enum StartingShip {
    LilliputianFreighter = 0,
    ShadowTransport = 1,
    SmaugarianPeepingTom = 2,
    ArmedProbe = 3,
    LongRangeScout = 4,
    ShadowSleuth = 5,
    Teamster = 6,
    StalwartDefender = 7,
    Swashbuckler = 8,
    SantaMaria = 9,
    Pinta = 10,
    Mayflower = 11,
    SporeCloud = 12,
    Gadfly = 13,
    CottonPicker = 14,
    PotatoBug = 15,
    LittleHen = 16,
    ChangeofHeart = 17,
    SpeedTurtle = 18,
    MTLifeboat = 19,
    MTScout = 20,
    MTProbe = 21,
} StartingShip;

typedef enum ThingType {
    ithMinefield = 0,
    ithMineralPacket = 1,
    ithWormhole = 2,
    ithMysteryTrader = 3,
} ThingType;

typedef enum MdBuild {
    mdBuildShdef      = 0,
    mdBuildHuldef     = 1,
    mdBuildEnemyShdef = 2,
    mdBuildComp       = 3,
    mdBuildEdit       = 4,
} MdBuild;

typedef enum ProdItemType {
    iobjMine = 0,
    iobjFactory = 1,
    iobjDefense = 2,
    iobjAlchemy = 3,
    iobjMinTerraform = 4,
    iobjMaxTerraform = 5,
    iobjPacket = 6,
    mdIdleFactory = 7,
    mdIdleMine = 8,
    mdIdleDefense = 9,
    /* 10 unused ? */
    mdIdleAlchemy = 11,
    mdIdleTerraform = 12,
    iobjGenesis = 13,
    iobjPacketIron = 14,
    iobjPacketBor = 15,
    iobjPacketGerm = 16,
    iobjPacketMixed = 17,

    iobjPlanetaryScannerFirst = 18,
    iobjPlanetaryScannerViewer50 = 18,
    iobjPlanetaryScannerViewer90 = 19,
    iobjPlanetaryScannerScoper150 = 20,
    iobjPlanetaryScannerScoper220 = 21,
    iobjPlanetaryScannerScoper280 = 22,
    iobjPlanetaryScannerSnooper320X = 23,
    iobjPlanetaryScannerSnooper400X = 24,
    iobjPlanetaryScannerSnooper500X = 25,
    iobjPlanetaryScannerSnooper620X = 26,
    iobjPlanetaryScannerLast = 26,
    iobjPlanetaryScanner = 27,
    iobjUnknown = 31,
} ProdItemType;

typedef enum StringId {
    idsUniverseDefinitionFileSeemsMissingCorrupt = 0x0000,
    idsPlayerLogFileAppearsCorruptUnableLoad = 0x0001,
    idsHistoryFileAppearsCorruptHistoricalDataWill = 0x0002,
    idsGameFileAppearsCorruptUnableLoadFile = 0x0003,
    idsCantOpenFile = 0x0004,
    idsUniverseCreationFileAppearsInvalid = 0x0005,
    idsIllegalGameTitle = 0x0006,
    idsLine2HasBadUniverseDefinitionParameter = 0x0007,
    idsLine3HasBadUniverseDefinitionParameter = 0x0008,
    idsLine4HasImproperNumberPlayerFiles = 0x0009,
    idsLineDUnableLoadRaceFileS = 0x000a,
    idsLineDHasImproperVictoryConditionDefinition = 0x000b,
    idsUniverseDefinitionFileAppearsTooShort = 0x000c,
    idsFileDoesBelongVersionStars = 0x000d,
    idsGameCurrentlyLoaded = 0x000e,
    idsCantChangeZoomFactorUntilGameOpen = 0x000f,
    idsLogFileHasReachedMaximumAllowableSize = 0x0010,
    idsUnableCreateLogFile = 0x0011,
    idsUnableCreateHistoryFile = 0x0012,
    idsUnableCreateHostFile = 0x0013,
    idsUnableCreateUniverseDefinitionFile = 0x0014,
    idsHostFileMarkedUseAnotherInstanceStars = 0x0015,
    idsErrorWritingFile = 0x0016,
    idsUnableLoadBitmaps = 0x0017,
    idsUnableInitializeStars = 0x0018,
    idsUnableOpenHostFile = 0x0019,
    idsMemory = 0x001a,
    idsUnableOpenNewTurnFile = 0x001b,
    idsFileDate = 0x001c,
    idsFileGame = 0x001d,
    idsLogFileRecentGameTryingLoadIgnoring = 0x001e,
    idsAmountCargoMaySpecifyHereMustBetween = 0x001f,
    idsThereIsntEnoughFreeMemoryModifyProduction = 0x0020,
    idsMessageTypeHasFilteredWillShownDefault = 0x0021,
    idsMessagesHaveSentYearFilteredIfWant = 0x0022,
    idsWarningColonizeMissionCannotCarriedBecauseNone = 0x0023,
    idsNoteShipsFleetWillDismantledProvideSupplies = 0x0024,
    idsNoteShipsFleetWillDismantledMineralsCan = 0x0025,
    idsNoteShipsFleetWillDismantledMineralsWill = 0x0026,
    idsFuelUsageVsWarpSpeed = 0x0027,
    idsShieldCoverageVsDefenseQuan = 0x0028,
    idsEngineCanMountedMiniColonizerHullRequires = 0x0029,
    idsEngineCreatesPowerfulWavesRadiationWillKill = 0x002a,
    idsEngineRequiresLesserRacialTraitImprovedFuel = 0x002b,
    idsEngineRequiresLesserRacialTraitImprovedFuel2 = 0x002c,
    idsEngineRequiresLesserRacialTraitRamScoop = 0x002d,
    idsStargateRequiresPrimaryRacialTraitInterstellarTr = 0x002e,
    idsMassDriverRequiresPrimaryRacialTraitPacket = 0x002f,
    idsHullWillHaveBuiltScannerIfJack = 0x0030,
    idsEnemyFleetsOrbitingPlanetCanDetectedD = 0x0031,
    idsEnemyFleetsCannotDetectedScannerUnlessSame = 0x0032,
    idsScannerCapableDeterminingPlanetsEnvironmentCompo = 0x0033,
    idsScannerCanDeterminePlanetsBasicStatsDistance = 0x0034,
    idsScannerCanDeterminePlanetsBasicStatsDistance2 = 0x0035,
    idsScannerCapablePenetratingDefensesEnemyFleetsAllo = 0x0036,
    idsScannerCanDeterminePlanetsStatsDistance120 = 0x0037,
    idsScannerRequiresPrimaryRacialTraitSuperStealth = 0x0038,
    idsCloakRequiresPrimaryRacialTraitSuperStealth = 0x0039,
    idsArmorShieldRequiresPrimaryRacialTraitSuper = 0x003a,
    idsShieldRequiresPrimaryRacialTraitInnerStrength = 0x003b,
    idsShieldDecreasesRangeWhichEnemyShipsCan = 0x003c,
    idsArmorDecreasesRangeWhichEnemyShipsCan = 0x003d,
    idsArmorAlsoActsPartShieldWhichWill = 0x003e,
    idsShieldAlsoContainsArmorComponentWhichWill = 0x003f,
    idsShieldAlsoProvides65dpArmor5Jamming = 0x0040,
    idsBombWillAvailableIfPrimaryRaceTrait = 0x0041,
    idsStargatesAvailableIfPrimaryRaceTraitHyper = 0x0042,
    idsCargo = 0x0043,
    idsTechnologyStatus = 0x0044,
    idsExpectedResearchBenefits = 0x0045,
    idsCurrentlyResearching = 0x0046,
    idsResourceAllocation = 0x0047,
    idsFuelCapacity = 0x0048,
    idsCargoCapacity = 0x0049,
    idsArmorStrength = 0x004a,
    idsInitiative = 0x004b,
    idsResourcesNeededComplete = 0x004c,
    idsEstimatedTimeCompletion = 0x004d,
    idsAnnualResourcesPlanets = 0x004e,
    idsTotalResourcesSpentResearchLastYear = 0x004f,
    idsResourcesBudgetedResearch = 0x0050,
    idsYearsProjectedResearchBudget = 0x0051,
    idsFieldResearch = 0x0052,
    idsSameField = 0x0053,
    idsEnergy = 0x0054,
    idsWeapons = 0x0055,
    idsPropulsion = 0x0056,
    idsConstruction = 0x0057,
    idsElectronics = 0x0058,
    idsBiotechnology = 0x0059,
    idsLowestField = 0x005a,
    idsEner = 0x005b,
    idsWeap = 0x005c,
    idsProp = 0x005d,
    idsConst = 0x005e,
    idsElect = 0x005f,
    idsBio = 0x0060,
    idsSureWantDeleteCurrentWaypoint = 0x0061,
    idsGameAlreadyHostedAnotherInstanceStarsWould = 0x0062,
    idsTaskHere = 0x0063,
    idsTransport = 0x0064,
    idsColonize = 0x0065,
    idsRemoteMining = 0x0066,
    idsMergeFleet = 0x0067,
    idsScrapFleet = 0x0068,
    idsLayMineField = 0x0069,
    idsPatrol = 0x006a,
    idsRoute = 0x006b,
    idsTransferFleet = 0x006c,
    idsAction = 0x006d,
    idsLoadAvailable = 0x006e,
    idsUnload = 0x006f,
    idsLoadExactly = 0x0070,
    idsUnloadExactly = 0x0071,
    idsFill = 0x0072,
    idsWait = 0x0073,
    idsLoadDunnage = 0x0074,
    idsSetAmount = 0x0075,
    idsSetWaypoint = 0x0076,
    idsLoadOptimal = 0x0077,
    idsNobody = 0x0078,
    idsEnemies = 0x0079,
    idsNeutralsEnemies = 0x007a,
    idsEveryone = 0x007b,
    idsImprovePlanet = 0x007c,
    idsUndoTerraforming = 0x007d,
    idsMines = 0x007e,
    idsFactories = 0x007f,
    idsDefenses = 0x0080,
    idsAlchemy = 0x0081,
    idsMinTerraform = 0x0082,
    idsMaxTerraform = 0x0083,
    idsMineralPackets = 0x0084,
    idsFactory = 0x0085,
    idsMine = 0x0086,
    idsDefenses2 = 0x0087,
    ids0136Blank = 0x0088,
    idsMineralAlchemy = 0x0089,
    idsTerraformEnvironment = 0x008a,
    idsGenesisDevice = 0x008b,
    idsIroniumMineralPacket = 0x008c,
    idsBoraniumMineralPacket = 0x008d,
    idsGermaniumMineralPacket = 0x008e,
    idsMixedMineralPacket = 0x008f,
    idsWindows = 0x0090,
    idsStarsIni = 0x0091,
    idsMisc = 0x0092,
    idsZiporders = 0x0093,
    idsMain = 0x0094,
    idsShiptiles = 0x0095,
    idsPlanettiles = 0x0096,
    idsScanzoom = 0x0097,
    idsSelection = 0x0098,
    idsFiles = 0x0099,
    idsWait2 = 0x009a,
    idsFile1 = 0x009b,
    idsTurn = 0x009c,
    idsScanmodev25 = 0x009d,
    idsScanfilterv25 = 0x009e,
    idsScanefilterv25 = 0x009f,
    idsScanmines = 0x00a0,
    idsScanradar = 0x00a1,
    idsMineralscale = 0x00a2,
    idsLayout = 0x00a3,
    idsGlobalsettings = 0x00a4,
    idsStyle1width = 0x00a5,
    idsStyle1height = 0x00a6,
    idsStyle1height2 = 0x00a7,
    idsStyle2width = 0x00a8,
    idsStyle2height = 0x00a9,
    idsStyle2height2 = 0x00aa,
    idsToolbar = 0x00ab,
    idsGameid = 0x00ac,
    idsResolution = 0x00ad,
    idsDefaultpassword = 0x00ae,
    idsProgress = 0x00af,
    idsBackups = 0x00b0,
    idsReportplanwin = 0x00b1,
    idsReportfleetwin = 0x00b2,
    idsReportefleetwin = 0x00b3,
    idsReportbtlwin = 0x00b4,
    idsReportplanfld = 0x00b5,
    idsReportplansort = 0x00b6,
    idsReportfleetfld = 0x00b7,
    idsReportfleetsort = 0x00b8,
    idsReportefleetfld = 0x00b9,
    idsReportefltsort = 0x00ba,
    idsReportbtlfld = 0x00bb,
    idsReportbtlsort = 0x00bc,
    idsReportdefgraph = 0x00bd,
    idsSoundfx = 0x00be,
    idsHistoryinfo = 0x00bf,
    idsVcrspeed = 0x00c0,
    idsMusic = 0x00c1,
    idsTracks = 0x00c2,
    idsTrack = 0x00c3,
    idsFonts = 0x00c4,
    idsArial = 0x00c5,
    idsArialbold = 0x00c6,
    idsArialitalic = 0x00c7,
    idsArialbolditalic = 0x00c8,
    idsNewreports = 0x00c9,
    idsNohostnames = 0x00ca,
    idsMessage = 0x00cb,
    idsLogging = 0x00cc,
    idsHave = 0x00cd,
    idsOn = 0x00ce,
    idsMayBuild = 0x00cf,
    idsHoweverColonistsCurrentlyCapableOperating = 0x00d0,
    idsThem = 0x00d1,
    idsApplyDefineProductionTemplate = 0x00d2,
    idsRightClickBlueDiamondApplyProductionTemplate = 0x00d3,
    idsPodIncreasesFuelCapacityShipDmg = 0x00d4,
    idsPodIncreasesCargoCapacityShipDkt = 0x00d5,
    idsPodIncreasesCargoCapacityShip250ktProvides = 0x00d6,
    idsPodAllowsShipColonizePlanetWillDismantle = 0x00d7,
    idsModuleContainsEmptyOrbitalHullWhichCan = 0x00d8,
    idsDeviceAllowsShipJumpPlanetaryStargatesRange = 0x00d9,
    idsDeflectorDecreasesDamageDoneBeamWeaponsShip = 0x00da,
    idsCloaksUnarmedHullsReducingRangeWhichScanners = 0x00db,
    idsCloaksAnyShipReducingRangeWhichScanners = 0x00dc,
    idsCloaksAnyShip30Acts10Jammer = 0x00dd,
    idsRememberLoadColonistsBeforeEmbarkingMission = 0x00de,
    idsModuleContainsRobotsCapableMining = 0x00df,
    idsKtEachMineralDependingConcentrationUninhabitedPl = 0x00e0,
    idsModuleAlsoActs30Cloak30Jammer = 0x00e1,
    idsWarningFleetContainsShipsRemoteMiningModules = 0x00e2,
    idsWarningFleetContainsShipsAdjusterModules = 0x00e3,
    idsWarningMustPlanetPerformRemoteTerraforming = 0x00e4,
    idsNoteCanMineUninhabitedPlanets = 0x00e5,
    idsWarningFleetHasMineLayingPods = 0x00e6,
    idsWarningFleetHasBeamsSweepMines = 0x00e7,
    idsFleetCanDestroyLdMinesPerYear = 0x00e8,
    idsFleetCanLayLdMinesPerYear = 0x00e9,
    idsPasswordHaveEnteredIncorrectPleaseTry = 0x00ea,
    idsPasswordsTypedTwoFieldsSamePleaseReenter = 0x00eb,
    idsHaveUsingDemoVersionStars20Days = 0x00ec,
    idsGenerates = 0x00ed,
    idsResourcesEachYear = 0x00ee,
    idsResourcesHaveAllocatedResearch = 0x00ef,
    idsLeaves = 0x00f0,
    idsResourcesUsePlanet = 0x00f1,
    idsResourcesPlanetEqualSquareRootPopulation = 0x00f2,
    idsPlanetaryDataAvailableEstimateMineralMiningRates = 0x00f3,
    idsShipDesignDoesHaveAnyEnginesMust = 0x00f4,
    idsMaximumColonistGrowthRatePerYear = 0x00f5,
    idsOneResourceGeneratedEachYearEvery = 0x00f6,
    idsColonists = 0x00f7,
    idsEvery10FactoriesProduce = 0x00f8,
    idsResourcesEachYear2 = 0x00f9,
    idsFactoriesRequire = 0x00fa,
    idsResourcesBuild = 0x00fb,
    idsEvery10000ColonistsMayOperate = 0x00fc,
    idsFactories2 = 0x00fd,
    idsEvery10MinesProduce = 0x00fe,
    idsEachMineralEveryYear = 0x00ff,
    idsMinesRequire = 0x0100,
    idsResourcesBuild2 = 0x0101,
    idsEvery10000ColonistsMayOperate2 = 0x0102,
    idsMines2 = 0x0103,
    idsAnnualResourcesPlanetValueSqrtPopulationEnergy = 0x0104,
    idsMsg0261 = 0x0105,
    idsSurfaceMinerals = 0x0106,
    idsMineralConcentrations = 0x0107,
    idsMines3 = 0x0108,
    idsFactories3 = 0x0109,
    idsDefenses3 = 0x010a,
    idsStarsUnableSaveRaceDataFilePlease = 0x010b,
    idsSettlersDelightEngineMayMountedDesignsBased = 0x010c,
    idsOrbitalConstructionModuleMayMountedDesignsBased = 0x010d,
    idsCustomRaceWizardStepD6 = 0x010e,
    idsViewRacePageD6 = 0x010f,
    idsAdvancedNewGameWizardStepD3 = 0x0110,
    idsViewGameParametersPageD3 = 0x0111,
    idsPrimaryRacialTrait = 0x0112,
    idsDescriptionTrait = 0x0113,
    idsMustExpandSurviveGivenSmallCheapColony = 0x0114,
    idsRaceWillGrowTwiceGrowthRateSelect = 0x0115,
    idsCompletelyFlexibleMetaMorphHullWillAvailable = 0x0116,
    idsCanSneakThroughEnemyTerritoryExecuteStunning = 0x0117,
    idsCargoDoesDecreaseCloakingAbilitiesStealthBomber = 0x0118,
    idsTwoScannersWhichAllowStealMineralsEnemy = 0x0119,
    idsRuleBattleFieldColonistsAttackBetterShips = 0x011a,
    idsStartGameKnowledgeTech6WeaponsTech = 0x011b,
    idsUnfortunatelyRaceDoesntUnderstandNecessityBuildi = 0x011c,
    idsExpertFiddlingPlanetaryEnvironmentsStartGameTech = 0x011d,
    idsBombsUnterraformEnemyWorldsTerraformingCostsNoth = 0x011e,
    idsVariable1PerYear = 0x011f,
    idsStrongHardDefeatColonistsRepelAttacksBetter = 0x0120,
    idsCanLaySpeedTrapMineFieldsHave = 0x0121,
    idsSmartBombsPlanetaryDefensesCost40Though = 0x0122,
    idsExpertLayingMineFieldsHaveVastArray = 0x0123,
    idsMineFieldsActScannersHaveAbilityRemote = 0x0124,
    idsMineFieldsStartGame2MineLaying = 0x0125,
    idsRaceExcelsAcceleratingMineralPacketsDistantPlane = 0x0126,
    idsWillEventuallyAbleFlingPacketsMindNumbing = 0x0127,
    idsWillStartGameOwningSecondPlanetDistance = 0x0128,
    idsRaceExcelsBuildingStargatesStartTech5 = 0x0129,
    idsHaveStargatesEventuallyMayBuildStargatesWhich = 0x012a,
    idsPlanetStargateWhichRangeOneStargatesExceeding = 0x012b,
    idsRaceDevelopedAlternatePlanePeopleCannotSurvive = 0x012c,
    idsHaveIntrinsicAbilityMineScanEnemyFleets = 0x012d,
    idsDeterminedTypeStarbaseHaveWillEventuallyAble = 0x012e,
    idsRaceDoesSpecializeSingleAreaStartGame = 0x012f,
    idsScoutDestroyerFrigateHullsHaveBuiltPenetrating = 0x0130,
    ids0305Blank = 0x0131,
    idsImprovedFuelEfficiency = 0x0132,
    idsTotalTerraforming = 0x0133,
    idsAdvancedRemoteMining = 0x0134,
    idsImprovedStarbases = 0x0135,
    idsGeneralizedResearch = 0x0136,
    idsUltimateRecycling = 0x0137,
    idsMineralAlchemy2 = 0x0138,
    idsRamScoopEngines = 0x0139,
    idsCheapEngines = 0x013a,
    idsBasicRemoteMining = 0x013b,
    idsAdvancedScanners = 0x013c,
    idsLowStartingPopulation = 0x013d,
    idsBleedingEdgeTechnology = 0x013e,
    idsRegeneratingShields = 0x013f,
    idsGivesFuelMizerGalaxyScoopEnginesIncreases = 0x0140,
    idsAllowsTerraformInvestingSolelyBiotechnologyMayTe = 0x0141,
    idsGivesThreeAdditionalMiningHullsTwoNew = 0x0142,
    idsGivesTwoNewStarbaseDesignsStardockAllows = 0x0143,
    idsRaceTakesHolisticApproachResearchHalfResources = 0x0144,
    idsWhenScrapFleetStarbaseRecover90Minerals = 0x0145,
    idsAllowsTurnResourcesMineralsFourTimesEfficiently = 0x0146,
    idsEnginesWhichTravelWarp5GreaterBurning = 0x0147,
    idsCanThrowEnginesTogetherHalfCostHowever = 0x0148,
    idsMiningShipAvailableWillMiniMinerTrait = 0x0149,
    idsPlanetPenetratingScannersWillAvailableHoweverCon = 0x014a,
    idsWillStart30FewerColonists = 0x014b,
    idsNewTechsInitiallyCostTwiceMuchBuild = 0x014c,
    idsShields40StrongerListedRatingShieldsRegenerate = 0x014d,
    idsRobotMinerRequiresLesserRacialTraitAdvanced = 0x014e,
    idsMiningHullRequiresLesserRacialTraitAdvanced = 0x014f,
    idsRobotMinerWillAvailableIfLesserRacial = 0x0150,
    idsMineRequiresPrimaryRacialTraitSpaceDemolition = 0x0151,
    idsMineRequiresPrimaryRacialTraitSpaceDemolition2 = 0x0152,
    idsHullRequiresPrimaryRacialTraitHyperExpansion = 0x0153,
    idsHullUnavailableIfHaveRaceDisadvantageBasic = 0x0154,
    idsPartRequiresPrimaryRacialTraitInnerStrength = 0x0155,
    idsPartRequiresPrimaryRacialTraitWarMonger = 0x0156,
    idsHullRequiresPrimaryRacialTraitWarMonger = 0x0157,
    idsHullRequiresPrimaryRacialTraitSuperStealth = 0x0158,
    idsHullRequiresPrimaryRacialTraitInnerStrength = 0x0159,
    idsHullRequiresPrimaryRacialTraitAlternateReality = 0x015a,
    idsPlanetaryDefenseUnavailablePrimaryRacialTraitWar = 0x015b,
    idsTotalTerraformingRequiresLesserRacialTraitTotal = 0x015c,
    idsPartAvailableAlternateRealityRaces = 0x015d,
    idsPartRequiresPrimaryRacialTraitAlternateReality = 0x015e,
    idsPartRequiresPrimaryRacialTraitClaimAdjuster = 0x015f,
    idsModifiedMiningRobotTerraformsInhabitedPlanets1 = 0x0160,
    idsBombDoesKillColonistsDestroyInstallationsBomb = 0x0161,
    idsAllowsModifyAnyPlanetsThreeEnvironmentVariables = 0x0162,
    idsAllowsModifyPlanetsSDOriginalValue = 0x0163,
    idsScannerWillUnavailableIfHaveLesserRacial = 0x0164,
    idsHullRequiresPrimaryRaceTraitSpaceDemolition = 0x0165,
    idsIfTerraform = 0x0166,
    idsPlanetsValueWouldImprove = 0x0167,
    idsValueDOutsideHabitableRangeRace = 0x0168,
    idsValueDAwayIdealValueRace = 0x0169,
    idsNormalView = 0x016a,
    idsSurfaceMineralView = 0x016b,
    idsMineralConcentrationView = 0x016c,
    idsPlanetValueView = 0x016d,
    idsPopulationView = 0x016e,
    idsPlayerInfoView = 0x016f,
    idsAddWayPointsMode = 0x0170,
    idsScannerCoverageOverlay = 0x0171,
    idsMineFieldsOverlay = 0x0172,
    idsFleetPathsOverlay = 0x0173,
    idsIdleFleetsFilter = 0x0174,
    idsPlanetNamesOverlay = 0x0175,
    idsShipDesignFilter = 0x0176,
    idsDesignFilterMenu = 0x0177,
    idsEnemyShipClassFilter = 0x0178,
    idsEnemyClassFilterMenu = 0x0179,
    idsZoomMenu = 0x017a,
    idsShipCountsOverlay = 0x017b,
    idsScannerEffective = 0x017c,
    idsColony = 0x017d,
    idsFreighter = 0x017e,
    idsScout = 0x017f,
    idsWarship = 0x0180,
    idsUtility = 0x0181,
    idsBomber = 0x0182,
    idsMiner = 0x0183,
    idsFuelTransport = 0x0184,
    idsMustHaveLeastOnePlayerGame = 0x0185,
    idsTransportCloakingModuleMayPlacedHullCould = 0x0186,
    idsCanExpect1DPlanetsWillHabitable = 0x0187,
    idsPlanetsWillHabitableRace = 0x0188,
    idsVirtuallyPlanetsWillHabitableRace = 0x0189,
    idsIncreasesSpeedBattle1DSquareMovement = 0x018a,
    idsYetImplemented = 0x018b,
    idsEngineWillUnavailableIfHaveLesserRacial = 0x018c,
    idsRightClickBlueDiamondBringPopupMenu = 0x018d,
    idsTurnHasSubmittedChangesMadeAfterTurn = 0x018e,
    idsNewTurnCurrentlyGeneratedHostNewTurn = 0x018f,
    idsNoneDisengage = 0x0190,
    idsAny = 0x0191,
    idsStarbase = 0x0192,
    idsArmedShips = 0x0193,
    idsBombersFreighters = 0x0194,
    idsUnarmedShips = 0x0195,
    idsFuelTransports = 0x0196,
    idsFreighters = 0x0197,
    idsDisengage = 0x0198,
    idsDisengageIfChallenged = 0x0199,
    idsMinimizeDamageSelf = 0x019a,
    idsMaximizeNetDamage = 0x019b,
    idsMaximizeDamageRatio = 0x019c,
    idsMaximizeDamage = 0x019d,
    idsDisengage2 = 0x019e,
    idsCargo2 = 0x019f,
    idsGoto = 0x01a0,
    idsMerge = 0x01a1,
    idsGoto2 = 0x01a2,
    idsPrev = 0x01a3,
    idsNext = 0x01a4,
    idsRename = 0x01a5,
    idsJettison = 0x01a6,
    idsSplit = 0x01a7,
    idsSplit2 = 0x01a8,
    idsMerge2 = 0x01a9,
    idsChange = 0x01aa,
    idsClear = 0x01ab,
    idsFuel = 0x01ac,
    idsCargoHold = 0x01ad,
    idsIronium = 0x01ae,
    idsBoranium = 0x01af,
    idsGermanium = 0x01b0,
    idsColonists2 = 0x01b1,
    idsPacketShell = 0x01b2,
    idsPlanets = 0x01b3,
    idsStarbases = 0x01b4,
    idsUnarmedShips2 = 0x01b5,
    idsEscortShips = 0x01b6,
    idsCapitalShips = 0x01b7,
    idsTechLevels = 0x01b8,
    idsResources = 0x01b9,
    idsScore = 0x01ba,
    idsRank = 0x01bb,
    idsCurrent = 0x01bc,
    idsFieldStudy = 0x01bd,
    idsN999999 = 0x01be,
    idsBombWillKillAnyPlanetsPopulation = 0x01bf,
    idsBombWillKillApproximatelyDDPlanets = 0x01c0,
    idsIfPlanetHasDefensesBombGuaranteedKill = 0x01c1,
    idsBombWillDamagePlanetsMinesFactories = 0x01c2,
    idsBombWillDestroyApproximatelyDPlanetsMines = 0x01c3,
    idsDifficultyLevel = 0x01c4,
    idsUniverseSize = 0x01c5,
    idsPlayerRace = 0x01c6,
    idsDensity = 0x01c7,
    idsPlayerPositions = 0x01c8,
    idsAdvancedGame = 0x01c9,
    idsButtonAllowsConfigureMultiPlayerGamesCustom = 0x01ca,
    idsShootingFishBarrel = 0x01cb,
    idsWalkPark = 0x01cc,
    idsSleepwalkingParadise = 0x01cd,
    idsBigEasy = 0x01ce,
    idsEternalBliss = 0x01cf,
    idsDuckHunt = 0x01d0,
    idsBarefootJaywalk = 0x01d1,
    idsRumbleJungle = 0x01d2,
    idsInfectedRootCanal = 0x01d3,
    idsJungleSafari = 0x01d4,
    idsMicroHardball = 0x01d5,
    idsRollerBall = 0x01d6,
    idsWallStreet = 0x01d7,
    idsBigLeague = 0x01d8,
    idsLongRoadMorning = 0x01d9,
    idsToughNuts = 0x01da,
    idsBladeRunner = 0x01db,
    idsDDay = 0x01dc,
    idsWorldWarIii = 0x01dd,
    idsEternityHell = 0x01de,
    idsNewGame = 0x01df,
    idsOpenGame = 0x01e0,
    idsContinueGame = 0x01e1,
    idsEXitStars = 0x01e2,
    idsAppearsHavePlanetaryDefenses = 0x01e3,
    idsHasPlanetaryDefensesApproximatelyDCoverage = 0x01e4,
    idsSureWantDeleteEverythingPlanetsProductionQueue = 0x01e5,
    idsTutorial = 0x01e6,
    idsTutorialGame = 0x01e7,
    idsTutorialHasRunBeforeWouldLikeDestroy = 0x01e8,
    idsCurrentlyRunningStarsTutorialDoWantExit = 0x01e9,
    idsTutorialTurnWillGeneratedHaveYetCompleted = 0x01ea,
    idsTutorialProductionQueueDoesContainRequestedItem = 0x01eb,
    idsTutorialHaveGivenFleetWrongDestinationPress = 0x01ec,
    idsTutorialHaveGivenFleetTaskDestinationWaypoint = 0x01ed,
    idsTutorialHaveGivenFleetWrongTaskDestination = 0x01ee,
    idsTutorialHaveLoadedWrongCargoFleetPlease = 0x01ef,
    idsTutorialProductionQueueDoesContainRightCount = 0x01f0,
    idsTutorialShouldDeleteShipDesignPointTutorial = 0x01f1,
    idsTutorialShouldCustomizeShipDesignPointTutorial = 0x01f2,
    idsTutorialHaveAlreadyCopiedAppropriateShipDesign = 0x01f3,
    idsTutorialHaveTriedCopyWrongShipDesign = 0x01f4,
    idsTutorialHaventPlacedCorrectNumberComponentsSlot = 0x01f5,
    idsTutorialHavePlacedWrongComponentSlotDrag = 0x01f6,
    idsTutorialMustFinishTutorialTasksBeforeExiting = 0x01f7,
    idsTutorialHaventYetFinishedCreatingNewDesign = 0x01f8,
    idsTutorialHaventPickedCorrectImageDesignPress = 0x01f9,
    idsTutorialVerifyHaveRadiatingHydroRamScoop = 0x01fa,
    idsTutorialDontHaveRightPartsDesignVerify = 0x01fb,
    idsTutorialDontHaveRightPartsDesignVerify2 = 0x01fc,
    idsTutorialDontHaveRightPartsDesignVerify3 = 0x01fd,
    idsTutorialDontHaveCorrectHullSelectedHull = 0x01fe,
    idsTutorialDontHaveCorrectShipSelectedShip = 0x01ff,
    idsTutorialNameDesignEditboxMustGaterChange = 0x0200,
    idsTutorialNameDesignEditboxMustMineLayer = 0x0201,
    idsTutorialHaventSelectedRightFleetsMergeReread = 0x0202,
    idsTutorialHaventAddedRightPartDesignVerify = 0x0203,
    idsTutorialHaventAskedMergeAnyFleetsYear = 0x0204,
    idsMineLayer = 0x0205,
    idsStinger = 0x0206,
    idsGater = 0x0207,
    idsTutorialHaventGivenZipOrderRightName = 0x0208,
    idsDropcol = 0x0209,
    idsTutorialFinishedCanContinuePlayGameStart = 0x020a,
    idsRandom = 0x020b,
    idsUnknownPlayer = 0x020c,
    idsExpansionPlayer = 0x020d,
    idsSorryCantFindPlanetFleetName = 0x020e,
    idsHumanControlled = 0x020f,
    idsAiControlled = 0x0210,
    idsHumanCurrentlyInactive = 0x0211,
    idsPopulation = 0x0212,
    idsWillGrowLd00Ld00Year = 0x0213,
    idsWillGrowYear = 0x0214,
    idsUnableCreateNewTurnFile = 0x0215,
    idsUnableUpdateTurnFile = 0x0216,
    idsNoteDYearsDataRead = 0x0217,
    idsCompletion = 0x0218,
    idsWaypointTaskS = 0x0219,
    idsWaypointS = 0x021a,
    idsWarpSpeedD = 0x021b,
    idsWarpSpeedStopped = 0x021c,
    idsFleetMassLdkt = 0x021d,
    idsNone = 0x021e,
    idsFuel2 = 0x021f,
    idsShipCountLd = 0x0220,
    idsCLd00 = 0x0221,
    idsPop = 0x0222,
    idsPopulation2 = 0x0223,
    idsVal = 0x0224,
    idsValue = 0x0225,
    idsUninhabited = 0x0226,
    idsOld = 0x0227,
    idsReportDYear = 0x0228,
    idsN999mr = 0x0229,
    idsPopulation1000000 = 0x022a,
    idsReportCurrent = 0x022b,
    idsWarningIgnoringUnexpectedDataAfterEof = 0x022c,
    idsVersionD02dC = 0x022d,
    idsYearDCMessagesDD = 0x022e,
    idsYearDCMessagesNone = 0x022f,
    idsDefault = 0x0230,
    idsCustomizeZipOrders = 0x0231,
    idsCustomizeProductionTemplates = 0x0232,
    idsResourceInfo = 0x0233,
    idsClear2 = 0x0234,
    idsRemoveTransportOrders = 0x0235,
    idsWaitload = 0x0236,
    idsWaitFullLoadMinerals = 0x0237,
    idsQuikload = 0x0238,
    idsLoadMineralsAvailable = 0x0239,
    idsQuikdrop = 0x023a,
    idsUnloadEverythingFleetCarrying = 0x023b,
    idsButtonClickOrderChoice = 0x023c,
    idsZipordClickDiamondRightMouse = 0x023d,
    idsTransportOrdersOne3CommonSetsSelect = 0x023e,
    idsZipordProvidesAbilityQuicklySetFleets = 0x023f,
    idsColonists3 = 0x0240,
    idsSupport = 0x0241,
    idsWould = 0x0242,
    idsIfColonize = 0x0243,
    idsPopulation3 = 0x0244,
    idsEnemyPopulation = 0x0245,
    idsApproximately = 0x0246,
    idsUnknown = 0x0247,
    idsUninhabited2 = 0x0248,
    idsWillKillOffApproximately = 0x0249,
    idsColonistsEachTurn = 0x024a,
    idsColonistsSettleEveryTurn = 0x024b,
    idsWillSupportPopulation = 0x024c,
    idsWithinRange = 0x024d,
    idsOf = 0x024e,
    idsIs = 0x024f,
    idsTo = 0x0250,
    idsOn2 = 0x0251,
    idsModify = 0x0252,
    idsCurrently = 0x0253,
    idsUnknown2 = 0x0254,
    idsColonistsImmune = 0x0255,
    idsEffects = 0x0256,
    idsColonistsPreferPlanetsWhere = 0x0257,
    idsBetween = 0x0258,
    idsAnd = 0x0259,
    idsCurrentlyPossessTechnology = 0x025a,
    idsShipName = 0x025b,
    idsPlanet = 0x025c,
    idsN9999 = 0x025d,
    idsMineralConcentration0000000kt = 0x025e,
    idsY = 0x025f,
    idsX = 0x0260,
    idsId = 0x0261,
    idsPlayerD = 0x0262,
    idsNone2 = 0x0263,
    idsMineralConcentration = 0x0264,
    idsSurface = 0x0265,
    idsMiningRate = 0x0266,
    idsZipord = 0x0267,
    idsTutorialHaveGivenIncorrectTransferOrderPlease = 0x0268,
    idsTutorialHaveGivenIncorrectQuantityTransferOrder = 0x0269,
    idsSureWishGenerateOptionDoesGuaranteePlayers = 0x026a,
    idsCurrentlyHaveDSSDProduction = 0x026b,
    idsCurrentlyHaveDSSProductionQueues = 0x026c,
    idsCurrentlyHaveDSSIfDelete = 0x026d,
    idsCurrentlyHaveDSSDProduction2 = 0x026e,
    idsCurrentlyHaveDSSProductionQueues2 = 0x026f,
    idsCurrentlyHaveDSSIfDelete2 = 0x0270,
    idsCurrentlyHaveDSSProductionQueues3 = 0x0271,
    idsTaskS = 0x0272,
    idsWpS = 0x0273,
    idsWarpD = 0x0274,
    idsWarpStopped = 0x0275,
    idsMassLdkt = 0x0276,
    idsDesignProgramming = 0x0277,
    ids0632Blank = 0x0278,
    idsJeffJohnson = 0x0279,
    idsJeffMcbride = 0x027a,
    ids0635Blank = 0x027b,
    ids0636Blank = 0x027c,
    idsAdditionalAi = 0x027d,
    ids0638Blank = 0x027e,
    idsJeffreyKrauss = 0x027f,
    ids0640Blank = 0x0280,
    ids0641Blank = 0x0281,
    idsArtwork = 0x0282,
    ids0643Blank = 0x0283,
    idsMichaelCMiller = 0x0284,
    idsEmblazonMultimediaInc = 0x0285,
    idsEricChang = 0x0286,
    idsMichaelReichmann = 0x0287,
    ids0648Blank = 0x0288,
    ids0649Blank = 0x0289,
    idsHelpFile = 0x028a,
    ids0651Blank = 0x028b,
    idsKurtKremer = 0x028c,
    idsBrettKremer = 0x028d,
    ids0654Blank = 0x028e,
    ids0655Blank = 0x028f,
    idsTechnicalAdvice = 0x0290,
    ids0657Blank = 0x0291,
    idsDavidPugh = 0x0292,
    ids0659Blank = 0x0293,
    ids0660Blank = 0x0294,
    idsMusic2 = 0x0295,
    ids0662Blank = 0x0296,
    idsEmilHerceg = 0x0297,
    ids0664Blank = 0x0298,
    ids0665Blank = 0x0299,
    idsSoundEffects = 0x029a,
    ids0667Blank = 0x029b,
    idsMahendraSampath = 0x029c,
    ids0669Blank = 0x029d,
    ids0670Blank = 0x029e,
    idsPlayTesters = 0x029f,
    ids0672Blank = 0x02a0,
    idsSamBelcher = 0x02a1,
    idsBillBolosky = 0x02a2,
    idsDaveBuchthal = 0x02a3,
    idsKentCedola = 0x02a4,
    idsPeterCelella = 0x02a5,
    idsDanielChenault = 0x02a6,
    idsPaulEnfield = 0x02a7,
    idsMichaelGrier = 0x02a8,
    idsPeterHenriksen = 0x02a9,
    idsWilliamHerlan = 0x02aa,
    idsPeteHorodan = 0x02ab,
    idsBrentJensen = 0x02ac,
    idsMarkKenworthy = 0x02ad,
    idsStuKlingman = 0x02ae,
    idsSteveKruy = 0x02af,
    idsRobertLamb = 0x02b0,
    idsJimLane = 0x02b1,
    idsHiltonLange = 0x02b2,
    idsJonLevee = 0x02b3,
    idsChrisMcbride = 0x02b4,
    idsJeffMccashland = 0x02b5,
    idsBethMoursund = 0x02b6,
    idsChrisNoon = 0x02b7,
    idsTonyPacheco = 0x02b8,
    idsChrisPeltz = 0x02b9,
    idsTonyReynolds = 0x02ba,
    idsJenniferSchlickbernd = 0x02bb,
    idsErikSnapper = 0x02bc,
    idsAndrewSterian = 0x02bd,
    idsJeffStone = 0x02be,
    idsRichardSun = 0x02bf,
    idsDavidThiel = 0x02c0,
    idsBradThompson = 0x02c1,
    idsThomasVoigt = 0x02c2,
    idsRossYoungs = 0x02c3,
    ids0708Blank = 0x02c4,
    idsNewTurnAvailableWouldLikeLoad = 0x02c5,
    idsNewTurnAvailable = 0x02c6,
    idsSorryTurnHasAlreadyGeneratedAnyChanges = 0x02c7,
    idsAutoGenerateDisabledBecauseHumanPlayersDead = 0x02c8,
    idsNoteStarsPrefersScreenResolutionLeast800x600 = 0x02c9,
    idsFileCreatedNewerVersionStarsMustUpgrade = 0x02ca,
    idsDead = 0x02cb,
    idsTurned = 0x02cc,
    idsStill = 0x02cd,
    idsPartiallyDone = 0x02ce,
    idsCorrupted = 0x02cf,
    idsRightYear = 0x02d0,
    idsRightGame = 0x02d1,
    idsLocationDD = 0x02d2,
    idsFieldTypeS = 0x02d3,
    idsFieldRadiusDLYLdMines = 0x02d4,
    idsDecayRateLdYear = 0x02d5,
    idsMinesLaidPerYear = 0x02d6,
    idsMaximumSafeSpeed = 0x02d7,
    idsChanceLYHit = 0x02d8,
    idsDmgDoneEachShip = 0x02d9,
    idsMinDamageDoneFleet = 0x02da,
    idsNumbersParenthesisFleetsContainingShipRamScoop = 0x02db,
    idsRenameFleet = 0x02dc,
    idsStarbaseHullRequiresLesserRacialTraitImproved = 0x02dd,
    idsStarbaseHullHasSpaceDockCanBuild = 0x02de,
    idsStarbaseHullDoesHaveSpaceDockCan = 0x02df,
    idsAllowsFleetsWithoutCargoJumpAnyOther = 0x02e0,
    idsAllowsPlanetsFlingMineralPacketsOtherPlanets = 0x02e1,
    idsDone = 0x02e2,
    idsCancel = 0x02e3,
    idsDesign = 0x02e4,
    idsView = 0x02e5,
    idsDeleteDesign = 0x02e6,
    idsEditDesign = 0x02e7,
    idsWorkDone = 0x02e8,
    idsCargo3 = 0x02e9,
    idsFuel3 = 0x02ea,
    idsDock = 0x02eb,
    idsMax = 0x02ec,
    idsUnlimited = 0x02ed,
    idsDD = 0x02ee,
    idsLdLd = 0x02ef,
    idsD = 0x02f0,
    idsD2 = 0x02f1,
    idsN16 = 0x02f2,
    idsNeedsD = 0x02f3,
    idsD3 = 0x02f4,
    idsDSeconds = 0x02f5,
    idsD02d = 0x02f6,
    idsD02d02d = 0x02f7,
    idsDDaysD02d02d = 0x02f8,
    idsRequiresExactly = 0x02f9,
    idsCanHold = 0x02fa,
    idsOne = 0x02fb,
    idsOr = 0x02fc,
    idsSS = 0x02fd,
    idsKt = 0x02fe,
    idsN00 = 0x02ff,
    idsCostS = 0x0300,
    idsHull = 0x0301,
    idsCostOneSS = 0x0302,
    idsMaxFuel = 0x0303,
    idsArmor = 0x0304,
    idsShields = 0x0305,
    idsRating = 0x0306,
    idsDamage = 0x0307,
    idsPredefinedRace = 0x0308,
    idsCustomRace = 0x0309,
    idsComputerPlayer = 0x030a,
    idsPlayer = 0x030b,
    idsPlayer2 = 0x030c,
    idsEditRace = 0x030d,
    idsNew = 0x030e,
    idsOpen = 0x030f,
    idsS = 0x0310,
    idsSS2 = 0x0311,
    idsSSComputerPlayer = 0x0312,
    idsSXD = 0x0313,
    idsSHD = 0x0314,
    idsGameXy = 0x0315,
    idsStarsGameFilesXy = 0x0316,
    idsStarsGameFilesRFiles = 0x0317,
    idsStarsGameFilesMHstRStars = 0x0318,
    idsPlayer16 = 0x0319,
    idsNewTurnAvailable2 = 0x031a,
    idsHostModeDPlayer = 0x031b,
    idsOut = 0x031c,
    idsC04d04d04d04d = 0x031d,
    idsCCD = 0x031e,
    idsRepeatOrders = 0x031f,
    idsDeceased = 0x0320,
    idsMineralsHand = 0x0321,
    idsMines4 = 0x0322,
    idsFactories4 = 0x0323,
    idsResourcesYear = 0x0324,
    idsPopulation4 = 0x0325,
    idsScannerType = 0x0326,
    idsScannerRange = 0x0327,
    idsDDLY = 0x0328,
    idsDLightYears = 0x0329,
    idsDLY = 0x032a,
    idsDefenses4 = 0x032b,
    idsDefenseType = 0x032c,
    idsDefCoverage = 0x032d,
    idsProduction = 0x032e,
    idsDYears = 0x032f,
    idsDDYears = 0x0330,
    idsDYear = 0x0331,
    idsNever = 0x0332,
    idsSkipped = 0x0333,
    idsNeeded = 0x0334,
    idsDanger = 0x0335,
    idsUnload2 = 0x0336,
    idsUncertain = 0x0337,
    idsFleetsOrbit = 0x0338,
    idsOtherFleetsHere = 0x0339,
    idsPlanetView = 0x033a,
    idsPlanet2 = 0x033b,
    idsQueueEmpty = 0x033c,
    idsTopQueue = 0x033d,
    idsProductionQueueS = 0x033e,
    idsRequiredMinerals = 0x033f,
    idsDDoneCompletion = 0x0340,
    idsSpace = 0x0341,
    idsCost = 0x0342,
    idsStarbase2 = 0x0343,
    idsDockCapacity = 0x0344,
    idsArmor2 = 0x0345,
    idsShields2 = 0x0346,
    idsDamage2 = 0x0347,
    idsLddp = 0x0348,
    idsMassDriver = 0x0349,
    idsMaxed = 0x034a,
    idsNever2 = 0x034b,
    idsLdYearC = 0x034c,
    idsNoneAvailable = 0x034d,
    idsTechReq = 0x034e,
    idsNone3 = 0x034f,
    idsCostLdk = 0x0350,
    idsCostLd = 0x0351,
    idsUnavail = 0x0352,
    idsAvailable = 0x0353,
    idsMassDkt = 0x0354,
    idsWarp = 0x0355,
    idsShieldStrength = 0x0356,
    idsArmorStrength2 = 0x0357,
    idsPower = 0x0358,
    idsRange = 0x0359,
    idsAccuracy = 0x035a,
    idsCurrentlyHaveFleetsUsingBattlePlanIf = 0x035b,
    idsNoteNewPasswordWillTakeEffectUntil = 0x035c,
    idsNotePasswordEffectiveImmediately = 0x035d,
    idsChangeHostPassword = 0x035e,
    idsEnterPassword = 0x035f,
    idsLdLdLightYears = 0x0360,
    idsLdLdLY = 0x0361,
    idsDeepSpace = 0x0362,
    idsSpaceDD = 0x0363,
    idsSSMineField = 0x0364,
    idsOrbitingS = 0x0365,
    idsSD = 0x0366,
    idsShipTransfer = 0x0367,
    idsStopped = 0x0368,
    idsWarpLd = 0x0369,
    idsWarpD2 = 0x036a,
    idsLdLdkt = 0x036b,
    idsLdLdmg = 0x036c,
    idsPercentCloaked = 0x036d,
    idsInfinite = 0x036e,
    idsEstRange = 0x036f,
    idsLdLY = 0x0370,
    idsBestWarp = 0x0371,
    idsFleetComposition = 0x0372,
    idsN9999LY = 0x0373,
    idsFuelCargo = 0x0374,
    idsJettison2 = 0x0375,
    idsXFer = 0x0376,
    idsDeepSpace2 = 0x0377,
    idsMiningRatePerYear = 0x0378,
    idsN99999Kt = 0x0379,
    idsWaypointTask = 0x037a,
    idsEstFuelUsage = 0x037b,
    idsLdkt = 0x037c,
    idsLdmg = 0x037d,
    idsWarpFactor = 0x037e,
    idsTravelTime = 0x037f,
    idsDistance = 0x0380,
    idsFleetWaypoints = 0x0381,
    idsComing = 0x0382,
    idsWayPt = 0x0383,
    idsBattlePlans = 0x0384,
    idsDYearC = 0x0385,
    idsIindefinitely = 0x0386,
    idsRelations = 0x0387,
    idsRelation = 0x0388,
    idsStatus = 0x0389,
    idsSetDest = 0x038a,
    idsRoute2 = 0x038b,
    idsTravelingWarpD = 0x038c,
    idsS2 = 0x038d,
    idsDestination = 0x038e,
    idsN9999992 = 0x038f,
    idsDD2 = 0x0390,
    idsDD3 = 0x0391,
    idsDDEngine = 0x0392,
    idsDD4 = 0x0393,
    idsUseStargate = 0x0394,
    idsSmineralPacket = 0x0395,
    idsSalvage = 0x0396,
    idsDeviceRequiresPrimaryRacialTraitInnerStrength = 0x0397,
    idsDeviceRequiresPrimaryRacialTraitSpaceDemolition = 0x0398,
    idsDeviceRequiresPrimaryRacialTraitInterstellarTrav = 0x0399,
    idsDeviceRequiresPrimaryRacialTraitHyperExpansion = 0x039a,
    idsSlowsShipsCombat1SquareMovement = 0x039b,
    idsReducesEffectivenessOtherPlayersCloaks5 = 0x039c,
    idsActs200mgAntiMatterFuelTankGenerates = 0x039d,
    idsIncreasesDamageDoneBeamWeaponsShipD = 0x039e,
    idsJammingDeviceRequiresPrimaryRacialTraitInner = 0x039f,
    idsHasDChanceDeflectingIncomingTorpedoesDeflected = 0x03a0,
    idsModuleIncreasesAccuracyTorpedoesDIncreasesInitia = 0x03a1,
    idsOriginPartUnknown = 0x03a2,
    idsOriginHullUnknown = 0x03a3,
    idsOriginProcessUnknown = 0x03a4,
    idsOriginEngineUnknownAdds14Square = 0x03a5,
    idsPartAlsoActs100dpShield20Cloak = 0x03a6,
    idsPartAlsoActs10CloakIncreasesTorpedo = 0x03a7,
    idsWeaponCanAlsoBombPlanets2Colonists = 0x03a8,
    idsProcessGivesPlanetNewBirthTracesCivilization = 0x03a9,
    idsOwns = 0x03aa,
    idsPlanets2 = 0x03ab,
    ids0940Blank = 0x03ac,
    idsAttainsTech = 0x03ad,
    idsIn = 0x03ae,
    idsFields = 0x03af,
    idsExceedsScore = 0x03b0,
    idsMsg0945 = 0x03b1,
    ids0946Blank = 0x03b2,
    idsExceedsSecondPlaceScore = 0x03b3,
    idsMsg0948 = 0x03b4,
    ids0949Blank = 0x03b5,
    idsHasProductionCapacity = 0x03b6,
    idsThousand = 0x03b7,
    ids0952Blank = 0x03b8,
    idsOwns2 = 0x03b9,
    idsCapitalShips2 = 0x03ba,
    ids0955Blank = 0x03bb,
    idsHasHighestScoreAfter = 0x03bc,
    idsYears = 0x03bd,
    ids0958Blank = 0x03be,
    idsWinnerMustMeet = 0x03bf,
    idsAboveSelectedCriteria = 0x03c0,
    ids0961Blank = 0x03c1,
    idsLeast = 0x03c2,
    idsYearsMustPassBeforeWinnerDeclared = 0x03c3,
    ids0964Blank = 0x03c4,
    idsPlanets3 = 0x03c5,
    idsHistory = 0x03c6,
    idsRockSolid = 0x03c7,
    idsStable = 0x03c8,
    idsMostlyStable = 0x03c9,
    idsAverage = 0x03ca,
    idsSlightlyVolatile = 0x03cb,
    idsVolatile = 0x03cc,
    idsExtremelyVolatile = 0x03cd,
    idsLocation = 0x03ce,
    idsDestination2 = 0x03cf,
    idsStability = 0x03d0,
    idsDD5 = 0x03d1,
    idsTraderRequestsInterestedPartiesSendFleetLeast = 0x03d2,
    idsTraderTravelingWarpD = 0x03d3,
    idsEasterBunny = 0x03d4,
    idsKilljoy = 0x03d5,
    idsMommasHelper = 0x03d6,
    idsTurtle = 0x03d7,
    idsPoodle = 0x03d8,
    idsMite = 0x03d9,
    idsGnat = 0x03da,
    idsRobin = 0x03db,
    idsOstrich = 0x03dc,
    idsGoose = 0x03dd,
    idsLyingBastard = 0x03de,
    idsPitBull = 0x03df,
    idsToothlessTiger = 0x03e0,
    idsRhodeIslandRed = 0x03e1,
    idsRamRod = 0x03e2,
    idsSpittingCobra = 0x03e3,
    idsVenomousDreadnought = 0x03e4,
    idsCrownJewel = 0x03e5,
    idsSilverSerpent = 0x03e6,
    idsXenocide = 0x03e7,
    idsTyphoon = 0x03e8,
    idsQuark = 0x03e9,
    idsWhip = 0x03ea,
    idsLash = 0x03eb,
    idsTerror = 0x03ec,
    idsDogWar = 0x03ed,
    idsPidgeon = 0x03ee,
    idsRagingRukh = 0x03ef,
    idsManifestDestiny = 0x03f0,
    idsFlyingCow = 0x03f1,
    idsBitterHarvest = 0x03f2,
    idsPeacock = 0x03f3,
    idsSaguaro = 0x03f4,
    idsBadlandsExpress = 0x03f5,
    idsBrightSpot = 0x03f6,
    idsStrangeLove = 0x03f7,
    idsDrDeath = 0x03f8,
    idsScorch = 0x03f9,
    idsGroundHog = 0x03fa,
    idsNakedMoleRat = 0x03fb,
    idsTerrier = 0x03fc,
    idsPick = 0x03fd,
    idsGouge = 0x03fe,
    idsGorge = 0x03ff,
    idsGut = 0x0400,
    idsAirdale = 0x0401,
    idsEgg = 0x0402,
    idsBusyBee = 0x0403,
    idsSeeder = 0x0404,
    idsSpore = 0x0405,
    idsPhoenix = 0x0406,
    idsPlymouth = 0x0407,
    idsDuty = 0x0408,
    idsVassal = 0x0409,
    idsGlovebox = 0x040a,
    idsPerfectLogic = 0x040b,
    idsBoxcar = 0x040c,
    idsBoot = 0x040d,
    idsPeet = 0x040e,
    idsC74 = 0x040f,
    idsLor = 0x0410,
    idsBlackHold = 0x0411,
    idsPricklyPear = 0x0412,
    idsBristlyLlama = 0x0413,
    idsSilentMule = 0x0414,
    idsSaguaro2 = 0x0415,
    idsCrunchyCritter = 0x0416,
    idsLongJohnSilver = 0x0417,
    idsBlackbeard = 0x0418,
    idsThistle = 0x0419,
    idsZombie = 0x041a,
    idsTyphoid = 0x041b,
    idsZeppo = 0x041c,
    idsLuckyEddie = 0x041d,
    idsWidget = 0x041e,
    idsPoly = 0x041f,
    idsMog = 0x0420,
    idsRanger = 0x0421,
    idsScrapper = 0x0422,
    idsBogey = 0x0423,
    idsHorseFly = 0x0424,
    idsHornet = 0x0425,
    idsDragonFly = 0x0426,
    idsWasp = 0x0427,
    idsIntruder = 0x0428,
    idsInterceptor = 0x0429,
    idsQuest = 0x042a,
    idsInfiniteVision = 0x042b,
    idsBrassKnuckle = 0x042c,
    idsTalon = 0x042d,
    idsNaagra = 0x042e,
    idsCattleProd = 0x042f,
    idsAsunder = 0x0430,
    idsBlade = 0x0431,
    idsGuardianAngel = 0x0432,
    idsSkyFort = 0x0433,
    idsSilverTower = 0x0434,
    idsPentagon = 0x0435,
    idsMonolith = 0x0436,
    idsRockGibraltar = 0x0437,
    idsPotato = 0x0438,
    idsCube = 0x0439,
    idsDeathDemand = 0x043a,
    idsHipSquare = 0x043b,
    idsSphereDoom = 0x043c,
    idsGatewayHell = 0x043d,
    idsEvilSpawn = 0x043e,
    idsAll = 0x043f,
    idsArmor3 = 0x0440,
    idsBeamWeapons = 0x0441,
    idsBombs = 0x0442,
    idsElectrical = 0x0443,
    idsEngines = 0x0444,
    idsMechanical = 0x0445,
    idsMineLayers = 0x0446,
    idsMiningRobots = 0x0447,
    idsOrbital = 0x0448,
    idsPlanetary = 0x0449,
    idsScanners = 0x044a,
    idsShields3 = 0x044b,
    idsShipHulls = 0x044c,
    idsStarbaseHulls = 0x044d,
    idsTerraforming = 0x044e,
    idsTorpedoes = 0x044f,
    idsWeapons2 = 0x0450,
    idsDevices = 0x0451,
    idsSafeHullMass = 0x0452,
    idsSafeRange = 0x0453,
    idsWarningShipsDktMightSuccessfullyGatedD = 0x0454,
    idsWarningShipsCanSuccessfullyGatedDL = 0x0455,
    idsWarningShipsDktCanSuccessfullyGatedExceeding = 0x0456,
    idsWarningReceivingPlanetMustHaveMassDriver = 0x0457,
    idsWarningReceivingPlanetMustHaveMassDriver2 = 0x0458,
    idsPlanetName = 0x0459,
    idsStarbase3 = 0x045a,
    idsPopulation5 = 0x045b,
    idsCap = 0x045c,
    idsValue2 = 0x045d,
    idsProduction2 = 0x045e,
    idsMine2 = 0x045f,
    idsFact = 0x0460,
    idsDefense = 0x0461,
    idsMinerals = 0x0462,
    idsMiningRate2 = 0x0463,
    idsMinConc = 0x0464,
    idsResources2 = 0x0465,
    idsDriverDest = 0x0466,
    idsRoutingDest = 0x0467,
    idsN100100 = 0x0468,
    idsN100 = 0x0469,
    idsN10001000 = 0x046a,
    idsN1000 = 0x046b,
    idsD4 = 0x046c,
    idsSort = 0x046d,
    idsReverseSort = 0x046e,
    idsHide = 0x046f,
    idsColumn = 0x0470,
    idsShow = 0x0471,
    idsFleetName = 0x0472,
    idsId2 = 0x0473,
    idsLocation2 = 0x0474,
    idsDestination3 = 0x0475,
    idsEta = 0x0476,
    idsTask = 0x0477,
    idsFuel4 = 0x0478,
    idsCargo4 = 0x0479,
    idsComposition = 0x047a,
    idsCloak = 0x047b,
    idsBattlePlan = 0x047c,
    idsMass = 0x047d,
    idsFleetName2 = 0x047e,
    idsId3 = 0x047f,
    idsLocation3 = 0x0480,
    idsWarp2 = 0x0481,
    idsMass2 = 0x0482,
    idsComposition2 = 0x0483,
    idsShips = 0x0484,
    idsUnarmed = 0x0485,
    idsScout2 = 0x0486,
    idsWarship2 = 0x0487,
    idsBomber2 = 0x0488,
    idsUtility2 = 0x0489,
    idsLocation4 = 0x048a,
    idsSb = 0x048b,
    idsSides = 0x048c,
    idsUnits = 0x048d,
    idsOurs = 0x048e,
    idsTheirs = 0x048f,
    idsUnarmed2 = 0x0490,
    idsScout3 = 0x0491,
    idsWarship3 = 0x0492,
    idsBomber3 = 0x0493,
    idsUtility3 = 0x0494,
    idsOurDead = 0x0495,
    idsDead2 = 0x0496,
    idsOursLeft = 0x0497,
    idsTheirsLeft = 0x0498,
    idsPlanetSummaryReportDPlanetC = 0x0499,
    idsFleetSummaryReportDFleetC = 0x049a,
    idsOthersFleetsSummaryReportDFleetC = 0x049b,
    idsBattleSummaryReportDBattleC = 0x049c,
    idsDelayed = 0x049d,
    idsDy = 0x049e,
    idsGeneratingDataYearD = 0x049f,
    idsLdD = 0x04a0,
    idsDestroyingDShip = 0x04a1,
    idsLdDamageShieldsS = 0x04a2,
    idsLdDamageArmorC = 0x04a3,
    idsSelectionDD = 0x04a4,
    idsShieldsLd = 0x04a5,
    idsShieldsNone = 0x04a6,
    idsDamageLdD = 0x04a7,
    idsDamageD = 0x04a8,
    idsDamageNone = 0x04a9,
    idsArmorLd = 0x04aa,
    idsMovementS = 0x04ab,
    idsInitiativeMoves = 0x04ac,
    idsInitMove = 0x04ad,
    idsCloakJam = 0x04ae,
    idsScannerRange2 = 0x04af,
    idsScanner = 0x04b0,
    idsDD6 = 0x04b1,
    idsDDD = 0x04b2,
    idsDS = 0x04b3,
    idsDDDoing = 0x04b4,
    idsWithinDLY = 0x04b5,
    idsAnyEnemy = 0x04b6,
    idsHullWillManufacture200UnitsFuelEach = 0x04b7,
    idsHullWillDoubleEfficiencyMineLayingPods = 0x04b8,
    idsDead3 = 0x04b9,
    idsPlayerScores = 0x04ba,
    idsVictoryConditions = 0x04bb,
    idsProgressTimeline = 0x04bc,
    idsDetonateMineFieldYear = 0x04bd,
    idsUnusedD = 0x04be,
    idsCustomD = 0x04bf,
    idsCustomize = 0x04c0,
    idsCustomOrders = 0x04c1,
    idsRenameZipOrder = 0x04c2,
    idsRenameProductionTemplate = 0x04c3,
    idsAutoBuildOrders = 0x04c4,
    idsSD2 = 0x04c5,
    idsContributeResearch = 0x04c6,
    idsDontContributeResearch = 0x04c7,
    idsEmptyCustomSlot = 0x04c8,
    idsC = 0x04c9,
    idsPleaseEnterUniqueEightCharacterSerialNumber = 0x04ca,
    idsMachineConfigurationAppearsHaveChangedPleaseRe = 0x04cb,
    idsPleaseEnterOwnUniqueSerialNumberPrevent = 0x04cc,
    idsSerialNumberHaveEnteredValid = 0x04cd,
    idsStarsTutorPageD80 = 0x04ce,
    idsTutorialHaveGivenFleetWrongNamePlease = 0x04cf,
    idsTorpedoesDeflected = 0x04d0,
    idsJammingD = 0x04d1,
    idsWarningDestinationWaypointFleetMergeWillSucessfu = 0x04d2,
    idsSorryFileCreatedOlderVersionStarsIncompatible = 0x04d3,
    idsArmorRequiresPrimaryRacialTraitInnerStrength = 0x04d4,
    idsCosts75ExtraResearchFieldsStartTech = 0x04d5,
    idsUniverseDefinitionHasSuccessfullyWrittenSMap = 0x04d6,
    idsUnableWriteUniverseDefinitionSMapOperation = 0x04d7,
    idsKnownPlanetInformationHasSuccessfullyWrittenS = 0x04d8,
    idsUnableWritePlanetInformationSOperationTerminated = 0x04d9,
    idsKnownFleetInformationHasSuccessfullyWrittenS = 0x04da,
    idsUnableWriteFleetInformationSOperationTerminated = 0x04db,
    idsPlanetNameOwnerStarbaseTypeReportAge = 0x04dc,
    idsIronMrBoraMrGermMrIron = 0x04dd,
    idsGravTempRadGravorigTemporigRadorigTerra = 0x04de,
    idsFleetNameXYPlanetDestinationBattle = 0x04df,
    idsShipCntIronBoraGermColFuel = 0x04e0,
    idsOwnerEtaWarpMassCloakScanPen = 0x04e1,
    idsMineField = 0x04e2,
    idsMineralPacket = 0x04e3,
    idsWormhole = 0x04e4,
    idsMysteryTrader = 0x04e5,
    idsSalvageField = 0x04e6,
    idsMysteryObject = 0x04e7,
    idsFleet = 0x04e8,
    idsRoute3 = 0x04e9,
    idsN = 0x04ea,
    idsRaceIncapableBuildingFactories = 0x04eb,
    idsRaceIncapableBuildingMinesHoweverColonistsHave = 0x04ec,
    idsOrganic = 0x04ed,
    idsRaceCannotBuildPlanetaryScannersStarbasesHave = 0x04ee,
    idsPlanetaryScannersDefensesAvailableAlternateReali = 0x04ef,
    idsMsg1264 = 0x04f0,
    idsDockCapacity2 = 0x04f1,
    idsPhaseDDRoundDD = 0x04f2,
    idsPlaybackSpeedD = 0x04f3,
    idsWeaponWillDamageShieldsHasEffectArmor = 0x04f4,
    idsWeaponHitsTargetsRangeEachTimeFired = 0x04f5,
    idsWeaponAlsoMakesExcellentMineSweeperCapable = 0x04f6,
    idsMaxPopulation = 0x04f7,
    idsMaxPop = 0x04f8,
    idsBattlePlan2 = 0x04f9,
    idsIntercept = 0x04fa,
    idsDesigns = 0x04fb,
    idsInvertFilter = 0x04fc,
    idsDesigns2 = 0x04fd,
    idsMineFields = 0x04fe,
    idsMineFields2 = 0x04ff,
    idsMineFields3 = 0x0500,
    idsMineFieldsFriends = 0x0501,
    idsMineFieldsNeutrals = 0x0502,
    idsMineFieldsEnemies = 0x0503,
    idsTacticS = 0x0504,
    idsTacticSDMoves = 0x0505,
    idsPrimayTargetS = 0x0506,
    idsSecondaryTargetS = 0x0507,
    idsAutomatic = 0x0508,
    idsInitiativeD = 0x0509,
    idsRaceHas = 0x050a,
    idsN9999999 = 0x050b,
    idsEvery = 0x050c,
    idsHours = 0x050d,
    ids1294Blank = 0x050e,
    ids1295Blank = 0x050f,
    idsMinutesAfter = 0x0510,
    idsPlayerSLeft = 0x0511,
    idsCantCopyShipDesignBecauseCantBuild = 0x0512,
    idsSmartBombsStrictlyAdditiveHaveMinimumKill = 0x0513,
    idsPartUnavailbleWarMonger = 0x0514,
    idsAdvantagePointsCurrentlyHoleDPointsCannot = 0x0515,
    idsCantHaveDWaypoints = 0x0516,
    idsSendMessagesDD = 0x0517,
    idsUpTo = 0x0518,
    idsTutorialContributeLeftoverCheckboxProductionQueu = 0x0519,
    idsMakeTutorialReappearCompleteTaskChooseTutorial = 0x051a,
    idsPlanetaryScanner = 0x051b,
    idsTorpedoesDeflected2 = 0x051c,
    idsDamage3 = 0x051d,
    idsHw = 0x051e,
    idsN30 = 0x051f,
    idsStarsUniverseMap = 0x0520,
    idsYearD = 0x0521,
    idsPlanet3 = 0x0522,
    idsOrbitalFort = 0x0523,
    idsStarbase4 = 0x0524,
    idsUnoccupiedPlanet = 0x0525,
    idsPlayer2sPlanet = 0x0526,
    idsMustSpecifyNumberBetween19 = 0x0527,
    idsUnablePrintGameMapPrinterMayOff = 0x0528,
    idsCapitalShipMissilesDoTwiceStatedDamage = 0x0529,
    idsDemoVersionStarsLimitedGames80Years = 0x052a,
    idsStarsSHostMode = 0x052b,
    idsWaitingNewTurn = 0x052c,
    idsTemperature = 0x052d,
    idsResearch = 0x052e,
    idsAdvantage = 0x052f,
    idsPointsLeft = 0x0530,
    idsStarsRaceFilesR = 0x0531,
    idsRandom2 = 0x0532,
    idsPredefinedRaces = 0x0533,
    idsN2 = 0x0534,
    idsTo2 = 0x0535,
    idsTo3 = 0x0536,
    idsArial2 = 0x0537,
    idsArialBold = 0x0538,
    idsArialItalic = 0x0539,
    idsArialBoldItalic = 0x053a,
    idsAttacksS = 0x053b,
    idsAnd2 = 0x053c,
    idsDmg = 0x053d,
    idsLevel = 0x053e,
    idsSTechLevelD = 0x053f,
    idsBleedingEdge = 0x0540,
    idsNum = 0x0541,
    idsStandard = 0x0542,
    idsSmart = 0x0543,
    idsDecreased = 0x0544,
    idsIncreased = 0x0545,
    idsOf2 = 0x0546,
    idsOrigin = 0x0547,
    idsFiltered = 0x0548,
    idsEverybody = 0x0549,
    idsSCC = 0x054a,
    idsSCC2 = 0x054b,
    idsPrev2 = 0x054c,
    idsGoto3 = 0x054d,
    idsNext2 = 0x054e,
    idsDelete = 0x054f,
    idsReply = 0x0550,
    idsLdktYr = 0x0551,
    idsSInfo = 0x0552,
    idsOld2 = 0x0553,
    idsSummary = 0x0554,
    idsDeepSpaceWaypoint = 0x0555,
    idsLy = 0x0556,
    idsLightYears = 0x0557,
    idsFrom = 0x0558,
    idsName = 0x0559,
    idsHave2 = 0x055a,
    idsAre = 0x055b,
    idsHas = 0x055c,
    idsIs2 = 0x055d,
    idsPlayerD2 = 0x055e,
    idsWeightedAverage = 0x055f,
    idsNone4 = 0x0560,
    idsFieldDD = 0x0561,
    idsSureWantForceGenerateDTurnsRow = 0x0562,
    idsGeneratingYearD = 0x0563,
    idsFailed = 0x0564,
    idsSucceeded = 0x0565,
    idsCantFindHostFile = 0x0566,
    idsHumanoid = 0x0567,
    idsRabbitoid = 0x0568,
    idsInsectoid = 0x0569,
    idsNucleotid = 0x056a,
    idsSilicanoid = 0x056b,
    idsAntetheral = 0x056c,
    idsRandom3 = 0x056d,
    idsBerserker = 0x056e,
    idsBulushi = 0x056f,
    idsGolem = 0x0570,
    idsNulon = 0x0571,
    idsTritizoid = 0x0572,
    idsValadiac = 0x0573,
    idsUbert = 0x0574,
    idsFelite = 0x0575,
    idsFerret = 0x0576,
    idsHouseCat = 0x0577,
    idsCrusher = 0x0578,
    idsPicardi = 0x0579,
    idsRushn = 0x057a,
    idsAmerican = 0x057b,
    idsHawk = 0x057c,
    idsEagle = 0x057d,
    idsMensoid = 0x057e,
    idsLoraxoid = 0x057f,
    idsHicardi = 0x0580,
    idsNairnian = 0x0581,
    idsCleaver = 0x0582,
    idsHooveron = 0x0583,
    idsNee = 0x0584,
    idsKurkonian = 0x0585,
} StringId;
typedef enum MessageId {
    idmColonistsDroppedMassacredGroundTroops = 0x0000,
    idmColonistsDroppedDestroyedPlanetaryDefensesRestMa = 0x0001,
    idmColonistsForcedTransportDiedBecauseDidColonize = 0x0002,
    idmGroundTroopsValiantlyDestroyedAttackingBarbarian = 0x0003,
    idmPlanetaryDefensesGroundTroopsDestroyedInvadingTr = 0x0004,
    idmMultitudeEnemiesHaveMountedProngAttackResulting = 0x0005,
    idmInvolvedWayAssaultNobodysTroopsSurvivedBrutal = 0x0006,
    idmHaveAttackedFirstRateStormTroopersThough = 0x0007,
    idmInvolvedWayRaceUninhabitedPlanetForcesCrush = 0x0008,
    idmColonistsDestroyedWayRaceUninhabitedPlanetContro = 0x0009,
    idmColonistsControl = 0x000a,
    idmColonistsHaveDeployedOrbitalConstructionModuleHa = 0x000b,
    idmTroopsCrushSColonistsControlPlanet = 0x000c,
    idmColonistsDroppedDestroyedSpiritedFighting = 0x000d,
    idmThereMassiveBloodBathInvolvingFleetsRaces = 0x000e,
    idmSlaughteredOppositionWithoutLosingSingleShip = 0x000f,
    idmDestroyedOppositionMinimalLosses = 0x0010,
    idmDefeatedOppositionSufferedHeavyLosses = 0x0011,
    idmFleetsDestroyedOpposition = 0x0012,
    idmFleetsDefeatedOppositionSufferedHeavyLosses = 0x0013,
    idmFleetObliteratedWayStruggleWhichSurvived = 0x0014,
    idmFleetsDestroyedWayBattleLeavingSoleSurvivor = 0x0015,
    idmPeopleWitnessedSpectacleFleetsOtherRacesOblitera = 0x0016,
    idmColonyObservedForcesDefeatingForcesOtherRaces = 0x0017,
    idmForcesDestroyedEachOther = 0x0018,
    idmGloriousStompedForces = 0x0019,
    idmMightyDefeatedForcesTookHeavyCasualties = 0x001a,
    idmFleetsTrouncedBarbarousForces = 0x001b,
    idmVigilantFleetsManagedDefeatSavageVerminWithout = 0x001c,
    idmBraveForcesObliteratedVastlyGreaterForcesCowardl = 0x001d,
    idmForcesDiedValiantlyTakingManyVerminThem = 0x001e,
    idmLostTerribleMassacrePerpetratedVillainous = 0x001f,
    idmCloseFightDidGreatDamageForcesBefore = 0x0020,
    idmPeopleWatchedAmazementForcesAnnihilatedEachOther = 0x0021,
    idmColonyObservedForcesDefeatingForces = 0x0022,
    idmColonistsHaveDiedOffLongerControlPlanet = 0x0023,
    idmColonistsOrbitingHaveDiedOffStarbaseHas = 0x0024,
    idmPopulationHasDecreased = 0x0025,
    idmPopulationHasDecreasedColonistsDueOvercrowding = 0x0026,
    idmHasRunFuel = 0x0027,
    idmSWaypointAppearsHaveDestroyedHasDisappeared = 0x0028,
    idmFleetTrackingAppearsHaveDuckedBehindOrders = 0x0029,
    idmFleetTrackingAppearsHaveOutrunRangeScanners = 0x002a,
    idmHasLoaded = 0x002b,
    idmHasBeamed = 0x002c,
    idmHasUnloaded = 0x002d,
    idmHasBeamed2 = 0x002e,
    idmStarbaseHasBuiltNew = 0x002f,
    idmStarbaseHasBuiltNewShips = 0x0030,
    idmStarbaseHasBuiltNewWhichRouted = 0x0031,
    idmStarbaseHasBuiltNewShipsWhichRouted = 0x0032,
    idmStarbaseHasBuiltNewWhichWillRouted = 0x0033,
    idmStarbaseHasBuiltNewShipsWhichWill = 0x0034,
    idmHaveBuiltFactory = 0x0035,
    idmHaveBuiltFactories = 0x0036,
    idmHaveBuiltMine = 0x0037,
    idmHaveBuiltMines = 0x0038,
    idmHaveBuiltDefenseOutpost = 0x0039,
    idmHaveBuiltDefenseOutposts = 0x003a,
    idmHaveUpgradedDefensesUseTechnology = 0x003b,
    idmThereIsntEnoughFuelAvailableAllowGet = 0x003c,
    idmWillNeverMakeWaypointFuelCapacityMg = 0x003d,
    idmHasCompletedOrdersProductionQueueEmpty = 0x003e,
    idmProductionQueueEmpty = 0x003f,
    idmColonistsHaveJumpedShipLongerControlPlanet = 0x0040,
    idmColonistsOrbitingHaveAbandonedStarbaseLongerCont = 0x0041,
    idmSuccessfullyTransferred = 0x0042,
    idmSuccessfullyTransferred2 = 0x0043,
    idmSuccessfullyReceived = 0x0044,
    idmSuccessfullyReceived2 = 0x0045,
    idmAttemptedTransferSuccessfullyReceived = 0x0046,
    idmAttemptedTransferColonistsSuccessfullyReceivedRe = 0x0047,
    idmReceivedHoweverSentRemainderLostSpace = 0x0048,
    idmReceivedHoweverColonistsSentRemainsOtherColonist = 0x0049,
    idmAttemptedTransferNoneSuccessfullyReceived = 0x004a,
    idmAttemptedTransferNoneColonistsSuccessfullyReceiv = 0x004b,
    idmAttemptedReceiveHoweverLostDeepSpace = 0x004c,
    idmAttemptedReceiveHoweverNoneColonistsSuccessfully = 0x004d,
    idmHasCompletedAssignedOrders = 0x004e,
    idmStarbaseFailedBuildNewShipTypeBecause = 0x004f,
    idmScientistsHaveCompletedResearchTechLevelWill = 0x0050,
    idmHasOrderColonizeCurrentlyOrbitPlanetOrder = 0x0051,
    idmHasOrdersColonizeAlreadyPopulatedColonizeOrder = 0x0052,
    idmHasOrdersColonizeHaveFailedBringAlong = 0x0053,
    idmHasOrdersColonizeNoneShipsHaveColonization = 0x0054,
    idmHasTriedBeamColonistsPlanetUninhabitedMust = 0x0055,
    idmCaptainHasAttemptedBeamColonistsOverruledBridge = 0x0056,
    idmColonistsAttemptingSetShopReducedProtoplasmicBlo = 0x0057,
    idmColonistsAssaultingHaveKilledForcesOrbitingStarb = 0x0058,
    idmHasDismantledKtMineralsWhichHaveDeposited = 0x0059,
    idmHasDismantledKtMineralsStarbaseOrbiting = 0x005a,
    idmHasDismantledScrapLeftDeepSpace = 0x005b,
    idmHasDismantledKtMineralsWhichHaveDeposited2 = 0x005c,
    idmHasDismantledKtMineralsStarbaseOrbitingUltimate = 0x005d,
    idmColonistsSettlingHaveFoundStrangeArtifactBoostin = 0x005e,
    idmRecentBreakthroughHasAlsoGivenBenefit = 0x005f,
    idmHasBombedKillingColonists = 0x0060,
    idmHasBombedDestroyingOneInstallation = 0x0061,
    idmHasBombedDestroyingDefensesFactoriesMines = 0x0062,
    idmHasBombedKillingColonistsDestroyingOneInstallati = 0x0063,
    idmHasBombedKillingColonistsDestroyingDefensesFacto = 0x0064,
    idmHasBombedKillingColonistsPlanetaryDefensesStoppe = 0x0065,
    idmHasBombedDestroyingOneInstallationPlanetaryDefen = 0x0066,
    idmHasBombedDestroyingFactoriesMinesPlanetaryDefens = 0x0067,
    idmHasBombedKillingColonistsDestroyingOneInstallati2 = 0x0068,
    idmHasBombedKillingColonistsDestroyingDefensesFacto2 = 0x0069,
    idmHasBombedKillingColonists2 = 0x006a,
    idmHasBombedDestroyingOneInstallations = 0x006b,
    idmHasBombedDestroyingDefensesFactoriesMines2 = 0x006c,
    idmHasBombedKillingColonistsDestroyingOneInstallati3 = 0x006d,
    idmHasBombedKillingColonistsDestroyingDefensesFacto3 = 0x006e,
    idmHasBombedKillingColonistsPlanetaryDefensesDestro = 0x006f,
    idmHasBombedDestroyingOneInstallationsPlanetaryDefe = 0x0070,
    idmHasBombedDestroyingDefensesFactoriesMinesPlaneta = 0x0071,
    idmHasBombedKillingColonistsDestroyingOneInstallati4 = 0x0072,
    idmHasBombedKillingColonistsDestroyingDefensesFacto4 = 0x0073,
    idmEngineRadiationHasKilledColonistsTraveling = 0x0074,
    idmHadOrdersMineFleetDoesntHaveAny = 0x0075,
    idmRemoteMiningRobotsHadOrdersMinePlanet = 0x0076,
    idmRemoteMiningRobotsHadOrdersMineDeep = 0x0077,
    idmRecentBreakthroughHasAlsoGivenHullType = 0x0078,
    idmHasLoaded2 = 0x0079,
    idmHasBeamed3 = 0x007a,
    idmTerraformingEffortsHave = 0x007b,
    idmHasBuiltNewPlanetaryScanner = 0x007c,
    idmHasLoadedMiningRobotsWorking = 0x007d,
    idmBattleTookPlacePressGotoButtonView = 0x007e,
    idmTipCanHideUnimportantMessagesClickingCheckmark = 0x007f,
    idmTipAddWaypointsSelectShipClickDesired = 0x0080,
    idmTipDesignOwnShipsPressF4Select = 0x0081,
    idmTipPopupHelpAvailableManyDisplayedStatistics = 0x0082,
    idmSmallCometHasCrashedBringingNewMinerals = 0x0083,
    idmMediumSizedCometHasCrashedBringingSignificant = 0x0084,
    idmLargeCometHasCrashedBringingWideVariety = 0x0085,
    idmHugeCometHasCrashedEmbeddingVastQuantities = 0x0086,
    idmSmallCometHasCrashedPlanetKilling25 = 0x0087,
    idmMediumSizedCometHasCrashedPlanetKilling = 0x0088,
    idmLargeCometHasCrashedPlanetKilling65 = 0x0089,
    idmHugeCometHasCrashedPlanetKilling85 = 0x008a,
    idmHasRunFuelFleetsSpeedHasDecreased = 0x008b,
    idmScientistsHaveTransmutedCommonMaterialsKtEach = 0x008c,
    idmBattleTookPlaceDestroyedScreamsColonistsEcho = 0x008d,
    idmBattleTookPlaceDestroyedColonistsHaveJoined = 0x008e,
    idmHasBombedKillingOffEnemyColonists = 0x008f,
    idmHasBombedKillingColonists3 = 0x0090,
    idmBattleTookPlaceDestroyedTakingDamage = 0x0091,
    idmBattleTookPlaceDestroyedWhichTookDamage = 0x0092,
    idmBattleTookPlaceDestroyedHoweverTookDamage = 0x0093,
    idmBattleTookPlaceDestroyedWhichDamagedFray = 0x0094,
    idmBattleTookPlaceNeitherNorDestroyedIncident = 0x0095,
    idmBattleTookPlaceDestroyedTakingDamage2 = 0x0096,
    idmBattleTookPlaceDestroyedWhichTookDamage2 = 0x0097,
    idmBattleTookPlaceDestroyedHoweverTookDamage2 = 0x0098,
    idmBattleTookPlaceDestroyedWhichDamagedFray2 = 0x0099,
    idmBattleTookPlaceNeitherNorCompletelyDestroyed = 0x009a,
    idmBattleTookPlaceAgainstForcesDestroyedEnemy = 0x009b,
    idmBattleTookPlaceAgainstForcesDestroyedEnemys = 0x009c,
    idmBattleTookPlaceAgainstForcesDestroyedEnemy2 = 0x009d,
    idmBattleTookPlaceAgainstForcesDestroyedEnemys2 = 0x009e,
    idmBattleTookPlaceAgainstNeitherForcesNor = 0x009f,
    idmBattleTookPlaceAgainstForcesDestroyedTaking = 0x00a0,
    idmBattleTookPlaceAgainstDestroyedEnemysForces = 0x00a1,
    idmBattleTookPlaceAgainstForcesDestroyedHowever = 0x00a2,
    idmBattleTookPlaceAgainstDestroyedEnemysForces2 = 0x00a3,
    idmBattleTookPlaceInvolvingRacesForcesDestroyed = 0x00a4,
    idmBattleTookPlaceInvolvingRacesLostForces = 0x00a5,
    idmBattleTookPlaceInvolvingRacesEntireArmada = 0x00a6,
    idmBattleTookPlaceInvolvingRacesEntireArmada2 = 0x00a7,
    idmBattleTookPlaceInvolvingRacesLostForces2 = 0x00a8,
    idmHomePlanetPeopleReadyLeaveNestExplore = 0x00a9,
    idmHaveFoundPlanetOccupiedSomeoneElseCurrently = 0x00aa,
    idmHaveFoundNewPlanetWhichUnfortunatelyHabitable = 0x00ab,
    idmHaveFoundNewHabitablePlanetColonistsWill = 0x00ac,
    idmHaveFoundNewPlanetDontKnowIf = 0x00ad,
    idmHaveFoundNewPlanetWhichHaveAbility = 0x00ae,
    idmHasBuiltManyMinesCurrentPopulationCan = 0x00af,
    idmHasBuiltManyMinesPlanetCanSupport = 0x00b0,
    idmHasBuiltManyFactoriesCurrentPopulationCan = 0x00b1,
    idmHasBuiltManyFactoriesPlanetCanSupport = 0x00b2,
    idmHasBuiltManyDefensesCurrentPopulationCan = 0x00b3,
    idmHasBuiltManyDefensesPlanetCanSupport = 0x00b4,
    idmForcesHaveDeclaredWinnerGameAdvisedAccept = 0x00b5,
    idmHaveDeclaredWinnerGameMayContinuePlay = 0x00b6,
    idmAlongHaveDeclaredWinnersGameMayContinue = 0x00b7,
    idmDeadPlanetsHaveOverrunSpaceshipsDefeated = 0x00b8,
    idmOrderBuildScannerCanceledAlreadyHaveScanner = 0x00b9,
    idmStarbaseBuiltNewShipSTypeLost = 0x00ba,
    idmTracesHaveEliminatedGalaxyMayRestPeace = 0x00bb,
    idmTracesEveryOtherRivalHaveEliminatedGalaxy = 0x00bc,
    idmHasAccomplishedRemoteTerraformingCurrentlyCapabl = 0x00bd,
    idmSomeoneHasSweptMinesMineField = 0x00be,
    idmHasAttemptedLayMinesOrderHasCanceled = 0x00bf,
    idmMysteryTraderHasDecidedMakeAnotherPass = 0x00c0,
    idmDueRigorsWarpAccelerationColonistsHaveDied = 0x00c1,
    idmHasSweptMinesMineField = 0x00c2,
    idmHasDispersedMines = 0x00c3,
    idmHasIncreasedMinefieldMines = 0x00c4,
    idmHasStoppedMineField = 0x00c5,
    idmHasStoppedMineFieldFleetHasTaken = 0x00c6,
    idmHasStoppedMineFieldFleetHasTaken2 = 0x00c7,
    idmHasAnnihilatedMineField = 0x00c8,
    idmHasStoppedMineField2 = 0x00c9,
    idmHasStoppedMineFieldMinesHaveInflicted = 0x00ca,
    idmHasStoppedMineFieldMinesHaveInflicted2 = 0x00cb,
    idmHasAnnihilatedMineField2 = 0x00cc,
    idmHasBuiltNew = 0x00cd,
    idmHasBuiltNewShipsKtTotalHull = 0x00ce,
    idmHasBuiltNewShipsAnySizeCan = 0x00cf,
    idmRecentBreakthroughHasAlsoGivenHullDesign = 0x00d0,
    idmMineralPacketFormedHasDisintegratedBecausePlanet = 0x00d1,
    idmMineralPacketFormedHasDisintegratedBecauseDidnt = 0x00d2,
    idmHasProducedMineralPacketWhichHasDestination = 0x00d3,
    idmHasProducedMineralPacketWhichHasCombined = 0x00d4,
    idmMassAcceleratorHasSuccessfullyCapturedPacketCont = 0x00d5,
    idmMassAcceleratorPartiallySuccessfullyCapturingKtM = 0x00d6,
    idmMassAcceleratorPartiallySuccessfullyCapturingKtM2 = 0x00d7,
    idmBombardedKtMineralPacketColonistsKilledCollision = 0x00d8,
    idmBombardedKtMineralPacketColonistsDefensesDestroy = 0x00d9,
    idmAnnihilatedMineralPacketColonistsKilled = 0x00da,
    idmDidntGetAttemptedTransferMineralPacketAnother = 0x00db,
    idmDidntGetAnyAttemptedTransferMineralPacket = 0x00dc,
    idmUnableTransferKtKtRequest = 0x00dd,
    idmAttemptedUseStargateStargateExistsThere = 0x00de,
    idmOneShipsDestroyedWhenEnginesReactedTrying = 0x00df,
    idmShipsDestroyedDueEngineStrain = 0x00e0,
    idmDestroyedMassiveReactorAccidentDueUnsafeOperatin = 0x00e1,
    idmAttemptedUseStargateReachCouldBecauseStargate = 0x00e2,
    idmAttemptedUseStargateReachCouldBecauseDestination = 0x00e3,
    idmAttemptedUseStargateReachCouldBecauseShips = 0x00e4,
    idmAttemptedUseStargateReachCouldBecauseStarbase = 0x00e5,
    idmAttemptedUseStargateCouldBecauseStarbaseOwned = 0x00e6,
    idmHeedlessDangerAttemptedUseStargateReachFleet = 0x00e7,
    idmUsedStargateReachLosingShipsTreacherousVoid = 0x00e8,
    idmUsedStargateReachLosingShipsUnforgivingVoid = 0x00e9,
    idmUsedStargateReachUnfortunatelyLosingShipsGreat = 0x00ea,
    idmUsedStargateReachLosingUnbelievableShipsJump = 0x00eb,
    idmHasUnloadedKtMineralsPreparationJumpingThrough = 0x00ec,
    idmHasUnloadedColonistsPreparationJumpingThroughSta = 0x00ed,
    idmHasUnloadedColonistsKtMineralsPreparationJumping = 0x00ee,
    idmWreckageDiscoveredBattleHasBoostedResearchResour = 0x00ef,
    idmWreckageBattleOccurredOrbitHasBoostedResearch = 0x00f0,
    idmFleetFoundWreckageBattleWhichHasBoosted = 0x00f1,
    idmUnableEngageEnginesDueBalkyEquipmentEngineers = 0x00f2,
    idmSRamScoopsHaveProducedMgFuel = 0x00f3,
    idmStarbaseHasSweptMinesMineField = 0x00f4,
    idmUnableCompleteMergeOrdersWaypointDestinationWasn = 0x00f5,
    idmUnableCompleteMergeOrdersDestinationFleetWasnt = 0x00f6,
    idmHasMerged = 0x00f7,
    idmWormholeHeadingHasVanishedOrdersHaveChanged = 0x00f8,
    idmColonyReportsBattleTookPlaceOrbitForces = 0x00f9,
    idmReportsBattleTookPlaceForcesInvolved = 0x00fa,
    idmColonistsHaveMadeGoodUseTimeIncreasing = 0x00fb,
    idmDoesHaveEnoughMineralsAvailableFlingAny = 0x00fc,
    idmFundamentalChangesEnvironmentHavePermanentlyAlte = 0x00fd,
    idmSurveyorsHaveDiscoveredPreviouslyUnknownDepositS = 0x00fe,
    idmPatrollingHasTargetedIntercept = 0x00ff,
    idmPopulationSuspectsUsurperProductivityOff20Growth = 0x0100,
    idmColonistsSuspectFactEmperorProductivityOff20 = 0x0101,
    idmHasRefusedMoveDoubtingAuthorityRulePress = 0x0102,
    idmFleetCaptainsHaveStagedStrikeDemandFree = 0x0103,
    idmHasDefectedRanksDueInabilityProjectLegitimate = 0x0104,
    idmCrewHasSoldOffCargoBlackMarket = 0x0105,
    idmFreedomFightersHaveAttackedDestroyedMinesPress = 0x0106,
    idmFreedomFightersHaveStolenKtStockpilesPress = 0x0107,
    idmMysteryTraderHasRefusedGiveCaptainAudience = 0x0108,
    idmHasAbsorbedMysteryTraderTraderHasGiven = 0x0109,
    idmHasAbsorbedMysteryTraderReturnTraderHas = 0x010a,
    idmHasAbsorbedMysteryTraderHaveGivenPlans = 0x010b,
    idmHasAbsorbedMysteryTraderReturnHaveGiven = 0x010c,
    idmHasAbsorbedMysteryTraderHoweverTraderUnable = 0x010d,
    idmHasAbsorbedMysteryTraderHoweverTraderUnable2 = 0x010e,
    idmHasAbsorbedMysteryTraderReturnHaveGiven2 = 0x010f,
    idmMysteryTraderHeadingHasVanishedOrdersHave = 0x0110,
    idmMineFieldHeadingHasVanishedOrdersHave = 0x0111,
    idmDoesHaveEnoughMineralsAvailableContinueAuto = 0x0112,
    idmBattleTookPlaceAgainstDestroyedEnemyForces = 0x0113,
    idmBattleTookPlaceAgainstForcesDestroyed = 0x0114,
    idmBattleTookPlaceAgainstNeitherNorEnemys = 0x0115,
    idmBattleTookPlaceAgainstNeitherForcesNor2 = 0x0116,
    idmRaceDefinitionHasTamperedStatisticsHaveAltered = 0x0117,
    idmMysteryTraderEyesCaptainSuspiciouslySuggestsHe = 0x0118,
    idmHasStolen = 0x0119,
    idmObsolete = 0x011a,
    idmStrongFundamentalForcesHaveRebirthed = 0x011b,
    idmAttemptedExecuteTransferOrdersInvolvingEitherFue = 0x011c,
    idmAttemptedExecuteTransferOrdersInvolvingFuelPlane = 0x011d,
    idmHadOrdersTransferCargoFutilePursuit = 0x011e,
    idmAttemptedLoadPlanetDontControlOrderHas = 0x011f,
    idmAttemptedLoadFleetDontControlOrderHas = 0x0120,
    idmAttemptedSetAmountBoardUnfortunatelyCouldntProvi = 0x0121,
    idmAttemptedSetNumberBoardUnfortunatelyCouldntProvi = 0x0122,
    idmAttemptedLoadDeepSpaceAttemptUnsuccessful = 0x0123,
    idmAttemptedShanghaiColonistsAttemptUnsuccessful = 0x0124,
    idmAttemptedStealMgFuelAttemptUnsuccessful = 0x0125,
    idmFailedLoadFuel = 0x0126,
    idmHasRerouted = 0x0127,
    idmHasReroutedUnfortuentlyDoesHaveEnoughFuel = 0x0128,
    idmHasOrdersBuildMineralPacketEitherDoesnt = 0x0129,
    idmHasOrdersBuildPlanetaryInstallationsBeyondMaximu = 0x012a,
    idmMysteriousTradingVesselBroadcastingProposalHasDe = 0x012b,
    idmHasImprovedValue = 0x012c,
    idmCurrentlyUnableImproveValueBeyond = 0x012d,
    idmHasRetroBombedUndoingTerraforming = 0x012e,
    idmHasOrdersTerraformBeyondMaximumAllowedOrders = 0x012f,
    idmMysteryTraderHasUnexplicablyChangedHisCourse = 0x0130,
    idmMineralPacketHasPermanentlyDefault = 0x0131,
    idmMineralPacketHasPermanentlyDefault2 = 0x0132,
    idmMineralPacketHas = 0x0133,
    idmMineralPacketHas2 = 0x0134,
    idmHasTriedBeamColonistsPlanetsStarbaseWould = 0x0135,
    idmScientistsHaveCompletedResearchTechLevelPrimary = 0x0136,
    idmHasExecutedOrdersFollowFleetAwaitsFurther = 0x0137,
    idmHadOrdersFollowFleetWhichDidntMove = 0x0138,
    idmStarbaseBuiltNewSDueLack27b = 0x0139,
    idmExaminationWreckageBattleUncoveredPlansNewPart = 0x013a,
    idmExaminationWreckageBattleUncoveredPlansNewShip = 0x013b,
    idmHasDismantledKtMineralsStarbaseOrbitingProcess = 0x013c,
    idmHasDismantledKtMineralsStarbaseOrbitingProcess2 = 0x013d,
    idmHasDismantledKtMineralsWhichHaveDeposited3 = 0x013e,
    idmHasDismantledKtMineralsStarbaseOrbitingHas = 0x013f,
    idmHasDismantledKtMineralsWhichHaveDeposited4 = 0x0140,
    idmHasDismantledKtMineralsStarbaseOrbiting2 = 0x0141,
    idmHasDismantledKtMineralsWhichHaveDeposited5 = 0x0142,
    idmHasDismantledKtMineralsStarbaseOrbitingUltimate2 = 0x0143,
    idmBattleTookPlaceDestroyedKillingColonistsBargain = 0x0144,
    idmRecentBreakthroughHasAlsoTaughtHowBuild = 0x0145,
    idmBombardedPacketContainingKtMineralsHoweverPacket = 0x0146,
    idmAttemptedReachViaStargateCouldBecauseStargate = 0x0147,
    idmCouldntGiveAwayBecausePlayerDead = 0x0148,
    idmCouldntGiveAwayBecauseThereColonistsBoard = 0x0149,
    idmCouldntGiveAwayBecauseDidntHaveAdministrative = 0x014a,
    idmAttemptedGiveFleetDontHaveEnoughExcess = 0x014b,
    idmSnubAttemptedGiftRefuseFleet = 0x014c,
    idmHasSuccessfullyGiven = 0x014d,
    idmHaveGiven = 0x014e,
    idmHasAbsorbedMysteryTraderReturnHaveGiven3 = 0x014f,
    idmHasAbsorbedMysteryTraderReturnTraderTried = 0x0150,
    idmMassPacketAppearsCollisionCourseWhichCurrently = 0x0151,
    idmStarbaseScheduledCompleteRemainingProductionItem = 0x0152,
    idmHaveReceivedOneBattleRecordingYear = 0x0153,
    idmHaveReceivedBattleRecordingsYear = 0x0154,
    idmAllowedTransferColonistsAnotherPlayer = 0x0155,
    idmHasAutoTerraformedValue = 0x0156,
    idmRecentBreakthroughHasAlsoTaughtHowBuild2 = 0x0157,
    idmBreedingActivitiesHaveOverflowedLivingSpaceColon = 0x0158,
    idmIntelligenceGatheringActivitiesCombinedSynergist = 0x0159,
    idmHasDegradedValue = 0x015a,
    idmCurrentlyUnableDegradeValueBeyond = 0x015b,
    idmEngineersHaveManagedImproveUnderlying1 = 0x015c,
    idmHaveInfoNewPlanetIfColonizeCan = 0x015d,
    idmUnableUseStargateBecauseHadColonistsBoard = 0x015e,
    idmHasAnnihilatedMineField3 = 0x015f,
    idmHasDamagedDetonatingMineFieldFleetHas = 0x0160,
    idmHasTakenDamageDetonatingMineFieldFleet = 0x0161,
    idmHasAnnihilatedMineField4 = 0x0162,
    idmHasDamagedDetonatingMineFieldMinesHave = 0x0163,
    idmHasDamagedDetonatingMineFieldMinesHave2 = 0x0164,
    idmHasTriedBeamColonistsDeepSpaceOrder = 0x0165,
    idmFleetsHaveBombedKillingColonists = 0x0166,
    idmFleetsHaveBombedDestroyingOneInstallation = 0x0167,
    idmFleetsHaveBombedDestroyingDefensesFactoriesMines = 0x0168,
    idmFleetsHaveBombedKillingColonistsDestroyingOne = 0x0169,
    idmFleetsHaveBombedKillingColonistsDestroyingDefens = 0x016a,
    idmFleetsHaveBombedKillingColonistsPlanetaryDefense = 0x016b,
    idmFleetsHaveBombedDestroyingOneInstallationPlaneta = 0x016c,
    idmFleetsHaveBombedDestroyingFactoriesMinesPlanetar = 0x016d,
    idmFleetsHaveBombedKillingColonistsDestroyingOne2 = 0x016e,
    idmFleetsHaveBombedKillingColonistsDestroyingDefens2 = 0x016f,
    idmFleetsHaveBombedKillingColonists2 = 0x0170,
    idmFleetsHaveBombedDestroyingOneInstallations = 0x0171,
    idmFleetsHaveBombedDestroyingDefensesFactoriesMines2 = 0x0172,
    idmFleetsHaveBombedKillingColonistsDestroyingOne3 = 0x0173,
    idmFleetsHaveBombedKillingColonistsDestroyingDefens3 = 0x0174,
    idmFleetsHaveBombedKillingColonistsPlanetaryDefense2 = 0x0175,
    idmFleetsHaveBombedDestroyingOneInstallationsPlanet = 0x0176,
    idmFleetsHaveBombedDestroyingDefensesFactoriesMines3 = 0x0177,
    idmFleetsHaveBombedKillingColonistsDestroyingOne4 = 0x0178,
    idmFleetsHaveBombedKillingColonistsDestroyingDefens4 = 0x0179,
    idmFleetsHaveRetroBombedUndoingTerraforming = 0x017a,
    idmFleetsHaveRetroBombedUndoingTerraforming2 = 0x017b,
    idmFleetsHaveBombedKillingOffEnemyColonists = 0x017c,
    idmFleetsHaveBombedKillingColonists3 = 0x017d,
    idmFailedLayMinesYearDueTechnicalDifficulties = 0x017e,
    idmFailedFlingMineralPacketDueTechnicalDifficulties = 0x017f,
    idmDueExcessiveFleetManeuveringBattleAreaFleets = 0x0180,
    idmBombardedKtMineralPacketFortunatelyOneHome = 0x0181,
    idmHackedRaceDiscoveredRaceStatisticsHaveAltered = 0x0182,
} MessageId;
typedef enum TutorId {
    idtWelcomeStarsTutorialWillGuideThrough36 = 0x1000,
    idtHomePlanetCoupleScoutsDestroyerFreighterColony = 0x1001,
    idtThereFiveMessagesMessagesPaneEachYear = 0x1002,
    idtAboutPlanetsFleetsAboutEventsKnownPlayers = 0x1003,
    idtYearMessagesPlayingTipsNoneThemRequire = 0x1004,
    idtReadMessages = 0x1005,
    idtCanClickButtonUseArrowKey = 0x1006,
    idt0007Blank = 0x1007,
    idtExamineTilesCommandPaneUpperLeftPortion = 0x1008,
    idtControlsTilesGiveFullInformationCommandPlanet = 0x1009,
    idtFleetsOrbitTileShowsFuelCargoBoard = 0x100a,
    idtPressTilesGotoButtonCommandArmedProbe = 0x100b,
    idtPaneGivesInformationCommandFleet = 0x100c,
    idtLetsSendScoutOffExploringHasAutomatically = 0x100d,
    idtScannerPaneShowsMapUniverse = 0x100e,
    idtHoldShiftKeyClickLeftMouseButton = 0x100f,
    idtAccordingFleetWaypointsTileWillTake2 = 0x1010,
    idtLongRangeScout2HasSixTimes = 0x1011,
    idtHitNKeyLookFleet = 0x1012,
    idtLongRangeScout2UnarmedWeLikely = 0x1013,
    idtTowardsPlanetsAboveRight = 0x1014,
    idtHoldShiftKeyLeftClickPlanet90210 = 0x1015,
    idt0022Blank = 0x1016,
    idt0023Blank = 0x1017,
    idtLetsMoveOurFleet = 0x1018,
    idtTimePressButtonTileShowingLongRange = 0x1019,
    idtSantaMaria3ColonyFleetWeDont = 0x101a,
    idtPress = 0x101b,
    idtTeamster4FreighterWeDontHaveAnything = 0x101c,
    idtPress2 = 0x101d,
    idtStalwartDefender5DestroyerWillUsefulScout = 0x101e,
    idtHoldShiftKeySelectAlexander = 0x101f,
    idtHitNKey = 0x1020,
    idtCottonPicker6RemoteMinerWellSend = 0x1021,
    idtHitNKey2 = 0x1022,
    idtBackArmedProbe1ThatsFleetsRight = 0x1023,
    idtOtherThingWeShouldDoYearPick = 0x1024,
    idtChooseResearchCommandsMenu = 0x1025,
    idtChangeFieldStudyWeaponsPressDone = 0x1026,
    idtThatsTurnHitF9GenerateYear = 0x1027,
    idtReadMessageMessagesPaneWeveGotPlenty = 0x1028,
    idtBuildingFactories = 0x1029,
    idtPressChangeButtonProductionTile = 0x102a,
    idtSelectFactoryLeftHandListboxHoldShift = 0x102b,
    idtShiftKeyCausesAddButtonAdd10 = 0x102c,
    idtMessageYearScannerPaneShowsFleetsHave = 0x102d,
    idtYetArrivedSoThereNothingDoYear = 0x102e,
    idtHitF9KeyGenerateYear = 0x102f,
    idtReadFirstMessagePressGotoMessagesPane = 0x1030,
    idtLetsGiveArmedProbe1BunchPlaces = 0x1031,
    idtHoldShiftKeyLeftClickHiho = 0x1032,
    idtVacancy = 0x1033,
    idtSlime = 0x1034,
    idtWallaby = 0x1035,
    idtOxygen = 0x1036,
    idtReadMessagePressGotoCommandLongRange = 0x1037,
    idtWeWantSendFleetExploreAreaAbove = 0x1038,
    idtHoldShiftKeySelectDwarte = 0x1039,
    idtMobius = 0x103a,
    idtCastle = 0x103b,
    idtMoholdi = 0x103c,
    idtReadMessage = 0x103d,
    idtAnd = 0x103e,
    idtGotoStalwartDefender5 = 0x103f,
    idtHoldShiftKeySelectShaggyDog = 0x1040,
    idtSeaSquared = 0x1041,
    idtRedStorm = 0x1042,
    idtBloop = 0x1043,
    idtKalamazoo = 0x1044,
    idtReadTwoMessages = 0x1045,
    idtAnd2 = 0x1046,
    idtPressGotoDisplayStatsPruneSummaryPane = 0x1047,
    idtTopGraphSummaryPaneShowsPruneHas = 0x1048,
    idtCurrentTechnology = 0x1049,
    idtDiamondsBottomGraphShowMineralConcentrationsPrun = 0x104a,
    idtThereMinePlanet = 0x104b,
    idtClickRightMouseButtonStoveTopSelect = 0x104c,
    idtShiftClickPrune = 0x104d,
    idtLookWaypointTaskTile = 0x104e,
    idtClickDropdownChangeTaskRemoteMining = 0x104f,
    idtMoveMessageGotoAlexander = 0x1050,
    idtDiamondsBottomGraphShowAlexandersMineralConcentr = 0x1051,
    idtSendingCottonPicker6PruneRightThing = 0x1052,
    idtClickVariousPlacesSummaryPaneGetPopup = 0x1053,
    idtReadMessage2 = 0x1054,
    idtAnd3 = 0x1055,
    idtGotoPlanet90210 = 0x1056,
    idt0087Blank = 0x1057,
    idtSince90210FinePlanetHighMineralConcentrations = 0x1058,
    idtRightClickStoveTopSelectSantaMaria = 0x1059,
    idtClickXferButtonTileLabeledOrbitingStove = 0x105a,
    idtClickDragColonistsGaugeFillingHold25kt = 0x105b,
    idtShiftClick90210 = 0x105c,
    idtSelectColonizeDropdownWaypointTaskTile = 0x105d,
    idtThatsYear = 0x105e,
    idtHitF9GenerateYear = 0x105f,
    idtFirstMessageQuiteCommonWeDontNeed = 0x1060,
    idtFilterClickingBlueCheckMarkUpperLeft = 0x1061,
    idtMoveMessageGotoStoveTop = 0x1062,
    idtWeDoWantHaveKeepAddingFactories = 0x1063,
    idtBuild30FactoriesEveryYear = 0x1064,
    idtPressChangeButtonProductionTile2 = 0x1065,
    idtSelectFactoriesAutoBuildLeftHandListbox = 0x1066,
    idt0103Blank = 0x1067,
    idtReadTwoMessagesGoto90210 = 0x1068,
    idtProductionQueueHereEmptyWeOughtDo = 0x1069,
    idtHitQKey = 0x106a,
    idtDoubleClickFactory3TimesMine3 = 0x106b,
    idtWillTake10YearsBuild3Factories = 0x106c,
    idtRightClickStoveTopSelectTeamster4 = 0x106d,
    idtClickXferButtonCommandPane = 0x106e,
    idtFillHoldColonistsHitOk = 0x106f,
    idtShiftClick902102 = 0x1070,
    idtChangeWaypointTaskTransport = 0x1071,
    idtRightClickBlueDiamondWaypointTaskTile = 0x1072,
    idtReadMessageGotoHiho = 0x1073,
    idtNoticeArmedProbe1WhichBlueTriangle = 0x1074,
    idtDarkYellowCircleSurroundingArmedProbe1 = 0x1075,
    idtDoubleClickArmedProbe1 = 0x1076,
    idt0119Blank = 0x1077,
    idtArmedProbe1DoesntNeedGoWay = 0x1078,
    idtClickHiho = 0x1079,
    idtAnd4 = 0x107a,
    idtHitDeleteKey = 0x107b,
    idtArmedProbe1WillGoVacancyWithout = 0x107c,
    idtThatsYear2 = 0x107d,
    idtChangePaceInsteadHittingF9 = 0x107e,
    idtSelectGenerateTurnMenu = 0x107f,
    idtReadFirstMessageGotoShaggyDog = 0x1080,
    idtWellAddColonizerQueueBitFirstLets = 0x1081,
    idtDoubleClickStalwartDefender5JustAbove = 0x1082,
    idtSelectWaypointShaggyDog = 0x1083,
    idtPressDeleteKey = 0x1084,
    idtReadMessageGotoDwarte = 0x1085,
    idtWhatUnpleasantPlace = 0x1086,
    idtDoubleClickStoveTop = 0x1087,
    idtHitChangeButtonProductionTileOpenStove = 0x1088,
    idtDoubleClickSantaMariaLeftHandListbox = 0x1089,
    idtNoticeFactoryTopQueueSantaMariaDisplayed = 0x108a,
    idtProductionTileMeansWillFinishedYear = 0x108b,
    idtBlueItemsProductionQueueWillMakePartial = 0x108c,
    idtRedItemsMayNeverFinish = 0x108d,
    idtGoAhead = 0x108e,
    idtGenerateWhenReady = 0x108f,
    idtReadFirstMessageGotoNewSantaMaria = 0x1090,
    idtHasReplacedOldSantaMariaFleet3 = 0x1091,
    idtClickCargoGaugeFuelCargoTile = 0x1092,
    idtDoesSameThingHittingXferButton = 0x1093,
    idtFillHoldFullColonistsHitOk = 0x1094,
    idtWhereWeWantedSendColonizer = 0x1095,
    idtClickButtonToolbarShowPlanetsHowHabitable = 0x1096,
    idtShiftClickBigGreenShaggyDogBelow = 0x1097,
    idtSetWaypointTaskColonize = 0x1098,
    idtSwitchScannerBackNormalViewClickingLeftmost = 0x1099,
    idtRead2MessagesGotoTeamster4 = 0x109a,
    idtShiftClickStoveTopSendHome = 0x109b,
    idtSelect90210PressingGotoButtonTileLabeled = 0x109c,
    idtNoticeFactoriesWillDone2YearsInstead = 0x109d,
    idtReadMessageDeleteArmedProbe1sWaypoint = 0x109e,
    idtThatsYearGenerateWhenReady = 0x109f,
    idtReadFirstMessageGotoTeamster4 = 0x10a0,
    idtCottonPicker6HasRemoteMiningPrune = 0x10a1,
    idtMineralsBackStoveTop = 0x10a2,
    idtShiftClickPrune2 = 0x10a3,
    idtSetWaypointTaskTransport = 0x10a4,
    idtRightClickBlueDiamondSelectQuikloadZip = 0x10a5,
    idtShiftClickBackStoveTop = 0x10a6,
    idt0167Blank = 0x10a7,
    idtNoticeWaypointTaskHasCopiedPreviousWaypoint = 0x10a8,
    idtPruneStovetop = 0x10a9,
    idtRightClickBlueDiamondSelectQuikdropZip = 0x10aa,
    idtClickRepeatOrdersCheckboxFleetWaypointsTile = 0x10ab,
    idtTeamster4WillContinueHaulMineralsPrune = 0x10ac,
    idtSelectStoveTop = 0x10ad,
    idtAddSantaMariaProductionQueue = 0x10ae,
    idtGoAheadGenerateIDare = 0x10af,
    idtReadFirstMessageGotoNewSantaMaria2 = 0x10b0,
    idtClickToolbarButtonPutScannerPlanetValue = 0x10b1,
    idtRedStormClearlyBestPlanetAvailable = 0x10b2,
    idtGiveSantaMaria7ColonizeTaskRed = 0x10b3,
    idtThereLeastTwoColonizablePlanetsSeaSquared = 0x10b4,
    idtAddThreeSantaMariasStoveTopsProduction = 0x10b5,
    idtPressLeftmostToolbarButtonPutScannerBack = 0x10b6,
    idt0183Blank = 0x10b7,
    idtReadMessage3 = 0x10b8,
    idtWellKeepGettingMineBuildingMessagesForever = 0x10b9,
    idtFilterThemClickingBlueCheckMarkMessages = 0x10ba,
    idtGoMessage = 0x10bb,
    idtGoto90210OpenProductionQueue = 0x10bc,
    idtShiftDoubleClickFactoriesAutoBuildMines = 0x10bd,
    idtReadRestMessages = 0x10be,
    idt0191Blank = 0x10bf,
    idtClickRedTriangleBetweenSlimeVacancy = 0x10c0,
    idtEnemyScoutShipRightClickingFleetImage = 0x10c1,
    idtArmedAccordingProjectedPathScannerHeadedVacancy = 0x10c2,
    idtArmedProbe1WontAbleCatchWe = 0x10c3,
    idtAddTwoArmedProbesStoveTopsQueue = 0x10c4,
    idtThatsYear3 = 0x10c5,
    idtGenerateWhenReady2 = 0x10c6,
    idt0199Blank = 0x10c7,
    idtReadFirstMessageGotoNewColonyShips = 0x10c8,
    idtWeHavePlacesSendTwoThemSo = 0x10c9,
    idtHitSplitButtonFleetCompositionTile = 0x10ca,
    idtMoveOneSantaMariasFleet10Hit = 0x10cb,
    idtLeavesTwoSantaMariasFleetCurrentlyCommanding = 0x10cc,
    idtLoadFleetColonistsGiveColonizeTaskSlime = 0x10cd,
    idtWeDontWantBothColonizersGoSlime = 0x10ce,
    idt0207Blank = 0x10cf,
    idtNoticeFleetHasOneSantaMariaOther = 0x10d0,
    idtClickWaypointSlimeDragSeaSquared = 0x10d1,
    idtNoticeThereWaypointLinesGoingStoveTop = 0x10d2,
    idtSantaMaria8SantaMaria11Have = 0x10d3,
    idtReadMessageGotoNewArmedScouts = 0x10d4,
    idtIfHeadDirectlyVacancyEnemyScoutWill = 0x10d5,
    idtLetsTryHeadThemOffPass = 0x10d6,
    idtShiftClickHiho = 0x10d7,
    idtReadMessageFilter = 0x10d8,
    idtReadLastMessageGotoWallaby = 0x10d9,
    idtWallabyOwnedBerserkersLightlyPopulatedNearlyTerr = 0x10da,
    idtClickGreenRadiationBarSummaryPaneRead = 0x10db,
    idtReason7SoundsFamiliar = 0x10dc,
    idtHitF5OpenResearchDialog = 0x10dd,
    idt0222Blank = 0x10de,
    idt0223Blank = 0x10df,
    idtRightRadiationTerraform7OneExpectedBenefits = 0x10e0,
    idtBenefitsListedBlueWillTakeOneAdditional = 0x10e1,
    idtClickWordRadiationDialogSeeRequirements = 0x10e2,
    idtCurrentRateResearchWeaponsTech5Will = 0x10e3,
    idtIncreaseResourcesBudgetedResearch30HitDone = 0x10e4,
    idtWellSendTroopShipWallabySoonThats = 0x10e5,
    idtGenerateWhenReady3 = 0x10e6,
    idt0231Blank = 0x10e7,
    idtReadFirstMessageGotoResearchDialog = 0x10e8,
    idtNoticeRadiation7ListedGreenIndicatingWill = 0x10e9,
    idtCurrentLevelStudyWeaponsEstimatedTimeCompletion = 0x10ea,
    idtNoticeFieldResearchCurrentlySetSameField = 0x10eb,
    idtChangeFieldResearchConstructionHitDone = 0x10ec,
    idtSoonReachTech5WeaponsResearchFocus = 0x10ed,
    idtReadMessageFilter2 = 0x10ee,
    idt0239Blank = 0x10ef,
    idtReadMessageGotoOxygen = 0x10f0,
    idtRightClickStoveTopSelectSantaMaria2 = 0x10f1,
    idtLoadColonists = 0x10f2,
    idtSendColonizeOxygen = 0x10f3,
    idtSinceWeveSeenOxygenAlreadyWeCan = 0x10f4,
    idtSelectArmedProbe1DragWaypointOxygen = 0x10f5,
    idtThatsYear4 = 0x10f6,
    idtGenerateWhenReady4 = 0x10f7,
    idtFilterMessageAboutDismantlingColonizer = 0x10f8,
    idtReadMessageGotoShaggyDog = 0x10f9,
    idtOpenShaggyDogsProductionQueue = 0x10fa,
    idtAdd3FactoriesAutoBuild3Mines = 0x10fb,
    idtWeShouldAlsoSetDefaultQueueSo = 0x10fc,
    idtColonizersHaveEnRoute = 0x10fd,
    idtOpenProductionQueue = 0x10fe,
    idtRightClickBlueDiamondSelectCustomize = 0x10ff,
    idtHitImportButtonCopyShaggyDogsQueue = 0x1100,
    idtOkProductionDialog = 0x1101,
    idtEveryNewPlanetColonizeWillAutomaticallyGet = 0x1102,
    idtReadMessageGotoBloopLooksLikeNice = 0x1103,
    idtAddSantaMariaStoveTopsQueue = 0x1104,
    idtWeWantTakeWallabyTeamster4Busy = 0x1105,
    idtAddNewTeamsterStoveTopsQueue = 0x1106,
    idtGenerateWhenReady5 = 0x1107,
    idtReadFirstMessageGotoArmedProbe1 = 0x1108,
    idtShiftClickHacker = 0x1109,
    idtReadMessageGotoLongRangeScout2 = 0x110a,
    idtGuyIsntWorthMuchAnymoreHesToo = 0x110b,
    idtShiftClickStoveTopChangeWaypointTask = 0x110c,
    idtReadMessageSendStalwartDefender5Stove = 0x110d,
    idtWillAutomaticallyRefueledStarbaseWhenArrives = 0x110e,
    idtReadMessageGotoArmedProbe9Shift = 0x110f,
    idtReadMessageGotoNewSantaMaria = 0x1110,
    idtUseToolbarScannerSummaryPaneFigureWhich = 0x1111,
    idtGiveSantaMaria3OrdersColonizeDont = 0x1112,
    idtReadMessageGotoTeamster12 = 0x1113,
    idtLoadColonistsAssignWaypointWallaby = 0x1114,
    idtChangeWaypointTaskTransport2 = 0x1115,
    idtSetSecondDropdownWaypointTaskTileColonists = 0x1116,
    idtThirdUnload = 0x1117,
    idtReadMessageAdd70MinesTopStove = 0x1118,
    idtReadMessageGotoResearchDialog = 0x1119,
    idtLeaveFieldStudyConstructionChangeFieldResearch = 0x111a,
    idtReadMessageHitGotoOpenTechnologyBrowser = 0x111b,
    idtWhenDoneReadingAboutBetaTorpedoClose = 0x111c,
    idtReadTwoMessagesLookingTechBrowserIf = 0x111d,
    idtReadMessageGotoArmedProbe = 0x111e,
    idt0287Blank = 0x111f,
    idtGollyNailedOneThemNoticeButtonNormally = 0x1120,
    idtPressViewOpenBattleVcr = 0x1121,
    idtUseVcrControlsWatchPlaybackBattleHit = 0x1122,
    idtReadRestMessages2 = 0x1123,
    idtStoveTopBusyBuildingMinesYearSo = 0x1124,
    idtGeneralWalkingThroughMessagesHandlingOnesSeem = 0x1125,
    idtWorkAnyParticularYear = 0x1126,
    idtGenerateWhenReady6 = 0x1127,
    idtReadFirstMessageGotoButtonDisabledI = 0x1128,
    idtReadMessageGotoArmedProbe9 = 0x1129,
    idtLooksLikeNailedAnotherOneThereYellow = 0x112a,
    idtSalvageLeftBattle = 0x112b,
    idtRightClickArmedProbe9SelectSalvage = 0x112c,
    idtSummaryPaneShowsSalvageConsistsFewKt = 0x112d,
    idtFreighterAfterAnyway = 0x112e,
    idtSelectViewFindTypeTeamster4Hit = 0x112f,
    idtIfCantFindWhereYellowSelectionArrow = 0x1130,
    idtTeamster4StoveTopWillBackPrune = 0x1131,
    idtReadMessage4 = 0x1132,
    idtWellExplainsWhatHappenedArmedProbe1 = 0x1133,
    idtWatchSadBattleIfWantMoveMessage = 0x1134,
    idtThatsLikeOneMayWantWatch = 0x1135,
    idtReadMessageGotoRedStorm = 0x1136,
    idt0311Blank = 0x1137,
    idtOtherBitShortColonistsRedStormDoing = 0x1138,
    idtReadMessageGotoSlime = 0x1139,
    idtLookSummaryPaneSlimeOutsideHabitableRange = 0x113a,
    idtWhatWeNeedDoAddTerraformingProduction = 0x113b,
    idtOpenSlimesProductionQueueAddTwoTerraform = 0x113c,
    idtLookProductionTileISuspectNeedFew = 0x113d,
    idtAddTwoTeamstersStoveTopsProductionQueue = 0x113e,
    idtGenerateWhenReady7 = 0x113f,
    idtReadFirstMessageLoadTeamster1Colonists = 0x1140,
    idtSendSlimeOrdersUnloadThem = 0x1141,
    idtReadMessage5 = 0x1142,
    idtLooksLikeLoadReinforcementsTeamster1Carrying = 0x1143,
    idtReadMessageOpenResearchDialogChangeField = 0x1144,
    idtReadMessageCheckRoboMinerTechBrowser = 0x1145,
    idtWeCouldUseMinersHelpStripPrune = 0x1146,
    idtHitF4OpenShipDesigner = 0x1147,
    idtSelectAvailableHullTypes = 0x1148,
    idtChooseMiniMinerDropdown = 0x1149,
    idtHitCopySelectedDesign = 0x114a,
    idtLeftSideDisplaysListEveryPartCapable = 0x114b,
    idtDragLongHump6EnginePartsList = 0x114c,
    idtDragRhinoScannerScannerElectMechSlot = 0x114d,
    idtSelectMiningRobotsPartsCategoryDropdown = 0x114e,
    idtDragRoboMinerEachMiningSlots = 0x114f,
    idtShipDesignNameImageJustFine = 0x1150,
    idtHitOkFinishEditingDesign = 0x1151,
    idtDoneCloseDesigner = 0x1152,
    idtAddOneNewMiniMinersStoveTops = 0x1153,
    idtWillTake6YearsFinishShipJust = 0x1154,
    idtOpenStoveTopsQueue = 0x1155,
    idtSelectMineLeftHandListboxTopQueue = 0x1156,
    idt0343Blank = 0x1157,
    idtClickEachItemsProductionTileMiniMiner = 0x1158,
    idtReadFinalMessage = 0x1159,
    idtLowEnoughMineralsPointDesigningAdditionalShips = 0x115a,
    idtWeLeftArmedProbe9HangingNear = 0x115b,
    idtSendArmedProbe9BackStoveTop = 0x115c,
    idtThatsEnoughYear = 0x115d,
    idtGenerateNewYear = 0x115e,
    idt0351Blank = 0x115f,
    idtReadFirstMessageGotoSeaSquared = 0x1160,
    idtOtherNeedingPeopleDoingJustFine = 0x1161,
    idtReadFinalMessageGotoOxygen = 0x1162,
    idtPlanetSlightlyHabitableRangeWeShouldAdd = 0x1163,
    idtAddMinTerraform2OxygensQueueRight = 0x1164,
    idtThatsYearAutomationMakesYearsFlyFaster = 0x1165,
    idtGenerateWhenReady8 = 0x1166,
    idt0359Blank = 0x1167,
    idtReadFirstMessageSendArmedProbe9 = 0x1168,
    idtReadMessageGotoNewTeamsterFillColonists = 0x1169,
    idtWeWouldLikeSendColonistsWhereNeeded = 0x116a,
    idtChoosePlanetsReportMenu = 0x116b,
    idtClickTitleValueColumnSortValue = 0x116c,
    idtPlanetLargestNegativeValueWallabyColonistsWill = 0x116d,
    idtHitEscKeyClosePlanetSummaryReport = 0x116e,
    idtSendTeamster7WallabyUnloadColonists = 0x116f,
    idtReadTwoMessagesOpenResearchDialog = 0x1170,
    idtExpectedResearchBenefitsEitherBlueBlackWhich = 0x1171,
    idtWeLearnAnythingFieldResearchAlreadySet = 0x1172,
    idtCloseDialog = 0x1173,
    idtReadRemainingMessagesSendTeamster12Back = 0x1174,
    idtMineDispenser50SoundedInterestingIsntGood = 0x1175,
    idtGenerateNewYear2 = 0x1176,
    idt0375Blank = 0x1177,
    idtReadFirstMessageSendStalwartDefender5 = 0x1178,
    idtReadMessageGotoNewMiniMiner = 0x1179,
    idtShiftClickPruneSetWaypointTaskMerge = 0x117a,
    idtNoticeWaypointPruneHasChangedCottonPicker = 0x117b,
    idtReadMessageOpenStoveTopsProductionQueue = 0x117c,
    idtIncreaseNumberAutoBuildFactories60Add = 0x117d,
    idtReadTwoMessagesChangeFieldResearchConstruction = 0x117e,
    idtReadFinalMessageGenerate = 0x117f,
    idtReadMessages2 = 0x1180,
    idtSendTeamster1BackStoveTop = 0x1181,
    idtSureEasyTurn = 0x1182,
    idtGenerateNewYear3 = 0x1183,
    idt0388Blank = 0x1184,
    idt0389Blank = 0x1185,
    idt0390Blank = 0x1186,
    idt0391Blank = 0x1187,
    idtReadFirstMessageAddTeamsterStoveTops = 0x1188,
    idtReadMessageGoto90210 = 0x1189,
    idtWellWeCouldPlayProductionQueueLooks = 0x118a,
    idtReadTwoMessagesOpenResearchDialog2 = 0x118b,
    idtClickDifferentItemsListedExpectedBenefitsBox = 0x118c,
    idtStargateSoundsLikeFunFrigateAlsoLooks = 0x118d,
    idtCloseDialogWithoutMakingAnyChanges = 0x118e,
    idtReadRemainingMessagesGenerateYear = 0x118f,
    idtReadFirstMessageLoadTeamster12Colonists = 0x1190,
    idtAddWaypointWallabyUnloadColonists = 0x1191,
    idtShiftClickBackStoveTopChangeTask = 0x1192,
    idtClickRepeatOrdersCheckboxFleetWaypointsTile2 = 0x1193,
    idtEstFuelUsageClaimsWeWillNeed = 0x1194,
    idtFleetWillLighterWillNeedMuchFuel = 0x1195,
    idtReadMessageGotoNewTeamster = 0x1196,
    idtLoadColonists2 = 0x1197,
    idtAddWaypointOxygenUnloadColonists = 0x1198,
    idtShiftClickBackStoveTopChangeTask2 = 0x1199,
    idtClickRepeatOrdersCheckboxFleetWaypointsTile3 = 0x119a,
    idtBothFreightersWillContinueMovingColonistsAway = 0x119b,
    idtReadMessageAddMaxTerraformAutoBuild = 0x119c,
    idtRead3MessagesSendTeamster7Back = 0x119d,
    idtReadLastMessageGenerateYear = 0x119e,
    idt0415Blank = 0x119f,
    idtReadFirstTwoMessagesSendArmedProbe = 0x11a0,
    idtRead3MessagesChangeFieldResearchWeapons = 0x11a1,
    idtReadFinalMessageHitF4OpenShip = 0x11a2,
    idtSelectStarbasesCopySelectedDesign = 0x11a3,
    idtSelectOrbitalPartsCategoryDragStargate100 = 0x11a4,
    idtChangeDesignNameGaterClickRightArrow = 0x11a5,
    idtAddGaterStoveTopsQueue = 0x11a6,
    idtGenerate = 0x11a7,
    idtReadFirstMessageLoadTeamster1Colonists2 = 0x11a8,
    idtSendUnloadColonistsWallaby = 0x11a9,
    idtWeDoFrequentlyEnoughWeShouldSimplify = 0x11aa,
    idtRightClickBlueDiamondWaypointTaskTile2 = 0x11ab,
    idtHitImportNameOrderDropcolOkBoth = 0x11ac,
    idtFutureWeCanSetFleetsTaskUsing = 0x11ad,
    idtShiftClickStoveTopChangeTransportOption = 0x11ae,
    idtClickRepeatOrders = 0x11af,
    idtReadRestMessages3 = 0x11b0,
    idtYouveUpgradedStarbaseStoveTopWeDont = 0x11b1,
    idtHitF3OpenPlanetSummaryReport = 0x11b2,
    idtFindMinConcColumnRightClickReverse = 0x11b3,
    idtOxygenSeaSquaredRedStormWallabyHave = 0x11b4,
    idtWallabyHasHighestPopulationClosestBerserkersPlan = 0x11b5,
    idtUnfortunatelyWallabyStillHasNegativeGrowthRate = 0x11b6,
    idtGenerateWhenReady9 = 0x11b7,
    idtReadFirstMessageGotoTeamster42 = 0x11b8,
    idtSeemsMiniMinerWeAddedPruneHas = 0x11b9,
    idtWeCouldDecreaseSpeedEachLegTeamster = 0x11ba,
    idtRemoteMinersProducing = 0x11bb,
    idtMineralsWeCanCarryEachTripWe = 0x11bc,
    idtClickDragFuelGaugeOtherFleetsHere = 0x11bd,
    idt0446Blank = 0x11be,
    idt0447Blank = 0x11bf,
    idtAddTeamsterStoveTopsQueue = 0x11c0,
    idtRead4MessagesChangeFieldResearchPropulsion = 0x11c1,
    idtReadRemainingMessagesOpenShipDesigner = 0x11c2,
    idtViewAvailableHullTypesSelectFrigateDropdown = 0x11c3,
    idtDragDaddyLongLegs7EngineSlot = 0x11c4,
    idtSelectMineLayersDropdownDrag3Mine = 0x11c5,
    idtChangeDesignNameMineLayerOkDesign = 0x11c6,
    idtAddMineLayerStoveTopsQueue = 0x11c7,
    idtClickRedTriangleWallaby = 0x11c8,
    idtBerserkerColonizerHeadedVacancyWarp6 = 0x11c9,
    idtRightClickWallabySelectStalwartDefender5 = 0x11ca,
    idtShiftClickEnemyFleet = 0x11cb,
    idtShouldSufficientYear = 0x11cc,
    idtGenerateWhenReady10 = 0x11cd,
    idt0462Blank = 0x11ce,
    idt0463Blank = 0x11cf,
    idtReadFirstMessageGotoStalwartDefender5 = 0x11d0,
    idtFleetDisplayedPurpleScannerMeansThereEnemy = 0x11d1,
    idtAlsoTellsUsAlthoughCaughtColonizerDidnt = 0x11d2,
    idtYoullSeeMessageAboutBattleLater = 0x11d3,
    idtReadMessageGotoTeamster7 = 0x11d4,
    idtOpenPlanetSummaryReportSortPopulation = 0x11d5,
    idtOxygenHasLowestPopulationYouveAlreadyGot = 0x11d6,
    idtHitEscCloseReport = 0x11d7,
    idtLoadTeamster7ColonistsSendSeaSquared = 0x11d8,
    idtSetWaypointTaskTransport2 = 0x11d9,
    idtRightClickBlueDiamondChooseDropcol = 0x11da,
    idtReadTwoMessagesGotoNewTeamster = 0x11db,
    idtFreighterWeBuiltMergeOneGoingBack = 0x11dc,
    idtSelectTeamster4ListboxOtherFleetsHere = 0x11dd,
    idtPressMergeButtonFleetCompositionTile = 0x11de,
    idtClickTeamster3MergeFleetsDialogHit = 0x11df,
    idtReadMessageSetMineLayer8sTask = 0x11e0,
    idtReadMessageAddMiniMinerStoveTops = 0x11e1,
    idtRead2MessagesWatchBattle = 0x11e2,
    idtBerserkersSantaMaria80DamagedCanKill = 0x11e3,
    idtRightClickBlueDiamondFleetWaypointsTile = 0x11e4,
    idtReadLastMessageDoubleClickArmedProbe = 0x11e5,
    idtDragWaypointLaTeDaSpeedBump = 0x11e6,
    idtShiftClickLeverGenerate = 0x11e7,
    idtReadFirstMessageSendStalwartDefender52 = 0x11e8,
    idtReadMessageGotoMiniMiner3Send = 0x11e9,
    idtReadRemainingMessages = 0x11ea,
    idtThats = 0x11eb,
    idtGenerateWill = 0x11ec,
    idt0493Blank = 0x11ed,
    idt0494Blank = 0x11ee,
    idt0495Blank = 0x11ef,
    idtReadFirstTwoMessages = 0x11f0,
    idtClickButtonToolbar = 0x11f1,
    idtNoticeThereLotGreenWorldsWeNeed = 0x11f2,
    idtHitF4OpenShipDesigner2 = 0x11f3,
    idtSelectSantaMariaDropdown = 0x11f4,
    idtHitEditSelectedDesign = 0x11f5,
    idtDragLongHump6EngineDesignParts = 0x11f6,
    idtOkDesignHitDoneCloseDialog = 0x11f7,
    idtAdd3ImprovedSantaMariasStoveTops = 0x11f8,
    idtReadMessageGotoSeaSquared = 0x11f9,
    idtSeaSquaredHasBuiltManyFactoriesMines = 0x11fa,
    idtAddMaxTerraformAutoBuild2End = 0x11fb,
    idtRead2MessagesChangeFieldResearchConstruction = 0x11fc,
    idtReadRestMessages4 = 0x11fd,
    idtAnd5 = 0x11fe,
    idtGenerateWhenReady11 = 0x11ff,
    idtReadFirstThreeMessages = 0x1200,
    idtGoto3NewSantaMarias = 0x1201,
    idtLoadThemColonistsSendThemColonizeLever = 0x1202,
    idtHitSplitButtonFleetCompositionTile2 = 0x1203,
    idtDragSantaMaria10sWaypointSpeedBump = 0x1204,
    idtSantaMaria11sBloop = 0x1205,
    idtReadRestMessages5 = 0x1206,
    idt0519Blank = 0x1207,
    idtTeamster12WillArriveStoveTopYear = 0x1208,
    idtAdd3TeamstersStoveTopsQueue = 0x1209,
    idtAugmentOurTroopLiftEffort = 0x120a,
    idtClickEnemyShipNearWallaby = 0x120b,
    idtBerserkersTryingColonizeVacancyFolksNeverLearn = 0x120c,
    idtSelectStalwartDefender5DragDestinationEnemy = 0x120d,
    idtYearWellHaveDesignNewDestroyerGo = 0x120e,
    idtGenerateWhenYoureReady = 0x120f,
    idtReadFirstMessageGotoStalwartDefender52 = 0x1210,
    idtOnceWeveWoundedFinishedOffColonizer = 0x1211,
    idtRightClickBlueDiamondFleetWaypointsTile2 = 0x1212,
    idtTellsDestroyerFollowDestroyEnemyColonizer = 0x1213,
    idtReadMessageGotoArmedProbe92 = 0x1214,
    idtWellLeaveFleetHereGuardLeverSince = 0x1215,
    idtReadMessageGotoNewFleet = 0x1216,
    idtFillColonists = 0x1217,
    idtWeWantMergeNewTeamstersOtherFleet = 0x1218,
    idtSelectTeamster12PressMergeButtonFleet = 0x1219,
    idtReadMessage6 = 0x121a,
    idtWellDealStoveTopAfterWeFinish = 0x121b,
    idtReadRestMessagesViewingBattleColonizerIf = 0x121c,
    idtPromisedLastYearLetsDesignDestroyerTake = 0x121d,
    idtHitF4OpenShipDesigner3 = 0x121e,
    idt0543Blank = 0x121f,
    idtWeWantPowerfulWeAlsoWantWeigh = 0x1220,
    idtSelectAvailableHullTypesChooseDestroyerDropdown = 0x1221,
    idtAddRadiatingHydroRamScoop2Carbonic = 0x1222,
    idtAddFuelTankMechanicalSlotBattleComputer = 0x1223,
    idtTotalMassDesign97kt = 0x1224,
    idtClickRightArrowButtonBelowShipImage = 0x1225,
    idtPut10DestroyersStoveTopsQueue = 0x1226,
    idtGenerate2 = 0x1227,
    idtReadFirstMessageGotoStalwartDefender53 = 0x1228,
    idtOnce = 0x1229,
    idtSendWallaby = 0x122a,
    idtRead4MessagesGotoNewDestroyerArmada = 0x122b,
    idtSendWreakHavocBerserkerStarbaseHacker = 0x122c,
    idtReadRestMessages6 = 0x122d,
    idtAnd6 = 0x122e,
    idtGenerateTurn = 0x122f,
    idtReadFirstMessageGotoTeamster43 = 0x1230,
    idtWeNeedSlowFleetLegStoveTop = 0x1231,
    idtClickStoveTopFleetWaypointsTileDecrease = 0x1232,
    idtOurReturnTripWillTakeExtraYear = 0x1233,
    idtRead4Messages = 0x1234,
    idtSendNewDestroyerHackerWell = 0x1235,
    idtAvoidRepeatWorkSendingEveryNewFleet = 0x1236,
    idtSelectStoveTopControlClickHacker = 0x1237,
    idtNoticeProductionTileNewShipsWillRouted = 0x1238,
    idtRead3Messages = 0x1239,
    idtOpenResearchDialogSetFieldResearchEnergy = 0x123a,
    idtReadRestMessages7 = 0x123b,
    idtGotoTeamster7 = 0x123c,
    idtDoesntHaveEnoughFuelGetBackStove = 0x123d,
    idtGiveTeamster7OrdersScrapFleet = 0x123e,
    idt0575Blank = 0x123f,
    idtLetsFinishOffBerserkersOnceBuildingBombing = 0x1240,
    idtHitF4OpenShipDesigner4 = 0x1241,
    idtSelectAvailableHullTypesChooseB17 = 0x1242,
    idtAddRadiatingHydroRamScoopEngines = 0x1243,
    idtHoldShiftKeyDrag4BlackCat = 0x1244,
    idtOkDesignCloseShipDesigner = 0x1245,
    idtAdd10B17BombersStoveTops = 0x1246,
    idtGenerateWhenReady12 = 0x1247,
    idtCongratulationsYouveDeclaredWinner = 0x1248,
    idtTutorialWillContinueFewYearsGiveAdditional = 0x1249,
    idtReadFirst3MessagesGotoNewB = 0x124a,
    idtNoticeTheyveAlreadyRoutedHacker = 0x124b,
    idtReadRestMessages8 = 0x124c,
    idtEverythingElseAutomated = 0x124d,
    idtGenerateWhenYoureReady2 = 0x124e,
    idt0591Blank = 0x124f,
    idtReadFirst4MessagesGotoWallaby = 0x1250,
    idtTerraformingEffortHasFinallyPaidOffWed = 0x1251,
    idtAdd100MinesWallabysQueue = 0x1252,
    idtControlClickAddButtonAdd100Item = 0x1253,
    idtReadRestMessages9 = 0x1254,
    idtThereIsntAnythingPressingDoYearOur = 0x1255,
    idtGenerateWill2 = 0x1256,
    idt0599Blank = 0x1257,
    idtReadFirst3MessagesGotoDestroyer13 = 0x1258,
    idtNotice9DestroyersShownRedBarAbout = 0x1259,
    idtClickDestroyerFleetCompositionTile = 0x125a,
    idtIfRunningLeast800x600ModeWillSee = 0x125b,
    idtRead8MessagesViewAssaultEnemyStarbase = 0x125c,
    idtWellBerserkersShouldntBuildingAnyColonizersNotic = 0x125d,
    idtReadRestMessagesGenerate = 0x125e,
    idt0607Blank = 0x125f,
    idtReadFirst6MessagesGotoStoveTop = 0x1260,
    idtAddAnother10B17BombersProduction = 0x1261,
    idtHoldingPatternWaitingOurBombersArriveHacker = 0x1262,
    idtReadRestMessagesWatchBattles = 0x1263,
    idtGenerateWhenReady13 = 0x1264,
    idt0613Blank = 0x1265,
    idt0614Blank = 0x1266,
    idt0615Blank = 0x1267,
    idtNothingMuchHappeningYearViewBattleHacker = 0x1268,
    idtWillHaveDesignFasterShipUsingFaster = 0x1269,
    idtFirstBombersArriveYear = 0x126a,
    idtReadMessagesGenerateWhenReady = 0x126b,
    idt0620Blank = 0x126c,
    idt0621Blank = 0x126d,
    idt0622Blank = 0x126e,
    idt0623Blank = 0x126f,
    idtReadThroughMessages = 0x1270,
    idtN2B17BombersKilledFewEnemy = 0x1271,
    idtNewArrivalsSeveralYearsShouldMakeDifference = 0x1272,
    idtThereNumberThingsWeCouldDoOur = 0x1273,
    idtGenerateWhenReady14 = 0x1274,
    idt0629Blank = 0x1275,
    idt0630Blank = 0x1276,
    idt0631Blank = 0x1277,
    idtCongratulationsHaveReachedEndTutorial = 0x1278,
    idtHitF10ViewScoreNoticeBerserkersHave = 0x1279,
    idtNeedFinishBombingBerserkerPlanetsBuildShip = 0x127a,
    idtWillTrulyRuleGalaxy = 0x127b,
    idtReadMessages3 = 0x127c,
    idtWhenGenerateYoureOwn = 0x127d,
    idt0638Blank = 0x127e,
    idt0639Blank = 0x127f,
} TutorId;

typedef enum ButtonNotify {
    BN_CLICKED        = 0,
    BN_PAINT          = 1,
    BN_HILITE         = 2,
    BN_UNHILITE       = 3,
    BN_DISABLE        = 4,
    BN_DOUBLECLICKED  = 5,
} ButtonNotify;

typedef enum EditNotify {
    EN_SETFOCUS  = 0x0100,
    EN_KILLFOCUS = 0x0200,
    EN_CHANGE    = 0x0300,
    EN_UPDATE    = 0x0400,
    EN_ERRSPACE  = 0x0500,
    EN_MAXTEXT   = 0x0501,
    EN_HSCROLL   = 0x0601,
    EN_VSCROLL   = 0x0602,
} EditNotify;

typedef enum ListBoxNotify {
    LBN_ERRSPACE  = -2,
    LBN_SELCHANGE = 1,
    LBN_DBLCLK    = 2,
    LBN_SELCANCEL = 3,
    LBN_SETFOCUS  = 4,
    LBN_KILLFOCUS = 5,
} ListBoxNotify;

typedef enum ComboBoxNotify {
    CBN_ERRSPACE     = -1,
    CBN_SELCHANGE    = 1,
    CBN_DBLCLK       = 2,
    CBN_SETFOCUS     = 3,
    CBN_KILLFOCUS    = 4,
    CBN_EDITCHANGE   = 5,
    CBN_EDITUPDATE   = 6,
    CBN_DROPDOWN     = 7,
    CBN_CLOSEUP      = 8,
    CBN_SELENDOK     = 9,
    CBN_SELENDCANCEL = 10,
} ComboBoxNotify;

typedef enum ScrollCode {
    SB_LINEUP        = 0,
    SB_LINELEFT      = 0,
    SB_LINEDOWN      = 1,
    SB_LINERIGHT     = 1,
    SB_PAGEUP        = 2,
    SB_PAGELEFT      = 2,
    SB_PAGEDOWN      = 3,
    SB_PAGERIGHT     = 3,
    SB_THUMBPOSITION = 4,
    SB_THUMBTRACK    = 5,
    SB_TOP           = 6,
    SB_LEFT          = 6,
    SB_BOTTOM        = 7,
    SB_RIGHT         = 7,
    SB_ENDSCROLL     = 8,
} ScrollCode;

// Windows enums for easier debugging
typedef enum WMType {
    WM_NULL = 0x0000,
    WM_CREATE = 0x0001,
    WM_DESTROY = 0x0002,
    WM_MOVE = 0x0003,
    WM_SIZE = 0x0005,
    WM_ACTIVATE = 0x0006,
    WM_SETFOCUS = 0x0007,
    WM_KILLFOCUS = 0x0008,
    WM_ENABLE = 0x000A,
    WM_SETREDRAW = 0x000B,
    WM_SETTEXT = 0x000C,
    WM_GETTEXT = 0x000D,
    WM_GETTEXTLENGTH = 0x000E,
    WM_PAINT = 0x000F,
    WM_CLOSE = 0x0010,
    WM_QUERYENDSESSION = 0x0011,
    WM_QUIT = 0x0012,
    WM_QUERYOPEN = 0x0013,
    WM_ERASEBKGND = 0x0014,
    WM_SYSCOLORCHANGE = 0x0015,
    WM_ENDSESSION = 0x0016,
    WM_SYSTEMERROR = 0x0017,
    WM_SHOWWINDOW = 0x0018,
    WM_CTLCOLOR = 0x0019,
    WM_WININICHANGE = 0x001A,
    WM_DEVMODECHANGE = 0x001B,
    WM_ACTIVATEAPP = 0x001C,
    WM_FONTCHANGE = 0x001D,
    WM_TIMECHANGE = 0x001E,
    WM_CANCELMODE = 0x001F,
    WM_SETCURSOR = 0x0020,
    WM_MOUSEACTIVATE = 0x0021,
    WM_CHILDACTIVATE = 0x0022,
    WM_QUEUESYNC = 0x0023,
    WM_GETMINMAXINFO = 0x0024,
    WM_ICONERASEBKGND = 0x0027,
    WM_NEXTDLGCTL = 0x0028,
    WM_SPOOLERSTATUS = 0x002A,
    WM_DRAWITEM = 0x002B,
    WM_MEASUREITEM = 0x002C,
    WM_DELETEITEM = 0x002D,
    WM_VKEYTOITEM = 0x002E,
    WM_CHARTOITEM = 0x002F,
    WM_SETFONT = 0x0030,
    WM_GETFONT = 0x0031,
    WM_QUERYDRAGICON = 0x0037,
    WM_COMPAREITEM = 0x0039,
    WM_COMPACTING = 0x0041,
    WM_COMMNOTIFY = 0x0044,
    WM_WINDOWPOSCHANGING = 0x0046,
    WM_WINDOWPOSCHANGED = 0x0047,
    WM_POWER = 0x0048,

    WM_NCMOUSEMOVE = 0x00A0,
    WM_NCLBUTTONDOWN = 0x00A1,
    WM_NCLBUTTONUP = 0x00A2,
    WM_NCLBUTTONDBLCLK = 0x00A3,
    WM_NCRBUTTONDOWN = 0x00A4,
    WM_NCRBUTTONUP = 0x00A5,
    WM_NCRBUTTONDBLCLK = 0x00A6,
    WM_NCMBUTTONDOWN = 0x00A7,
    WM_NCMBUTTONUP = 0x00A8,
    WM_NCMBUTTONDBLCLK = 0x00A9,

    WM_KEYDOWN = 0x0100,
    WM_KEYUP = 0x0101,
    WM_CHAR = 0x0102,
    WM_DEADCHAR = 0x0103,
    WM_SYSKEYDOWN = 0x0104,
    WM_SYSKEYUP = 0x0105,
    WM_SYSCHAR = 0x0106,
    WM_SYSDEADCHAR = 0x0107,

    WM_INITDIALOG = 0x0110,
    WM_COMMAND = 0x0111,
    WM_SYSCOMMAND = 0x0112,
    WM_TIMER = 0x0113,
    WM_HSCROLL = 0x0114,
    WM_VSCROLL = 0x0115,
    WM_INITMENU = 0x0116,
    WM_INITMENUPOPUP = 0x0117,
    WM_MENUSELECT = 0x011F,
    WM_MENUCHAR = 0x0120,
    WM_ENTERIDLE = 0x0121,

    WM_MOUSEMOVE = 0x0200,
    WM_LBUTTONDOWN = 0x0201,
    WM_LBUTTONUP = 0x0202,
    WM_LBUTTONDBLCLK = 0x0203,
    WM_RBUTTONDOWN = 0x0204,
    WM_RBUTTONUP = 0x0205,
    WM_RBUTTONDBLCLK = 0x0206,
    WM_MBUTTONDOWN = 0x0207,
    WM_MBUTTONUP = 0x0208,
    WM_MBUTTONDBLCLK = 0x0209,

    WM_PARENTNOTIFY = 0x0210,

    WM_MDICREATE = 0x0220,
    WM_MDIDESTROY = 0x0221,
    WM_MDIACTIVATE = 0x0222,
    WM_MDIRESTORE = 0x0223,
    WM_MDINEXT = 0x0224,
    WM_MDIMAXIMIZE = 0x0225,
    WM_MDITILE = 0x0226,
    WM_MDICASCADE = 0x0227,
    WM_MDIICONARRANGE = 0x0228,
    WM_MDIGETACTIVE = 0x0229,
    WM_MDISETMENU = 0x0230,
    WM_DROPFILES = 0x0233,

    WM_CUT = 0x0300,
    WM_COPY = 0x0301,
    WM_PASTE = 0x0302,
    WM_CLEAR = 0x0303,
    WM_UNDO = 0x0304,
    WM_RENDERFORMAT = 0x0305,
    WM_RENDERALLFORMATS = 0x0306,
    WM_DESTROYCLIPBOARD = 0x0307,
    WM_DRAWCLIPBOARD = 0x0308,
    WM_PAINTCLIPBOARD = 0x0309,
    WM_VSCROLLCLIPBOARD = 0x030A,
    WM_SIZECLIPBOARD = 0x030B,
    WM_ASKCBFORMATNAME = 0x030C,
    WM_CHANGECBCHAIN = 0x030D,
    WM_HSCROLLCLIPBOARD = 0x030E,
    WM_QUERYNEWPALETTE = 0x030F,
    WM_PALETTEISCHANGING = 0x0310,
    WM_PALETTECHANGED = 0x0311,

    WM_PENWINFIRST = 0x0380,
    WM_PENWINLAST = 0x038F,
    WM_COALESCE_FIRST = 0x0390,
    WM_COALESCE_LAST = 0x039F,

    WM_USER = 0x0400,

    // combo box messages
    CB_LIMITTEXT = 0x0401,
    CB_ADDSTRING = 0x0403,
    CB_DELETESTRING = 0x0404,
    CB_DIR = 0x0405,
    CB_GETCOUNT = 0x0406,
    CB_GETCURSEL = 0x0407,
    CB_GETLBTEXT = 0x0408,
    CB_GETLBTEXTLEN = 0x0409,
    CB_INSERTSTRING = 0x040A,
    CB_RESETCONTENT = 0x040B,
    CB_SETCURSEL = 0x040E,
    CB_SETEXTENDEDUI = 0x0415,
    CB_FINDSTRINGEXACT = 0x0418,

    WM_STARS_STARTUP = 0x0464,
    WM_STARS_HOST = 0x0465,
    WM_STARS_CONTINUE = 0x0466,
} WMType;

typedef enum CtlColorType {
    CTLCOLOR_MSGBOX    = 0,
    CTLCOLOR_EDIT      = 1,
    CTLCOLOR_LISTBOX   = 2,
    CTLCOLOR_BTN       = 3,
    CTLCOLOR_DLG       = 4,
    CTLCOLOR_SCROLLBAR = 5,
    CTLCOLOR_STATIC    = 6
} CtlColorType;

typedef enum MessageBoxType {
    /* buttons */
    MB_OK = 0x0000,
    MB_OKCANCEL = 0x0001,
    MB_ABORTRETRYIGNORE = 0x0002,
    MB_YESNOCANCEL = 0x0003,
    MB_YESNO = 0x0004,
    MB_RETRYCANCEL = 0x0005,

    /* icons */
    MB_ICONHAND = 0x0010, /* stop / error */
    MB_ICONQUESTION = 0x0020,
    MB_ICONEXCLAMATION = 0x0030,
    MB_ICONASTERISK = 0x0040,

    /* default button */
    // MB_DEFBUTTON1 = 0x0000,
    MB_DEFBUTTON2 = 0x0100,
    MB_DEFBUTTON3 = 0x0200,

    /* modality */
    // MB_APPLMODAL = 0x0000,
    MB_SYSTEMMODAL = 0x1000,
    MB_TASKMODAL = 0x2000,

    /* Win16-specific / misc */
    MB_HELP = 0x4000,
    MB_NOFOCUS = 0x8000
} MessageBoxType;


typedef enum GrStat {
    grStatFuel = 1,
    grStatCargo = 2,

} GrStat;

typedef enum iengine {
    iengineSettlersDelight = 0,
    iengineQuickJump5 = 1,
    iengineFuelMizer = 2,
    iengineLongHump6 = 3,
    iengineDaddyLongLegs7 = 4,
    iengineAlphaDrive8 = 5,
    iengineTransGalacticDrive = 6,
    iengineInterspace10 = 7,
    iengineEnigmaPulsar = 8,
    iengineTransStar10 = 9,
    iengineRadiatingHydroRamScoop = 10,
    iengineSubGalacticFuelScoop = 11,
    iengineTransGalacticFuelScoop = 12,
    iengineTransGalacticSuperScoop = 13,
    iengineTransGalacticMizerScoop = 14,
    iengineGalaxyScoop = 15,
    iengineCount = 16,
} iengine;

typedef enum iarmor {
    iarmorTritanium = 0,
    iarmorCrobmnium = 1,
    iarmorCarbonicArmor = 2,
    iarmorStrobnium = 3,
    iarmorOrganicArmor = 4,
    iarmorKelarium = 5,
    iarmorFieldedKelarium = 6,
    iarmorDepletedNeutronium = 7,
    iarmorNeutronium = 8,
    iarmorMegaPolyShell = 9,
    iarmorValanium = 10,
    iarmorSuperlatanium = 11,
    iarmorCount = 12,
} iarmor;

typedef enum iscanner {
    iscannerBatScanner = 0,
    iscannerRhinoScanner = 1,
    iscannerMoleScanner = 2,
    iscannerDNAScanner = 3,
    iscannerPossumScanner = 4,
    iscannerPickPocketScanner = 5,
    iscannerChameleonScanner = 6,
    iscannerFerretScanner = 7,
    iscannerDolphinScanner = 8,
    iscannerGazelleScanner = 9,
    iscannerRNAScanner = 10,
    iscannerCheetahScanner = 11,
    iscannerElephantScanner = 12,
    iscannerEagleEyeScanner = 13,
    iscannerRobberBaronScanner = 14,
    iscannerPeerlessScanner = 15,
    iscannerCount = 16,
} iscanner;

typedef enum ishield {
    ishieldMoleSkinShield = 0,
    ishieldCowHideShield = 1,
    ishieldWolverineDiffuseShield = 2,
    ishieldCrobySharmor = 3,
    ishieldShadowShield = 4,
    ishieldBearNeutrinoBarrier = 5,
    ishieldLangstonShell = 6,
    ishieldGorillaDelagator = 7,
    ishieldElephantHideFortress = 8,
    ishieldCompletePhaseShield = 9,
    ishieldCount = 10,
} ishield;

typedef enum ispecialE {
    ispecialETransportCloaking = 0,
    ispecialEStealthCloak = 1,
    ispecialESuperStealthCloak = 2,
    ispecialEUltraStealthCloak = 3,
    ispecialEMultiFunctionPod = 4,
    ispecialEBattleComputer = 5,
    ispecialEBattleSuperComputer = 6,
    ispecialEBattleNexus = 7,
    ispecialEJammer10 = 8,
    ispecialEJammer20 = 9,
    ispecialEJammer30 = 10,
    ispecialEJammer50 = 11,
    ispecialEEnergyCapacitor = 12,
    ispecialEFluxCapacitor = 13,
    ispecialEEnergyDampener = 14,
    ispecialETachyonDetector = 15,
    ispecialEAntiMatterGenerator = 16,
    ispecialECount = 17,
} ispecialE;

typedef enum ispecialM {
    ispecialMColonizationModule = 0,
    ispecialMOrbitalConstructionModule = 1,
    ispecialMCargoPod = 2,
    ispecialMSuperCargoPod = 3,
    ispecialMMultiCargoPod = 4,
    ispecialMFuelTank = 5,
    ispecialMSuperFuelTank = 6,
    ispecialMManeuveringJet = 7,
    ispecialMOverthruster = 8,
    ispecialMJumpGate = 9,
    ispecialMBeamDeflector = 10,
    ispecialMCount = 11,
} ispecialM;

typedef enum imines {
    iminesMineDispenser40 = 0,
    iminesMineDispenser50 = 1,
    iminesMineDispenser80 = 2,
    iminesMineDispenser130 = 3,
    iminesHeavyDispenser50 = 4,
    iminesHeavyDispenser110 = 5,
    iminesHeavyDispenser200 = 6,
    iminesSpeedTrap20 = 7,
    iminesSpeedTrap30 = 8,
    iminesSpeedTrap50 = 9,
    iminesCount = 10,
} imines;

typedef enum imining {
    iminingRoboMidgetMiner = 0,
    iminingRoboMiniMiner = 1,
    iminingRoboMiner = 2,
    iminingRoboMaxiMiner = 3,
    iminingRoboSuperMiner = 4,
    iminingRoboUltraMiner = 5,
    iminingAlienMiner = 6,
    iminingOrbitalAdjuster = 7,
    iminingCount = 8,
} imining;

typedef enum iplanetary {
    iplanetaryViewer50 = 0,
    iplanetaryViewer90 = 1,
    iplanetaryScoper150 = 2,
    iplanetaryScoper220 = 3,
    iplanetaryScoper280 = 4,
    iplanetarySnooper320X = 5,
    iplanetarySnooper400X = 6,
    iplanetarySnooper500X = 7,
    iplanetarySnooper620X = 8,
    iplanetarySDI = 9,
    iplanetaryMissileBattery = 10,
    iplanetaryLaserBattery = 11,
    iplanetaryPlanetaryShield = 12,
    iplanetaryNeutronShield = 13,
    iplanetaryGenesisDevice = 14,
    iplanetaryCount = 15,
} iplanetary;

typedef enum iterra {
    iterraTotalTerraform3 = 0,
    iterraTotalTerraform5 = 1,
    iterraTotalTerraform7 = 2,
    iterraTotalTerraform10 = 3,
    iterraTotalTerraform15 = 4,
    iterraTotalTerraform20 = 5,
    iterraTotalTerraform25 = 6,
    iterraTotalTerraform30 = 7,
    iterraGravityTerraform3 = 8,
    iterraGravityTerraform7 = 9,
    iterraGravityTerraform11 = 10,
    iterraGravityTerraform15 = 11,
    iterraTempTerraform3 = 12,
    iterraTempTerraform7 = 13,
    iterraTempTerraform11 = 14,
    iterraTempTerraform15 = 15,
    iterraRadiationTerraform3 = 16,
    iterraRadiationTerraform7 = 17,
    iterraRadiationTerraform11 = 18,
    iterraRadiationTerraform15 = 19,
    iterraCount = 20,
} iterra;

typedef enum ibomb {
    ibombLadyFingerBomb = 0,
    ibombBlackCatBomb = 1,
    ibombM70Bomb = 2,
    ibombM80Bomb = 3,
    ibombCherryBomb = 4,
    ibombLBU17Bomb = 5,
    ibombLBU32Bomb = 6,
    ibombLBU74Bomb = 7,
    ibombHushABoom = 8,
    ibombRetroBomb = 9,
    ibombSmartBomb = 10,
    ibombNeutronBomb = 11,
    ibombEnrichedNeutronBomb = 12,
    ibombPeerlessBomb = 13,
    ibombAnnihilatorBomb = 14,
    ibombCount = 15,
} ibomb;

typedef enum itorp {
    itorpAlphaTorpedo = 0,
    itorpBetaTorpedo = 1,
    itorpDeltaTorpedo = 2,
    itorpEpsilonTorpedo = 3,
    itorpRhoTorpedo = 4,
    itorpUpsilonTorpedo = 5,
    itorpOmegaTorpedo = 6,
    itorpAntiMatterTorpedo = 7,
    itorpJihadMissile = 8,
    itorpJuggernautMissile = 9,
    itorpDoomsdayMissile = 10,
    itorpArmageddonMissile = 11,
    itorpCount = 12,
} itorp;

typedef enum ibeam {
    ibeamLaser = 0,
    ibeamXRayLaser = 1,
    ibeamMiniGun = 2,
    ibeamYakimoraLightPhaser = 3,
    ibeamBlackjack = 4,
    ibeamPhaserBazooka = 5,
    ibeamPulsedSapper = 6,
    ibeamColloidalPhaser = 7,
    ibeamGatlingGun = 8,
    ibeamMiniBlaster = 9,
    ibeamBludgeon = 10,
    ibeamMarkIVBlaster = 11,
    ibeamPhasedSapper = 12,
    ibeamHeavyBlaster = 13,
    ibeamGatlingNeutrinoCannon = 14,
    ibeamMyopicDisruptor = 15,
    ibeamBlunderbuss = 16,
    ibeamDisruptor = 17,
    ibeamMultiContainedMunition = 18,
    ibeamSyncroSapper = 19,
    ibeamMegaDisruptor = 20,
    ibeamBigMuthaCannon = 21,
    ibeamStreamingPulverizer = 22,
    ibeamAntiMatterPulverizer = 23,
    ibeamCount = 24,
} ibeam;

typedef enum ispecialSB {
    ispecialSBStargate100250 = 0,
    ispecialSBStargateAny300 = 1,
    ispecialSBStargate150600 = 2,
    ispecialSBStargate300500 = 3,
    ispecialSBStargate100Any = 4,
    ispecialSBStargateAny800 = 5,
    ispecialSBStargateAnyAny = 6,
    ispecialSBMassDriver5 = 7,
    ispecialSBMassDriver6 = 8,
    ispecialSBMassDriver7 = 9,
    ispecialSBSuperDriver8 = 10,
    ispecialSBSuperDriver9 = 11,
    ispecialSBUltraDriver10 = 12,
    ispecialSBUltraDriver11 = 13,
    ispecialSBUltraDriver12 = 14,
    ispecialSBUltraDriver13 = 15,
    ispecialSBCount = 16,
} ispecialSB;

typedef enum GrbitTrader {
    grbitTraderNone = 0x0000,
    grbitTraderCargo = 0x0001,
    grbitTraderSpecial = 0x0002,
    grbitTraderShield = 0x0004,
    grbitTraderArmor = 0x0008,
    grbitTraderMiner = 0x0010,
    grbitTraderBomb = 0x0020,
    grbitTraderTorp = 0x0040,
    grbitTraderBeam = 0x0080,
    grbitTraderHull = 0x0100,
    grbitTraderEngine = 0x0200,
    grbitTraderGenesis = 0x0400,
    grbitTraderJumpgate = 0x0800,
    grbitTraderLifeboat = 0x1000,
    grbitTraderAll = 0x1fff,
} GrbitTrader;

typedef enum LookupResult {
    LookupInvalid = 0,     // “out of range” / not a valid part id in group
    LookupDisallowed = -1, // disallowed for race/trait/other rule
    LookupOk = 1,          // meets tech reqs (original CheckTechRequirements == 1)
    LookupNear = 2,        // “one level away in current research field”
    LookupNeedMany = 99    // multiple tech deficits
} LookupResult;

typedef enum RecordType {
    /*
     * NOTE: Stars! file records encode a 6-bit "record type" (rt) plus a 10-bit
     * byte count (cb) in a 16-bit header word.
     *
     * In .HST files (and others), record type 0x00 is used for the footer record
     * (cb=2, data=0000). The original code treats "rt==0" as a terminator while
     * reading, so we keep rtEOF=0 for that behavior.
     */
    rtEOF = 0,
    rtLogCargoXfer8 = 1,         /* quantities are int8  (lpb[6+iLook]) */
    rtLogCargoXfer16 = 2,        /* quantities are int16 (lpb[6+2*iLook]) */
    rtLogFleetOrderDelete = 3,   /* delete 1 or 2 orders; index in *(u16*)(lpb+2), high bit => delete extra */
    rtLogFleetOrderInsert = 4,   /* insert new order at index *(i16*)(lpb+2); payload from lpb+4 */
    rtLogFleetOrderUpdate = 5,   /* overwrite existing order at index *(i16*)(lpb+2); payload from lpb+4 */
    rtPlr = 6,                   /* Player */
    rtGame = 7,                  /* Game */
    rtBOF = 8,                   /* FileHeader / BOF */
    rtLogFleetFlagBit9 = 10,     /* lpfl->wFlags_0x4 bit 9 set/cleared by (*(u16*)(lpb+2) & 1) */
    rtLogFleetOrderAttrNib = 11, /* order[index].word10 low nibble set to (*(i16*)(lpb+4) & 0xF), value constrained <=9 */
    rtMsg = 12,                  /* Message */
    rtPlanet = 13,
    rtPlanetB = 14,
    rtFleetA = 16,
    rtOrderA = 19, /* other order-like record type seen in decompile */
    rtOrderB = 20, // waypoint only
    rtString = 21, /* decompile: alloc/copy string from rgbCur when rt == 0x15 */
    rtSel = 22,    /* decompile: after things, if (rt == 0x16) ReadRt(); matches file.c rtSel */
    rtLogFleetCargoXfer = 23,
    rtLogFleetSplit = 24,  /* LpflNewSplit(&fleet) */
    rtLogCargoXfer32 = 25, /* quantities are int32 (lpb[6+4*iLook]) */
    rtShDef = 26,
    rtLogShDef = 27, /* Ship design definition (SHDEF) create/update/delete for the current player. */
    rtProdQ = 28,
    rtLogPlanetProdQ = 29,   /* Planet production queue set/clear (planet->lpplprod). */
    rtBtlPlan = 30,          /* decompile: while (rt == 0x1e) { ...battle plan... } */
    rtBtlData = 31,          /* decompile: while (rt == 0x1f || rt == 0x27) { ... } */
    rtContinue = 39,         /* decompile: inside loop: if (rt != 0x27) { ... } matches `rt != rtContinue` */
    rtHistHdr = 32,          /* decompile: after opening dtHist, expects rt == 0x20 */
    rtMsgFilt = 33,          /* decompile: checks cbbitfMsg vs cb and memcpy(bitfMsgFiltered, ...) */
    rtLogResearch = 34,      /* Research settings: pctResearch + iTechCur (packed nibble fields). */
    rtLogPlanetRouting = 35, /* Planet routing / starbase / infrastructure bitfields mutation. */
    rtLogRelations = 38,     /* memcpy rgplr[idPlayer].rgmdRelation[0..cPlayer) */
    rtChgPassword = 36,      /* file.c: if (hdrCur.rt == rtChgPassword) { lSaltCur = *(long*)rgbCur; } */
    rtLogFleetMerge = 37,    /* merge all-at-location (cb==2) or merge listed fleet ids (cb>2) */
    rtPlrMsg = 40,
    rtAiData = 41,            /* decompile: loop skips/reads while (rt == 0x29) around vlpbAiData */
    rtThing = 43,             /* decompile: if (rt == 0x2b) { cThing = rgbCur; alloc things } */
    rtLogFleetPlan = 42,      /* lpfl->iplan = *(u16*)(lpb+2) truncated */
    rtLogThingByteParam = 43, /* sets 1 byte inside THING union for a restricted subtype */
    rtLogFleetName = 44,      /* User string (fleet rename); may be compressed via FDecompressUserString. */
    rtScore = 45,             /* decompile: loop `if (rt != 0x2d) break;` in score load path */
    rtLogPlayerZpq1 = 46,     /* Host-only opaque blob (size capped at 0x1A bytes) copied into rgplr[idPlayer].zpq1. */
    rtMax = 47                /* one past highest observed (0x2d) */
} RecordType;

typedef enum cbStructSize {
    cbABC = 6,
    cbAIHIST = 1284,
    cbAIPART = 2,
    cbAISTARBASE = 20,
    cbARMOR = 54,
    cbBEAM = 60,
    cbBITMAP = 14,
    cbBITMAPCOREHEADER = 12,
    cbBITMAPCOREINFO = 15,
    cbBITMAPFILEHEADER = 14,
    cbBITMAPINFO = 44,
    cbBITMAPINFOHEADER = 40,
    cbBOMB = 58,
    cbBTLDATA = 14,
    cbBTLPLAN = 36,
    cbBTLREC = 6,
    cbBTLREC26 = 6,
    cbBTN = 14,
    cbBTNT = 24,
    cbCBTACTIVATESTRUCT = 4,
    cbCHOOSECOLOR = 32,
    cbCHOOSEFONT = 46,
    cbCLIENTCREATESTRUCT = 4,
    cbCOLDROP = 12,
    cbCOMPAREITEMSTRUCT = 18,
    cbCOMPART = 52,
    cbCOMPLEX = 16,
    cbCOMPLEXL = 20,
    cbCOMSTAT = 5,
    cbCREATESTRUCT = 34,
    cbCYBERINFO = 2,
    cbCYBERINFOTEMP = 2,
    cbDCB = 25,
    cbDEBUGHOOKINFO = 14,
    cbDELETEITEMSTRUCT = 12,
    cbDEVNAMES = 8,
    cbDOCINFO = 10,
    cbDRAWCIR = 24,
    cbDRAWITEMSTRUCT = 26,
    cbDRIVERINFOSTRUCT = 134,
    cbDRVCONFIGINFO = 12,
    cbDV = 2,
    cbENGINE = 78,
    cbENUMLOGFONT = 146,
    cbEVENTMSG = 10,
    cbEXCEPTION = 28,
    cbEXCEPTIONL = 34,
    cbFINDREPLACE = 36,
    cbFIXED = 4,
    cbFLEET = 124,
    cbFLEETID = 2,
    cbFLEETSOME = 12,
    cbFRAMESTUFF = 22,
    cbGAME = 64,
    cbGDATA = 10,
    cbGLYPHMETRICS = 12,
    cbHANDLETABLE = 2,
    cbHARDWAREHOOKSTRUCT = 10,
    cbHB = 16,
    cbHDR = 2,
    cbHELPWININFO = 14,
    cbHS = 4,
    cbHUL = 123,
    cbHULDEF = 143,
    cbINI = 26,
    cbITEMACTION = 2,
    cbKERNINGPAIR = 6,
    cbKILL = 8,
    cbLOGBRUSH = 8,
    cbLOGFONT = 50,
    cbLOGPALETTE = 8,
    cbLOGPEN = 10,
    cbLOGXFER = 24,
    cbLOGXFERF = 36,
    cbLSB = 4,
    cbMAT2 = 16,
    cbMDICREATESTRUCT = 26,
    cbMDPLR = 2,
    cbMEASUREITEMSTRUCT = 14,
    cbMENUITEMTEMPLATE = 5,
    cbMENUITEMTEMPLATEHEADER = 4,
    cbMETAFILEPICT = 8,
    cbMETAHEADER = 18,
    cbMETARECORD = 8,
    cbMINES = 54,
    cbMINING = 54,
    cbMINMAXINFO = 20,
    cbMOUSEHOOKSTRUCT = 12,
    cbMSG = 18,
    cbMSGBIG = 18,
    cbMSGHDR = 4,
    cbMSGPLR = 12,
    cbMSGTURN = 5,
    cbMULTIKEYHELP = 4,
    cbNEWTEXTMETRIC = 41,
    cbOBJ = 2,
    cbOFN = 72,
    cbOFSTRUCT = 136,
    cbORDER = 18,
    cbOUTLINETEXTMETRIC = 114,
    cbPAINTSTRUCT = 32,
    cbPALETTEENTRY = 4,
    cbPANOSE = 10,
    cbPART = 8,
    cbPD = 52,
    cbPL = 4,
    cbPLANET = 56,
    cbPLANETARY = 54,
    cbPLANETMINIMAL = 6,
    cbPLANETSOME = 23,
    cbPLAYER = 192,
    cbPLORD = 4,
    cbPLPROD = 4,
    cbPOINT = 4,
    cbPOINTFX = 8,
    cbPOPUPDATA = 22,
    cbPROD = 4,
    cbPRODQ1 = 2,
    cbRECT = 8,
    cbRGBQUAD = 4,
    cbRGBTRIPLE = 3,
    cbRPT = 54,
    cbRTBOF = 16,
    cbRTCHGNAME = 37,
    cbRTCHGPLANETLONG = 6,
    cbRTCHGPRODQ = 2,
    cbRTCHGSHDEF = 19,
    cbRTHISTHDR = 4,
    cbRTLOGHDR = 17,
    cbRTLOGTHING = 4,
    cbRTPLANET = 4,
    cbRTSHDEF = 17,
    cbRTSHIPINT = 4,
    cbRTSHIPINT2 = 6,
    cbRTWAYPT = 22,
    cbRTXFER = 7,
    cbRTXFERF = 9,
    cbRTXFERL = 10,
    cbRTXFERX = 8,
    cbSBAR = 12,
    cbSCAN = 16,
    cbSCANNER = 56,
    cbSCORE = 20,
    cbSCOREX = 24,
    cbSEGINFO = 16,
    cbSEL = 226,
    cbSELSOME = 28,
    cbSHDEF = 147,
    cbSHIELD = 54,
    cbSIZE = 4,
    cbSPECIAL = 54,
    cbSPECIALSB = 56,
    cbSTARPACK = 4,
    cbTASKLAYMINES = 4,
    cbTASKPATROL = 4,
    cbTASKSELL = 2,
    cbTASKXPORT = 10,
    cbTERRA = 54,
    cbTEXTMETRIC = 31,
    cbTHING = 18,
    cbTHMINE = 10,
    cbTHPACK = 10,
    cbTHTRADER = 10,
    cbTHWORM = 8,
    cbTILE = 16,
    cbTIMER = 10,
    cbTIMERINFO = 12,
    cbTOK = 29,
    cbTORP = 60,
    cbTTPOLYCURVE = 12,
    cbTTPOLYGONHEADER = 16,
    cbTURNSERIAL = 16,
    cbTUTOR = 44,
    cbVERS = 2,
    cbWINDEBUGINFO = 26,
    cbWINDOWPLACEMENT = 22,
    cbWINDOWPOS = 14,
    cbWN = 10,
    cbWNDCLASS = 26,
    cbXFER = 128,
    cbXFERFULL = 25,
    cbZIPORDER = 24,
    cbZIPPRODQ = 40,
    cbZIPPRODQ1 = 26,
    cbcomplex = 16,
} cbStructSize;

typedef enum DialogId {
    /* ship / fleet */
    IDD_MERGE_FLEETS = 82, /* MergeFleetsDlg */
    IDD_TRANSFER = 91,     /* TransferDlg */
    IDD_SLOT = 92,         /* SlotDlg */
    IDD_PRODUCTION = 93,   /* ProductionDlg */
    IDD_ORDER_INFO = 97,   /* OrderInfoDlg */

    /* common / utility */
    IDD_GENERIC_SMALL = 86, /* reused generic dialog (PrintMap, msg, browser host) */
    IDD_ZIP_PROD = 89,      /* ZipProdDlg / ZipOrderDlg */
    IDD_ABOUT = 90,         /* About */
    IDD_HOST_MODE = 115,    /* HostOptionsDialog */
    IDD_PASSWORD = 140,     /* PASSWORD dialog */
    IDD_NEW_PASSWORD = 141, /* NewPasswordDlg */

    /* research / browser */
    IDD_RESEARCH = 127, /* ResearchDlg */
    IDD_BROWSER = 128,  /* Browser child dialog */

    /* race wizard */
    IDD_RACE_WIZARD_1 = 146, /* RaceWizardDlg1 */
    IDD_RACE_WIZARD_2 = 147, /* unnamed proc (slot between 1 and 3) */
    IDD_RACE_WIZARD_3 = 148, /* RaceWizardDlg3 */
    IDD_RACE_WIZARD_4 = 149, /* RaceWizardDlg4 */
    IDD_RACE_WIZARD_5 = 150, /* RaceWizardDlg5 */
    IDD_RACE_WIZARD_6 = 151, /* RaceWizardDlg6 */

    /* VCR */
    IDD_VCR = 160, /* VCRDlg */

    /* new game */
    IDD_SIMPLE_NEW_GAME = 209, /* SimpleNewGameDlg */
    IDD_NEW_GAME_1 = 390,      /* NewGameDlg */
    IDD_NEW_GAME_2 = 391,      /* NewGameDlg2 */
    IDD_NEW_GAME_3 = 392,      /* NewGameDlg3 */

    IDD_Gauge = 393, /* never seen invoked */

    /* host / options */
    IDD_HOST_OPTIONS = 1026, /* HostOptionsDialog */
    IDD_SCORE = 102,         /* ScoreXDlg */

    /* battle / plans */
    IDD_BATTLE_PLANS = 2013, /* BattlePlansDlg */
    IDD_RENAME = 2019,       /* RenameDlg / NewPlanNameDlg (shared template) */

    /* relations / diplomacy */
    IDD_RELATIONS = 2008, /* RelationsDlg */

    IDD_SAVE_TURN1 = 1068, /* Save  */
    IDD_SAVE_TURN2 = 2025, /* Save  */

    /* tutorial / panic */
    IDD_PANIC = 2504, /* PanicDlg */
    IDD_TUTOR = 2502, /* never referenced */

    /* find */
    IDD_FIND = 4202, /* FindDlg */
} DialogId;

typedef enum ControlId {
    IDOK = 1,
    IDCANCEL = 2,
    IDHELP = 9,
    IDC_HELP = 118,

    // race wizard
    IDC_EDIT1 = 268,
    IDC_EDITNAME = 2075,
    IDC_RADRACE1 = 271,

    IDC_IMMUNE_TO_TEMPERATURE = 292,
    IDC_IMMUNE_TO_RADIATION = 293,
    IDC_RENAME = 1051,
    IDC_SAVE = 1065,
    IDC_NO_DON_T_SAVE = 1067,
    IDC_NEXT = 1071,
    IDC_FINISH = 1072,
    IDC_DELETE = 2071,

    IDC_IMPORT = 2070,
    IDC_EDIT = 2072,

    IDC_PREV = 2064,
    IDC_NEXT2 = 2065,
    IDC_FIRST = 2066,
    IDC_LAST = 2067,
    IDC_UP = 2068,
    IDC_DOWN = 2069,

    IDC_SHIPLIST = 1035,
    IDC_EDITTEXT = 0x010C, /* 268 */

    IDC_U16_0x0051 = 0x0051, /* 81 */
    IDC_U16_0x008B = 0x008B, /* 139 */
    IDC_U16_0x00A1 = 0x00A1, /* 161 */
    IDC_U16_0x00A3 = 0x00A3, /* 163 */
    IDC_U16_0x00C6 = 0x00C6, /* 198 */
    IDC_U16_0x00CB = 0x00CB, /* 203 */
    IDC_U16_0x00D3 = 0x00D3, /* 211 */

    IDC_U16_0x010B = 0x010B, /* 267 */
    IDC_U16_0x010D = 0x010D, /* 269 */
    IDC_U16_0x0116 = 0x0116, /* 278 */
    IDC_U16_0x0118 = 0x0118, /* 280 */
    IDC_U16_0x0123 = 0x0123, /* 291 */
    IDC_U16_0x0130 = 0x0130, /* 304 */

    IDC_U16_0x0406 = 0x0406, /* 1030 */
    IDC_U16_0x0416 = 0x0416, /* 1046 */
    IDC_U16_0x0417 = 0x0417, /* 1047 */
    IDC_U16_0x041A = 0x041A, /* 1050 */
    IDC_U16_0x041D = 0x041D, /* 1053 */
    IDC_U16_0x041E = 0x041E, /* 1054 */
    IDC_U16_0x041F = 0x041F, /* 1055 */
    IDC_U16_0x0420 = 0x0420, /* 1056 */
    IDC_U16_0x0421 = 0x0421, /* 1057 */
    IDC_U16_0x0422 = 0x0422, /* 1058 */
    IDC_U16_0x042E = 0x042E, /* 1070 */

    IDC_U16_0x0434 = 0x0434, /* 1076 */

    IDC_U16_0x07D3 = 0x07D3, /* 2003 */
    IDC_U16_0x07D5 = 0x07D5, /* 2005 */
    IDC_U16_0x07D6 = 0x07D6, /* 2006 */

    IDC_U16_0x07DF = 0x07DF, /* 2015 */
    IDC_U16_0x07E0 = 0x07E0, /* 2016 */
    IDC_U16_0x07E1 = 0x07E1, /* 2017 */
    IDC_U16_0x07E2 = 0x07E2, /* 2018 */

    IDC_U16_0x080C = 0x080C, /* 2060 */

    IDC_COMBOBOX = 0x081A,

} ControlId;

typedef enum WParamMessageId {
    IDM_DEBUG_DUMP_FLEETS = 0x0053,   // DumpFleets()
    IDM_DEBUG_DUMP_PLANETS = 0x0054,  // DumpPlanets()
    IDM_DEBUG_DUMP_UNIVERSE = 0x0055, // DumpUniverse()

    // ---- About / score dialogs -------------------------------------------
    IDM_GAME_SCORE = 0x005F,  // Score dialog (one entry point)
    IDM_GAME_SCORE2 = 0x0060, // Score dialog (alternate entry point)
    IDM_HELP_ABOUT = 0x0063,  // About dialog

    // ---- Fleet waypoint editing ------------------------------------------
    IDM_FLEET_DELETE_WAYPOINT = 0x0067, // Delete current waypoint (confirm)
    IDM_FLEET_INSERT_WAYPOINT = 0x0068, // Waypoint insert/delete sibling command

    // ---- File / game lifecycle -------------------------------------------
    IDM_FILE_HOST_GAME = 0x0069,       // Host game ?
    IDM_FILE_OPEN_GAME = 0x006D,       // Open game
    IDM_FILE_NEW_GAME = 0x006E,        // New game wizard
    IDM_FILE_RETURN_TO_TITLE = 0x0071, // Close game, return to title screen

    // Toolbar/accelerator aliases that jump to the same paths
    IDM_TOOL_NEW_GAME = 0x0ED8,  // Alias: New game
    IDM_TOOL_OPEN_GAME = 0x0ED9, // Alias: Open game

    // ---- Commands (ship design / research / diplomacy) --------------------
    IDM_GAME_SHIP_BUILDER = 0x007D, // ShipBuilder
    IDM_GAME_RESEARCH = 0x007E,     // Research dialog

    // Diplomacy / battle plans / turn control cluster
    IDM_GAME_RELATIONS = 0x07D9,     // Relations dialog
    IDM_GAME_WAIT_FOR_TURN = 0x07DA, // Wait-for-turn dialog/command
    IDM_GAME_BATTLE_PLANS1 = 0x07DB, // Battle plans dialog
    IDM_GAME_BATTLE_PLANS2 = 0x07DC, // Battle plans dialog (alias)
    IDM_GAME_RELATIONS2 = 0x07DE,    // Relations dialog (alias)

    // ---- View / window layout --------------------------------------------
    IDM_VIEW_LAYOUT_0 = 0x0082, // Window layout 0
    IDM_VIEW_LAYOUT_1 = 0x0083, // Window layout 1
    IDM_VIEW_LAYOUT_2 = 0x0084, // Window layout 2 ("small" layout)

    // Browser toggle (menu vs alias ID)
    IDM_VIEW_BROWSER_TOGGLE = 0x0088,  // Toggle tech browser window
    IDM_VIEW_BROWSER_TOGGLE2 = 0x0100, // Alias: browser toggle

    // Help index (menu vs alias ID)
    IDM_HELP_CONTENTS = 0x008A,  // Help index/contents
    IDM_HELP_CONTENTS2 = 0x0101, // Alias: help index/contents

    // ---- Race wizards -----------------------------------------------------
    IDM_RACE_CREATE = 0x0081, // Race creation wizard (default players)
    IDM_RACE_EDIT1 = 0x009C,  // Race edit wizard (existing player)
    IDM_RACE_EDIT2 = 0x009D,  // Race edit wizard (alias)

    // ---- Reports ----------------------------------------------------------
    IDM_REPORT_PLANET = 0x08FD,      // Planet report
    IDM_REPORT_CYCLE = 0x08FE,       // Cycle report type
    IDM_REPORT_FLEET = 0x08FF,       // Fleet report
    IDM_REPORT_ENEMY_FLEET = 0x0900, // Enemy fleets report
    IDM_REPORT_BATTLE = 0x0901,      // Battles report

    // ---- MRU (Most Recently Used) slots ----------------------------------
    IDM_FILE_MRU1 = 0x10CC, // MRU slot 1
    IDM_FILE_MRU2 = 0x10CD, // MRU slot 2
    IDM_FILE_MRU3 = 0x10CE, // MRU slot 3
    IDM_FILE_MRU4 = 0x10CF, // MRU slot 4
    IDM_FILE_MRU5 = 0x10D0, // MRU slot 5
    IDM_FILE_MRU6 = 0x10D1, // MRU slot 6
    IDM_FILE_MRU7 = 0x10D2, // MRU slot 7
    IDM_FILE_MRU8 = 0x10D3, // MRU slot 8
    IDM_FILE_MRU9 = 0x10D4, // MRU slot 9

    // ---- Scanner zoom factors (radio group) -------------------------------
    IDM_SCAN_ZOOM_0 = 0x0F3D, // scanner zoom (entry 0)
    IDM_SCAN_ZOOM_1 = 0x0F3E, // scanner zoom (entry 1)
    IDM_SCAN_ZOOM_2 = 0x0F3F, // scanner zoom (entry 2)
    IDM_SCAN_ZOOM_3 = 0x0F40, // scanner zoom (entry 3)
    IDM_SCAN_ZOOM_4 = 0x0F41, // scanner zoom (entry 4) (baseline in code)
    IDM_SCAN_ZOOM_5 = 0x0F42, // scanner zoom (entry 5)
    IDM_SCAN_ZOOM_6 = 0x0F43, // scanner zoom (entry 6)
    IDM_SCAN_ZOOM_7 = 0x0F44, // scanner zoom (entry 7)
    IDM_SCAN_ZOOM_8 = 0x0F45, // scanner zoom (entry 8)

    // ---- Turn ending / host/generate variants ------------------------------
    IDM_TURN_END_A = 0x0EDA, // end turn variant A
    IDM_TURN_END_B = 0x0EDB, // end turn variant B (toggles an internal bit)

    // ---- Dynamic popup range ----------------------------------------------
    IDM_POPUP_BASE = 15000, // Dynamic popup items start here (inferred)

    // ---- Debug: force-generate turns (decompiler had type confusion) -------
    IDM_DEBUG_GEN_10_TURNS = 21000,  // generate 10 turns (inferred)
    IDM_DEBUG_GEN_100_TURNS = 21001, // generate 100 turns (0x5209)
    IDM_DEBUG_GEN_1000_TURNS = 21002,
    IDM_UNKNOWN_098D = 0x098D,
    IDM_UNKNOWN_09C1 = 0x09C1,
    IDM_UNKNOWN_09C2 = 0x09C2,
    IDM_UNKNOWN_09C4 = 0x09C4,
    IDM_UNKNOWN_09C5 = 0x09C5,
    IDM_UNKNOWN_0EE2 = 0x0EE2,
    IDM_FRAME_POST_OPEN = 0x0FA1,
    IDM_UNKNOWN_1068 = 0x1068,
    IDM_UNKNOWN_1069 = 0x1069,

    IDC_UNKNOWN_0087 = 0x0087,
    IDC_UNKNOWN_0089 = 0x0089,
    IDC_UNKNOWN_009E = 0x009E,
    IDC_UNKNOWN_009F = 0x009F,
    IDC_UNKNOWN_00B3 = 0x00B3,
    IDC_UNKNOWN_00D5 = 0x00D5,
    IDC_UNKNOWN_00FA = 0x00FA,
    IDC_UNKNOWN_00FB = 0x00FB,
    IDC_UNKNOWN_00FC = 0x00FC,
    IDC_UNKNOWN_00FD = 0x00FD,
    IDC_UNKNOWN_010E = 0x010E,
    IDC_UNKNOWN_0428 = 0x0428,

    WMX_UNKNOWN_0069 = 0x0069,
    WMX_UNKNOWN_006A = 0x006A,
    WMX_UNKNOWN_006C = 0x006C,
    WMX_UNKNOWN_006F = 0x006F,

} WParamMessageId;

typedef enum VictoryCondition {
    vcOwnsPercentPlanets = 0,     /* "Owns % of all planets." */
    vcAttainsTechLevel = 1,       /* "Attains Tech X in Y fields." (level) */
    vcAttainsTechFields = 2,      /* number of tech fields */
    vcExceedsScore = 3,           /* "Exceeds a score of X." */
    vcExceedsSecondPlaceBy = 4,   /* "Exceeds second place score by X." */
    vcProductionCapacity = 5,     /* "Has a production capacity of X thousand." */
    vcOwnsCapitalShips = 6,       /* "Owns X capital ships." */
    vcHighestScoreAfterYears = 7, /* "Has the highest score after X years." */
    vcMeetsNumCriteria = 8,       /* "Winner must meet X of the above selected criteria." */
    vcMinYearsBeforeWin = 9       /* "At least X years must pass before a winner is declared." */
} VictoryCondition;

typedef enum MdOpenFlags {
    /* access + share combinations */
    mdRead = 0x0020,      /* OF_READ | OF_SHARE_DENY_WRITE */
    mdReadWrite = 0x0012, /* OF_READWRITE | OF_SHARE_EXCLUSIVE */

    /* create/truncate */
    mdCreate = 0x1012, /* OF_CREATE | OF_READWRITE | OF_SHARE_EXCLUSIVE */

    /* Stars!-specific modifier */
    mdNoOpenErr = 0x4000,
} MdOpenFlags;


typedef enum TaskType {
    grTaskNone = 0,
    grTaskXfer = 1, /* transport / transfer cargo */
    grTaskColonize = 2,
    grTaskMine = 3, /* remote mining */
    grTaskMerge = 4,
    grTaskScrap = 5,
    grTaskLayMines = 6,
    grTaskPatrol = 7,
    grTaskAutoRoute = 8, /* auto-route / auto-order */
    grTaskGive = 9,
} TaskType;

typedef enum XferActionType {
    iActionNone = 0, /* implicit / cleared */

    iActionLoadAll = 1,     /* "Load All Available"        */
    iActionUnloadAll = 2,   /* "Unload All"                */
    iActionLoadExact = 3,   /* "Load Exactly..."           */
    iActionUnloadExact = 4, /* "Unload Exactly..."         */
    iActionFillPercent = 5, /* "Fill Up to %..."           */
    iActionWaitPercent = 6, /* "Wait for %..."             */
    iActionLoadDunnage = 7, /* "Load Dunnage"              */
    iActionSetAmount = 8,   /* "Set Amount to..."          */
    iActionSetWaypoint = 9, /* "Set Waypoint to..."        */
    /* iActionLoadOptimal is encoded via iActionLoadDunnage + fuel path */
} XferActionType;

typedef enum MdTarget {
    mdTargetNone = 0,              /* "None/Disengage" */
    mdTargetAny = 1,               /* "Any" */
    mdTargetStarbase = 2,          /* "Starbase" */
    mdTargetArmedShips = 3,        /* "Armed Ships" */
    mdTargetBombersFreighters = 4, /* "Bombers/Freighters" */
    mdTargetUnarmedShips = 5,      /* "Unarmed Ships" */
    mdTargetFuelTransports = 6,    /* "Fuel Transports" */
    mdTargetFreighters = 7,        /* "Freighters" */
} MdTarget;

typedef enum BattleTactic {
    mdTacticDisengage = 0,             /* "Disengage" */
    mdTacticDisengageIfChallenged = 1, /* "Disengage if challenged" */
    mdTacticMinDamageToSelf = 2,       /* "Minimize damage to self" */
    mdTacticMaxNetDamage = 3,          /* "Maximize net damage" */
    mdTacticMaxDamageRatio = 4,        /* "Maximize damage ratio" */
    mdTacticMaxDamage = 5,             /* "Maximize damage" */
} BattleTactic;

typedef enum GrfWeapon {
    bitFBeamLow = 0x0001,
    bitFBeamHigh = 0x0002,
    bitFTorp = 0x0004,
    bitFMissile = 0x0008,
    bitFDeflected = 0x0080,
} GrfWeapon;

typedef enum {
    BLACKNESS   = 0x42,
    WHITENESS   = 0xFF,
    PATCOPY     = 0x00F00021,
    PATINVERT   = 0x005A0049,
    DSTINVERT   = 0x00550009,
    NOTCOPYPEN  = 0x33,
} PatBltRop;

typedef enum SystemMetric {
    SM_CXSCREEN   = 0,
    SM_CYSCREEN   = 1,
    SM_CXVSCROLL  = 2,
    SM_CYHSCROLL  = 3,
    SM_CYCAPTION  = 4,
    SM_CXDLGFRAME = 7,
    SM_CYDLGFRAME = 8,
    SM_CXFRAME    = 32,
    SM_CYFRAME    = 33,
} SystemMetric;

typedef enum DeviceCapsIndex {
    HORZRES    = 8,
    VERTRES    = 10,
    BITSPIXEL  = 12,
    PLANES     = 14,
    LOGPIXELSX = 88,
    LOGPIXELSY = 90,
} DeviceCapsIndex;

typedef enum ShowWindowCmd {
    SW_HIDE           = 0,
    SW_SHOWNORMAL     = 1,
    SW_SHOWMINIMIZED  = 2,
    SW_SHOWMAXIMIZED  = 3,
    SW_SHOWNOACTIVATE = 4,
    SW_SHOW           = 5,
    SW_MINIMIZE       = 6,
    SW_SHOWMINNOACTIVE= 7,
    SW_SHOWNA         = 8,
    SW_RESTORE        = 9,
} ShowWindowCmd;

typedef enum WindowStyle {
    WS_OVERLAPPED       = 0x00000000,
    WS_POPUP            = 0x80000000,
    WS_CHILD            = 0x40000000,
    WS_CLIPSIBLINGS     = 0x04000000,
    WS_CLIPCHILDREN     = 0x02000000,
    WS_VISIBLE          = 0x10000000,
    WS_DISABLED         = 0x08000000,
    WS_MINIMIZE         = 0x20000000,
    WS_MAXIMIZE         = 0x01000000,
    WS_CAPTION          = 0x00c00000,
    WS_BORDER           = 0x00800000,
    WS_DLGFRAME         = 0x00400000,
    WS_VSCROLL          = 0x00200000,
    WS_HSCROLL          = 0x00100000,
    WS_SYSMENU          = 0x00080000,
    WS_THICKFRAME       = 0x00040000,
    WS_MINIMIZEBOX      = 0x00020000,
    WS_MAXIMIZEBOX      = 0x00010000,
    WS_GROUP            = 0x00020000,
    WS_TABSTOP          = 0x00010000,
    WS_OVERLAPPEDWINDOW = 0x00cf0000,
    WS_POPUPWINDOW      = 0x80880000,
    WS_CHILDWINDOW      = 0x40000000,
    WS_TILED            = 0x00000000,
    WS_ICONIC           = 0x20000000,
    WS_SIZEBOX          = 0x00040000,
    WS_TILEDWINDOW      = 0x00cf0000,
} WindowStyle;

typedef enum WindowExStyle {
    WS_EX_DLGMODALFRAME  = 0x00000001,
    WS_EX_NOPARENTNOTIFY = 0x00000004,
    WS_EX_TOPMOST        = 0x00000008,
    WS_EX_ACCEPTFILES    = 0x00000010,
    WS_EX_TRANSPARENT    = 0x00000020,
} WindowExStyle;

typedef enum SetWindowPosFlags {
    SWP_NOSIZE        = 0x0001,
    SWP_NOMOVE        = 0x0002,
    SWP_NOZORDER      = 0x0004,
    SWP_NOREDRAW      = 0x0008,
    SWP_NOACTIVATE    = 0x0010,
    SWP_FRAMECHANGED  = 0x0020,
    SWP_SHOWWINDOW    = 0x0040,
    SWP_HIDEWINDOW    = 0x0080,
    SWP_NOCOPYBITS    = 0x0100,
    SWP_NOOWNERZORDER = 0x0200,
} SetWindowPosFlags;

typedef enum WindowLongOffset {
    GWL_WNDPROC    = -4,
    GWW_HINSTANCE  = -6,
    GWW_HWNDPARENT = -8,
    GWW_ID         = -12,
    GWL_STYLE      = -16,
    GWL_EXSTYLE    = -20,
} WindowLongOffset;

typedef enum GetWindowCmd {
    GW_HWNDFIRST = 0,
    GW_HWNDLAST  = 1,
    GW_HWNDNEXT  = 2,
    GW_HWNDPREV  = 3,
    GW_OWNER     = 4,
    GW_CHILD     = 5,
} GetWindowCmd;

typedef enum WinHelpCommand {
    HELP_CONTEXT  = 0x0001,
    HELP_QUIT     = 0x0002,
    HELP_INDEX    = 0x0003,
} WinHelpCommand;

typedef enum StockObjectId {
    WHITE_BRUSH         = 0,
    LTGRAY_BRUSH        = 1,
    GRAY_BRUSH          = 2,
    DKGRAY_BRUSH        = 3,
    BLACK_BRUSH         = 4,
    NULL_BRUSH          = 5,
    WHITE_PEN           = 6,
    BLACK_PEN           = 7,
    NULL_PEN            = 8,
    OEM_FIXED_FONT      = 10,
    ANSI_FIXED_FONT     = 11,
    ANSI_VAR_FONT       = 12,
    SYSTEM_FONT         = 13,
    DEVICE_DEFAULT_FONT = 14,
    DEFAULT_PALETTE     = 15,
    SYSTEM_FIXED_FONT   = 16,
} StockObjectId;

typedef enum BkMode {
    TRANSPARENT = 1,
    OPAQUE      = 2,
} BkMode;

typedef enum BitBltRop {
    SRCCOPY    = 0x00CC0020,
    SRCPAINT   = 0x00EE0086,
    SRCAND     = 0x008800C6,
    SRCINVERT  = 0x00660046,
    SRCERASE   = 0x00440328,
    NOTSRCCOPY = 0x00330008,
    NOTSRCERASE= 0x001100A6,
    MERGECOPY  = 0x00C000CA,
    MERGEPAINT = 0x00BB0226,
    PATPAINT   = 0x00FB0A09,
} BitBltRop;

typedef enum MenuFlags {
    MF_BYCOMMAND   = 0x0000,
    MF_GRAYED      = 0x0001,
    MF_DISABLED    = 0x0002,
    MF_BITMAP      = 0x0004,
    MF_CHECKED     = 0x0008,
    MF_POPUP       = 0x0010,
    MF_MENUBARBREAK= 0x0020,
    MF_MENUBREAK   = 0x0040,
    MF_HILITE      = 0x0080,
    MF_OWNERDRAW   = 0x0100,
    MF_BYPOSITION  = 0x0400,
    MF_SEPARATOR   = 0x0800,
} MenuFlags;

typedef enum TrackPopupMenuFlags {
    TPM_LEFTBUTTON = 0x0000,
    TPM_RIGHTBUTTON= 0x0002,
    TPM_LEFTALIGN  = 0x0000,
    TPM_CENTERALIGN= 0x0004,
    TPM_RIGHTALIGN = 0x0008,
} TrackPopupMenuFlags;

typedef enum StandardCursorId {
    IDC_ARROW = 32512,
    IDC_IBEAM = 32513,
    IDC_WAIT  = 32514,
    IDC_CROSS = 32515,
} StandardCursorId;

/* ---- structs (tmp/stars-asm/decompiled/structs.h) ---- */



typedef struct _aipart {
    uint16_t ibit : 4, /* +0x0000 (2) @bit0 */
        iItem : 5,     /* @bit4 */
        cItem : 4,     /* @bit9 */
        fRandom : 3;   /* @bit13 */
} AIPART;              /* size=0x2 */

typedef struct _aistarbase {
    int16_t idPlanet;   /* +0x0000 (2) */
    int16_t cFreighter; /* +0x0002 (2) */
    int16_t rgflid[8];  /* +0x0004 (16) */
} AISTARBASE;           /* size=0x14 */

typedef struct _aihist {
    uint16_t   cbAiHist;  /* +0x0000 (2) */
    int16_t    cStarbase; /* +0x0002 (2) */
    AISTARBASE rgasb[64]; /* +0x0004 (1280) */
} AIHIST;                 /* size=0x504 */

typedef struct _armor {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  dp;             /* +0x0034 (2) */
} ARMOR;                     /* size=0x36 */

typedef struct _beam {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  dRangeMax;      /* +0x0034 (2) */
    int16_t  dp;             /* +0x0036 (2) */
    int16_t  init;           /* +0x0038 (2) */
    int16_t  grfAbilities;   /* +0x003A (2) */
} BEAM;                      /* size=0x3c */

typedef struct _bomb {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  cRounds;        /* +0x0034 (2) */
    int16_t  dDmgCol;        /* +0x0036 (2) */
    int16_t  dDmgBldg;       /* +0x0038 (2) */
} BOMB;                      /* size=0x3a */

typedef struct _btlplan {
    uint16_t iplr : 4,      /* +0x0000 (2) @bit0 */
        iplan : 4,          /* @bit4 */
        mdTactic : 4,       /* @bit8 */
        unused1 : 2,        /* @bit12 */
        fDelete : 1,        /* @bit14 */
        fDumpCargo : 1;     /* @bit15 */
    uint16_t mdTarget1 : 4, /* +0x0002 (2) @bit0 */
        mdTarget2 : 4,      /* @bit4 */
        iplrAttack : 5,     /* @bit8 */
        unused2 : 3;        /* @bit13 */
    char szName[32];        /* +0x0004 (32) */
} BTLPLAN;                  /* size=0x24 */

typedef struct _coldrop {
    int16_t  idFleetSrc;       /* +0x0000 (2) */
    int16_t  idPlr;            /* +0x0002 (2) */
    int16_t  idPlanetDst;      /* +0x0004 (2) */
    uint16_t fCanColonize : 1, /* +0x0006 (2) @bit0 */
        unused : 15;           /* @bit1 */
    int32_t cColonist;         /* +0x0008 (4) */
} COLDROP;                     /* size=0xc */

typedef struct _compart {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
} COMPART;                   /* size=0x34 */

typedef struct _cyberinfo {
    union {
        uint16_t iLstPktDir : 3, /* +0x0000 (2) @bit0 */
            fBltColony : 1,      /* @bit3 */
            fLaunchedPkt : 1,    /* @bit4 */
            iPktTarget : 2,      /* @bit5 */
            fNeedScanPkt : 1,    /* @bit7 */
            unused : 8;          /* @bit8 */
        uint16_t wInfo;          /* +0x0000 (2) */
    };
} CYBERINFO; /* size=0x2 */

typedef struct _cyberinfotemp {
    union {
        uint16_t fIdleColonizers : 1, /* +0x0000 (2) @bit0 */
            cIdleFreighters : 2,      /* @bit1 */
            cFreightersDst : 2,       /* @bit3 */
            fNeedDefenders : 1,       /* @bit5 */
            fDefended : 1,            /* @bit6 */
            fUnderAttack : 1,         /* @bit7 */
            fNeedsMin1 : 1,           /* @bit8 */
            fNeedsMin2 : 1,           /* @bit9 */
            fNeedsMin3 : 1,           /* @bit10 */
            unused : 5;               /* @bit11 */
        uint16_t wInfo1;              /* +0x0000 (2) */
    };
} CYBERINFOTEMP; /* size=0x2 */

typedef struct _dv {
    union {
        uint16_t dp;        /* +0x0000 (2) */
        uint16_t pctSh : 7, /* +0x0000 (2) @bit0 */
            pctDp : 9;      /* @bit7 */
    };
} DV; /* size=0x2 */

typedef struct _engine {
    int16_t  id;              /* +0x0000 (2) */
    char     rgTech[6];       /* +0x0002 (6) */
    char     szName[32];      /* +0x0008 (32) */
    int16_t  cMass;           /* +0x0028 (2) */
    uint16_t resCost;         /* +0x002A (2) */
    int16_t  rgwtOreCost[3];  /* +0x002C (6) */
    int16_t  ibmp;            /* +0x0032 (2) */
    int16_t  grfAbilities;    /* +0x0034 (2) */
    int16_t  rgcFuelUsed[12]; /* +0x0036 (24) */
} ENGINE;                     /* size=0x4e */

typedef struct _fleetid {
    uint16_t ifl : 9, /* +0x0000 (2) @bit0 */
        iplr : 4,     /* @bit9 */
        junk : 3;     /* @bit13 */
} FLEETID;            /* size=0x2 */

typedef struct _framestuff {
    int16_t dx;          /* +0x0000 (2) */
    int16_t dy;          /* +0x0002 (2) */
    int16_t xTop;        /* +0x0004 (2) */
    int16_t y1;          /* +0x0006 (2) */
    int16_t y2;          /* +0x0008 (2) */
    int16_t dxPlanWant;  /* +0x000A (2) */
    int16_t dyMsgWant;   /* +0x000C (2) */
    int16_t dyMinWant;   /* +0x000E (2) */
    int16_t dx2PlanWant; /* +0x0010 (2) */
    int16_t dy2MsgWant;  /* +0x0012 (2) */
    int16_t dy2MinWant;  /* +0x0014 (2) */
} FRAMESTUFF;            /* size=0x16 */

typedef struct _game {
    int32_t lid;         /* +0x0000 (4) */
    int16_t mdSize;      /* +0x0004 (2) */
    int16_t mdDensity;   /* +0x0006 (2) */
    int16_t cPlayer;     /* +0x0008 (2) */
    int16_t cPlanMax;    /* +0x000A (2) */
    int16_t mdStartDist; /* +0x000C (2) */
    int16_t fDirty;      /* +0x000E (2) */
    union {
        uint16_t fExtraFuel : 1, /* +0x0010 (2) @bit0 */
            fSlowTech : 1,       /* @bit1 */
            fSinglePlr : 1,      /* @bit2 */
            fTutorial : 1,       /* @bit3 */
            fAisBand : 1,        /* @bit4 */
            fBBSPlay : 1,        /* @bit5 */
            fVisScores : 1,      /* @bit6 */
            fNoRandom : 1,       /* @bit7 */
            fClumping : 1,       /* @bit8 */
            wGen : 3,            /* @bit9 */
            unused : 4;          /* @bit12 */
        uint16_t wCrap;          /* +0x0010 (2) */
    };
    uint16_t turn;       /* +0x0012 (2) */
    uint8_t  rgvc[12];   /* +0x0014 (12) */
    char     szName[32]; /* +0x0020 (32) */
} GAME;                  /* size=0x40 */

typedef struct _gdata {
    union {
        int32_t grBits; /* +0x0000 (4) */
        struct {
            uint16_t fUnknownShip : 1, /* +0x0000 (2) @bit0 */
                fGeneratingTurn : 1,   /* @bit1 */
                fForceTurn : 1,        /* @bit2 */
                fHostMode : 1,         /* @bit3 */
                fSubmit : 1,           /* @bit4 */
                fNoResearchSav : 1,    /* @bit5 */
                fRadiatingEngine : 1,  /* @bit6 */
                fNoIdleChecks : 1,     /* @bit7 */
                fSendMsgMode : 1,      /* @bit8 */
                fRetryOpens : 1,       /* @bit9 */
                fAisDone : 1,          /* @bit10 */
                fTutorial : 1,         /* @bit11 */
                fGotoVCR : 1,          /* @bit12 */
                fVCRTimer : 1,         /* @bit13 */
                mdScreenSize : 2;      /* @bit14 */
            uint16_t fGameOverMan : 1, /* +0x0002 (2) @bit0 */
                fDontDoLogFiles : 1,   /* @bit1 */
                fFileCrippled : 1,     /* @bit2 */
                fSmallTileMode : 1,    /* @bit3 */
                fAllAis : 1,           /* @bit4 */
                fReadOnly : 1,         /* @bit5 */
                fExitWindows : 1,      /* @bit6 */
                fPartialTurn : 1,      /* @bit7 */
                fSetMassMode : 1,      /* @bit8 */
                fRptSafeDraw : 1,      /* @bit9 */
                fProgressTxt : 1,      /* @bit10 */
                fSoundFX : 1,          /* @bit11 */
                fNoSound : 1,          /* @bit12 */
                fSetRouteMode : 1,     /* @bit13 */
                fBleedingEdge : 1,     /* @bit14 */
                fToolbar : 1;          /* @bit15 */
        };
    };
    union {
        int32_t grBits2; /* +0x0004 (4) */
        struct {
            uint16_t fNoScannerDraw : 1, /* +0x0004 (2) @bit0 */
                fTrialPeriodOver : 1,    /* @bit1 */
                fClose : 1,              /* @bit2 */
                fDontCalcBleed : 1,      /* @bit3 */
                fChgZipOrd : 1,          /* @bit4 */
                fChgZipProd : 1,         /* @bit5 */
                fChgScanner : 1,         /* @bit6 */
                fChgReports : 1,         /* @bit7 */
                fWriteTurnNum : 1,       /* @bit8 */
                fHotSeat : 1,            /* @bit9 */
                fFleetLinkValid : 1,     /* @bit10 */
                fScoreVictory : 2;       /* @bit11 */
            uint16_t iCurGraph : 4,      /* +0x0006 (2) @bit0 */
                fMusic : 1,              /* @bit4 */
                fPerPlayerDumps : 1,     /* @bit5 */
                fNoHostNames : 1;        /* @bit6 */
        };
    };
    uint16_t fUnused2 : 14; /* +0x0008 (2) @bit0 */
} GDATA;                    /* size=0xa */

typedef struct _hb {
    uint16_t cbFree;   /* +0x0000 (2) */
    uint16_t cbBlock;  /* +0x0002 (2) */
    uint16_t cbSlop;   /* +0x0004 (2) */
    uint16_t ibTop;    /* +0x0006 (2) */
    HB      *lphbNext; /* +0x0008 (4) */
    uint16_t hmem;     /* +0x000C (2) */
    uint8_t  ht;       /* +0x000E (1) */
    uint8_t  unused1;  /* +0x000F (1) */
} HB;                  /* size=0x10 */

typedef struct _hdr {
    uint16_t cb : 10, /* +0x0000 (2) @bit0 */
        rt : 6;       /* @bit10 */
} HDR;                /* size=0x2 */

typedef struct _hs {
    HullSlotType grhst;     /* +0x0000 (2) */
    uint16_t     iItem : 8, /* +0x0002 (2) @bit0 */
        cItem : 8;          /* @bit8 */
} HS;                       /* size=0x4 */

typedef struct _hul {
    HulDef   ihuldef;        /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szClass[32];    /* +0x0008 (32) */
    uint16_t wtEmpty;        /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    uint16_t rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    uint16_t wtCargoMax;     /* +0x0034 (2) */
    uint16_t wtFuelMax;      /* +0x0036 (2) */
    uint16_t dp;             /* +0x0038 (2) */
    HS       rghs[16];       /* +0x003A (64) */
    uint8_t  chs;            /* +0x007A (1) */
} HUL;                       /* size=0x7b */

typedef struct _huldef {
    HUL      hul;        /* +0x0000 (123) */
    uint16_t init : 6,   /* +0x007B (2) @bit0 */
        imdAttack : 4,   /* @bit6 */
        imdCategory : 4, /* @bit10 */
        unused : 2;      /* @bit14 */
    uint16_t wrcCargo;   /* +0x007D (2) */
    uint8_t  rgbrc[16];  /* +0x007F (16) */
} HULDEF;                /* size=0x8f */

typedef struct _itemaction {
    uint16_t cQuan : 12, /* +0x0000 (2) @bit0 */
        iAction : 4;     /* @bit12 */
} ITEMACTION;            /* size=0x2 */

typedef struct _kill {
    uint8_t  itok;      /* +0x0000 (1) */
    uint8_t  grfWeapon; /* +0x0001 (1) */
    uint16_t cshKill;   /* +0x0002 (2) */
    uint16_t dpShield;  /* +0x0004 (2) */
    DV       dv;        /* +0x0006 (2) */
} KILL;                 /* size=0x8 */

typedef struct _btlrec {
    uint8_t  itok;       /* +0x0000 (1) */
    uint8_t  brcDest;    /* +0x0001 (1) */
    int16_t  ctok;       /* +0x0002 (2) */
    uint16_t iRound : 4, /* +0x0004 (2) @bit0 */
        dzDis : 4,       /* @bit4 */
        itokAttack : 8;  /* @bit8 */
    KILL rgkill[0];      /* +0x0006 (0) */
} BTLREC;                /* size=0x6 */

typedef struct _btlrec26 {
    uint8_t  itok;       /* +0x0000 (1) */
    uint8_t  brcDest;    /* +0x0001 (1) */
    uint8_t  itokAttack; /* +0x0002 (1) */
    uint8_t  ctok;       /* +0x0003 (1) */
    uint16_t iRound : 4, /* +0x0004 (2) @bit0 */
        dzDis : 4,       /* @bit4 */
        unused : 8;      /* @bit8 */
    KILL rgkill[0];      /* +0x0006 (0) */
} BTLREC26;              /* size=0x6 */

typedef struct _logxfer {
    int16_t    id;         /* +0x0000 (2) */
    GrobjClass grobj;      /* +0x0002 (2) */
    int32_t    rgdItem[5]; /* +0x0004 (20) */
} LOGXFER;                 /* size=0x18 */

typedef struct _logxferf {
    int16_t    id;          /* +0x0000 (2) */
    GrobjClass grobj;       /* +0x0002 (2) */
    int16_t    rgdItem[16]; /* +0x0004 (32) */
} LOGXFERF;                 /* size=0x24 */

typedef struct _lsb {
    uint16_t isb : 4,      /* +0x0000 (2) @bit0 */
        pctDp : 12;        /* @bit4 */
    uint16_t idFling : 10, /* +0x0002 (2) @bit0 */
        iWarpFling : 4,    /* @bit10 */
        unused3 : 2;       /* @bit14 */
} LSB;                     /* size=0x4 */

typedef struct _mdplr {
    uint16_t reserved : 9, /* +0x0000 (2) @bit0 */
        fAi : 1,           /* @bit9 */
        lvlAi : 3,         /* @bit10 */
        idAi : 3;          /* @bit13 */
} MDPLR;                   /* size=0x2 */

typedef struct _mines {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  grAbility;      /* +0x0034 (2) */
} MINES;                     /* size=0x36 */

typedef struct _mining {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  grAbility;      /* +0x0034 (2) */
} MINING;                    /* size=0x36 */

typedef struct _msgbig {
    int16_t iMsg;       /* +0x0000 (2) */
    int16_t wGoto;      /* +0x0002 (2) */
    int16_t rgParam[7]; /* +0x0004 (14) */
} MSGBIG;               /* size=0x12 */

typedef struct _msghdr {
    uint16_t iMsg : 9, /* +0x0000 (2) @bit0 */
        grWord : 7;    /* @bit9 */
    int16_t wGoto;     /* +0x0002 (2) */
} MSGHDR;              /* size=0x4 */

typedef struct _msgplr {
    MSGPLR *lpmsgplrNext; /* +0x0000 (4) */
    int16_t iPlrFrom;     /* +0x0004 (2) */
    int16_t iPlrTo;       /* +0x0006 (2) */
    int16_t iInRe;        /* +0x0008 (2) */
    int16_t cLen;         /* +0x000A (2) */
    uint8_t rgbMsg[0];    /* +0x000C (0) */
} MSGPLR;                 /* size=0xc */

typedef struct _msgturn {
    uint8_t iPlr : 4, /* +0x0000 (1) @bit0 */
        cbParams : 4; /* @bit4 */
    MSGHDR msghdr;    /* +0x0001 (4) */
} MSGTURN;            /* size=0x5 */

typedef struct _obj {
    union {
        PLANET *ppl; /* +0x0000 (2) */
        FLEET  *pfl; /* +0x0000 (2) */
        THING  *pth; /* +0x0000 (2) */
    };
} OBJ; /* size=0x2 */

typedef struct _part {
    HS hs; /* +0x0000 (4) */
    union {
        COMPART   *pcom;       /* +0x0004 (4) */
        ARMOR     *parmor;     /* +0x0004 (4) */
        HUL       *phul;       /* +0x0004 (4) */
        ENGINE    *pengine;    /* +0x0004 (4) */
        SCANNER   *pscanner;   /* +0x0004 (4) */
        BEAM      *pbeam;      /* +0x0004 (4) */
        TORP      *ptorp;      /* +0x0004 (4) */
        BOMB      *pbomb;      /* +0x0004 (4) */
        SHIELD    *pshield;    /* +0x0004 (4) */
        SPECIAL   *pspecial;   /* +0x0004 (4) */
        SPECIALSB *pspecialsb; /* +0x0004 (4) */
        MINES     *pmines;     /* +0x0004 (4) */
        MINING    *pmining;    /* +0x0004 (4) */
        PLANETARY *pplanetary; /* +0x0004 (4) */
        TERRA     *pterra;     /* +0x0004 (4) */
    };
} PART; /* size=0x8 */

typedef struct _pl {
    uint16_t cbItem : 8, /* +0x0000 (2) @bit0 */
        fMark : 1,       /* @bit8 */
        ht : 3,          /* @bit9 */
        cAlloc : 4;      /* @bit12 */
    uint8_t iMax;        /* +0x0002 (1) */
    uint8_t iMac;        /* +0x0003 (1) */
    uint8_t rgb[0];      /* +0x0004 (0) */
} PL;                    /* size=0x4 */

typedef struct _planet {
    int16_t  id;              /* +0x0000 (2) */
    int16_t  iPlayer;         /* +0x0002 (2) */
    uint16_t det : 8,         /* +0x0004 (2) @bit0 */
        fInclude : 1,         /* @bit8 */
        fStarbase : 1,        /* @bit9 */
        fHomeworld : 1,       /* @bit10 */
        fFirstYear : 1,       /* @bit11 */
        unusedC : 1,          /* @bit12 */
        fWasInhabited : 1,    /* @bit13 */
        unusedD : 2;          /* @bit14 */
    uint8_t rgpctMinLevel[3]; /* +0x0006 (3) */
    uint8_t rgMinConc[3];     /* +0x0009 (3) */
    char    rgEnvVar[3];      /* +0x000C (3) */
    char    rgEnvVarOrig[3];  /* +0x000F (3) */
    union {
        uint16_t uPopGuess : 12, /* +0x0012 (2) @bit0 */
            uDefGuess : 4;       /* @bit12 */
        uint16_t uGuesses;       /* +0x0012 (2) */
    };
    union {
        struct {
            uint32_t iDeltaPop : 8,  /* +0x0014 (4) @bit0 */
                cMines : 12,         /* @bit8 */
                cFactories : 12;     /* @bit20 */
            uint32_t cDefenses : 12, /* +0x0018 (4) @bit0 */
                iScanner : 5,        /* @bit12 */
                unused5 : 5,         /* @bit17 */
                fArtifact : 1,       /* @bit22 */
                fNoResearch : 1,     /* @bit23 */
                unused2 : 8;         /* @bit24 */
        };
        uint8_t rgbImp[8]; /* +0x0014 (8) */
    };
    int32_t rgwtMin[4]; /* +0x001C (16) */
    union {
        struct {
            uint16_t isb : 4,      /* +0x002C (2) @bit0 */
                pctDp : 12;        /* @bit4 */
            uint16_t idFling : 10, /* +0x002E (2) @bit0 */
                iWarpFling : 4,    /* @bit10 */
                fNoHeal : 1,       /* @bit14 */
                unused3 : 1;       /* @bit15 */
        };
        int32_t lStarbase; /* +0x002C (4) */
    };
    union {
        uint16_t idRoute : 10, /* +0x0030 (2) @bit0 */
            unused4 : 6;       /* @bit10 */
        uint16_t wRouting;     /* +0x0030 (2) */
    };
    int16_t turn;     /* +0x0032 (2) */
    PLPROD *lpplprod; /* +0x0034 (4) */
} PLANET;             /* size=0x38 */

typedef struct _planetary {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  grAbility;      /* +0x0034 (2) */
} PLANETARY;                 /* size=0x36 */

typedef struct _planetminimal {
    int16_t  id;        /* +0x0000 (2) */
    int16_t  iPlayer;   /* +0x0002 (2) */
    uint16_t det : 8,   /* +0x0004 (2) @bit0 */
        fInclude : 1,   /* @bit8 */
        fStarbase : 1,  /* @bit9 */
        unusedA : 1,    /* @bit10 */
        fFirstYear : 1, /* @bit11 */
        unusedB : 4;    /* @bit12 */
} PLANETMINIMAL;        /* size=0x6 */

typedef struct _planetsome {
    int16_t  id;               /* +0x0000 (2) */
    int16_t  iPlayer;          /* +0x0002 (2) */
    uint16_t det : 8,          /* +0x0004 (2) @bit0 */
        fInclude : 1,          /* @bit8 */
        fStarbase : 1,         /* @bit9 */
        unusedA : 1,           /* @bit10 */
        fFirstYear : 1,        /* @bit11 */
        unusedB : 4;           /* @bit12 */
    uint16_t rgpctMinLevel[3]; /* +0x0006 (6) */
    char     rgMinConc[3];     /* +0x000C (3) */
    char     rgEnvVar[3];      /* +0x000F (3) */
    char     rgEnvVarOrig[3];  /* +0x0012 (3) */
    union {
        uint16_t uPopGuess : 12, /* +0x0015 (2) @bit0 */
            uDefGuess : 4;       /* @bit12 */
        uint16_t uGuesses;       /* +0x0015 (2) */
    };
} PLANETSOME; /* size=0x17 */

typedef struct _fleet {
    union {
        int16_t  id;      /* +0x0000 (2) */
        uint16_t ifl : 9, /* +0x0000 (2) @bit0 */
            iplr : 4,     /* @bit9 */
            junk : 3;     /* @bit13 */
    };
    int16_t  iPlayer;     /* +0x0002 (2) */
    uint16_t det : 8,     /* +0x0004 (2) @bit0 */
        fInclude : 1,     /* @bit8 */
        fRepOrders : 1,   /* @bit9 */
        fDead : 1,        /* @bit10 */
        fDone : 1,        /* @bit11 */
        fBombed : 1,      /* @bit12 */
        fHereAllTurn : 1, /* @bit13 */
        fNoHeal : 1,      /* @bit14 */
        fMark : 1;        /* @bit15 */
    int16_t idPlanet;     /* +0x0006 (2) */
    POINT   pt;           /* +0x0008 (4) */
    int16_t rgcsh[16];    /* +0x000C (32) */
    union {
        DV      rgdv[16]; /* +0x002C (32) */
        int32_t wtFleet;  /* +0x002C (4) */
    };
    int32_t rgwtMin[5]; /* +0x004C (20) */
    uint8_t iplan;      /* +0x0060 (1) */
    uint8_t bUnused;    /* +0x0061 (1) */
    int16_t cord;       /* +0x0062 (2) */
    PLORD  *lpplord;    /* +0x0064 (4) */
    FLEET  *lpflNext;   /* +0x0068 (4) */
    union {
        int32_t lPower; /* +0x006C (4) */
        struct {
            int16_t dMoveLeft; /* +0x006C (2) */
            int16_t dMoveUsed; /* +0x006E (2) */
        };
    };
    int32_t lFuelUsed; /* +0x0070 (4) */
    union {
        int32_t dirLong; /* +0x0074 (4) */
        struct {
            uint16_t dirFltX : 8,  /* +0x0074 (2) @bit0 */
                dirFltY : 8;       /* @bit8 */
            uint16_t iwarpFlt : 4, /* +0x0076 (2) @bit0 */
                fdirValid : 1,     /* @bit4 */
                fCompChg : 1,      /* @bit5 */
                fTargeted : 1,     /* @bit6 */
                fSkipped : 1,      /* @bit7 */
                fUnused : 8;       /* @bit8 */
        };
    };
    char *lpszName; /* +0x0078 (4) */
} FLEET;            /* size=0x7c */

typedef struct _fleetsome {
    int16_t  id;        /* +0x0000 (2) */
    int16_t  iPlayer;   /* +0x0002 (2) */
    uint16_t det : 8,   /* +0x0004 (2) @bit0 */
        fInclude : 1,   /* @bit8 */
        fRepOrders : 1, /* @bit9 */
        fDead : 1,      /* @bit10 */
        fByteCsh : 1,   /* @bit11 */
        unused : 4;     /* @bit12 */
    int16_t idPlanet;   /* +0x0006 (2) */
    POINT   pt;         /* +0x0008 (4) */
} FLEETSOME;            /* size=0xc */

typedef struct _popupdata {
    GrPopupType grPopup; /* +0x0000 (2) */
    union {
        int32_t rgi[5]; /* +0x0002 (20) */
        struct {
            int16_t idPlanet;   /* +0x0002 (2) */
            int16_t iPlanetVar; /* +0x0004 (2) */
            int16_t iPlanVal;   /* +0x0006 (2) */
            int16_t iPlanMin;   /* +0x0008 (2) */
            int16_t iPlanMax;   /* +0x000A (2) */
            int16_t iPlrVal;    /* +0x000C (2) */
            int16_t iPlrMin;    /* +0x000E (2) */
            int16_t iPlrMax;    /* +0x0010 (2) */
        };
        struct {
            int16_t idPlan;   /* +0x0002 (2) */
            int16_t cMax;     /* +0x0004 (2) */
            int16_t cCur;     /* +0x0006 (2) */
            int16_t cOperate; /* +0x0008 (2) */
            int16_t fFactory; /* +0x000A (2) */
        };
        struct {
            int16_t dxOut; /* +0x0002 (2) */
            char   *psz;   /* +0x0004 (2) */
        };
        int16_t iPlayer; /* +0x0002 (2) */
        struct {
            FLEET   *lpfl;       /* +0x0002 (4) */
            int16_t  fRedDamage; /* +0x0006 (2) */
            int16_t  dxDamage;   /* +0x0008 (2) */
            uint16_t grbit;      /* +0x000A (2) */
        };
        PART part; /* +0x0002 (8) */
        struct {
            SHDEF  *lpshdef;     /* +0x0002 (4) */
            int16_t fShowDamage; /* +0x0006 (2) */
            int16_t fHideCounts; /* +0x0008 (2) */
            int16_t fToken;      /* +0x000A (2) */
            int16_t fSummary;    /* +0x000C (2) */
            int16_t itok;        /* +0x000E (2) */
        };
    };
} POPUPDATA; /* size=0x16 */

typedef struct _prod {
    uint32_t cItem : 10, /* +0x0000 (4) @bit0 */
        iItem : 7,       /* @bit10 */
        grobj : 3,       /* @bit17 */
        pct : 7,         /* @bit20 */
        unused : 5;      /* @bit27 */
} PROD;                  /* size=0x4 */

typedef struct PLPROD {
    uint16_t cbItem : 8, /* +0x0000 (2) @bit0 */
        fMark : 1,       /* @bit8 */
        ht : 3,          /* @bit9 */
        cAlloc : 4;      /* @bit12 */
    uint8_t iprodMax;    /* +0x0002 (1) */
    uint8_t iprodMac;    /* +0x0003 (1) */
    PROD    rgprod[0];   /* +0x0004 (0) */
} PLPROD;                /* size=0x4 */

typedef struct _prodq1 {
    union {
        uint16_t w;          /* +0x0000 (2) */
        uint16_t mdIdle : 6, /* +0x0000 (2) @bit0 */
            cQuan : 10;      /* @bit6 */
    };
} PRODQ1; /* size=0x2 */

typedef struct _btn {
    RECT     rc;           /* +0x0000 (8) */
    int16_t  bt;           /* +0x0008 (2) */
    int16_t  iVal;         /* +0x000A (2) */
    uint16_t fVisible : 1, /* +0x000C (2) @bit0 */
        fDisabled : 1,     /* @bit1 */
        iSide : 2,         /* @bit2 */
        fUnused : 12;      /* @bit4 */
} BTN;                     /* size=0xe */

typedef struct _btnt {
    HWND     hwnd;        /* +0x0000 (2) */
    HDC      hdc;         /* +0x0002 (2) */
    RECT     rc;          /* +0x0004 (8) */
    int16_t  dTimer;      /* +0x000C (2) */
    int16_t  btf;         /* +0x000E (2) */
    char    *szText;      /* +0x0010 (2) */
    int32_t  lTicks;      /* +0x0012 (4) */
    uint16_t fFirst : 1,  /* +0x0016 (2) @bit0 */
        fDown : 1,        /* @bit1 */
        fInitDown : 1,    /* @bit2 */
        fCreatedDC : 1,   /* @bit3 */
        fNoEndRedraw : 1, /* @bit4 */
        fUnused : 11;     /* @bit5 */
} BTNT;                   /* size=0x18 */

typedef struct _drawcir {
    int16_t *rgx;        /* +0x0000 (2) */
    int16_t *rgy;        /* +0x0002 (2) */
    int16_t *rgrad;      /* +0x0004 (2) */
    int16_t  cCur;       /* +0x0006 (2) */
    int16_t  cMax;       /* +0x0008 (2) */
    HDC      hdc;        /* +0x000A (2) */
    RECT     rcClip;     /* +0x000C (8) */
    int16_t  fCovered;   /* +0x0014 (2) */
    int16_t  fHollowOut; /* +0x0016 (2) */
} DRAWCIR;               /* size=0x18 */

typedef struct _rpt {
    int32_t grbitVisible; /* +0x0000 (4) */
    int16_t irpt;         /* +0x0004 (2) */
    int16_t cFields;      /* +0x0006 (2) */
    int16_t cFieldFirst;  /* +0x0008 (2) */
    int16_t icolSort;     /* +0x000A (2) */
    int16_t fAscending;   /* +0x000C (2) */
    int16_t irowFirst;    /* +0x000E (2) */
    POINT   ptDlg;        /* +0x0010 (4) */
    POINT   ptSize;       /* +0x0014 (4) */
    int16_t fCached;      /* +0x0018 (2) */
    uint8_t rgbdx[16];    /* +0x001A (16) */
    int16_t cRows;        /* +0x002A (2) */
    int16_t cRowsVis;     /* +0x002C (2) */
    int16_t iSubsort;     /* +0x002E (2) */
    HWND    hwndVScroll;  /* +0x0030 (2) */
    HWND    hwndHScroll;  /* +0x0032 (2) */
    int16_t cColScroll;   /* +0x0034 (2) */
} RPT;                    /* size=0x36 */

typedef struct _rtbof {
    char    rgid[4]; /* +0x0000 (4) */
    int32_t lidGame; /* +0x0004 (4) */
    union {
        uint16_t verInc : 5, /* +0x0008 (2) @bit0 */
            verMinor : 7,    /* @bit5 */
            verMajor : 4;    /* @bit12 */
        uint16_t wVersion;   /* +0x0008 (2) */
    };
    uint16_t turn;        /* +0x000A (2) */
    int16_t  iPlayer : 5, /* +0x000C (2) @bit0 */
        lSaltTime : 11;   /* @bit5 */
    uint16_t dt : 8,      /* +0x000E (2) @bit0 */
        fDone : 1,        /* @bit8 */
        fInUse : 1,       /* @bit9 */
        fMulti : 1,       /* @bit10 */
        fGameOverMan : 1, /* @bit11 */
        fCrippled : 1,    /* @bit12 */
        wGen : 3;         /* @bit13 */
} RTBOF;                  /* size=0x10 */

typedef struct _rtchgname {
    int16_t id;      /* +0x0000 (2) */
    int16_t grobj;   /* +0x0002 (2) */
    uint8_t rgb[33]; /* +0x0004 (33) */
} RTCHGNAME;         /* size=0x25 */

typedef struct _rtChgPlanetLong {
    int16_t id; /* +0x0000 (2) */
    union {
        uint32_t ul;              /* +0x0002 (4) */
        uint32_t fNoResearch : 1, /* +0x0002 (4) @bit0 */
            idFling : 10,         /* @bit1 */
            iWarpFling : 4,       /* @bit11 */
            idRoute : 10,         /* @bit15 */
            unused : 7;           /* @bit25 */
    };
} RTCHGPLANETLONG; /* size=0x6 */

typedef struct _rtChgProdQ {
    int16_t id;        /* +0x0000 (2) */
    PROD    rgprod[0]; /* +0x0002 (0) */
} RTCHGPRODQ;          /* size=0x2 */

typedef struct _rthisthdr {
    int16_t cPlanet;      /* +0x0000 (2) */
    int16_t cPlanetExtra; /* +0x0002 (2) */
} RTHISTHDR;              /* size=0x4 */

typedef struct _rtloghdr {
    int16_t cbLog;         /* +0x0000 (2) */
    int32_t lSerialNumber; /* +0x0002 (4) */
    uint8_t rgbConfig[11]; /* +0x0006 (11) */
} RTLOGHDR;                /* size=0x11 */

typedef struct _rtlogthing {
    uint16_t idFull;    /* +0x0000 (2) */
    int16_t  fDetonate; /* +0x0002 (2) */
} RTLOGTHING;           /* size=0x4 */

typedef struct _rtplanet {
    int16_t id : 11,     /* +0x0000 (2) @bit0 */
        iPlayer : 5;     /* @bit11 */
    uint16_t det : 7,    /* +0x0002 (2) @bit0 */
        fHomeworld : 1,  /* @bit7 */
        fInclude : 1,    /* @bit8 */
        fStarbase : 1,   /* @bit9 */
        fIncEVO : 1,     /* @bit10 */
        fIncImp : 1,     /* @bit11 */
        fIsArtifact : 1, /* @bit12 */
        fIncSurfMin : 1, /* @bit13 */
        fRouting : 1,    /* @bit14 */
        fFirstYear : 1;  /* @bit15 */
} RTPLANET;              /* size=0x4 */

typedef struct _rtshdef {
    union {
        uint16_t det : 8, /* +0x0000 (2) @bit0 */
            fInclude : 1, /* @bit8 */
            fFree : 1,    /* @bit9 */
            ishdef : 5,   /* @bit10 */
            fGift : 1;    /* @bit15 */
        uint16_t wFlags;  /* +0x0000 (2) */
    };
    uint8_t ihuldef; /* +0x0002 (1) */
    uint8_t ibmp;    /* +0x0003 (1) */
    union {
        uint16_t wtEmpty; /* +0x0004 (2) */
        uint16_t dp;      /* +0x0004 (2) */
    };
    uint8_t  chs;     /* +0x0006 (1) */
    uint16_t turn;    /* +0x0007 (2) */
    uint32_t cBuilt;  /* +0x0009 (4) */
    uint32_t cExist;  /* +0x000D (4) */
    HS       rghs[0]; /* +0x0011 (0) */
} RTSHDEF;            /* size=0x11 */

typedef struct _rtchgshdef {
    uint16_t mdChg : 4, /* +0x0000 (2) @bit0 */
        iPlr : 4,       /* @bit4 */
        ishdef : 5,     /* @bit8 */
        junk : 3;       /* @bit13 */
    RTSHDEF rtshdef;    /* +0x0002 (17) */
} RTCHGSHDEF;           /* size=0x13 */

typedef struct _rtshipint {
    int16_t id; /* +0x0000 (2) */
    int16_t i;  /* +0x0002 (2) */
} RTSHIPINT;    /* size=0x4 */

typedef struct _rtshipint2 {
    int16_t id; /* +0x0000 (2) */
    int16_t i;  /* +0x0002 (2) */
    int16_t i2; /* +0x0004 (2) */
} RTSHIPINT2;   /* size=0x6 */

typedef struct _rtxfer {
    uint16_t id1;        /* +0x0000 (2) */
    uint16_t id2;        /* +0x0002 (2) */
    uint8_t  grobj1 : 4, /* +0x0004 (1) @bit0 */
        grobj2 : 4;      /* @bit4 */
    uint8_t grbitItems;  /* +0x0005 (1) */
    char    rgcQuan[1];  /* +0x0006 (1) */
} RTXFER;                /* size=0x7 */

typedef struct _rtxferf {
    uint16_t id1;        /* +0x0000 (2) */
    uint16_t id2;        /* +0x0002 (2) */
    uint8_t  grobj1 : 4, /* +0x0004 (1) @bit0 */
        grobj2 : 4;      /* @bit4 */
    uint16_t grbitItems; /* +0x0005 (2) */
    int16_t  rgcQuan[1]; /* +0x0007 (2) */
} RTXFERF;               /* size=0x9 */

typedef struct _rtxferl {
    uint16_t id1;        /* +0x0000 (2) */
    uint16_t id2;        /* +0x0002 (2) */
    uint8_t  grobj1 : 4, /* +0x0004 (1) @bit0 */
        grobj2 : 4;      /* @bit4 */
    uint8_t grbitItems;  /* +0x0005 (1) */
    int32_t rgcQuan[1];  /* +0x0006 (4) */
} RTXFERL;               /* size=0xa */

typedef struct _rtxferx {
    uint16_t id1;        /* +0x0000 (2) */
    uint16_t id2;        /* +0x0002 (2) */
    uint8_t  grobj1 : 4, /* +0x0004 (1) @bit0 */
        grobj2 : 4;      /* @bit4 */
    uint8_t grbitItems;  /* +0x0005 (1) */
    int16_t rgcQuan[1];  /* +0x0006 (2) */
} RTXFERX;               /* size=0x8 */

typedef struct _sbar {
    int16_t grbit; /* +0x0000 (2) */
    int16_t id;    /* +0x0002 (2) */
    POINT   pt;    /* +0x0004 (4) */
    char   *psz;   /* +0x0008 (2) */
    SCAN   *pscan; /* +0x000A (2) */
} SBAR;            /* size=0xc */

typedef struct _scan {
    POINT      pt;        /* +0x0000 (4) */
    GrobjClass grobj;     /* +0x0004 (2) */
    GrobjClass grobjFull; /* +0x0006 (2) */
    int16_t    idpl;      /* +0x0008 (2) */
    int16_t    ifl;       /* +0x000A (2) */
    int16_t    iwp;       /* +0x000C (2) */
    int16_t    ith;       /* +0x000E (2) */
} SCAN;                   /* size=0x10 */

typedef struct _scanner {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  dRange;         /* +0x0034 (2) */
    int16_t  grfAbilities;   /* +0x0036 (2) */
} SCANNER;                   /* size=0x38 */

typedef struct _score {
    int32_t  lScore;      /* +0x0000 (4) */
    int32_t  cResources;  /* +0x0004 (4) */
    int16_t  cPlanet;     /* +0x0008 (2) */
    int16_t  cStarbase;   /* +0x000A (2) */
    uint16_t rgcsh[3];    /* +0x000C (6) */
    int16_t  cTechLevels; /* +0x0012 (2) */
} SCORE;                  /* size=0x14 */

typedef struct _scorex {
    union {
        uint16_t wWord;       /* +0x0000 (2) */
        uint16_t iPlayer : 5, /* +0x0000 (2) @bit0 */
            fValid : 1,       /* @bit5 */
            grbitVC : 8,      /* @bit6 */
            fWinner : 1,      /* @bit14 */
            fHistory : 1;     /* @bit15 */
    };
    union {
        int16_t  iRank; /* +0x0002 (2) */
        uint16_t turn;  /* +0x0002 (2) */
    };
    SCORE score; /* +0x0004 (20) */
} SCOREX;        /* size=0x18 */

typedef struct _selSome {
    POINT      pt;        /* +0x0000 (4) */
    int16_t    grobj;     /* +0x0004 (2) */
    GrobjClass grobjFull; /* +0x0006 (2) */
    int16_t    id;        /* +0x0008 (2) */
    int16_t    iwpAct;    /* +0x000A (2) */
    SCAN       scan;      /* +0x000C (16) */
} SELSOME;                /* size=0x1c */

typedef struct _shdef {
    HUL hul; /* +0x0000 (123) */
    union {
        uint16_t det : 8, /* +0x007B (2) @bit0 */
            fInclude : 1, /* @bit8 */
            fFree : 1,    /* @bit9 */
            ishdef : 5,   /* @bit10 */
            fGift : 1;    /* @bit15 */
        uint16_t wFlags;  /* +0x007B (2) */
    };
    uint16_t turn;   /* +0x007D (2) */
    uint32_t cBuilt; /* +0x007F (4) */
    uint32_t cExist; /* +0x0083 (4) */
    union {
        int32_t lPower;   /* +0x0087 (4) */
        int32_t lVisible; /* +0x0087 (4) */
    };
    uint16_t grbitPlr;    /* +0x008B (2) */
    uint16_t dScanRange;  /* +0x008D (2) */
    uint16_t dScanRange2; /* +0x008F (2) */
    uint8_t  pctDetect;   /* +0x0091 (1) */
    uint8_t  iSteal;      /* +0x0092 (1) */
} SHDEF;                  /* size=0x93 */

typedef struct _shield {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  dp;             /* +0x0034 (2) */
} SHIELD;                    /* size=0x36 */

typedef struct _special {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  grAbility;      /* +0x0034 (2) */
} SPECIAL;                   /* size=0x36 */

typedef struct _specialsb {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  grAbility;      /* +0x0034 (2) */
    int16_t  grAbility2;     /* +0x0036 (2) */
} SPECIALSB;                 /* size=0x38 */

typedef struct _starpack {
    uint32_t dx : 10, /* +0x0000 (4) @bit0 */
        y : 12,       /* @bit10 */
        id : 10;      /* @bit22 */
} STARPACK;           /* size=0x4 */

typedef struct _tasklaymines {
    uint16_t cTime;    /* +0x0000 (2) */
    uint16_t cTimeOld; /* +0x0002 (2) */
} TASKLAYMINES;        /* size=0x4 */

typedef struct _taskpatrol {
    uint16_t iWarp; /* +0x0000 (2) */
    uint16_t iDist; /* +0x0002 (2) */
} TASKPATROL;       /* size=0x4 */

typedef struct _tasksell {
    uint16_t iPlrX; /* +0x0000 (2) */
} TASKSELL;         /* size=0x2 */

typedef struct _taskxport {
    ITEMACTION rgia[5]; /* +0x0000 (10) */
} TASKXPORT;            /* size=0xa */

typedef struct _order {
    POINT    pt;          /* +0x0000 (4) */
    int16_t  id;          /* +0x0004 (2) */
    uint16_t grTask : 4,  /* +0x0006 (2) @bit0 */
        iWarp : 4,        /* @bit4 */
        grobj : 4,        /* @bit8 */
        fValidTask : 1,   /* @bit12 */
        fNoAutoTrack : 1, /* @bit13 */
        fUnused : 2;      /* @bit14 */
    union {
        TASKXPORT    txp;   /* +0x0008 (10) */
        TASKLAYMINES tlm;   /* +0x0008 (4) */
        TASKPATROL   tptl;  /* +0x0008 (4) */
        TASKSELL     tsell; /* +0x0008 (2) */
    };
} ORDER; /* size=0x12 */

typedef struct PLORD {
    uint16_t cbItem : 8, /* +0x0000 (2) @bit0 */
        fMark : 1,       /* @bit8 */
        ht : 3,          /* @bit9 */
        cAlloc : 4;      /* @bit12 */
    uint8_t iordMax;     /* +0x0002 (1) */
    uint8_t iordMac;     /* +0x0003 (1) */
    ORDER   rgord[0];    /* +0x0004 (0) */
} PLORD;                 /* size=0x4 */

typedef struct _rtwaypt {
    int16_t id;     /* +0x0000 (2) */
    int16_t iWaypt; /* +0x0002 (2) */
    ORDER   order;  /* +0x0004 (18) */
} RTWAYPT;          /* size=0x16 */

typedef struct _terra {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  grAbility;      /* +0x0034 (2) */
} TERRA;                     /* size=0x36 */

typedef struct _thmine {
    int32_t  cMines;      /* +0x0000 (4) */
    uint16_t grbitPlr;    /* +0x0004 (2) */
    uint8_t  iType;       /* +0x0006 (1) */
    uint8_t  fDetonate;   /* +0x0007 (1) */
    uint16_t grbitPlrNow; /* +0x0008 (2) */
} THMINE;                 /* size=0xa */

typedef struct _thpack {
    uint16_t idPlanet : 10, /* +0x0000 (2) @bit0 */
        iWarp : 4,          /* @bit10 */
        fMoved : 1,         /* @bit14 */
        fInclude : 1;       /* @bit15 */
    int16_t  rgwtMin[3];    /* +0x0002 (6) */
    uint16_t wtMax : 14,    /* +0x0008 (2) @bit0 */
        iDecayRate : 2;     /* @bit14 */
} THPACK;                   /* size=0xa */

typedef struct _thtrader {
    POINT    ptDest;      /* +0x0000 (4) */
    uint16_t iWarp : 4,   /* +0x0004 (2) @bit0 */
        fInclude : 1,     /* @bit4 */
        unused : 11;      /* @bit5 */
    uint16_t grbitPlr;    /* +0x0006 (2) */
    uint16_t grbitTrader; /* +0x0008 (2) */
} THTRADER;               /* size=0xa */

typedef struct _thworm {
    uint16_t iStable : 2,  /* +0x0000 (2) @bit0 */
        cLastMove : 10,    /* @bit2 */
        fDestKnown : 1,    /* @bit12 */
        fInclude : 1;      /* @bit13 */
    uint16_t grbitPlr;     /* +0x0002 (2) */
    uint16_t grbitPlrTrav; /* +0x0004 (2) */
    uint16_t idPartner;    /* +0x0006 (2) */
} THWORM;                  /* size=0x8 */

typedef struct _thing {
    union {
        uint16_t idFull; /* +0x0000 (2) */
        uint16_t id : 9, /* +0x0000 (2) @bit0 */
            iplr : 4,    /* @bit9 */
            ith : 3;     /* @bit13 */
    };
    POINT pt; /* +0x0002 (4) */
    union {
        uint8_t  rgb[10]; /* +0x0006 (10) */
        THMINE   thm;     /* +0x0006 (10) */
        THPACK   thp;     /* +0x0006 (10) */
        THWORM   thw;     /* +0x0006 (8) */
        THTRADER tht;     /* +0x0006 (10) */
    };
    uint16_t turn; /* +0x0010 (2) */
} THING;           /* size=0x12 */

typedef struct _sel {
    POINT      pt;        /* +0x0000 (4) */
    GrobjClass grobj;     /* +0x0004 (2) */
    GrobjClass grobjFull; /* +0x0006 (2) */
    int16_t    id;        /* +0x0008 (2) */
    int16_t    iwpAct;    /* +0x000A (2) */
    SCAN       scan;      /* +0x000C (16) */
    FLEET      fl;        /* +0x001C (124) */
    PLANET     pl;        /* +0x0098 (56) */
    THING      th;        /* +0x00D0 (18) */
} SEL;                    /* size=0xe2 */

typedef struct _tile {
    int16_t yTop;                        /* +0x0000 (2) */
    int16_t dyFull;                      /* +0x0002 (2) */
    int16_t grbit;                       /* +0x0004 (2) */
    void (**pfn)(uint16_t, TILE *, OBJ); /* +0x0006 (4) */
    uint16_t iCol : 3,                   /* +0x000A (2) @bit0 */
        id : 4,                          /* @bit3 */
        fPopped : 1,                     /* @bit7 */
        fNullPtr : 1,                    /* @bit8 */
        fMinTitle : 1,                   /* @bit9 */
        fErase : 1,                      /* @bit10 */
        fFixCtls : 1,                    /* @bit11 */
        fMinDraw : 1;                    /* @bit12 */
    uint16_t fUnused : 4;                /* +0x000C (2) @bit0 */
    uint16_t idh;                        /* +0x000E (2) */
} TILE;                                  /* size=0x10 */

typedef struct _timer {
    int16_t mdForce;        /* +0x0000 (2) */
    int16_t fAutoGenWhenIn; /* +0x0002 (2) */
    union {
        int16_t  hrsForce;      /* +0x0004 (2) */
        uint16_t minForce : 12, /* +0x0004 (2) @bit0 */
            cPlr : 4;           /* @bit12 */
    };
    int32_t tickcount; /* +0x0006 (4) */
} TIMER;               /* size=0xa */

typedef struct _tok {
    uint16_t   id;            /* +0x0000 (2) */
    uint8_t    iplr;          /* +0x0002 (1) */
    GrobjClass grobj;         /* +0x0003 (1) */
    uint8_t    ishdef;        /* +0x0004 (1) */
    uint8_t    brc;           /* +0x0005 (1) */
    uint8_t    initBase;      /* +0x0006 (1) */
    uint8_t    initMin;       /* +0x0007 (1) */
    uint8_t    initMac;       /* +0x0008 (1) */
    uint8_t    itokTarget;    /* +0x0009 (1) */
    uint8_t    pctCloak;      /* +0x000A (1) */
    uint8_t    pctJam;        /* +0x000B (1) */
    uint8_t    pctBC;         /* +0x000C (1) */
    uint8_t    pctCap;        /* +0x000D (1) */
    uint8_t    pctBeamDef;    /* +0x000E (1) */
    uint16_t   wt;            /* +0x000F (2) */
    uint16_t   dpShield;      /* +0x0011 (2) */
    uint16_t   csh;           /* +0x0013 (2) */
    DV         dv;            /* +0x0015 (2) */
    uint16_t   mdTarget1 : 4, /* +0x0017 (2) @bit0 */
        mdTarget2 : 4,        /* @bit4 */
        mdTactic : 4,         /* @bit8 */
        mdTarget0 : 4;        /* @bit12 */
    uint16_t dxyLim : 4,      /* +0x0019 (2) @bit0 */
        dxyMax : 4,           /* @bit4 */
        spd : 4,              /* @bit8 */
        cTarget : 4;          /* @bit12 */
    union {
        uint16_t fActive : 1, /* +0x001B (2) @bit0 */
            fDetector : 1,    /* @bit1 */
            fTorp : 1,        /* @bit2 */
            fRegen : 1,       /* @bit3 */
            fMoved : 1,       /* @bit4 */
            dzDis : 5,        /* @bit5 */
            dwt : 4,          /* @bit10 */
            dMovesLeft : 2;   /* @bit14 */
        uint16_t wFlags;      /* +0x001B (2) */
    };
} TOK; /* size=0x1d */

typedef struct _btldata {
    uint16_t id;       /* +0x0000 (2) */
    uint8_t  cplr;     /* +0x0002 (1) */
    uint8_t  ctok;     /* +0x0003 (1) */
    uint16_t grfPlr;   /* +0x0004 (2) */
    uint16_t cbData;   /* +0x0006 (2) */
    uint16_t idPlanet; /* +0x0008 (2) */
    POINT    pt;       /* +0x000A (4) */
    TOK      rgtok[0]; /* +0x000E (0) */
} BTLDATA;             /* size=0xe */

typedef struct _torp {
    int16_t  id;             /* +0x0000 (2) */
    char     rgTech[6];      /* +0x0002 (6) */
    char     szName[32];     /* +0x0008 (32) */
    int16_t  cMass;          /* +0x0028 (2) */
    uint16_t resCost;        /* +0x002A (2) */
    int16_t  rgwtOreCost[3]; /* +0x002C (6) */
    int16_t  ibmp;           /* +0x0032 (2) */
    int16_t  dRangeMax;      /* +0x0034 (2) */
    int16_t  dp;             /* +0x0036 (2) */
    int16_t  init;           /* +0x0038 (2) */
    int16_t  dHitChance;     /* +0x003A (2) */
} TORP;                      /* size=0x3c */

typedef struct _turnserial {
    int32_t lSerialNumber; /* +0x0000 (4) */
    uint8_t rgbConfig[11]; /* +0x0004 (11) */
    uint8_t bPad;          /* +0x000F (1) */
} TURNSERIAL;              /* size=0x10 */

typedef struct _vers {
    uint16_t verInc : 5, /* +0x0000 (2) @bit0 */
        verMinor : 7,    /* @bit5 */
        verMajor : 4;    /* @bit12 */
} VERS;                  /* size=0x2 */

typedef struct _wn {
    RECT     rc;             /* +0x0000 (8) */
    uint16_t fMaximized : 1, /* +0x0008 (2) @bit0 */
        fMinimized : 1,      /* @bit1 */
        fInitalized : 1,     /* @bit2 */
        fUnused : 13;        /* @bit3 */
} WN;                        /* size=0xa */

typedef struct _ini {
    WN wnFrame; /* +0x0000 (10) */
    union {
        uint16_t fStartupFile : 1, /* +0x000A (2) @bit0 */
            fCmdLine : 1,          /* @bit1 */
            fWait : 1,             /* @bit2 */
            fGen : 1,              /* @bit3 */
            fTry : 1,              /* @bit4 */
            grobjSel : 4,          /* @bit5 */
            fBatch : 1,            /* @bit9 */
            fNewGame : 1,          /* @bit10 */
            fDumpFleets : 1,       /* @bit11 */
            fDumpPlanets : 1,      /* @bit12 */
            fDumpMap : 1,          /* @bit13 */
            fValidate : 1,         /* @bit14 */
            fLogging : 1;          /* @bit15 */
        uint16_t wFlags;           /* +0x000A (2) */
    };
    uint16_t turn;     /* +0x000C (2) */
    int16_t  iObjSel;  /* +0x000E (2) */
    int16_t  idPlayer; /* +0x0010 (2) */
    int32_t  lid;      /* +0x0012 (4) */
    int16_t  cTurnGen; /* +0x0016 (2) */
    int16_t  iMsg;     /* +0x0018 (2) */
} INI;                 /* size=0x1a */

typedef struct _xfer {
    int16_t    id;    /* +0x0000 (2) */
    GrobjClass grobj; /* +0x0002 (2) */
    union {
        FLEET  fl; /* +0x0004 (124) */
        PLANET pl; /* +0x0004 (56) */
        THING  th; /* +0x0004 (18) */
    };
} XFER; /* size=0x80 */

typedef struct _xferfull {
    uint16_t id1;        /* +0x0000 (2) */
    uint16_t id2;        /* +0x0002 (2) */
    uint8_t  grobj1 : 4, /* +0x0004 (1) @bit0 */
        grobj2 : 4;      /* @bit4 */
    int32_t rgcQuan[5];  /* +0x0005 (20) */
} XFERFULL;              /* size=0x19 */

typedef struct _ziporder {
    TASKXPORT txp;        /* +0x0000 (10) */
    char      szName[13]; /* +0x000A (13) */
    uint8_t   fValid;     /* +0x0017 (1) */
} ZIPORDER;               /* size=0x18 */

typedef struct _zipprodq1 {
    uint8_t fNoResearch; /* +0x0000 (1) */
    uint8_t cpq;         /* +0x0001 (1) */
    PRODQ1  rgpq[12];    /* +0x0002 (24) */
} ZIPPRODQ1;             /* size=0x1a */

typedef struct _player {
    char     iPlayer;     /* +0x0000 (1) */
    char     cShDef;      /* +0x0001 (1) */
    int16_t  cPlanet;     /* +0x0002 (2) */
    uint16_t cFleet : 12, /* +0x0004 (2) @bit0 */
        cshdefSB : 4;     /* @bit12 */
    union {
        uint16_t det : 3, /* +0x0006 (2) @bit0 */
            reserved : 9, /* @bit0 */
            iPlrBmp : 5,  /* @bit3 */
            fInclude : 1, /* @bit8 */
            mdPlayer : 7, /* @bit9 */
            fAi : 1,      /* @bit9 */
            lvlAi : 3,    /* @bit10 */
            idAi : 3;     /* @bit13 */
        uint16_t wMdPlr;  /* +0x0006 (2) */
    };
    int16_t  idPlanetHome;   /* +0x0008 (2) */
    uint16_t wScore;         /* +0x000A (2) */
    int32_t  lSalt;          /* +0x000C (4) */
    char     rgEnvVar[3];    /* +0x0010 (3) */
    char     rgEnvVarMin[3]; /* +0x0013 (3) */
    char     rgEnvVarMax[3]; /* +0x0016 (3) */
    char     pctIdealGrowth; /* +0x0019 (1) */
    int8_t   rgTech[6];      /* +0x001A (6) */
    uint32_t rgResSpent[6];  /* +0x0020 (24) */
    char     pctResearch;    /* +0x0038 (1) */
    char     iTechCur;       /* +0x0039 (1) */
    int32_t  lResLastYear;   /* +0x003A (4) */
    char     rgAttr[16];     /* +0x003E (16) */
    uint32_t grbitAttr;      /* +0x004E (4) */
    uint16_t grbitTrader;    /* +0x0052 (2) */
    union {
        uint16_t fDead : 1, /* +0x0054 (2) @bit0 */
            fCrippled : 1,  /* @bit1 */
            fCheater : 1,   /* @bit2 */
            fLearned : 1,   /* @bit3 */
            fHacker : 1,    /* @bit4 */
            unused : 11;    /* @bit5 */
        uint16_t wFlags;    /* +0x0054 (2) */
    };
    ZIPPRODQ1 zpq1;             /* +0x0056 (26) */
    int8_t    rgmdRelation[16]; /* +0x0070 (16) */
    char      szName[32];       /* +0x0080 (32) */
    char      szNames[32];      /* +0x00A0 (32) */
} PLAYER;                       /* size=0xc0 */

typedef struct _tutor {
    union {
        int16_t  wFlags;       /* +0x0000 (2) */
        uint16_t fVisible : 1, /* +0x0000 (2) @bit0 */
            fGameSaved : 1,    /* @bit1 */
            fChange : 1,       /* @bit2 */
            fTurnDone : 1,     /* @bit3 */
            fTutorDone : 1,    /* @bit4 */
            fNoErrors : 1,     /* @bit5 */
            cError : 3,        /* @bit6 */
            fAutoComplete : 1, /* @bit9 */
            fProgress : 1,     /* @bit10 */
            fTBVis : 1,        /* @bit11 */
            fValidQ : 1,       /* @bit12 */
            fFreeing : 1,      /* @bit13 */
            fShowHidMsg : 1,   /* @bit14 */
            unused : 1;        /* @bit15 */
    };
    int16_t   idt;       /* +0x0002 (2) */
    int16_t   idtBold;   /* +0x0004 (2) */
    int16_t   idh;       /* +0x0006 (2) */
    int16_t   idsError;  /* +0x0008 (2) */
    int16_t   iScanZoom; /* +0x000A (2) */
    int16_t   icolFSort; /* +0x000C (2) */
    uint16_t  grbitScan; /* +0x000E (2) */
    HWND      hwnd;      /* +0x0010 (2) */
    ZIPPRODQ1 zpq;       /* +0x0012 (26) */
} TUTOR;                 /* size=0x2c */

typedef struct _zipprodq {
    char    szName[13]; /* +0x0000 (13) */
    uint8_t fValid;     /* +0x000D (1) */
    union {
        ZIPPRODQ1 zpq1; /* +0x000E (26) */
        struct {
            uint8_t fNoResearch; /* +0x000E (1) */
            uint8_t cpq;         /* +0x000F (1) */
            PRODQ1  rgpq[12];    /* +0x0010 (24) */
        };
    };
} ZIPPRODQ; /* size=0x28 */


/* ======================== END generated game types ========================= */
