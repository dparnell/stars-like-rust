//! Writing a race out as a `.rN` file.
//!
//! The race wizard's output has to be a file the original will load, so the
//! test that matters is against the original's own output: take each of the
//! seven races that ship with the game, read it in, turn it back into a race
//! file, and require the bytes to match. That covers the header, the cipher,
//! the block framing and every field of the record at once.
//!
//! Tests skip rather than fail when the fixtures are absent.

use stars_core::{load, save};
use stars_formats::{PlayerRecord, RaceRecord, StarsFile};

/// The seven races that ship with the game.
const RACES: [&str; 7] = [
    "antetherial.r1",
    "humanoid.r1",
    "insectoid.r1",
    "nucleoid.r1",
    "rabitoid.r1",
    "random.r1",
    "silicanoid.r1",
];

/// Read a race fixture, or `None` when the fixtures are not checked out.
fn fixture(name: &str) -> Option<Vec<u8>> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/r")
            .join(name),
    )
    .ok()
}

#[test]
fn the_shipped_races_are_written_back_byte_for_byte() {
    let mut checked = 0;
    for name in RACES {
        let Some(bytes) = fixture(name) else { continue };
        let file = StarsFile::decode(&bytes).expect("decodes");
        let block = file
            .blocks
            .iter()
            .find(|b| b.type_id == 6)
            .expect("a race block");
        let player = PlayerRecord::from_payload(&block.data).expect("a player record");
        let record = RaceRecord::from_payload(&block.data).expect("a race record");

        // Written under the header the original wrote — the game id, the
        // version and the cipher salt are the header's, and everything after
        // it is ours — this has to be the same file.
        let written =
            save::race_file_with_header(&file.header, &record, player.logo).expect("writes");
        assert_eq!(written, bytes, "{name} did not come back byte for byte");
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no race fixtures present in fixtures/r/");
    }
}

#[test]
fn a_race_survives_the_trip_through_a_file() {
    let mut checked = 0;
    for name in RACES {
        let Some(bytes) = fixture(name) else { continue };
        let file = StarsFile::decode(&bytes).expect("decodes");
        let record = RaceRecord::from_file(&file).expect("a race record");
        let race = load::race_from_record(&record);

        // The wizard's direction: a `Race` in hand, write the file, read it
        // back, and get the same race.
        let written =
            save::race_file(&race, &record.singular_name, &record.plural_name, 0).expect("writes");
        let reread = RaceRecord::from_file(&StarsFile::decode(&written).expect("decodes"))
            .expect("a race record");

        assert_eq!(reread.singular_name, record.singular_name, "{name} name");
        assert_eq!(reread.plural_name, record.plural_name, "{name} plural");
        assert_eq!(reread.gravity, record.gravity, "{name} gravity");
        assert_eq!(reread.temperature, record.temperature, "{name} temperature");
        assert_eq!(reread.radiation, record.radiation, "{name} radiation");
        assert_eq!(reread.growth_rate, record.growth_rate, "{name} growth");
        assert_eq!(reread.economy, record.economy, "{name} economy");
        assert_eq!(
            reread.research_cost, record.research_cost,
            "{name} research"
        );
        assert_eq!(reread.prt, record.prt, "{name} PRT");
        assert_eq!(reread.lrt_bits, record.lrt_bits, "{name} LRTs");
        assert_eq!(
            reread.spend_leftover_points, record.spend_leftover_points,
            "{name} leftover points"
        );
        assert_eq!(
            (
                reread.expensive_tech_starts_at_level_3,
                reread.factories_cost_one_less_germanium
            ),
            (
                record.expensive_tech_starts_at_level_3,
                record.factories_cost_one_less_germanium
            ),
            "{name} checkboxes"
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no race fixtures present in fixtures/r/");
    }
}

#[test]
fn the_presets_are_the_races_the_game_ships() {
    // `RACES` is in file order; `presets::ALL` is in the wizard's button order,
    // which is the order of the table it comes from.
    const FILES: [&str; 7] = [
        "humanoid.r1",
        "rabitoid.r1",
        "insectoid.r1",
        "nucleoid.r1",
        "silicanoid.r1",
        "antetherial.r1",
        "random.r1",
    ];
    let mut checked = 0;
    for (preset, file) in stars_core::presets::ALL.iter().zip(FILES) {
        let Some(bytes) = fixture(file) else { continue };
        let decoded = StarsFile::decode(&bytes).expect("decodes");

        // The whole race — its stats, its habitable ranges, its traits, its
        // emblem and its two names — written from the transcribed table under
        // the header the original wrote, has to be the shipped file.
        let written = save::race_file_with_header(
            &decoded.header,
            &save::record_from_race(&preset.race, preset.name, preset.plural),
            preset.emblem,
        )
        .expect("writes");
        assert_eq!(written, bytes, "preset {} is not {file}", preset.name);
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skipping: no race fixtures present in fixtures/r/");
    }
}

#[test]
fn the_same_race_written_twice_is_the_same_file() {
    let race = stars_core::Race::humanoid();
    let once = save::race_file(&race, "Humanoid", "Humanoids", 1).expect("writes");
    let twice = save::race_file(&race, "Humanoid", "Humanoids", 1).expect("writes");
    assert_eq!(once, twice);
}
