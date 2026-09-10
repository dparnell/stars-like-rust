//! The tutorial: eighty pages that walk a new player through 36 years.
//!
//! The interesting thing about it, for this project, is not that it teaches.
//! It is that fifty-five call sites reach into it and its checks read almost
//! everything the UI can set, so a tutorial that runs to the end is a
//! statement that the UI does what the game says it does.
//!
//! Three pieces:
//!
//! * the **text**, eighty pages of eight paragraphs, read at run time out of
//!   the player's own copy of the game — see [`stars_formats::tutorial`];
//! * the **step table**, one step per page, each naming the paragraph to
//!   embolden and the thing the player has to do;
//! * the **checks**, fifteen verbs that between them ask about the selection,
//!   a fleet's waypoints and cargo, the production queue, research, the
//!   scanner, the designer and the messages.
//!
//! `AdvanceTutor` (`10f8:0a30`) is the whole of the machine: ask whether this
//! page's task is done, and while it is, step on by eight and ask again — so
//! a page you satisfied in advance is skipped rather than shown.
//!
//! See `docs/ui/tutorial.md`.

use stars_formats::tutorial::PARAGRAPHS_PER_PAGE;

/// The object classes the checks name things by, which are the game's own
/// `GrobjClass` bits.
pub use stars_core::fleet::grobj;

/// "Any" — the value the checks pass when a field is not to be compared.
///
/// `FCheckFleetWP` (`10f8:6df4`) tests `id & 0x7fff == 0x7fff` and compares
/// the task and warp only when they are not `0xffff`, so one sentinel does
/// for all three.
pub const ANY: u16 = 0xffff;

/// Where the tutorial has got to.
///
/// The original keeps this in one 0x2c-byte global, `tutor`, which
/// `StartTutor` (`10f8:06b4`) zeroes before it begins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tutor {
    /// The current paragraph. A page is eight of them, so the page number in
    /// the title is `idt / 8 + 1`.
    pub idt: usize,
    /// Which paragraph to embolden — how a page points at the one thing you
    /// have to do. It is an absolute paragraph number, not an offset.
    pub bold: usize,
    /// A complaint, when the last thing the player did was wrong rather than
    /// merely not right yet (`TutorError`, `10f8:67ae`).
    pub error: Option<u16>,
    /// The help topic the current page offers, which the checks set as they
    /// work out *why* the task is not done.
    pub help: u16,
    /// Whether the tutorial has run off the end of page eighty.
    pub finished: bool,
}

impl Tutor {
    /// Which page is showing, one-based, as the title bar counts them.
    #[must_use]
    pub fn page(&self) -> usize {
        self.idt / PARAGRAPHS_PER_PAGE + 1
    }

    /// Which paragraph of the page is emboldened, `0` to `7`, if the bold
    /// paragraph is on this page at all.
    #[must_use]
    pub fn bold_line(&self) -> Option<usize> {
        (self.bold / PARAGRAPHS_PER_PAGE == self.idt / PARAGRAPHS_PER_PAGE)
            .then_some(self.bold % PARAGRAPHS_PER_PAGE)
    }
}

/// One thing a page asks the player to do.
///
/// These are the fifteen `FCheck*` verbs, with the arguments the step table
/// passes them. Where the original passes `0xffff` for "don't care" this
/// carries [`ANY`], and where it passes `-1` this carries `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// `FCheckSelection` (`10f8:6af4`): that object is selected.
    Selection { class: u8, id: i16 },
    /// `FCheckSummary` (`10f8:69e2`): the summary pane is showing it.
    Summary { class: u8, id: i16 },
    /// `FCheckMessages` (`10f8:6c48`): the messages have been read.
    ///
    /// `9999` means "all of them"; anything else is "you have reached this
    /// one". `kind` is `None` for "any message"; with a kind, `filter` says
    /// which question is being asked about it — whether that kind has been
    /// **filtered out**, or whether the message in front is one of them.
    Messages {
        message: i32,
        kind: Option<u16>,
        filter: bool,
    },
    /// `FCheckFleetWP` (`10f8:6df4`): a fleet's waypoint is where it should
    /// be, doing what it should.
    FleetWaypoint {
        /// The fleet's own id, not its index.
        fleet: u16,
        /// Which waypoint, counting the fleet's own position as zero.
        order: usize,
        /// What the waypoint should be on.
        class: u8,
        /// Which object, or [`ANY`].
        id: u16,
        /// The task it should carry, or [`ANY`].
        task: u16,
        /// The warp it should be flown at, or [`ANY`].
        warp: u16,
    },
    /// `FCheckColonizeWP` (`10f8:70c0`): a colonize task set on waypoint one.
    ColonizeWaypoint { fleet: u16, id: u16, warp: u16 },
    /// `FCheckXferWP` (`10f8:7280`): a Transport task with the right cargo
    /// instructions on it.
    ///
    /// Only the **action** of each cargo is compared, unless the action is
    /// `UnloadExact` or `SetAmount`, where the quantity is compared too —
    /// the other actions carry no meaningful figure.
    TransportWaypoint {
        fleet: u16,
        order: usize,
        id: u16,
        warp: u16,
        /// One action per cargo, ironium first and fuel last.
        goal: [stars_formats::XferAction; 5],
    },
    /// Whether the Research dialog is open.
    ///
    /// The arms read `pctResGlob`, which is `-1` while the dialog is shut
    /// and holds its pending percentage while it is up. Page 28 wants it
    /// opened and page 29 wants it closed again, so both senses are used.
    ResearchDialog { open: bool },
    /// How many fleets the player has.
    ///
    /// Read off `rgplr[idPlayer].cFleet & 0xfff` in the arms. It is how the
    /// tutorial checks that a fleet has been **split**: page 26 wants eleven
    /// where there were nine.
    FleetCount { count: usize, cmp: Cmp },
    /// Whether a fleet has **Repeat Orders** ticked.
    ///
    /// Read off `pfl->det` bit 9 in the arms, the same bit
    /// `ShipCommandProc` sets from the pane's checkbox. Page 22 wants it on
    /// so the freighter shuttles back and forth without being told again.
    RepeatOrders { fleet: u16 },
    /// How many entries a planet's production queue has.
    ///
    /// Read straight off the queue's own count byte in the arms, like
    /// [`Check::FleetOrders`], and used the same way: page 18 is done when
    /// the homeworld's queue has grown to three.
    QueueLength { planet: i16, count: usize, cmp: Cmp },
    /// How many orders a fleet has.
    ///
    /// Not a verb of its own in the original — the arms read `pfl->cord`
    /// directly — but it is how the tutorial checks that a waypoint has been
    /// **deleted**: page 16 is done when Armed Probe #1 no longer has six.
    FleetOrders { fleet: u16, count: usize, cmp: Cmp },
    /// `FCheckCargo` (`10f8:7664`): exactly this aboard and nothing else.
    Cargo {
        fleet: u16,
        /// Ironium, boranium, germanium.
        minerals: [i32; 3],
        colonists: i32,
    },
    /// `FCheckQueue` (`10f8:7442`): that item, that many, in that slot.
    Queue {
        planet: i16,
        slot: usize,
        /// Whether the entry builds a ship.
        ship: bool,
        item: u16,
        count: u16,
        /// Whether the entry is marked *contribute only leftover resources
        /// to research*, or `None` when the page does not care.
        no_research: Option<bool>,
    },
    /// `FCheckResearch` (`10f8:6da4`): the field, what follows it, and the
    /// percentage.
    Research { field: usize, next: u8, pct: u8 },
    /// `FCheckScanner` (`10f8:685c`): the scanner's view and zoom.
    ///
    /// A view below six is one of the exclusive views and is compared against
    /// the low nibble; anything larger is a mask of overlay bits and must all
    /// be on.
    Scanner { view: Option<u16>, zoom: Option<i8> },
    /// `FCheckPlanetRoute` (`10f8:6f86`): a planet's route destination.
    PlanetRoute { planet: i16, to: i16 },
    /// `FCheckShipBuilder` (`10f8:7964`): the designer open on that design.
    ShipBuilder {
        starbase: Option<bool>,
        design: Option<usize>,
    },
}

impl Check {
    /// A short name for the verb, for a frontend that wants to say which
    /// check a page is waiting on.
    #[must_use]
    pub fn verb(&self) -> &'static str {
        match self {
            Check::Selection { .. } => "selection",
            Check::Summary { .. } => "summary",
            Check::Messages { .. } => "messages",
            Check::FleetWaypoint { .. } => "fleet waypoint",
            Check::ColonizeWaypoint { .. } => "colonize waypoint",
            Check::Cargo { .. } => "cargo",
            Check::Queue { .. } => "production queue",
            Check::Research { .. } => "research",
            Check::Scanner { .. } => "scanner",
            Check::PlanetRoute { .. } => "planet route",
            Check::ShipBuilder { .. } => "ship designer",
            Check::TransportWaypoint { .. } => "transport waypoint",
            Check::FleetOrders { .. } => "order count",
            Check::QueueLength { .. } => "queue length",
            Check::RepeatOrders { .. } => "repeat orders",
            Check::FleetCount { .. } => "fleet count",
            Check::ResearchDialog { .. } => "research dialog",
        }
    }
}

/// How a count is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    /// Fewer than the number given.
    Fewer,
    /// Exactly it.
    Exactly,
    /// Anything but it — which is how "delete one of these" is asked.
    NotExactly,
    /// At least it. Page 23 wants the homeworld's queue to have grown to
    /// three or more, whatever else has been put in it.
    AtLeast,
}

/// The last paragraph there is.
///
/// `AdvanceTutor` (`10f8:0a30`) ends the tutorial once `idt` passes this.
pub const LAST_PARAGRAPH: usize = 0x27f;

/// One rung of a page's task.
///
/// A page is not one check but a **chain** of them: the original writes
/// `if (FCheckSelection(...)) { idtBold = ...; done = FCheckFleetWP(...); }`,
/// so each rung both emboldens a paragraph and gates the next. The page is
/// done when every rung passes, and the paragraph emboldened is the first
/// rung that does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    /// The paragraph to embolden while this rung is the one being waited on.
    pub bold: usize,
    /// What has to be true. `None` is a rung that only moves the emphasis —
    /// the closing pages of a year, which ask for nothing.
    pub check: Option<Check>,
    /// Whether this rung **gates** the page, or only moves the emphasis.
    ///
    /// Not every check in an arm is part of the answer. Some are asked only
    /// to decide which paragraph to embolden — page 9 asks whether you have
    /// read the fourth message and then, whatever the answer, gates on the
    /// summary pane instead. A rung that does not gate is skipped when
    /// deciding whether the page is done, but still catches the emphasis on
    /// its way past.
    pub gates: bool,
}

/// One page of the tutorial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The year this page belongs to, counted from the tutorial's first.
    ///
    /// `FTutorTaskDone` (`10f8:0fbc`) is a `switch (game.turn)`, so a page is
    /// only ever asked about in its own year.
    pub turn: i16,
    /// The first paragraph of the page.
    pub idt: usize,
    /// A way past the page without doing any of it.
    ///
    /// Several arms open with `if (FCheckX(...)) done = 1; else { ...the
    /// chain... }` — where `FCheckX` is usually the *next* page's task. A
    /// player who has run ahead is not made to go back and do this page
    /// step by step. It is the same idea as `AdvanceTutor`'s skipping loop,
    /// written inside one page.
    pub escape: Option<Check>,
    /// The rungs, in order.
    pub stages: &'static [Stage],
}

impl Step {
    /// Which page this is, one-based.
    #[must_use]
    pub fn page(&self) -> usize {
        self.idt / PARAGRAPHS_PER_PAGE + 1
    }
}

/// The step whose page begins at `idt`.
#[must_use]
pub fn step(idt: usize) -> Option<&'static Step> {
    STEPS.iter().find(|step| step.idt == idt)
}

/// A rung that has to be satisfied for the page to be done.
const fn ask(bold: usize, check: Check) -> Stage {
    Stage {
        bold,
        check: Some(check),
        gates: true,
    }
}

/// A rung asked only to decide where the emphasis goes.
const fn hint(bold: usize, check: Check) -> Stage {
    Stage {
        bold,
        check: Some(check),
        gates: false,
    }
}

/// The tutorial's pages, in order.
///
/// Recovered from `FTutorTaskDone` (`10f8:0fbc`), a `switch (game.turn)` with
/// a chain of `if (tutor.idt == n)` inside each arm. Pages not yet transcribed
/// are simply absent, and the tutorial stops at the first gap rather than
/// pretending to know what comes next.
pub static STEPS: &[Step] = &[
    // Year 0. Read the messages, then look at each of the fleets in turn and
    // send it somewhere, then set the research going.
    Step {
        turn: 0,
        idt: 0,
        escape: None,
        stages: &[ask(
            5,
            Check::Messages {
                message: 9999,
                kind: None,
                filter: false,
            },
        )],
    },
    Step {
        turn: 0,
        idt: 8,
        escape: None,
        stages: &[
            ask(
                11,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 0,
                },
            ),
            ask(
                15,
                Check::FleetWaypoint {
                    fleet: 0,
                    order: 1,
                    class: grobj::PLANET,
                    id: 0x0c,
                    task: 0,
                    warp: ANY,
                },
            ),
        ],
    },
    Step {
        turn: 0,
        idt: 16,
        escape: None,
        stages: &[
            ask(
                18,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 1,
                },
            ),
            ask(
                21,
                Check::FleetWaypoint {
                    fleet: 1,
                    order: 1,
                    class: grobj::PLANET,
                    id: 0x10,
                    task: 0,
                    warp: ANY,
                },
            ),
        ],
    },
    Step {
        turn: 0,
        idt: 24,
        escape: None,
        stages: &[
            // The original picks a different paragraph for each of the three
            // wrong fleets you might have selected instead; this points at
            // the one that names the right one.
            ask(
                25,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 4,
                },
            ),
            ask(
                31,
                Check::FleetWaypoint {
                    fleet: 4,
                    order: 1,
                    class: grobj::PLANET,
                    id: 0x0f,
                    task: 0,
                    warp: ANY,
                },
            ),
        ],
    },
    Step {
        turn: 0,
        idt: 32,
        escape: None,
        stages: &[ask(
            32,
            Check::Research {
                field: 1,
                next: 6,
                pct: 15,
            },
        )],
    },
    // Year 1. The first thing the production queue is asked for.
    Step {
        turn: 1,
        idt: 40,
        escape: None,
        stages: &[ask(
            40,
            Check::Queue {
                planet: 0x0d,
                slot: 0,
                ship: false,
                item: stars_core::production::item::FACTORY,
                count: 20,
                no_research: Some(false),
            },
        )],
    },
    // Year 2. The scouts have arrived and are given a string of waypoints
    // each, one planet per paragraph.
    //
    // These pages set the pattern for the rest of the tutorial: the
    // selection is a **hint**, not a gate — the original never makes you
    // select the right fleet, it just tells you to while you have not — and
    // each leg laid moves the emphasis to the paragraph naming the next
    // planet.
    Step {
        turn: 2,
        idt: 48,
        // Fleet 1 already sent on: the next page's work is done, so this one
        // is not asked for.
        escape: Some(leg(1, 1, 0x15)),
        stages: &[
            hint(
                0x30,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 0,
                },
            ),
            ask(0x32, leg(0, 1, 0x09)),
            ask(0x33, leg(0, 2, 0x03)),
            ask(0x34, leg(0, 3, 0x08)),
            ask(0x35, leg(0, 4, 0x05)),
            ask(0x36, leg(0, 5, 0x02)),
            ask(
                0x37,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 1,
                },
            ),
        ],
    },
    Step {
        turn: 2,
        idt: 56,
        escape: Some(leg(4, 1, 0x0e)),
        stages: &[
            ask(0x39, leg(1, 1, 0x15)),
            ask(0x3a, leg(1, 2, 0x13)),
            ask(0x3b, leg(1, 3, 0x14)),
            ask(0x3c, leg(1, 4, 0x07)),
            hint(
                0x3d,
                Check::Messages {
                    message: 2,
                    kind: None,
                    filter: false,
                },
            ),
            ask(
                0x3f,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 4,
                },
            ),
        ],
    },
    Step {
        turn: 2,
        idt: 64,
        // The miner already sent to mine: page 10's work.
        escape: Some(mine(5, 0x0c)),
        stages: &[
            ask(0x40, leg(4, 1, 0x0e)),
            ask(0x41, leg(4, 2, 0x11)),
            ask(0x42, leg(4, 3, 0x12)),
            ask(0x43, leg(4, 4, 0x17)),
            ask(0x44, leg(4, 5, 0x16)),
            // Reading the fourth message moves the emphasis on; it is not
            // part of the answer.
            hint(
                0x45,
                Check::Messages {
                    message: 4,
                    kind: None,
                    filter: false,
                },
            ),
            ask(
                0x47,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x0c,
                },
            ),
        ],
    },
    Step {
        turn: 2,
        idt: 72,
        // The colony ship already sent: page 12's work.
        escape: Some(Check::ColonizeWaypoint {
            fleet: 2,
            id: 0x10,
            warp: ANY,
        }),
        stages: &[
            // Three questions about one waypoint, and only the last is the
            // answer: select the miner, lay the leg, then set the task. The
            // first two walk the emphasis from "select it" to "now use the
            // dropdown", which is the whole of what a tutorial does.
            hint(
                0x4c,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 5,
                },
            ),
            hint(0x4d, leg(5, 1, 0x0c)),
            ask(0x4f, mine(5, 0x0c)),
        ],
    },
    Step {
        turn: 2,
        idt: 80,
        escape: Some(Check::ColonizeWaypoint {
            fleet: 2,
            id: 0x10,
            warp: ANY,
        }),
        stages: &[
            hint(
                0x50,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x0f,
                },
            ),
            hint(
                0x53,
                Check::Messages {
                    message: 9999,
                    kind: None,
                    filter: false,
                },
            ),
            ask(
                0x56,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x10,
                },
            ),
        ],
    },
    Step {
        turn: 2,
        idt: 88,
        escape: None,
        stages: &[
            hint(
                0x59,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 2,
                },
            ),
            // Load the colony ship. How much is not readable from the
            // decompilation — `FCheckCargo`'s argument list comes out
            // garbled — but the page itself says it: "Click and drag in the
            // Colonists gauge filling the hold with 25kT of colonists".
            hint(
                0x5a,
                Check::Cargo {
                    fleet: 2,
                    minerals: [0, 0, 0],
                    colonists: 25,
                },
            ),
            hint(0x5c, leg(2, 1, 0x10)),
            ask(
                0x5d,
                Check::ColonizeWaypoint {
                    fleet: 2,
                    id: 0x10,
                    warp: ANY,
                },
            ),
        ],
    },
    // Year 3. The first message the tutorial asks you to **filter**, and the
    // first auto-build order.
    Step {
        turn: 3,
        idt: 96,
        escape: None,
        stages: &[
            // "Your first message is quite common and we don't need to look
            // at it every year. Filter it out by clicking the blue check
            // mark in the upper left hand corner of the Messages pane."
            ask(
                0x61,
                Check::Messages {
                    message: -1,
                    kind: Some(stars_core::message::id::BUILT_FACTORIES),
                    filter: true,
                },
            ),
            hint(
                0x62,
                Check::Selection {
                    class: grobj::PLANET,
                    id: 0x0d,
                },
            ),
            // Thirty on auto-build: shift-Add three times, ten a go.
            ask(
                0x66,
                Check::Queue {
                    planet: 0x0d,
                    slot: 0,
                    ship: false,
                    item: stars_core::production::item::AUTO_FACTORY,
                    count: 30,
                    no_research: Some(false),
                },
            ),
        ],
    },
    Step {
        turn: 3,
        idt: 104,
        escape: None,
        stages: &[
            hint(
                0x68,
                Check::Selection {
                    class: grobj::PLANET,
                    id: 0x10,
                },
            ),
            // Three factories then three mines, with "contribute only
            // leftover resources to research" ticked — which is what the
            // `fNoResearch` argument is.
            ask(
                0x6b,
                Check::Queue {
                    planet: 0x10,
                    slot: 0,
                    ship: false,
                    item: stars_core::production::item::FACTORY,
                    count: 3,
                    no_research: Some(true),
                },
            ),
            ask(
                0x6b,
                Check::Queue {
                    planet: 0x10,
                    slot: 1,
                    ship: false,
                    item: stars_core::production::item::MINE,
                    count: 3,
                    no_research: Some(true),
                },
            ),
            hint(
                0x6d,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 3,
                },
            ),
            // Fill the freighter with colonists for the new colony.
            ask(
                0x6e,
                Check::Cargo {
                    fleet: 3,
                    minerals: [0, 0, 0],
                    colonists: 25,
                },
            ),
        ],
    },
    Step {
        turn: 3,
        idt: 112,
        escape: None,
        stages: &[
            hint(0x70, at_planet(3, 1, 0x10, ANY)),
            hint(0x71, at_planet(3, 1, 0x10, TRANSPORT_TASK)),
            // "change the waypoint task to Transport. Then right click on
            // the blue diamond and select QuikDrop to empty the freighter's
            // hold at 90210."
            ask(
                0x72,
                Check::TransportWaypoint {
                    fleet: 3,
                    order: 1,
                    id: 0x10,
                    warp: ANY,
                    goal: [stars_formats::XferAction::UnloadAll; 5],
                },
            ),
            hint(
                0x73,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x09,
                },
            ),
            ask(
                0x76,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 0,
                },
            ),
        ],
    },
    Step {
        turn: 3,
        idt: 120,
        escape: None,
        stages: &[
            hint(
                0x79,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x09,
                },
            ),
            // "Armed Probe #1 doesn't need to go all the way to Hiho so
            // let's get rid of that waypoint. Click on Hiho and hit the
            // Delete key." Done when it no longer has its six.
            ask(
                0x7b,
                Check::FleetOrders {
                    fleet: 0,
                    count: 6,
                    cmp: Cmp::NotExactly,
                },
            ),
        ],
    },
    // Year 4. Tidying up: a waypoint deleted from the destroyer's path, and
    // the first ship queued.
    Step {
        turn: 4,
        idt: 128,
        // The colony ship already queued: page 18's work.
        escape: Some(Check::QueueLength {
            planet: 0x0d,
            count: 3,
            cmp: Cmp::Exactly,
        }),
        stages: &[
            hint(
                0x80,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x0e,
                },
            ),
            ask(
                0x82,
                Check::FleetOrders {
                    fleet: 4,
                    count: 6,
                    cmp: Cmp::NotExactly,
                },
            ),
            hint(
                0x85,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x15,
                },
            ),
            ask(
                0x87,
                Check::Selection {
                    class: grobj::PLANET,
                    id: 0x0d,
                },
            ),
        ],
    },
    Step {
        turn: 4,
        idt: 136,
        escape: None,
        stages: &[
            ask(
                0x88,
                Check::QueueLength {
                    planet: 0x0d,
                    count: 3,
                    cmp: Cmp::Exactly,
                },
            ),
            // "Double click on Santa Maria in the left hand listbox and hit
            // OK" — a ship, not an installation, so the entry's class is a
            // fleet and its item is a design slot.
            ask(
                0x8f,
                Check::Queue {
                    planet: 0x0d,
                    slot: 1,
                    ship: true,
                    item: 2,
                    count: 1,
                    no_research: Some(false),
                },
            ),
        ],
    },
    // Year 5. The second colony goes out, and the scanner is switched to a
    // value view and back.
    Step {
        turn: 5,
        idt: 144,
        escape: None,
        stages: &[
            hint(
                0x90,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 2,
                },
            ),
            ask(
                0x92,
                Check::Cargo {
                    fleet: 2,
                    minerals: [0, 0, 0],
                    colonists: 25,
                },
            ),
            hint(
                0x96,
                Check::Scanner {
                    view: Some(3),
                    zoom: None,
                },
            ),
            ask(0x97, at_planet(2, 1, 0x0e, ANY)),
        ],
    },
    Step {
        turn: 5,
        idt: 152,
        escape: None,
        stages: &[
            ask(
                0x98,
                Check::ColonizeWaypoint {
                    fleet: 2,
                    id: 0x0e,
                    warp: ANY,
                },
            ),
            // "Switch the Scanner back to normal view by clicking the
            // leftmost toolbar button" — view 0 is that button.
            hint(
                0x99,
                Check::Scanner {
                    view: Some(0),
                    zoom: None,
                },
            ),
            hint(
                0x9a,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 3,
                },
            ),
            ask(0x9b, at_planet(3, 1, 0x0d, ANY)),
            hint(
                0x9c,
                Check::Messages {
                    message: 9999,
                    kind: None,
                    filter: false,
                },
            ),
            ask(
                0x9f,
                Check::FleetOrders {
                    fleet: 0,
                    count: 5,
                    cmp: Cmp::NotExactly,
                },
            ),
        ],
    },
    // Year 6. The freighter is set to shuttle: load at Prune, unload at the
    // homeworld, and repeat.
    Step {
        turn: 6,
        idt: 160,
        escape: None,
        stages: &[
            hint(
                0xa0,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 3,
                },
            ),
            hint(0xa3, at_planet(3, 1, 0x0c, ANY)),
            hint(0xa4, at_planet(3, 1, 0x0c, TRANSPORT_TASK)),
            // "right click on the blue diamond and select QuikLoad from the
            // Zip menu."
            ask(
                0xa5,
                Check::TransportWaypoint {
                    fleet: 3,
                    order: 1,
                    id: 0x0c,
                    warp: ANY,
                    goal: [stars_formats::XferAction::LoadAll; 5],
                },
            ),
            ask(0xa6, at_planet(3, 2, 0x0d, ANY)),
        ],
    },
    Step {
        turn: 6,
        idt: 168,
        escape: None,
        stages: &[
            // "Right click on the blue diamond and select QuikDrop from the
            // Zip menu."
            ask(
                0xaa,
                Check::TransportWaypoint {
                    fleet: 3,
                    order: 2,
                    id: 0x0d,
                    warp: ANY,
                    goal: [stars_formats::XferAction::UnloadAll; 5],
                },
            ),
            ask(0xab, Check::RepeatOrders { fleet: 3 }),
            hint(
                0xad,
                Check::Selection {
                    class: grobj::PLANET,
                    id: 0x0d,
                },
            ),
            ask(
                0xae,
                Check::QueueLength {
                    planet: 0x0d,
                    count: 3,
                    cmp: Cmp::Exactly,
                },
            ),
            ask(
                0xaf,
                Check::Queue {
                    planet: 0x0d,
                    slot: 1,
                    ship: true,
                    item: 2,
                    count: 1,
                    no_research: Some(false),
                },
            ),
        ],
    },
    // Year 7. A third colony, and the homeworld's queue grows to three
    // colony ships.
    Step {
        turn: 7,
        idt: 176,
        escape: None,
        stages: &[
            ask(
                0xb0,
                Check::Cargo {
                    fleet: 6,
                    minerals: [0, 0, 0],
                    colonists: 25,
                },
            ),
            hint(
                0xb1,
                Check::Scanner {
                    view: Some(3),
                    zoom: None,
                },
            ),
            ask(
                0xb3,
                Check::ColonizeWaypoint {
                    fleet: 6,
                    id: 0x12,
                    warp: ANY,
                },
            ),
            ask(
                0xb5,
                Check::QueueLength {
                    planet: 0x0d,
                    count: 3,
                    cmp: Cmp::AtLeast,
                },
            ),
            ask(
                0xb5,
                Check::Queue {
                    planet: 0x0d,
                    slot: 1,
                    ship: true,
                    item: 2,
                    count: 3,
                    no_research: Some(false),
                },
            ),
            ask(
                0xb6,
                Check::Scanner {
                    view: Some(0),
                    zoom: None,
                },
            ),
        ],
    },
    Step {
        turn: 7,
        idt: 184,
        escape: None,
        stages: &[
            // The second message the tutorial has you filter, and its
            // neighbour: mines rather than factories.
            hint(
                0xb8,
                Check::Messages {
                    message: 3,
                    kind: None,
                    filter: false,
                },
            ),
            ask(
                0xba,
                Check::Messages {
                    message: -1,
                    kind: Some(stars_core::message::id::BUILT_MINES),
                    filter: true,
                },
            ),
            hint(
                0xbb,
                Check::Selection {
                    class: grobj::PLANET,
                    id: 0x10,
                },
            ),
            // "Shift-double click on 'Factories (Auto Build)' and then on
            // 'Mines (Auto Build)' in the left hand listbox queueing 10 of
            // each" — both with leftover-only research still ticked.
            ask(
                0xbd,
                Check::Queue {
                    planet: 0x10,
                    slot: 0,
                    ship: false,
                    item: stars_core::production::item::AUTO_FACTORY,
                    count: 10,
                    no_research: Some(true),
                },
            ),
            ask(
                0xbd,
                Check::Queue {
                    planet: 0x10,
                    slot: 1,
                    ship: false,
                    item: stars_core::production::item::AUTO_MINE,
                    count: 10,
                    no_research: Some(true),
                },
            ),
            ask(
                0xbe,
                Check::Messages {
                    message: 9999,
                    kind: None,
                    filter: false,
                },
            ),
        ],
    },
    Step {
        turn: 7,
        idt: 192,
        escape: None,
        stages: &[
            // "Click on the red triangle between Slime and No Vacancy. This
            // is an enemy scout ship." Fleet ids carry their owner in the
            // high bits, so `0x200` is player 1's fleet 0 — somebody
            // else's, which is the point of the page.
            hint(
                0xc0,
                Check::Summary {
                    class: grobj::FLEET,
                    id: 0x200,
                },
            ),
            ask(
                0xc4,
                Check::QueueLength {
                    planet: 0x0d,
                    count: 4,
                    cmp: Cmp::AtLeast,
                },
            ),
            // "Add two Armed Probes to Stove Top's queue."
            ask(
                0xc6,
                Check::Queue {
                    planet: 0x0d,
                    slot: 2,
                    ship: true,
                    item: 0,
                    count: 2,
                    no_research: Some(false),
                },
            ),
        ],
    },
    // Year 8. The colony ships are split up so they do not all go to the
    // same planet.
    Step {
        turn: 8,
        idt: 200,
        escape: None,
        stages: &[
            hint(
                0xc8,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 7,
                },
            ),
            hint(
                0xcd,
                Check::Cargo {
                    fleet: 7,
                    minerals: [0, 0, 0],
                    colonists: 25,
                },
            ),
            hint(0xcd, at_planet(7, 1, 0x08, COLONIZE_TASK)),
            // "hit the Split button ... Move one of the Santa Marias over to
            // Fleet #10": nine fleets become eleven.
            ask(
                0xce,
                Check::FleetCount {
                    count: 11,
                    cmp: Cmp::Exactly,
                },
            ),
            ask(
                0xce,
                Check::ColonizeWaypoint {
                    fleet: 10,
                    id: 0x08,
                    warp: ANY,
                },
            ),
        ],
    },
    Step {
        turn: 8,
        idt: 208,
        escape: None,
        stages: &[
            // "Click on the waypoint at Slime and drag it to Sea Squared" —
            // the colonize order moves with the waypoint, from planet 8 to
            // planet 0x11.
            ask(
                0xd1,
                Check::ColonizeWaypoint {
                    fleet: 7,
                    id: 0x11,
                    warp: ANY,
                },
            ),
            hint(
                0xd4,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 8,
                },
            ),
            ask(0xd6, at_planet(8, 1, 0x09, ANY)),
        ],
    },
    Step {
        turn: 8,
        idt: 216,
        // Research already set the way page 29 wants it.
        escape: Some(Check::Research {
            field: 1,
            next: 6,
            pct: 30,
        }),
        stages: &[
            // The third message the tutorial teaches you to hide.
            ask(
                0xd8,
                Check::Messages {
                    message: -1,
                    kind: Some(stars_core::message::id::HAS_UNLOADED),
                    filter: true,
                },
            ),
            hint(
                0xd9,
                Check::Messages {
                    message: 9999,
                    kind: None,
                    filter: false,
                },
            ),
            hint(
                0xd9,
                Check::Summary {
                    class: grobj::PLANET,
                    id: 0x05,
                },
            ),
            ask(0xdd, Check::ResearchDialog { open: true }),
        ],
    },
    Step {
        turn: 8,
        idt: 224,
        escape: None,
        stages: &[
            ask(0xe4, Check::ResearchDialog { open: false }),
            ask(
                0xe6,
                Check::Research {
                    field: 1,
                    next: 6,
                    pct: 30,
                },
            ),
        ],
    },
    // Year 9. Research is switched to a different next-field, and the
    // miner's yearly report is silenced.
    Step {
        turn: 9,
        idt: 232,
        escape: None,
        stages: &[
            hint(0xe8, Check::ResearchDialog { open: false }),
            // The same field as page 29 but a different **next**: 3 rather
            // than 6, so once Weapons is done research moves on by itself
            // instead of staying put.
            ask(
                0xec,
                Check::Research {
                    field: 1,
                    next: 3,
                    pct: 30,
                },
            ),
            ask(
                0xee,
                Check::Messages {
                    message: -1,
                    kind: Some(stars_core::message::id::MINING_ROBOTS_LOADED),
                    filter: true,
                },
            ),
        ],
    },
];

/// The Colonize task id.
const COLONIZE_TASK: u16 = stars_formats::task::COLONIZE as u16;

/// The Transport task id, as a `Check`'s `task` field wants it.
const TRANSPORT_TASK: u16 = stars_formats::task::TRANSPORT as u16;

/// A waypoint on a planet with a given task, or [`ANY`] task.
const fn at_planet(fleet: u16, order: usize, id: u16, task: u16) -> Check {
    Check::FleetWaypoint {
        fleet,
        order,
        class: grobj::PLANET,
        id,
        task,
        warp: ANY,
    }
}

/// A waypoint with the **Remote Mining** task on it.
const fn mine(fleet: u16, id: u16) -> Check {
    Check::FleetWaypoint {
        fleet,
        order: 1,
        class: grobj::PLANET,
        id,
        task: stars_formats::task::REMOTE_MINING as u16,
        warp: ANY,
    }
}

/// A plain waypoint: fleet `fleet`'s leg `order` on planet `id`, no task and
/// any warp. Most of the table is these.
const fn leg(fleet: u16, order: usize, id: u16) -> Check {
    Check::FleetWaypoint {
        fleet,
        order,
        class: grobj::PLANET,
        id,
        task: 0,
        warp: ANY,
    }
}
