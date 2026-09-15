//! The planet pane's tile table — `rgtilePlanet` (`1120:07fc`), sized by
//! `InitTiles` (`1000:0eb8`) and laid out by `ReflowColumn`.
//!
//! See `docs/ui/planet-pane.md`.

use stars_ui::tiles::{self, Tile, PLANET_TILES};

/// Three tiles down each column, in the table's order.
#[test]
fn the_table_is_two_columns_of_three() {
    let left: Vec<&Tile> = PLANET_TILES.iter().filter(|t| t.column == 0).collect();
    let right: Vec<&Tile> = PLANET_TILES.iter().filter(|t| t.column == 1).collect();
    assert_eq!(left.len(), 3);
    assert_eq!(right.len(), 3);
    // The left column is the planet, its minerals and its status.
    assert_eq!(left[1].title, "Minerals On Hand");
    assert_eq!(left[2].title, "Status");
    // The right is what is in orbit, what it is building and what from; the
    // first and last take their titles from the game.
    assert_eq!(right[1].title, "Production");
    assert_eq!(right[0].title, "");
    assert_eq!(right[2].title, "");
}

/// A tile's height is `remainder + lines * dyArial8`, which is what `InitTiles`
/// folds the table's first two words into.
#[test]
fn a_tiles_height_is_its_lines_plus_its_remainder() {
    // Minerals On Hand is six lines plus five: three minerals, a rule, mines
    // and factories.
    let minerals = PLANET_TILES[1];
    assert_eq!(minerals.lines, 6);
    assert_eq!(minerals.extra, 5);
    // — with the leading of one row kept for the last row's descenders,
    // since a line here is the glyphs alone.
    assert_eq!(
        minerals.height(13.0, false),
        5.0 + 6.0 * 13.0 + tiles::LEADING
    );
    // It has no `EnsureTileSize` adjustment in the planet table, so the small
    // layout leaves it be — even though the fleet table's tile with the same
    // `grbit` does move.
    assert_eq!(minerals.resize, (0, 0));
    assert_eq!(minerals.height(13.0, true), minerals.height(13.0, false));
}

/// Three of the six do move when the window layout changes, and each by its
/// own amount.
#[test]
fn the_small_layout_shrinks_three_of_them() {
    let picture = PLANET_TILES[0];
    let ships = PLANET_TILES[3];
    let production = PLANET_TILES[4];
    // The table ships holding the **large** size, and `EnsureTileSize` takes
    // the resize off once on the way into the small layout — so the gap
    // between the two layouts is the resize itself, not twice it.
    assert_eq!(
        picture.height(13.0, false) - picture.height(13.0, true),
        10.0,
        "a flat ten for the picture"
    );
    assert_eq!(
        ships.height(13.0, false) - ships.height(13.0, true),
        34.0,
        "(dyArial8 + 4) * 2 for the ship list"
    );
    assert_eq!(
        production.height(13.0, false) - production.height(13.0, true),
        30.0,
        "(dyArial8 + 2) * 2 for the queue"
    );

    // And the large size is the table's own, plus the leading kept.
    assert_eq!(
        picture.height(13.0, false),
        f32::from(picture.extra) + f32::from(picture.lines) * 13.0 + tiles::LEADING
    );
}

/// A column starts four pixels down and leaves four between one tile and the
/// next, and closing a tile takes everything below it up.
#[test]
fn closing_a_tile_reflows_the_column() {
    let line = 13.0_f32;
    let all_open = [true; 6];
    let open = tiles::column_tops(&PLANET_TILES, line, false, &all_open, 0);
    assert_eq!(open.len(), 3);
    assert_eq!(open[0].1, tiles::TILE_GAP, "four pixels down");
    assert_eq!(
        open[1].1,
        open[0].1 + open[0].2 + tiles::TILE_GAP,
        "and four between"
    );

    // Close the first and the two below it move up by what it gave back.
    let mut closed = all_open;
    closed[0] = false;
    let after = tiles::column_tops(&PLANET_TILES, line, false, &closed, 0);
    assert_eq!(after[0].2, Tile::closed_height(line), "just its title bar");
    let gave_back = open[0].2 - after[0].2;
    assert_eq!(after[1].1, open[1].1 - gave_back);
    assert_eq!(after[2].1, open[2].1 - gave_back);

    // The other column is untouched by any of it.
    let right = tiles::column_tops(&PLANET_TILES, line, false, &closed, 1);
    assert_eq!(right[0].1, tiles::TILE_GAP);
}

/// The fleet pane is the same window with a different table: seven tiles,
/// **four** down the left column rather than three.
#[test]
fn the_ship_table_is_four_left_and_three_right() {
    use stars_ui::tiles::SHIP_TILES;

    let left: Vec<&Tile> = SHIP_TILES.iter().filter(|t| t.column == 0).collect();
    let right: Vec<&Tile> = SHIP_TILES.iter().filter(|t| t.column == 1).collect();
    assert_eq!(left.len(), 4);
    assert_eq!(right.len(), 3);
    assert_eq!(left[2].title, "Fleet Waypoints");
    assert_eq!(left[3].title, "Waypoint Task");
    assert_eq!(right[0].title, "Fuel & Cargo");
    assert_eq!(right[1].title, "Fleet Composition");
    // The last is `DrawPlanetShipList` again, titled by the game.
    assert_eq!(right[2].title, "");
    assert_eq!(right[2].grbit, PLANET_TILES[3].grbit, "the shared tile");
}

/// `grbit` alone does not say how a tile resizes: `0x40` means the planet
/// pane's production queue in one table and the fleet pane's location tile in
/// the other, and the two move by different amounts.
#[test]
fn the_two_tables_read_grbit_differently() {
    use stars_ui::tiles::SHIP_TILES;

    let queue = PLANET_TILES[4];
    let location = SHIP_TILES[1];
    assert_eq!(queue.grbit, 0x40);
    assert_eq!(location.grbit, 0x40);
    assert_ne!(queue.resize, location.resize);
    // `(dyArial8 + 2) * 2` against a flat six.
    assert_eq!(queue.height(13.0, false) - queue.height(13.0, true), 30.0);
    assert_eq!(
        location.height(13.0, false) - location.height(13.0, true),
        6.0
    );
    // `0x01` is the other disagreement: Minerals On Hand does not move at all
    // and Fuel & Cargo moves by `dyArial8 * 4 + 2`.
    assert_eq!(PLANET_TILES[1].grbit, SHIP_TILES[4].grbit);
    assert_eq!(PLANET_TILES[1].resize, (0, 0));
    assert_eq!(SHIP_TILES[4].resize, (4, 2));

    // The shared tile does agree in both tables.
    assert_eq!(
        PLANET_TILES[3].height(13.0, false) - PLANET_TILES[3].height(13.0, true),
        SHIP_TILES[6].height(13.0, false) - SHIP_TILES[6].height(13.0, true)
    );
}

/// `fSmallTiles` is the **Window Layout**, not the screen: `FrameWndProc`
/// (`1020:072d`) asks `EnsureTileSize` for `iWindowLayout == 2`, so only
/// the smallest of the three settings shrinks the tiles.
#[test]
fn the_smallest_window_layout_is_what_shrinks_them() {
    use stars_ui::{App, WindowLayout};

    let mut app = App::new();
    let line = 13.0_f32;
    let small = |app: &App| app.window_layout == WindowLayout::Small;

    for layout in [WindowLayout::Large, WindowLayout::Medium] {
        app.window_layout = layout;
        assert!(!small(&app), "{layout:?} leaves them full size");
    }
    app.window_layout = WindowLayout::Small;
    assert!(small(&app));

    // Which is the difference the tiles actually see.
    let picture = PLANET_TILES[0];
    assert_eq!(
        picture.height(line, small(&app)) + 10.0,
        picture.height(line, false)
    );
}
