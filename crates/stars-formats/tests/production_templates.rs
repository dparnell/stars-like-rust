//! The `stars.ini` production templates (`ZIPPRODQ`).
//!
//! The game keeps its four production templates in `stars.ini` rather than in
//! a save, packed into printable letters. These check that encoding both ways,
//! and the two values the reader clamps rather than rejects.

use stars_formats::{
    DefaultQueue, DefaultQueueItem, ProductionTemplate, TEMPLATE_INI_SECTION, TEMPLATE_NAME_MAX,
};

fn template(no_research: bool, items: &[(u8, u16)], name: &str) -> ProductionTemplate {
    ProductionTemplate {
        name: name.to_string(),
        queue: Some(DefaultQueue {
            no_research,
            items: items
                .iter()
                .map(|(item, count)| DefaultQueueItem {
                    item: *item,
                    count: *count,
                })
                .collect(),
        }),
    }
}

#[test]
fn a_template_survives_the_round_trip() {
    // The manual's example default for new colonies (p. 7-4), in the ids the
    // template uses: minimum terraform, factories, mines, defences.
    let example = template(
        true,
        &[(4, 10), (1, 10), (0, 10), (2, 2), (1, 25), (0, 25), (5, 10)],
        "New Colony",
    );
    let text = example.encode_ini();
    let back = ProductionTemplate::decode_ini(&text).expect("it decodes");
    assert_eq!(back, example);

    // Every character of the packed body is a letter in 'a'..='p', which is
    // what the reader checks.
    let body = 2 + example.queue.as_ref().unwrap().items.len() * 4;
    assert!(text.as_bytes()[..body]
        .iter()
        .all(|c| (b'a'..=b'p').contains(c)));
    assert!(text.ends_with("New Colony"));
}

#[test]
fn the_header_is_the_flag_and_the_count() {
    let one = template(false, &[(1, 10)], "x");
    let text = one.encode_ini();
    // 'a' for "contribute to research", 'b' for one entry, four characters of
    // entry, then the name.
    assert_eq!(&text[..2], "ab");
    assert_eq!(text.len(), 2 + 4 + 1);

    let no_research = template(true, &[(1, 10)], "x");
    assert_eq!(&no_research.encode_ini()[..1], "b");
    assert!(
        ProductionTemplate::decode_ini(&no_research.encode_ini())
            .unwrap()
            .queue
            .unwrap()
            .no_research
    );
}

#[test]
fn an_empty_slot_writes_nothing_and_reads_as_unused() {
    let empty = ProductionTemplate::default();
    assert_eq!(empty.encode_ini(), "");
    assert_eq!(ProductionTemplate::decode_ini(""), None);
}

/// What the reader refuses, and what it merely clamps.
#[test]
fn the_reader_rejects_what_the_game_rejects() {
    // Too short, and too long.
    assert_eq!(ProductionTemplate::decode_ini("ab"), None);
    assert_eq!(ProductionTemplate::decode_ini(&"a".repeat(0x41)), None);
    // A count past twelve.
    assert_eq!(ProductionTemplate::decode_ini("azname"), None);
    // A body that is not all letters.
    assert_eq!(ProductionTemplate::decode_ini("ab!aaaname"), None);
    // A name of thirteen characters or more.
    let long = template(false, &[(1, 1)], "");
    let text = format!("{}{}", long.encode_ini(), "n".repeat(13));
    assert_eq!(ProductionTemplate::decode_ini(&text), None);

    // A quantity past 1020 is taken as one, and an item that is not an
    // auto-build one becomes item 0.
    let silly = template(false, &[(1, 1023), (9, 5)], "x");
    let back = ProductionTemplate::decode_ini(&silly.encode_ini())
        .expect("it decodes")
        .queue
        .expect("it has a queue");
    assert_eq!(back.items[0], DefaultQueueItem { item: 1, count: 1 });
    assert_eq!(back.items[1], DefaultQueueItem { item: 0, count: 5 });

    // 1020 itself is fine.
    let fine = template(false, &[(1, 1020)], "x");
    let back = ProductionTemplate::decode_ini(&fine.encode_ini())
        .unwrap()
        .queue
        .unwrap();
    assert_eq!(back.items[0].count, 1020);
}

#[test]
fn the_ini_keys_are_the_ones_the_game_writes() {
    assert_eq!(TEMPLATE_INI_SECTION, "ZipOrders");
    assert_eq!(ProductionTemplate::ini_key(0), "ZipOrdersP1");
    assert_eq!(ProductionTemplate::ini_key(4), "ZipOrdersP5");
}

#[test]
fn a_name_is_clipped_to_the_field() {
    let long = template(false, &[(1, 1)], "a very long template name");
    let text = long.encode_ini();
    let back = ProductionTemplate::decode_ini(&text).expect("it decodes");
    assert_eq!(back.name.len(), TEMPLATE_NAME_MAX);
    assert_eq!(back.name, "a very long ");
}
