//! # stars-ui
//!
//! Shared, frontend-agnostic view code for the Stars! reimplementation.
//!
//! The intent is that every screen — galaxy map/starfield, the race-creation
//! wizard (mirroring `RACEWIZARDDLG1-6`), the production queue (`ZIPPRODDLG`),
//! and the ship/planet browsers and reports (`BROWSERWNDPROC`,
//! `ORDERINFODLG`) — is written once here (against `stars-core` state) and
//! reused by both the native (`stars-desktop`) and wasm (`stars-web`) shells.
//!
//! The egui/eframe views are built in Step 5 of the delivery plan. For now
//! this crate only holds the shared application-state container so the frontend
//! crates have something concrete to wire up.
//!
//! Before starting those views, read the Step 5 notes in
//! `docs/plans/stars-re-reimplementation.md`. Several things the screens need
//! are already settled and should be driven off `stars-core` rather than
//! reimplemented: which planets a player merely knows about
//! (`Planet::detail`), which production items a race may build
//! (`ground::template_allows`), what a queue entry's fields actually mean, and
//! the two habitability figures the Selection Summary shows, and where planet
//! coordinates and names come from (the `.xy`, via
//! `GameState::apply_universe`).
//!
//! Two of those notes are worth repeating here because they shape every screen.
//! **Anything the game counts down is a running balance, not a record of a
//! decision** — a production queue entry shows what is left, not what was
//! ordered. And **a player's file holds only that player's own ship designs**,
//! so a screen showing an enemy ship cannot look its design up by slot number;
//! the battle recording's initiative range is all there is.

#![forbid(unsafe_code)]

pub mod app;
pub mod art;
pub mod popup;
pub mod statusbar;
pub mod toolbar;
pub mod vcr;
pub mod views;

pub use popup::{FleetRow, FleetSummary, PlanetSummary, Popup};
pub use statusbar::{Distance, StatusBar};

pub use app::{
    copied_plan_name, distance_figure, distance_text, fleet_arrow, leg_outside_waypoints, App,
    BattlePlans, Browser, CoverageDisc, DesignView, Designer, DesignerDrag, Editing, FilterCommand,
    FilterEntry, FindResult, MineralBar, OrbitRing, PartRow, PasswordDialog, PasswordPrompt,
    PathLeg, PlanetMark, PlanetNameStyle, Production, RacePage, RaceWizard, ResearchDialog,
    ScaleLine, ScanMenuItem, ScanObject, ScanOverlays, ScanThing, ScanView, SchematicSlot, Screen,
    Selection, SurveyBar, SurveySubject, TurnStatus, WindowLayout, ARROW_SHEET, COVERAGE_NORMAL,
    COVERAGE_PENETRATING, DIGIT_HEIGHT, DIGIT_SHEET, DIGIT_WIDTH, DOUBLED_LEG_COLOUR,
    DOUBLED_LEG_PLAIN, DRIVER_COLOUR, MAX_BATTLE_PLANS, MINERAL_BAR_LAYOUT, MINERAL_GRAPH_MAX,
    PATH_COLOUR, PLANET_UNEXPLORED, POPULATION_STEPS, RACE_WIZARD_PAGES, ROUTE_COLOUR, SCALE_YEARS,
    SCAN_FRIEND, SCAN_OTHER, SCAN_YOURS, SHIP_PATH_COLOUR, WAYPOINT_HOLE, WORMHOLE_CELL,
    WORMHOLE_MASK, WORMHOLE_SIDE,
};
