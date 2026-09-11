//! `stars.ini`: the settings the game keeps between runs.
//!
//! `ReadIniSettings` (`1000:1db3`) reads it at startup and
//! `WriteIniSettings` (`1020:7a76`) writes it back, key by key, through the
//! Windows profile calls. This is that file — the same sections, the same
//! keys, the same value formats — and an [`Ini`] that **keeps whatever it
//! does not understand**, so a file written by the original survives a pass
//! through this one.
//!
//! What this module handles today is the four reports: which columns each
//! shows and what it sorts on, from `[Misc]`, and the window rectangles from
//! `[Windows]`. The rest of what the original stores there is inventoried in
//! `docs/ui/menus.md`.

use std::fmt::Write as _;

use crate::report::{Report, ReportState, Reports};

/// One section of the file, in the order its keys were written.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    name: String,
    entries: Vec<(String, String)>,
}

/// A `stars.ini`, parsed but not interpreted.
///
/// Sections and keys keep their order and their spelling, and anything this
/// project has no use for is carried through untouched — which matters,
/// because the original stores a great deal here that this one does not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ini {
    sections: Vec<Section>,
    /// Anything before the first `[Section]`, kept as it was.
    preamble: Vec<String>,
}

impl Ini {
    /// Read one.
    #[must_use]
    pub fn parse(text: &str) -> Ini {
        let mut ini = Ini::default();
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                ini.sections.push(Section {
                    name: name.trim().to_string(),
                    entries: Vec::new(),
                });
            } else if let Some(section) = ini.sections.last_mut() {
                if let Some((key, value)) = trimmed.split_once('=') {
                    section
                        .entries
                        .push((key.trim().to_string(), value.trim().to_string()));
                }
            } else if !trimmed.is_empty() {
                ini.preamble.push(line.to_string());
            }
        }
        ini
    }

    /// One value, or nothing. Section and key names are matched without
    /// regard to case, as the profile calls match them.
    #[must_use]
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(section))?
            .entries
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
    }

    /// One value as a number, falling back the way `GetPrivateProfileInt`
    /// does: a missing or unreadable value gives the default.
    #[must_use]
    pub fn int(&self, section: &str, key: &str, default: i64) -> i64 {
        self.get(section, key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }

    /// Write one value, in place if the key is already there and at the end
    /// of the section otherwise. A section that does not exist is added.
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let Some(found) = self
            .sections
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(section))
        else {
            self.sections.push(Section {
                name: section.to_string(),
                entries: vec![(key.to_string(), value.to_string())],
            });
            return;
        };
        match found
            .entries
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
        {
            Some(entry) => entry.1 = value.to_string(),
            None => found.entries.push((key.to_string(), value.to_string())),
        }
    }

    /// Take a key out altogether, which is what writing an empty value does
    /// through the profile calls.
    pub fn remove(&mut self, section: &str, key: &str) {
        if let Some(found) = self
            .sections
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(section))
        {
            found
                .entries
                .retain(|(name, _)| !name.eq_ignore_ascii_case(key));
        }
    }
}

impl std::fmt::Display for Ini {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.preamble {
            writeln!(f, "{line}")?;
        }
        for section in &self.sections {
            writeln!(f, "[{}]", section.name)?;
            for (key, value) in &section.entries {
                writeln!(f, "{key}={value}")?;
            }
        }
        Ok(())
    }
}

/// The `[Windows]` section, which holds one rectangle per window.
pub const WINDOWS: &str = "Windows";

/// The `[Misc]` section, where the reports' columns and sort live.
pub const MISC: &str = "Misc";

/// How a window was left, which is the letter a rectangle starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// `R`.
    Normal,
    /// `M`. The report windows are always written with this, whatever they
    /// were; only the frame's is read back.
    Maximised,
    /// `I`.
    Iconised,
}

/// A window's place, as `[Windows]` stores it.
///
/// `GetIniWinRc` (`1000:1020`) takes a value of **exactly seventeen
/// characters**: the state letter and four fixed four-character fields, in
/// the order left, top, right, bottom. A field is read digit by digit, and a
/// `-` anywhere in it makes the whole field negative. Anything else — a
/// different length, an unknown letter, a stray character — is no rectangle
/// at all, and the window falls back to its built-in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    /// The letter.
    pub state: WindowState,
    /// Left, top, right, bottom.
    pub rect: (i16, i16, i16, i16),
}

impl WindowRect {
    /// How long the value has to be: `1 + 4 * 4`.
    pub const LENGTH: usize = 17;

    /// Read one, or decide it is not one.
    #[must_use]
    pub fn parse(value: &str) -> Option<WindowRect> {
        if value.len() != Self::LENGTH {
            return None;
        }
        let bytes = value.as_bytes();
        let state = match bytes[0] {
            b'M' => WindowState::Maximised,
            b'R' => WindowState::Normal,
            b'I' => WindowState::Iconised,
            _ => return None,
        };
        let mut fields = [0_i16; 4];
        for (index, field) in fields.iter_mut().enumerate() {
            let mut value = 0_i16;
            let mut negative = false;
            for &byte in &bytes[1 + index * 4..5 + index * 4] {
                if byte == b'-' {
                    negative = true;
                } else if byte.is_ascii_digit() {
                    value = value.checked_mul(10)?.checked_add(i16::from(byte - b'0'))?;
                } else {
                    return None;
                }
            }
            *field = if negative { -value } else { value };
        }
        Some(WindowRect {
            state,
            rect: (fields[0], fields[1], fields[2], fields[3]),
        })
    }

    /// Write one, as `%c%04d%04d%04d%04d`.
    #[must_use]
    pub fn format(&self) -> String {
        let letter = match self.state {
            WindowState::Normal => 'R',
            WindowState::Maximised => 'M',
            WindowState::Iconised => 'I',
        };
        let (left, top, right, bottom) = self.rect;
        format!("{letter}{left:04}{top:04}{right:04}{bottom:04}")
    }
}

/// The keys each report's columns and sort are stored under.
///
/// Note the third: the original spells it `ReportEFltSort`, not
/// `ReportEFleetSort` as the pattern would have it, and spelling it the
/// obvious way would silently lose everybody else's fleets' sort.
#[must_use]
pub fn report_keys(report: Report) -> (&'static str, &'static str, &'static str) {
    match report {
        Report::Planets => ("ReportPlanFld", "ReportPlanSort", "ReportPlanWin"),
        Report::Fleets => ("ReportFleetFld", "ReportFleetSort", "ReportFleetWin"),
        Report::EnemyFleets => ("ReportEFleetFld", "ReportEFltSort", "ReportEFleetWin"),
        Report::Battles => ("ReportBtlFld", "ReportBtlSort", "ReportBtlWin"),
    }
}

/// Where `fAscending` sits in the packed sort value.
pub const ASCENDING_BIT: i64 = 0x100;

impl Reports {
    /// Restore the four reports' columns and sort from a `stars.ini`.
    ///
    /// `ReportPlanFld` and its three are `grbitVisible`'s **low word**, so a
    /// report can never have more than sixteen columns' worth of state; the
    /// most any of them has is fifteen. `ReportPlanSort` and its three pack
    /// the column into the low byte and `fAscending` into bit 8.
    ///
    /// `iSubsort` is **not** stored, so a report sorted on germanium comes
    /// back sorted on ironium.
    pub fn read_ini(&mut self, ini: &Ini) {
        for report in Report::ALL {
            let (fld, sort, _) = report_keys(report);
            let columns = ini.int(MISC, fld, 0xffff);
            let packed = ini.int(MISC, sort, 0);
            let state = self.state_mut(report);
            #[expect(
                clippy::cast_sign_loss,
                clippy::cast_possible_truncation,
                reason = "a sixteen-bit mask"
            )]
            let visible = (columns as u16) as u32;
            state.visible = visible;
            state.sort = i16::try_from(packed & 0xff).unwrap_or(0);
            state.ascending = packed & ASCENDING_BIT != 0;
            // Not stored, so it starts again at the first mineral.
            state.subsort = 0;
        }
    }

    /// Write them back.
    ///
    /// `WriteIniSettings` only does this when something changed; here the
    /// file is written whole each time, which comes to the same thing.
    pub fn write_ini(&self, ini: &mut Ini) {
        for report in Report::ALL {
            let (fld, sort, _) = report_keys(report);
            let state = self.state(report);
            ini.set(MISC, fld, &(state.visible & 0xffff).to_string());
            let packed = i64::from(state.sort) | if state.ascending { ASCENDING_BIT } else { 0 };
            ini.set(MISC, sort, &packed.to_string());
        }
    }
}

impl ReportState {
    /// The rectangle a report window was left at, when the file has one.
    ///
    /// This project's reports are **screens** rather than windows, so
    /// nothing here has a rectangle to restore; the keys are read and
    /// written back unchanged so that a file the original wrote is not
    /// damaged by passing through. See `docs/ui/reports.md`.
    #[must_use]
    pub fn window_rect(ini: &Ini, report: Report) -> Option<WindowRect> {
        let (_, _, win) = report_keys(report);
        WindowRect::parse(ini.get(WINDOWS, win)?)
    }
}

/// Render a whole settings file, for a caller that has nothing else to add.
#[must_use]
pub fn to_text(ini: &Ini) -> String {
    let mut out = String::new();
    let _ = write!(out, "{ini}");
    out
}

/// The scanner's own keys, all of them in `[Windows]` — `ReadIniSettings`
/// keeps that section current from the window rectangles all the way down to
/// the mineral scale, and only switches to `[Files]` afterwards.
pub mod scanner {
    /// `grbitScan`: the view in the low nibble and ten overlay bits above it.
    pub const MODE: &str = "ScanModeV25";
    /// `grbitScanShip`: which of your designs the Ship Design filter counts.
    pub const SHIP_FILTER: &str = "ScanFilterV25";
    /// `grbitScanEShip`: which classes the Enemy Ship Class filter counts.
    pub const CLASS_FILTER: &str = "ScanEFilterV25";
    /// `grbitScanMines`: whose minefields the overlay draws.
    pub const MINE_FILTER: &str = "ScanMines";
    /// `vpctRadarView`: what the coverage overlay pretends a scanner is worth.
    pub const RADAR: &str = "ScanRadar";
    /// `iScanZoom`, stored as **one character** — the digit `zoom + 5`.
    pub const ZOOM: &str = "ScanZoom";
    /// Whether the toolbar shows.
    pub const TOOLBAR: &str = "Toolbar";
    /// `iWindowLayout`.
    pub const LAYOUT: &str = "Layout";

    /// What `grbitScan` is when the file says nothing: planet names and ship
    /// counts on, everything else off.
    pub const MODE_DEFAULT: i64 = 0xe0;
    /// The minefield filter starts **full**, unlike the two ship filters.
    pub const MINE_DEFAULT: i64 = 0xf;
    /// The coverage overlay starts at full strength.
    pub const RADAR_DEFAULT: i64 = 100;
    /// What `iScanZoom` is added to on the way out, and taken off on the way
    /// in, so that the value is a single printable digit.
    pub const ZOOM_BIAS: i64 = 5;
}

impl crate::App {
    /// Restore the scanner from a `stars.ini`.
    ///
    /// Two details of `ReadIniSettings` are worth keeping. The zoom is
    /// stored as one character — the digit `iScanZoom + 5` — and a stored
    /// `0` is **rejected**, not read as `-5`: the test is
    /// `if (v != 0 && v < 10)`. And `grbitScan` is sanity-checked on the way
    /// in: if `(v & 0xc00f) > 5` — a view the game does not have, or either
    /// of the two top bits — both it **and the ship filter** are thrown away
    /// and start again at nothing.
    pub fn read_scanner_ini(&mut self, ini: &Ini) {
        use scanner as key;

        let zoom = ini.int(WINDOWS, key::ZOOM, 4);
        if zoom != 0 && zoom < 10 {
            self.scan_zoom = i8::try_from(zoom - key::ZOOM_BIAS).unwrap_or(0);
        }
        let mut mode = ini.int(WINDOWS, key::MODE, scanner::MODE_DEFAULT);
        let mut ships = ini.int(WINDOWS, key::SHIP_FILTER, 0);
        if mode & 0xc00f > 5 {
            mode = 0;
            ships = 0;
        }
        #[expect(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "a mask"
        )]
        self.set_grbit_scan(mode as u16);
        #[expect(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "a mask"
        )]
        {
            self.scan_design_filter = ships as u16;
            self.scan_class_filter = ini.int(WINDOWS, key::CLASS_FILTER, 0) as u8;
            self.scan_minefield_filter =
                (ini.int(WINDOWS, key::MINE_FILTER, scanner::MINE_DEFAULT) & 0xf) as u8;
            self.scan_coverage_pct = ini
                .int(WINDOWS, key::RADAR, scanner::RADAR_DEFAULT)
                .clamp(0, 100) as u8;
        }
        self.toolbar_hidden = ini.int(WINDOWS, key::TOOLBAR, 1) == 0;
        self.window_layout = match ini.int(WINDOWS, key::LAYOUT, 1) {
            0 => crate::WindowLayout::Large,
            1 => crate::WindowLayout::Medium,
            _ => crate::WindowLayout::Small,
        };
    }

    /// Write it back.
    ///
    /// `WriteIniSettings` writes the five scanner keys only when a dirty bit
    /// is set and the rest every time; here the file is written whole, which
    /// comes to the same thing.
    pub fn write_scanner_ini(&self, ini: &mut Ini) {
        use scanner as key;

        ini.set(
            WINDOWS,
            key::ZOOM,
            &(i64::from(self.scan_zoom) + key::ZOOM_BIAS).to_string(),
        );
        ini.set(WINDOWS, key::MODE, &self.grbit_scan().to_string());
        ini.set(
            WINDOWS,
            key::SHIP_FILTER,
            &self.scan_design_filter.to_string(),
        );
        ini.set(
            WINDOWS,
            key::CLASS_FILTER,
            &self.scan_class_filter.to_string(),
        );
        ini.set(
            WINDOWS,
            key::MINE_FILTER,
            &(self.scan_minefield_filter & 0xf).to_string(),
        );
        ini.set(WINDOWS, key::RADAR, &self.scan_coverage_pct.to_string());
        ini.set(
            WINDOWS,
            key::TOOLBAR,
            if self.toolbar_hidden { "0" } else { "1" },
        );
        ini.set(
            WINDOWS,
            key::LAYOUT,
            &(self.window_layout as u8).to_string(),
        );
    }
}
