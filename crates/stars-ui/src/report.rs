//! The four report windows: their columns, and how they sort.
//!
//! `SortReportCache` (`1108:589c`) knows four reports — planets, your fleets,
//! everybody else's fleets, and battles — and each keeps a [`ReportState`] of
//! its own. The window that shows one is a plain grid: a row of raised header
//! cells, then one row per object, drawn by `DrawReport` (`1108:0bae`) and
//! `DrawReportItem` (`1108:3398`).
//!
//! Three things about the sort are worth knowing before reading the code.
//!
//! **You do not click a header to sort.** Clicking one — with either button;
//! `ReportDlg` (`1108:0018`) passes `WM_LBUTTONDOWN`, `WM_LBUTTONDBLCLK` and
//! `WM_RBUTTONDOWN` to the same place — opens a menu, and the sort is one of
//! its items. See [`ColumnMenu`].
//!
//! **The previous column is remembered and breaks ties.** `vicolSortPrev`,
//! `viSubsortPrev` and `vfAscendingPrev` take the current sort's values before
//! it is overwritten, and `ICompReport` (`1108:5bb8`) falls back to them when
//! the chosen column cannot separate two rows. Sort by population and then by
//! name and equally-named planets stay in population order. Those three are
//! **globals**, not per-report fields, so the tie-break survives switching
//! reports.
//!
//! **Three columns sort by a chosen mineral.** [`Subsort`] — the planets'
//! Minerals, Mining Rate and Min Conc, and the fleets' Cargo — and the choice
//! is part of the sort state, so it too is remembered and tie-broken.
//!
//! Written up in `docs/ui/reports.md`.

use std::cmp::Ordering;

use stars_core::GameState;
use stars_formats::battle::BattleRecord;

/// Which of the four reports (`RPT.irpt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Report {
    /// Your planets (`irpt` 0, `vrptPlanet`).
    Planets,
    /// Your fleets (`irpt` 1, `vrptFleet`).
    Fleets,
    /// Everybody else's fleets (`irpt` 2, `vrptEFleet`).
    EnemyFleets,
    /// Battles (`irpt` 3, `vrptBattle`).
    Battles,
}

/// What a column's menu offers below the sort, when it offers anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsort {
    /// Sorts on the column alone.
    None,
    /// The three minerals, then their sum — the planets' Minerals, Mining
    /// Rate and Min Conc columns.
    Minerals,
    /// The three minerals and colonists, then the lot — the fleets' Cargo
    /// column.
    Cargo,
}

/// One column of a report.
#[derive(Debug, Clone, Copy)]
pub struct Column {
    /// The header, as the game's own string table spells it.
    pub name: &'static str,
    /// Whether the column's sort menu opens onto a choice.
    pub subsort: Subsort,
}

const fn col(name: &'static str) -> Column {
    Column {
        name,
        subsort: Subsort::None,
    }
}

const fn minerals(name: &'static str) -> Column {
    Column {
        name,
        subsort: Subsort::Minerals,
    }
}

/// `idsPlanetName` (`0x459`) and the fourteen strings after it.
static PLANET_COLUMNS: &[Column] = &[
    col("Planet Name"),
    col("Starbase"),
    col("Population"),
    col("Cap"),
    col("Value"),
    col("Production"),
    col("Mine"),
    col("Fact"),
    col("Defense"),
    minerals("Minerals"),
    minerals("Mining Rate"),
    minerals("Min Conc"),
    col("Resources"),
    col("Driver Dest"),
    col("Routing Dest"),
];

/// `idsFleetName` (`0x472`) and the eleven strings after it.
static FLEET_COLUMNS: &[Column] = &[
    col("Fleet Name"),
    col("ID"),
    col("Location"),
    col("Destination"),
    col("ETA"),
    col("Task"),
    col("Fuel"),
    Column {
        name: "Cargo",
        subsort: Subsort::Cargo,
    },
    col("Composition"),
    col("Cloak"),
    col("Battle Plan"),
    col("Mass"),
];

/// `idsFleetName2` (`0x47e`) and the eleven strings after it.
static ENEMY_COLUMNS: &[Column] = &[
    col("Fleet Name"),
    col("ID"),
    col("Location"),
    col("Warp"),
    col("Mass"),
    col("Composition"),
    col("# of Ships"),
    col("Unarmed"),
    col("Scout"),
    col("Warship"),
    col("Bomber"),
    col("Utility"),
];

/// `idsLocation4` (`0x48a`) and the fourteen strings after it.
static BATTLE_COLUMNS: &[Column] = &[
    col("Location"),
    col("SB"),
    col("Sides"),
    col("Units"),
    col("Ours"),
    col("Theirs"),
    col("Unarmed"),
    col("Scout"),
    col("Warship"),
    col("Bomber"),
    col("Utility"),
    col("Our Dead"),
    col("Their Dead"),
    col("Ours Left"),
    col("Theirs Left"),
];

/// The names the mineral submenu lists, from the table of `char *` at
/// `1120:04cc`. The planets' columns take the first three; the fleets' Cargo
/// column takes all four.
pub static MINERAL_NAMES: [&str; 4] = ["Ironium", "Boranium", "Germanium", "Colonists"];

/// `idsWeightedAverage` (`0x55f`): the submenu's last item, which sorts on
/// every mineral at once. For Min Conc it really is an average of a sort; for
/// the others it is a plain sum.
pub const WEIGHTED_AVERAGE: &str = "Weighted Average";

/// `idsSort` (`0x46d`).
pub const SORT_BY: &str = "Sort by ";
/// `idsReverseSort` (`0x46e`).
pub const REVERSE_SORT_BY: &str = "Reverse Sort by ";
/// `idsHide` (`0x46f`).
pub const HIDE_THE: &str = "Hide the ";
/// `idsShow` (`0x471`).
pub const SHOW_THE: &str = "Show the ";
/// `idsColumn` (`0x470`).
pub const COLUMN: &str = " column";

/// The most rows the Others' Fleets report will hold — `SortReportCache`
/// stops filling its index array at `0x3fb`.
pub const ENEMY_ROW_MAX: usize = 0x3fc;

impl Report {
    /// All four, in the order the Report menu lists them.
    pub const ALL: [Report; 4] = [
        Report::Planets,
        Report::Fleets,
        Report::EnemyFleets,
        Report::Battles,
    ];

    /// The original's `irpt`.
    #[must_use]
    pub fn irpt(self) -> i16 {
        match self {
            Report::Planets => 0,
            Report::Fleets => 1,
            Report::EnemyFleets => 2,
            Report::Battles => 3,
        }
    }

    /// Its columns, left to right.
    #[must_use]
    pub fn columns(self) -> &'static [Column] {
        match self {
            Report::Planets => PLANET_COLUMNS,
            Report::Fleets => FLEET_COLUMNS,
            Report::EnemyFleets => ENEMY_COLUMNS,
            Report::Battles => BATTLE_COLUMNS,
        }
    }

    /// The window's caption, which counts what it is showing —
    /// `Planet Summary Report -- %d Planet%c` (`0x499`) and the three after
    /// it. The `%c` is the plural's `s`, or a space when there is one row.
    #[must_use]
    pub fn title(self, rows: usize) -> String {
        let what = match self {
            Report::Planets => "Planet Summary Report",
            Report::Fleets => "Fleet Summary Report",
            Report::EnemyFleets => "Others' Fleets Summary Report",
            Report::Battles => "Battle Summary Report",
        };
        let noun = match self {
            Report::Planets => "Planet",
            Report::Fleets | Report::EnemyFleets => "Fleet",
            Report::Battles => "Battle",
        };
        let plural = if rows == 1 { " " } else { "s" };
        format!("{what} -- {rows} {noun}{plural}")
    }

    /// How many names the mineral submenu lists for this report: three for
    /// the planets' columns, four — colonists as well — for the fleets'
    /// Cargo. `ReportColumnPopup` writes it as `(irpt == 1) + 3`.
    #[must_use]
    pub fn subsort_names(self) -> usize {
        if self == Report::Fleets {
            4
        } else {
            3
        }
    }
}

/// One report's saved state — the fields of `RPT` the player can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportState {
    /// Which columns are shown (`grbitVisible`), bit per column.
    pub visible: u32,
    /// The column sorted on (`icolSort`).
    pub sort: i16,
    /// Which mineral of a [`Subsort`] column (`iSubsort`).
    pub subsort: i16,
    /// Ascending (`fAscending`).
    pub ascending: bool,
    /// The leftmost scrolled-to column (`cFieldFirst`); column 0 never
    /// scrolls off.
    pub first_field: usize,
    /// The first row shown (`irowFirst`).
    pub first_row: usize,
}

impl ReportState {
    /// What the four `RPT` blocks at `1120:1494` hold before anyone touches
    /// them: every column visible, sorted ascending by column 0.
    #[must_use]
    pub fn new() -> ReportState {
        ReportState {
            visible: 0x0000_ffff,
            sort: 0,
            subsort: 0,
            ascending: true,
            first_field: 1,
            first_row: 0,
        }
    }

    /// Whether a column is shown.
    #[must_use]
    pub fn shows(&self, column: usize) -> bool {
        column < 32 && self.visible & (1 << column) != 0
    }

    /// The columns drawn, left to right: column 0 always, then every visible
    /// column from [`Self::first_field`] on. `DrawReport`'s loop.
    #[must_use]
    pub fn drawn(&self, report: Report) -> Vec<usize> {
        (0..report.columns().len())
            .filter(|&c| self.shows(c) && (c == 0 || c >= self.first_field))
            .collect()
    }
}

impl Default for ReportState {
    fn default() -> ReportState {
        ReportState::new()
    }
}

/// What the sort was before the current one, for breaking ties.
///
/// The original keeps this in three globals — `vicolSortPrev`,
/// `viSubsortPrev`, `vfAscendingPrev` — shared by all four reports, and
/// `vicolSortPrev` starts negative, which is how `ICompReport` knows there is
/// nothing to fall back to yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Previous {
    /// The column, or negative for "none yet".
    pub sort: i16,
    /// Its mineral.
    pub subsort: i16,
    /// Its direction.
    pub ascending: bool,
}

impl Default for Previous {
    fn default() -> Previous {
        Previous {
            sort: -1,
            subsort: 0,
            ascending: true,
        }
    }
}

/// All four reports' state, and which one is open.
#[derive(Debug, Clone, Default)]
pub struct Reports {
    planets: ReportState,
    fleets: ReportState,
    enemy: ReportState,
    battles: ReportState,
    /// The tie-break sort, shared by all four.
    pub previous: Previous,
    /// Which report's window is up (`vprptCur`), if any. The tutorial asks
    /// about this directly.
    pub open: Option<Report>,
}

impl Reports {
    /// One report's state.
    #[must_use]
    pub fn state(&self, report: Report) -> &ReportState {
        match report {
            Report::Planets => &self.planets,
            Report::Fleets => &self.fleets,
            Report::EnemyFleets => &self.enemy,
            Report::Battles => &self.battles,
        }
    }

    /// One report's state, to change.
    pub fn state_mut(&mut self, report: Report) -> &mut ReportState {
        match report {
            Report::Planets => &mut self.planets,
            Report::Fleets => &mut self.fleets,
            Report::EnemyFleets => &mut self.enemy,
            Report::Battles => &mut self.battles,
        }
    }

    /// The state of whichever report is open.
    #[must_use]
    pub fn current(&self) -> Option<&ReportState> {
        self.open.map(|r| self.state(r))
    }

    /// Take a column, direction and mineral, remembering what the sort was.
    ///
    /// This is what the menu's items do. The save into [`Self::previous`] is
    /// unconditional here, exactly as `ReportColumnPopup` does it — re-sorting
    /// by the same column still pushes it down, which makes the tie-break a
    /// no-op rather than changing what it was.
    pub fn sort_by(&mut self, report: Report, column: usize, ascending: bool, subsort: i16) {
        let state = self.state_mut(report);
        self.previous = Previous {
            sort: state.sort,
            subsort: state.subsort,
            ascending: state.ascending,
        };
        let state = self.state_mut(report);
        state.sort = i16::try_from(column).unwrap_or(0);
        state.ascending = ascending;
        state.subsort = subsort;
    }

    /// Act on the item the player picked from a column's menu.
    ///
    /// `index` is the item's place in [`ColumnMenu::entries`], which is how
    /// `PopupMenu` (`10c0:136c`) reports a choice and what
    /// `ReportColumnPopup`'s arithmetic is written against.
    pub fn choose(&mut self, report: Report, column: usize, index: usize) {
        let menu = ColumnMenu::build(report, column, self.state(report));
        if index >= menu.entries.len() {
            return;
        }
        if index < menu.sort_below {
            let (ascending, subsort) = menu.sort_choice(report, index);
            self.sort_by(report, column, ascending, subsort);
        } else if Some(index) == menu.hide {
            let state = self.state_mut(report);
            state.visible &= !(1u32 << column);
        } else if index >= menu.show_from {
            if let Some(&which) = menu.show.get(index - menu.show_from) {
                let state = self.state_mut(report);
                state.visible |= 1u32 << which;
            }
        }
    }
}

/// One entry of a column's menu.
///
/// `PopupMenu` takes an array of strings with two markers in it: a null entry
/// opens a submenu whose title is the entry after it and whose contents run to
/// the next null, and an entry of `"\xff"` is a separator. Keeping that shape
/// keeps the indices the same as the original's, which is what
/// `ReportColumnPopup`'s `(index - 2) % n` arithmetic is written against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A null: opens a submenu, or closes the one open.
    Submenu,
    /// A separator line.
    Separator,
    /// A line of text. Directly after a [`Entry::Submenu`] it is the
    /// submenu's title and cannot be picked.
    Item(String),
}

/// The menu a column header opens.
#[derive(Debug, Clone)]
pub struct ColumnMenu {
    /// Every entry, in order; the index of the one picked is what
    /// [`Reports::choose`] takes.
    pub entries: Vec<Entry>,
    /// Indices below this belong to the sort (`iVar2` in the original).
    pub sort_below: usize,
    /// Where "Hide the … column" sits, when the column can be hidden. Column
    /// 0 cannot: it is the one that never scrolls away.
    pub hide: Option<usize>,
    /// Where the "Show the … column" entries start.
    pub show_from: usize,
    /// Which column each of those shows.
    pub show: Vec<usize>,
}

impl ColumnMenu {
    /// Build the menu for one column, as `ReportColumnPopup` (`1108:74d4`)
    /// builds it.
    #[must_use]
    pub fn build(report: Report, column: usize, state: &ReportState) -> ColumnMenu {
        let columns = report.columns();
        let header = columns.get(column).map_or("", |c| c.name);
        let grouped = columns
            .get(column)
            .is_some_and(|c| c.subsort != Subsort::None);
        let names = report.subsort_names();

        let mut entries = Vec::new();
        for reverse in [false, true] {
            let title = format!(
                "{}{header}",
                if reverse { REVERSE_SORT_BY } else { SORT_BY }
            );
            if grouped {
                entries.push(Entry::Submenu);
                entries.push(Entry::Item(title));
                for name in MINERAL_NAMES.iter().take(names) {
                    entries.push(Entry::Item((*name).to_string()));
                }
                entries.push(Entry::Separator);
                entries.push(Entry::Item(WEIGHTED_AVERAGE.to_string()));
                entries.push(Entry::Submenu);
            } else {
                entries.push(Entry::Item(title));
            }
        }

        entries.push(Entry::Separator);
        let hide_at = entries.len();
        // Written whatever the column, then overwritten by the first Show
        // entry when the column is column 0.
        entries.push(Entry::Item(format!("{HIDE_THE}{header}{COLUMN}")));
        entries.push(Entry::Separator);
        let (hide, show_from) = if column == 0 {
            entries.truncate(hide_at);
            (None, hide_at)
        } else {
            (Some(hide_at), hide_at + 2)
        };

        let mut show = Vec::new();
        for (index, hidden) in columns.iter().enumerate() {
            if !state.shows(index) {
                entries.push(Entry::Item(format!("{SHOW_THE}{}{COLUMN}", hidden.name)));
                show.push(index);
            }
        }
        if show.is_empty() {
            // Nothing to show: the trailing separator goes too.
            entries.truncate(show_from.saturating_sub(1));
        }

        ColumnMenu {
            entries,
            sort_below: hide_at,
            hide,
            show_from,
            show,
        }
    }

    /// What picking item `index` in the sort region means.
    ///
    /// A plain column has two items and the first is ascending. A [`Subsort`]
    /// column has two submenus of `names + 2` entries each, and the original
    /// works the choice out with arithmetic over the flat index rather than
    /// by remembering which submenu it built.
    ///
    /// That arithmetic has an off-by-one in it, and this reproduces it: the
    /// direction is `index < 7`, which is right for a three-mineral submenu,
    /// where Weighted Average sits at 6, and wrong for the fleets' Cargo
    /// column, where it sits at 7. Choosing **Sort by Cargo → Weighted
    /// Average** in the original sorts descending. See `docs/ui/reports.md`.
    #[must_use]
    pub fn sort_choice(&self, report: Report, index: usize) -> (bool, i16) {
        let grouped = self.entries.first() == Some(&Entry::Submenu);
        if !grouped {
            return (index == 0, 0);
        }
        let names = i32::try_from(report.subsort_names()).unwrap_or(3);
        // `local_67c + 3 + (irpt == 1)`, with `local_67c` always 5.
        let modulus = 5 + names;
        let index = i32::try_from(index).unwrap_or(0);
        let mut subsort = (index - 2) % modulus;
        if subsort > names {
            subsort = names;
        }
        (index < 7, i16::try_from(subsort).unwrap_or(0))
    }
}

/// What one cell sorts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// A number.
    Num(i64),
    /// Text, compared byte by byte and case-sensitively, as the original's
    /// `strcmp` does.
    Text(String),
    /// Nothing there — no starbase, no destination, an unknown warp. The
    /// original's comparators put these after everything else when ascending.
    Missing,
}

impl Key {
    fn cmp_key(&self, other: &Key) -> Ordering {
        match (self, other) {
            (Key::Missing, Key::Missing) => Ordering::Equal,
            (Key::Missing, _) => Ordering::Greater,
            (_, Key::Missing) => Ordering::Less,
            (Key::Num(a), Key::Num(b)) => a.cmp(b),
            (Key::Text(a), Key::Text(b)) => a.as_bytes().cmp(b.as_bytes()),
            // Columns do not mix the two; keep it total anyway.
            (Key::Num(_), Key::Text(_)) => Ordering::Less,
            (Key::Text(_), Key::Num(_)) => Ordering::Greater,
        }
    }
}

/// Everything the columns need to read.
pub struct Data<'a> {
    /// The game.
    pub game: &'a GameState,
    /// Whose report it is.
    pub player: usize,
    /// The battle recordings the file carries.
    pub battles: &'a [BattleRecord],
}

impl Data<'_> {
    /// Which objects a report lists, as indices into whichever collection it
    /// draws from — `SortReportCache`'s first job.
    ///
    /// The planets report lists **your own** planets, and nothing you have
    /// merely scanned.
    #[must_use]
    pub fn rows(&self, report: Report) -> Vec<usize> {
        let me = i16::try_from(self.player).unwrap_or(-1);
        match report {
            Report::Planets => self
                .game
                .planets
                .iter()
                .enumerate()
                .filter(|(_, p)| p.owner == Some(me) && p.detail.is_full())
                .map(|(i, _)| i)
                .collect(),
            Report::Fleets => self
                .game
                .fleets
                .iter()
                .enumerate()
                .filter(|(_, f)| f.owner == me)
                .map(|(i, _)| i)
                .collect(),
            Report::EnemyFleets => self
                .game
                .fleets
                .iter()
                .enumerate()
                .filter(|(_, f)| f.owner != me)
                .map(|(i, _)| i)
                .take(ENEMY_ROW_MAX)
                .collect(),
            Report::Battles => (0..self.battles.len()).collect(),
        }
    }

    /// Sort a report's rows.
    ///
    /// A stable sort, where the original uses `qsort`. Two rows the
    /// comparator cannot separate even after the tie-break keep the order the
    /// galaxy gives them here, rather than whatever the quicksort's
    /// partitioning happened to leave.
    pub fn sort(&self, report: Report, reports: &Reports, rows: &mut [usize]) {
        let state = *reports.state(report);
        let previous = reports.previous;
        rows.sort_by(|&a, &b| self.compare(report, &state, previous, a, b));
    }

    /// `ICompReport` (`1108:5bb8`): compare two rows on the sorted column and,
    /// when that cannot separate them, on the one sorted before it.
    #[must_use]
    pub fn compare(
        &self,
        report: Report,
        state: &ReportState,
        previous: Previous,
        a: usize,
        b: usize,
    ) -> Ordering {
        let (mut column, mut subsort, mut ascending) = (state.sort, state.subsort, state.ascending);
        let mut second = false;
        loop {
            let index = usize::try_from(column).unwrap_or(0);
            let mut order = self
                .key(report, a, index, subsort)
                .cmp_key(&self.key(report, b, index, subsort));
            if !ascending {
                order = order.reverse();
            }
            let settled = order != Ordering::Equal
                || second
                || (column == previous.sort && subsort == previous.subsort)
                || previous.sort < 0;
            if settled {
                return order;
            }
            column = previous.sort;
            subsort = previous.subsort;
            ascending = previous.ascending;
            second = true;
        }
    }

    /// What one cell sorts on.
    #[must_use]
    pub fn key(&self, report: Report, row: usize, column: usize, subsort: i16) -> Key {
        match report {
            Report::Planets => self.planet_key(row, column, subsort),
            Report::Fleets => self.fleet_key(row, column, subsort),
            Report::EnemyFleets => self.enemy_key(row, column),
            Report::Battles => self.battle_key(row, column),
        }
    }

    /// The name a planet id goes by, for the columns that point at one.
    fn planet_name(&self, id: i16) -> Key {
        self.game
            .planets
            .iter()
            .chain(self.game.known_planets.iter())
            .find(|p| p.id == id)
            .and_then(|p| p.name)
            .map_or(Key::Missing, |n| Key::Text(n.to_string()))
    }

    fn race(&self) -> Option<&stars_core::race::Race> {
        self.game.players.get(self.player).map(|p| &p.race)
    }

    fn tech(&self) -> [u8; 6] {
        self.game
            .players
            .get(self.player)
            .map_or([0; 6], |p| p.research.levels)
    }

    /// The range of minerals a [`Subsort`] column looks at: one, or all of
    /// them when the choice is the last item.
    fn mineral_range(subsort: i16, names: usize) -> std::ops::RangeInclusive<usize> {
        let last = names.saturating_sub(1);
        let chosen = usize::try_from(subsort).unwrap_or(0);
        if chosen >= names {
            0..=last
        } else {
            chosen..=chosen
        }
    }

    fn planet_key(&self, row: usize, column: usize, subsort: i16) -> Key {
        let Some(planet) = self.game.planets.get(row) else {
            return Key::Missing;
        };
        let designs = self.game.designs.get(self.player);
        match column {
            0 => planet
                .name
                .map_or(Key::Missing, |n| Key::Text(n.to_string())),
            1 => match (planet.starbase_design, designs) {
                (Some(slot), Some(designs)) => designs
                    .get(usize::from(slot))
                    .map_or(Key::Missing, |d| Key::Text(d.name.clone())),
                _ => Key::Missing,
            },
            2 => Key::Num(i64::from(planet.pop)),
            3 => match (self.race(), planet.pop) {
                (Some(race), pop) => stars_core::hab::calc_planet_max_pop(planet, race)
                    .filter(|max| *max > 0)
                    .map_or(Key::Num(0), |max| {
                        Key::Num(i64::from(pop) * 100 / i64::from(max))
                    }),
                _ => Key::Num(0),
            },
            4 => self.race().map_or(Key::Num(0), |race| {
                Key::Num(i64::from(stars_core::hab::pct_planet_desirability(
                    planet, race,
                )))
            }),
            5 => planet.queue.first().map_or(Key::Missing, |item| {
                if item.ship {
                    designs
                        .and_then(|d| d.get(usize::from(item.item)))
                        .map_or(Key::Missing, |d| Key::Text(d.name.clone()))
                } else {
                    Key::Text(stars_core::production::item_name(item.item).to_string())
                }
            }),
            6 => Key::Num(i64::from(planet.mines)),
            7 => Key::Num(i64::from(planet.factories)),
            8 => {
                if planet.defenses <= 0 {
                    return Key::Num(0);
                }
                self.race().map_or(Key::Num(0), |race| {
                    let (ordinary, _) = stars_core::bombing::pct_survive(planet, race, self.tech());
                    // The original sorts on `1 - pctSurvive`, in ten-thousandths.
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "a fraction of 10000 is small"
                    )]
                    Key::Num(((1.0 - ordinary) * 10_000.0) as i64)
                })
            }
            9 => Key::Num(
                Self::mineral_range(subsort, 3)
                    .map(|m| i64::from(planet.surface_min[m]))
                    .sum(),
            ),
            10 => self.race().map_or(Key::Num(0), |race| {
                let mined = stars_core::mining::minerals_mined(planet, race, None, None);
                Key::Num(
                    Self::mineral_range(subsort, 3)
                        .map(|m| i64::from(mined[m]))
                        .sum(),
                )
            }),
            11 => Key::Num(
                Self::mineral_range(subsort, 3)
                    .map(|m| i64::from(planet.min_conc[m]))
                    .sum(),
            ),
            12 => self.race().map_or(Key::Num(0), |race| {
                Key::Num(i64::from(
                    stars_core::resources::resources_at_planet(planet, race, self.energy_tech())
                        .unwrap_or(0),
                ))
            }),
            13 => planet
                .fling_dest
                .map_or(Key::Missing, |id| self.planet_name(id)),
            14 => planet
                .route_dest
                .map_or(Key::Missing, |id| self.planet_name(id)),
            _ => Key::Missing,
        }
    }

    fn energy_tech(&self) -> i16 {
        self.game
            .players
            .get(self.player)
            .map_or(0, |p| i16::from(p.research.levels[0]))
    }

    /// The design most of a fleet's ships are, as the Composition column
    /// names it — `IshdefPrimaryFromLpfl`.
    fn primary(&self, fleet: &stars_core::fleet::Fleet) -> Option<String> {
        let owner = usize::try_from(fleet.owner).ok()?;
        let designs = self.game.designs.get(owner)?;
        let primary = stars_core::fleet::primary_design(fleet, designs)?;
        Some(designs.get(primary.design)?.name.clone())
    }

    fn fleet_key(&self, row: usize, column: usize, subsort: i16) -> Key {
        let Some(fleet) = self.game.fleets.get(row) else {
            return Key::Missing;
        };
        match column {
            0 => fleet
                .name
                .as_ref()
                .map_or(Key::Missing, |n| Key::Text(n.clone())),
            1 => Key::Num(i64::from(fleet.id)),
            2 => self.location_key(fleet),
            3 => match fleet.waypoints.last() {
                Some(w) => match w.target {
                    Some(id) => self.planet_name(i16::try_from(id).unwrap_or(-1)),
                    None => Key::Text(format!("({}, {})", w.position.x, w.position.y)),
                },
                None => Key::Missing,
            },
            4 => Key::Num(i64::try_from(fleet.waypoints.len().saturating_sub(1)).unwrap_or(0)),
            5 => fleet
                .waypoints
                .get(1)
                .map_or(Key::Missing, |w| Key::Num(i64::from(w.task))),
            6 => Key::Num(i64::from(fleet.cargo.fuel)),
            7 => Key::Num(
                Self::mineral_range(subsort, 4)
                    .map(|m| match m {
                        3 => i64::from(fleet.cargo.colonists),
                        m => i64::from(fleet.cargo.minerals[m]),
                    })
                    .sum(),
            ),
            8 => self.primary(fleet).map_or(Key::Missing, Key::Text),
            9 => Key::Num(0),
            10 => Key::Num(i64::from(fleet.battle_plan)),
            11 => self
                .game
                .designs
                .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX))
                .map_or(Key::Num(0), |designs| {
                    Key::Num(i64::from(fleet.mass(designs)))
                }),
            _ => Key::Missing,
        }
    }

    fn location_key(&self, fleet: &stars_core::fleet::Fleet) -> Key {
        match fleet.orbiting {
            Some(id) => self.planet_name(i16::try_from(id).unwrap_or(-1)),
            // `Space: (%d, %d)` — a fleet in deep space sorts by that text,
            // which puts every one of them together.
            None => Key::Text(format!(
                "Space: ({}, {})",
                fleet.position.x, fleet.position.y
            )),
        }
    }

    /// How many of a fleet's ships fall in one class, and how many in none of
    /// the four the report names.
    ///
    /// A player's own file holds only that player's designs, so for another
    /// player's fleet this can only count what the file knows. Ships whose
    /// design is not there are counted as unarmed, which is where the
    /// original puts anything outside classes 2 to 5.
    fn class_count(&self, fleet: &stars_core::fleet::Fleet, want: Option<u8>) -> i64 {
        let designs = self
            .game
            .designs
            .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX));
        fleet
            .stacks
            .iter()
            .filter(|s| {
                let class = designs
                    .and_then(|d| d.get(usize::from(s.design)))
                    .and_then(stars_core::design::ShipDesign::ship_class)
                    .map(|c| c.index());
                match want {
                    Some(class_wanted) => class == Some(class_wanted),
                    None => !matches!(class, Some(2..=5)),
                }
            })
            .map(|s| i64::from(s.count))
            .sum()
    }

    fn enemy_key(&self, row: usize, column: usize) -> Key {
        let Some(fleet) = self.game.fleets.get(row) else {
            return Key::Missing;
        };
        match column {
            0 => fleet
                .name
                .as_ref()
                .map_or(Key::Missing, |n| Key::Text(n.clone())),
            1 => Key::Num(i64::from(fleet.id)),
            2 => self.location_key(fleet),
            3 => fleet.warp.map_or(Key::Missing, |w| Key::Num(i64::from(w))),
            4 => self
                .game
                .designs
                .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX))
                .map_or(Key::Num(0), |designs| {
                    Key::Num(i64::from(fleet.mass(designs)))
                }),
            5 => self.primary(fleet).map_or(Key::Missing, Key::Text),
            6 => Key::Num(i64::from(fleet.ships())),
            7 => Key::Num(self.class_count(fleet, None)),
            8 => Key::Num(self.class_count(fleet, Some(2))),
            9 => Key::Num(self.class_count(fleet, Some(3))),
            10 => Key::Num(self.class_count(fleet, Some(5))),
            11 => Key::Num(self.class_count(fleet, Some(4))),
            _ => Key::Missing,
        }
    }

    /// `CBattleUnits` (`10e8:…`) as the report's columns ask for it: count the
    /// ships in a battle, filtered by side, by whether starbases count, and by
    /// hull class.
    fn battle_units(&self, battle: &BattleRecord, ours: bool, theirs: bool, class: Class) -> i64 {
        let me = u8::try_from(self.player).unwrap_or(u8::MAX);
        battle
            .tokens
            .iter()
            .filter(|t| if t.player == me { ours } else { theirs })
            .filter(|t| match class {
                Class::EverythingWithBases => true,
                Class::BasesOnly => t.is_starbase(),
                Class::Ship(want) => {
                    if t.is_starbase() {
                        return false;
                    }
                    let class = self
                        .game
                        .designs
                        .get(usize::from(t.player))
                        .and_then(|d| d.get(usize::from(t.design)))
                        .and_then(stars_core::design::ShipDesign::ship_class)
                        .map(|c| c.index());
                    match want {
                        Some(class_wanted) => class == Some(class_wanted),
                        None => !matches!(class, Some(2..=5)),
                    }
                }
            })
            .map(|t| i64::from(t.ships))
            .sum()
    }

    fn battle_kills(&self, battle: &BattleRecord, ours: bool) -> i64 {
        let me = u8::try_from(self.player).unwrap_or(u8::MAX);
        battle
            .actions
            .iter()
            .flat_map(|a| a.kills.iter())
            .filter(|k| {
                battle
                    .tokens
                    .get(usize::from(k.token))
                    .is_some_and(|t| (t.player == me) == ours)
            })
            .map(|k| i64::from(k.ships_killed))
            .sum()
    }

    fn battle_key(&self, row: usize, column: usize) -> Key {
        let Some(battle) = self.battles.get(row) else {
            return Key::Missing;
        };
        let class = |c: Option<u8>| Class::Ship(c);
        match column {
            0 => {
                if battle.planet == u16::MAX {
                    Key::Text(format!(
                        "Space: ({}, {})",
                        battle.position.0, battle.position.1
                    ))
                } else {
                    self.planet_name(i16::try_from(battle.planet).unwrap_or(-1))
                }
            }
            // 'O' when the starbase was ours, 'T' when theirs, blank when
            // there was none. The original sorts on the count, not the letter.
            1 => {
                let ours = self.battle_units(battle, true, false, Class::BasesOnly);
                Key::Num(if ours > 0 {
                    ours
                } else {
                    self.battle_units(battle, false, true, Class::BasesOnly)
                })
            }
            2 => Key::Num(i64::from(battle.players)),
            3 => Key::Num(self.battle_units(battle, true, true, Class::EverythingWithBases)),
            4 => Key::Num(self.battle_units(battle, true, false, Class::EverythingWithBases)),
            5 => Key::Num(self.battle_units(battle, false, true, Class::EverythingWithBases)),
            6 => Key::Num(self.battle_units(battle, true, true, class(None))),
            7 => Key::Num(self.battle_units(battle, true, true, class(Some(2)))),
            8 => Key::Num(self.battle_units(battle, true, true, class(Some(3)))),
            9 => Key::Num(self.battle_units(battle, true, true, class(Some(5)))),
            10 => Key::Num(self.battle_units(battle, true, true, class(Some(4)))),
            11 => Key::Num(self.battle_kills(battle, true)),
            12 => Key::Num(self.battle_kills(battle, false)),
            13 => Key::Num(
                self.battle_units(battle, true, false, Class::EverythingWithBases)
                    - self.battle_kills(battle, true),
            ),
            14 => Key::Num(
                self.battle_units(battle, false, true, Class::EverythingWithBases)
                    - self.battle_kills(battle, false),
            ),
            _ => Key::Missing,
        }
    }
}

/// Which tokens a battle column counts.
#[derive(Debug, Clone, Copy)]
enum Class {
    /// Ships and starbases alike — `grbitBU` with bit 2 set and every class
    /// bit set.
    EverythingWithBases,
    /// Starbases alone — bit 2 set and no class bit, which is how the report
    /// asks whose base was there.
    BasesOnly,
    /// Ships of one class, or of none of the four named.
    Ship(Option<u8>),
}
