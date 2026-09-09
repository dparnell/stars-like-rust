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
    assert_eq!(minerals.height(13.0, false), 5.0 + 6.0 * 13.0);
    // It has no `EnsureTileSize` adjustment, so the small layout leaves it be.
    assert_eq!(minerals.height(13.0, true), minerals.height(13.0, false));
}

/// Three of the six do move when the window layout changes, and each by its
/// own amount.
#[test]
fn the_small_layout_shrinks_three_of_them() {
    let picture = PLANET_TILES[0];
    let ships = PLANET_TILES[3];
    let production = PLANET_TILES[4];
    // A flat ten for the picture.
    assert_eq!(
        picture.height(13.0, false) - picture.height(13.0, true),
        20.0
    );
    // `(dyArial8 + 4) * 2` for the ship list and `(dyArial8 + 2) * 2` for the
    // queue — so the difference between large and small is twice that.
    assert_eq!(ships.height(13.0, false) - ships.height(13.0, true), 68.0);
    assert_eq!(
        production.height(13.0, false) - production.height(13.0, true),
        60.0
    );
}

/// A column starts four pixels down and leaves four between one tile and the
/// next, and closing a tile takes everything below it up.
#[test]
fn closing_a_tile_reflows_the_column() {
    let line = 13.0_f32;
    let all_open = [true; 6];
    let open = tiles::column_tops(line, false, &all_open, 0);
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
    let after = tiles::column_tops(line, false, &closed, 0);
    assert_eq!(after[0].2, Tile::closed_height(line), "just its title bar");
    let gave_back = open[0].2 - after[0].2;
    assert_eq!(after[1].1, open[1].1 - gave_back);
    assert_eq!(after[2].1, open[2].1 - gave_back);

    // The other column is untouched by any of it.
    let right = tiles::column_tops(line, false, &closed, 1);
    assert_eq!(right[0].1, tiles::TILE_GAP);
}
