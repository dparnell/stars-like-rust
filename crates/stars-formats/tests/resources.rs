//! The pictures, read out of the game's own executable.
//!
//! These tests need a copy of `stars.exe`; without one they skip, like every
//! other fixture-backed test here. Nothing is extracted into the repository —
//! the point of the module under test is that the artwork stays in the
//! executable the player already has.

use std::path::PathBuf;

use stars_formats::resources::{
    art::{self, EmblemSize, ShipSize},
    find, read_bitmap, read_dib, resources, Image, Name, RT_BITMAP,
};

fn executable() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    for name in ["stars.2.7j.exe", "stars.exe", "STARS!.EXE"] {
        if let Ok(bytes) = std::fs::read(root.join(name)) {
            return Some(bytes);
        }
    }
    None
}

fn bitmap(exe: &[u8], name: Name) -> Image {
    read_bitmap(exe, &name).unwrap_or_else(|e| panic!("{name:?}: {e}"))
}

/// The resource table reads, and holds what the loader expects to find in it.
#[test]
fn the_resource_table_holds_the_pictures() {
    let Some(exe) = executable() else { return };
    let all = resources(&exe).expect("a resource table");
    let bitmaps: Vec<_> = all
        .iter()
        .filter(|r| r.kind == Name::Id(RT_BITMAP))
        .collect();
    assert_eq!(bitmaps.len(), 38, "every picture the game draws");

    // Five of them are named rather than numbered, and the executable stores
    // those names in upper case while the code asks for them in mixed case.
    let named: Vec<&str> = bitmaps
        .iter()
        .filter_map(|r| match &r.name {
            Name::Text(text) => Some(text.as_str()),
            Name::Id(_) => None,
        })
        .collect();
    assert_eq!(named.len(), 5, "{named:?}");
    assert!(named.contains(&"CARGOBMP"));
    assert!(
        find(&exe, RT_BITMAP, &Name::Text("CargoBmp".to_string())).is_some(),
        "asking for it the way the code does still finds it"
    );

    // Everything the catalogue names is really there.
    for (variable, which, _) in art::CATALOGUE {
        assert!(
            find(&exe, RT_BITMAP, &which.name()).is_some(),
            "{variable} ({which:?})"
        );
    }

    // And there are icons and cursors alongside them.
    assert!(all.iter().any(|r| r.kind == Name::Id(3)), "icons");
    assert!(all.iter().any(|r| r.kind == Name::Id(1)), "cursors");
}

/// The three colour depths the game uses all decode, and to the same pixels an
/// independent decoder gets.
#[test]
fn every_colour_depth_decodes() {
    let Some(exe) = executable() else { return };

    // Eight bits a pixel, with a 256-entry palette.
    let races = bitmap(&exe, Name::Id(133));
    assert_eq!((races.width, races.height), (256, 128));
    assert_eq!(races.pixel(40, 10), Some((225, 225, 225, 255)));
    assert_eq!(races.pixel(255, 127), Some((0, 63, 0, 255)));
    assert_eq!(races.pixel(256, 0), None, "off the right-hand edge");

    // Four bits a pixel.
    let cargo = bitmap(&exe, Name::Text("CargoBmp".to_string()));
    assert_eq!((cargo.width, cargo.height), (8, 8));
    assert_eq!(cargo.pixel(0, 0), Some((192, 192, 192, 255)));

    // One bit a pixel: the strip of glyphs the victory report blits.
    let mono = bitmap(&exe, Name::Id(199));
    assert_eq!((mono.width, mono.height), (15, 84));
    assert_eq!(mono.pixel(0, 0), Some((0, 0, 0, 255)));
    assert_eq!(mono.pixel(7, 3), Some((255, 255, 255, 255)));

    // Everything comes back opaque; these carry no alpha and are blitted with
    // `SRCCOPY`.
    assert!(races.pixels.chunks(4).all(|p| p[3] == 0xff));
}

/// Every bitmap in the executable decodes, and to the size its header claims.
#[test]
fn all_thirty_eight_decode() {
    let Some(exe) = executable() else { return };
    let mut seen = 0;
    for resource in resources(&exe).expect("a resource table") {
        if resource.kind != Name::Id(RT_BITMAP) {
            continue;
        }
        let data = resource.data(&exe).expect("its bytes");
        let image = read_dib(data).unwrap_or_else(|e| panic!("{:?}: {e}", resource.name));
        assert_eq!(
            image.pixels.len(),
            (image.width * image.height * 4) as usize,
            "{:?}",
            resource.name
        );
        assert!(image.width > 0 && image.height > 0);
        seen += 1;
    }
    assert_eq!(seen, 38);
}

/// The race emblems tile the way `DibBlt` says: eight across, four down, and
/// all thirty-two of them have something in them.
#[test]
fn the_race_emblems_tile_eight_across() {
    let Some(exe) = executable() else { return };
    let sheet = bitmap(&exe, Name::Id(133));

    assert_eq!(
        art::emblem(0, EmblemSize::Large),
        Some(art::Cell {
            resource: 133,
            x: 0,
            y: 0,
            width: 32,
            height: 32
        })
    );
    // Emblem 8 is the start of the second row, not the second column.
    let ninth = art::emblem(8, EmblemSize::Large).expect("emblem 8");
    assert_eq!((ninth.x, ninth.y), (0, 32));
    assert_eq!(art::emblem(art::EMBLEMS, EmblemSize::Large), None);

    for logo in 0..art::EMBLEMS {
        let cell = art::emblem(logo, EmblemSize::Large).expect("a cell");
        let picture = sheet
            .crop(cell.x, cell.y, cell.width, cell.height)
            .unwrap_or_else(|| panic!("emblem {logo} is inside the sheet"));
        assert!(
            picture.pixels.chunks(4).any(|p| p[..3] != [0, 0, 0]),
            "emblem {logo} is not blank"
        );
    }

    // The same emblems again at two smaller sizes.
    for (size, side, id) in [(EmblemSize::Medium, 16, 80), (EmblemSize::Small, 8, 79)] {
        let sheet = bitmap(&exe, Name::Id(id));
        let cell = art::emblem(31, size).expect("the last emblem");
        assert_eq!(cell.resource, id);
        assert_eq!((cell.width, cell.height), (side, side));
        assert!(sheet.crop(cell.x, cell.y, side, side).is_some());
    }
}

/// The component pictures are thirty-two to a sheet, eight across.
#[test]
fn the_component_pictures_are_thirty_two_to_a_sheet() {
    let Some(exe) = executable() else { return };

    assert_eq!(
        art::component(0).map(|c| (c.resource, c.x, c.y)),
        Some((500, 0, 0))
    );
    assert_eq!(
        art::component(7).map(|c| (c.resource, c.x, c.y)),
        Some((500, 448, 0))
    );
    assert_eq!(
        art::component(8).map(|c| (c.resource, c.x, c.y)),
        Some((500, 0, 64))
    );
    assert_eq!(art::component(32).map(|c| c.resource), Some(501));
    assert_eq!(
        art::component(7 * 32).map(|c| c.resource),
        None,
        "past the last sheet"
    );

    // Every cell of the six full-width sheets is really there.
    for sheet in 0..6u16 {
        let picture = bitmap(&exe, Name::Id(art::COMPONENT_SHEETS[sheet as usize]));
        assert_eq!((picture.width, picture.height), (512, 256));
        for slot in 0..32u16 {
            let cell = art::component(sheet * 32 + slot).expect("a cell");
            assert!(
                picture.crop(cell.x, cell.y, 64, 64).is_some(),
                "sheet {sheet} slot {slot}"
            );
        }
    }

    // The last sheet is half as wide, so its right-hand columns are cells that
    // do not exist. Cropping is what catches that.
    let last = bitmap(&exe, Name::Id(506));
    assert_eq!((last.width, last.height), (256, 256));
    let inside = art::component(6 * 32 + 3).expect("a cell");
    assert!(last.crop(inside.x, inside.y, 64, 64).is_some());
    let outside = art::component(6 * 32 + 7).expect("a cell");
    assert!(last.crop(outside.x, outside.y, 64, 64).is_none());
}

/// The ships run down the columns and the planets run up the sheet — the two
/// sheets disagree, and both are transcribed as they are.
#[test]
fn ships_and_planets_are_indexed_differently() {
    let Some(exe) = executable() else { return };

    // Ships: four to a column, so 0..4 is the first column.
    let first = art::ship(0, ShipSize::Large).expect("ship 0");
    let second = art::ship(1, ShipSize::Large).expect("ship 1");
    assert_eq!((first.x, first.y), (0, 0));
    assert_eq!((second.x, second.y), (0, 64), "down, not across");
    assert_eq!(art::ship(4, ShipSize::Large).map(|c| c.x), Some(64));
    assert_eq!(
        art::ship(32, ShipSize::Large).map(|c| c.resource),
        Some(553)
    );
    assert_eq!(art::ship(5 * 32, ShipSize::Large), None);
    // And the small sheet is the same index at half the size.
    let small = art::ship(1, ShipSize::Small).expect("ship 1, small");
    assert_eq!((small.resource, small.x, small.y), (557, 0, 32));

    // Planets: seven across, and the rows run up the decoded picture.
    let sheet = bitmap(&exe, Name::Id(art::PLANET_SHEET));
    assert_eq!((sheet.width, sheet.height), (448, 256));
    assert_eq!(art::planet(0).map(|c| (c.x, c.y)), Some((0, 192)));
    assert_eq!(art::planet(6).map(|c| (c.x, c.y)), Some((384, 192)));
    assert_eq!(art::planet(7).map(|c| (c.x, c.y)), Some((0, 128)));
    assert_eq!(art::planet(27).map(|c| (c.x, c.y)), Some((384, 0)));
    assert_eq!(art::planet(28), None, "twenty-eight of them");
    for index in 0..28 {
        let cell = art::planet(index).expect("a cell");
        assert!(
            sheet.crop(cell.x, cell.y, 64, 64).is_some(),
            "planet {index}"
        );
    }
}

/// A file that is not an executable is refused rather than misread.
#[test]
fn rubbish_is_refused() {
    assert!(resources(b"").is_err());
    assert!(resources(&[0u8; 4096]).is_err());
    let mut almost = vec![0u8; 4096];
    almost[0..2].copy_from_slice(b"MZ");
    assert!(resources(&almost).is_err(), "a DOS stub with no NE header");
    assert!(read_dib(&[]).is_err());
    assert!(read_dib(&[0u8; 40]).is_err(), "a header claiming nothing");
}
