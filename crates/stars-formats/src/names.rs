//! Planet-name table for the `.xy` universe file.
//!
//! Each planet in a `.xy` file stores its name as a 10-bit index (`nameid`)
//! into a fixed master list of planet names that ships inside `STARS!.EXE`. The
//! list is not stored in the save files themselves, only the indices, so a
//! reader needs the same table to turn an index back into a name.
//!
//! The table embedded here (`data/star-names.txt`, index → name, tab-delimited)
//! was recovered from the original game (via the Map2XY tool's `planets.txt`,
//! which the tool's README notes "really come from Stars!.exe"). It is verified
//! against real universes: every planet in the six sample `.xy` files
//! (`fixtures/`) resolves to a **unique** entry in this table (see
//! `docs/formats/xy.md`).

use std::sync::OnceLock;

/// The raw embedded table (`"<index>\t<name>"` per line, indices `0..=998`).
const STAR_NAMES_RAW: &str = include_str!("../data/star-names.txt");

/// Parsed table, indexed directly by `nameid`.
fn table() -> &'static Vec<&'static str> {
    static TABLE: OnceLock<Vec<&'static str>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut names: Vec<&'static str> = Vec::new();
        for line in STAR_NAMES_RAW.lines() {
            let Some((idx, name)) = line.split_once('\t') else {
                continue;
            };
            let Ok(idx) = idx.trim().parse::<usize>() else {
                continue;
            };
            if idx >= names.len() {
                names.resize(idx + 1, "");
            }
            names[idx] = name;
        }
        names
    })
}

/// The number of planet names in the master table.
#[must_use]
pub fn planet_name_count() -> usize {
    table().len()
}

/// Resolve a planet `nameid` (the 10-bit index from a `.xy` planet record) to
/// its name, or `None` if the index is outside the master table.
#[must_use]
pub fn planet_name(name_id: u16) -> Option<&'static str> {
    table()
        .get(name_id as usize)
        .copied()
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_fully_populated() {
        // The recovered table holds 999 names, indices 0..=998.
        assert_eq!(planet_name_count(), 999);
        for i in 0..999u16 {
            assert!(planet_name(i).is_some(), "missing name for index {i}");
        }
    }

    #[test]
    fn known_endpoints_resolve() {
        assert_eq!(planet_name(0), Some("007"));
        assert_eq!(planet_name(998), Some("Zulu"));
    }

    #[test]
    fn out_of_range_is_none() {
        assert_eq!(planet_name(999), None);
        assert_eq!(planet_name(1023), None);
    }
}
