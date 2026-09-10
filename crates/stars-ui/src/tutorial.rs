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
    /// one". `kind` is `None` for "any message".
    Messages { message: i32, kind: Option<u16> },
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
        }
    }
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
                item: 7,
                count: 20,
            },
        )],
    },
    // Year 2. The scouts have arrived and are given a string of waypoints
    // each, one planet per paragraph.
    Step {
        turn: 2,
        idt: 48,
        // Fleet 1 already sent on: the next page's work is done, so this one
        // is not asked for.
        escape: Some(leg(1, 1, 0x15)),
        stages: &[
            ask(
                0x32,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 0,
                },
            ),
            ask(0x33, leg(0, 1, 0x09)),
            ask(0x34, leg(0, 2, 0x03)),
            ask(0x35, leg(0, 3, 0x08)),
            ask(0x36, leg(0, 4, 0x05)),
            ask(0x37, leg(0, 5, 0x02)),
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
            ask(
                0x3d,
                Check::Messages {
                    message: 2,
                    kind: None,
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
            hint(
                0x4d,
                Check::Selection {
                    class: grobj::FLEET,
                    id: 5,
                },
            ),
            // The leg without the task on it: the emphasis moves to the
            // paragraph about the dropdown.
            hint(0x4f, leg(5, 1, 0x0c)),
            ask(0x4f, mine(5, 0x0c)),
        ],
    },
];

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
