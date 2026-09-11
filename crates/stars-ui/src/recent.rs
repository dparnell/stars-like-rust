//! The File menu's list of recently opened games.
//!
//! `InitializeMenu` (`1020:5560`) rebuilds the tail of the File menu every
//! time it drops: it deletes ids `0x10cc`–`0x10d4` and reinserts one item per
//! name held in `vrgszMRU`, **by position at index 9** — which is between the
//! last separator and `E&xit`. Each is captioned `&1 ` and the whole path,
//! not the file's name alone.
//!
//! `vrgszMRU` is nine slots of `0x100` bytes, allocated once in
//! `ReadIniSettings` (`1000:1db3`) and filled from `stars.ini`:
//!
//! ```ini
//! [Files]
//! File1=C:\STARS\GAME.M1
//! File2=...
//! ```
//!
//! Three rules come out of those two routines and are kept here.
//!
//! A name **shorter than four characters is no name**: `ReadIniSettings`
//! drops anything the profile call returns fewer than four characters for.
//! After reading, the list is **compacted** — the non-empty slots are shuffled
//! up so there are no holes, which means a missing `File3` does not strand
//! `File4`. And `FLoadGame` (`1070:303d`) compares **case-insensitively**
//! (`_fstricmp`) and short-circuits: a game that is already first is not
//! promoted, and nothing else moves.
//!
//! `File1` does double duty. `ReadIniSettings` copies it into `szBase` and
//! sets the startup-file bit, so a launch with nothing else to go on reopens
//! the game last played.

/// The list, most recent first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recent {
    paths: Vec<String>,
}

impl Recent {
    /// How many the original keeps: `vrgszMRU` is `0x900` bytes of `0x100`.
    pub const SLOTS: usize = 9;

    /// The shortest string `ReadIniSettings` will take as a name.
    pub const SHORTEST: usize = 4;

    /// The `stars.ini` section these live in.
    pub const SECTION: &'static str = "Files";

    /// Nothing opened yet.
    #[must_use]
    pub fn new() -> Recent {
        Recent::default()
    }

    /// The names, most recent first.
    #[must_use]
    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Whether there is anything to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// The game a launch with no argument reopens — `File1`, which
    /// `ReadIniSettings` copies into `szBase`.
    #[must_use]
    pub fn startup_file(&self) -> Option<&str> {
        self.paths.first().map(String::as_str)
    }

    /// A menu caption: `&1 ` and the path, as `InitializeMenu` builds it.
    ///
    /// `index` is zero-based; the digit shown is one more.
    #[must_use]
    pub fn caption(&self, index: usize) -> Option<String> {
        let path = self.paths.get(index)?;
        Some(format!("&{} {path}", index + 1))
    }

    /// Take note of a game that has just been opened.
    ///
    /// `FLoadGame`: compare case-insensitively against the first entry and do
    /// nothing at all when it matches; otherwise take the name out of
    /// wherever else it is in the list, put it at the front, and drop
    /// whatever falls off the end.
    ///
    /// Returns whether anything moved, which is what decides whether the
    /// list needs writing out again.
    pub fn opened(&mut self, path: &str) -> bool {
        if path.chars().count() < Self::SHORTEST {
            return false;
        }
        if self
            .paths
            .first()
            .is_some_and(|first| first.eq_ignore_ascii_case(path))
        {
            return false;
        }
        self.paths.retain(|held| !held.eq_ignore_ascii_case(path));
        self.paths.insert(0, path.to_string());
        self.paths.truncate(Self::SLOTS);
        true
    }

    /// Read the list out of a `stars.ini`'s `[Files]` section.
    ///
    /// Keys are `File1` to `File9`. A value the original would have rejected
    /// — fewer than four characters — is dropped, and the rest are compacted,
    /// so `File1` missing does not hide `File2`.
    #[must_use]
    pub fn read_ini(ini: &crate::settings::Ini) -> Recent {
        let paths = (1..=Self::SLOTS)
            .filter_map(|slot| ini.get(Self::SECTION, &format!("File{slot}")))
            .filter(|value| value.chars().count() >= Self::SHORTEST)
            .map(str::to_string)
            .collect();
        Recent { paths }
    }

    /// Write the section back, compacted, taking out any slot that is no
    /// longer used so a shorter list does not leave the old tail behind.
    pub fn write_ini(&self, ini: &mut crate::settings::Ini) {
        for slot in 1..=Self::SLOTS {
            let key = format!("File{slot}");
            match self.paths.get(slot - 1) {
                Some(path) => ini.set(Self::SECTION, &key, path),
                None => ini.remove(Self::SECTION, &key),
            }
        }
    }
}
