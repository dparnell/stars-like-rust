//! The Ship and Starbase Designer, driven the way a player drives it.
//!
//! A generated game gives a real player with real starting designs, so these
//! go through the dialog's own doors: pick a hull, copy it, drag parts on and
//! off, OK it, and delete a design that has ships built to it.

use stars_core::components::slot;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, DesignView, DesignerDrag};

/// A generated two-player game, in memory.
fn a_game() -> App {
    let mut app = App::new();
    let config = NewGame {
        name: "designer".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("Turindrones").as_player(),
        ],
        ..NewGame::default()
    };
    app.new_game(&config).expect("creates the game");
    app
}

/// The dialog opens on the player's own designs, and every one of them is a
/// design they really have.
#[test]
fn the_designer_opens_on_the_players_own_designs() {
    let mut app = a_game();
    app.open_designer();
    let designer = app.designer.as_ref().expect("the dialog is open");
    assert_eq!(designer.view, DesignView::Existing);
    assert!(!designer.starbase);
    assert!(designer.editing.is_none());

    let list = app.designer_list();
    assert!(
        !list.is_empty(),
        "a new game starts the player with several designs"
    );
    let subject = app.designer_subject().expect("the first design");
    assert_eq!(subject.name, list[0]);
    assert!(subject.hull_id >= 0);

    // Starbase mode has its own, shorter, list.
    app.designer.as_mut().expect("open").starbase = true;
    let bases = app.designer_list();
    assert!(
        !bases.is_empty(),
        "the player starts with a starbase design"
    );
    let base = app.designer_subject().expect("a starbase");
    assert!(base.is_starbase(), "{} should be a starbase", base.name);
}

/// **Available Hull Types** lists only hulls the player has researched, and
/// their schematic comes out empty.
#[test]
fn available_hull_types_lists_what_can_be_built() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;

    let hulls = app.designer_hulls();
    assert!(!hulls.is_empty());
    let names: Vec<&str> = hulls.iter().map(|h| h.name).collect();
    // Everybody can build these at tech zero.
    assert!(names.contains(&"Scout"), "{names:?}");
    assert!(names.contains(&"Small Freighter"), "{names:?}");
    // A Humanoid is Jack of All Trades, so the traits' own hulls are not there.
    assert!(!names.contains(&"Meta Morph"), "{names:?}");
    assert!(!names.contains(&"Dreadnought"), "{names:?}");

    let schematic = app.designer_schematic();
    assert!(!schematic.is_empty());
    assert!(schematic.iter().all(|s| s.fitted.is_none()));
    // A bare hull's engine slot asks to be filled; the others merely may be.
    let engine = schematic
        .iter()
        .find(|s| s.allowed & slot::ENGINE != 0)
        .expect("a ship hull has an engine slot");
    assert!(engine.label.starts_with("needs "), "{}", engine.label);
    let other = schematic
        .iter()
        .find(|s| s.allowed & slot::ENGINE == 0)
        .expect("and something else");
    assert!(other.label.starts_with("up to "), "{}", other.label);
}

/// Copy a hull, fit it out, and OK it: the design lands in a free slot.
#[test]
fn a_hull_can_be_copied_fitted_and_saved() {
    let mut app = a_game();
    app.open_designer();
    let before = app.designer_list().len();

    // Pick the Scout from the hull list.
    app.designer.as_mut().expect("open").view = DesignView::Hulls;
    let scout = app
        .designer_hulls()
        .iter()
        .position(|h| h.name == "Scout")
        .expect("a Scout");
    app.designer.as_mut().expect("open").selected = scout;

    assert!(app.designer_can_copy());
    app.designer_copy();
    let editing = app
        .designer
        .as_ref()
        .expect("open")
        .editing
        .clone()
        .expect("copying opens the editor");
    assert!(editing.fresh, "a copied design is thrown away by Cancel");
    assert_eq!(editing.design.name, "Scout");

    // With no engine, OK is refused and says why.
    app.designer_ok();
    assert!(app.designer.as_ref().expect("open").editing.is_some());
    let complaint = app
        .designer
        .as_ref()
        .expect("open")
        .complaint
        .clone()
        .expect("the original complains too");
    assert!(complaint.contains("engines"), "{complaint}");

    // Drag an engine onto the engine slot. The Quick Jump 5 is index 1.
    let engine_slot = app
        .designer_schematic()
        .iter()
        .position(|s| s.allowed & slot::ENGINE != 0)
        .expect("an engine slot");
    let dropped = app.designer_drop_on_slot(
        DesignerDrag {
            category: slot::ENGINE,
            item: 1,
            count: 1,
            from_slot: None,
        },
        engine_slot,
    );
    assert!(dropped);

    // An engine slot fills whatever the drag was carrying, so one drag fills
    // the whole slot.
    let fitted = app.designer_schematic();
    let (name, count) = fitted[engine_slot]
        .fitted
        .clone()
        .expect("the engine went in");
    assert_eq!(name, "Quick Jump 5");
    assert_eq!(count, fitted[engine_slot].capacity);

    // Name it and save it.
    app.designer_rename("Test Scout");
    app.designer_ok();
    assert!(app.designer.as_ref().expect("open").editing.is_none());
    assert_eq!(
        app.designer.as_ref().expect("open").view,
        DesignView::Existing
    );

    let after = app.designer_list();
    assert_eq!(after.len(), before + 1);
    assert!(after.contains(&"Test Scout".to_string()), "{after:?}");
    assert!(app.dirty);
}

/// Only identical components stack, a slot fills no further than its capacity,
/// and dropping back on the list takes them off again.
#[test]
fn a_slot_takes_only_what_it_will_hold() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;

    // A hull with an armour slot that holds more than one, so stacking has
    // something to do. The small freighters' slots each hold exactly one.
    let (hull, armour_slot) = {
        let hulls = app.designer_hulls();
        hulls
            .iter()
            .enumerate()
            .find_map(|(index, hull)| {
                let slot = hull
                    .real_slots()
                    .iter()
                    .position(|s| s.allowed & slot::ARMOR != 0 && s.capacity > 1)?;
                Some((index, slot))
            })
            .expect("some hull stacks armour")
    };
    app.designer.as_mut().expect("open").selected = hull;
    app.designer_copy();
    let capacity = app.designer_schematic()[armour_slot].capacity;
    assert!(capacity > 1);

    let armour = DesignerDrag {
        category: slot::ARMOR,
        item: 0,
        count: 1,
        from_slot: None,
    };
    assert!(app.designer_drop_on_slot(armour, armour_slot));
    assert_eq!(
        app.designer_schematic()[armour_slot]
            .fitted
            .as_ref()
            .unwrap()
            .1,
        1
    );

    // A different component will not stack on it, and neither will one the
    // slot does not take at all.
    let other_armour = DesignerDrag { item: 1, ..armour };
    assert!(!app.designer_drop_on_slot(other_armour, armour_slot));
    let engine = DesignerDrag {
        category: slot::ENGINE,
        ..armour
    };
    assert!(!app.designer_drop_on_slot(engine, armour_slot));

    // More of the same stacks, up to the slot's capacity and no further.
    let many = DesignerDrag {
        count: 100,
        ..armour
    };
    assert!(app.designer_drop_on_slot(many, armour_slot));
    assert_eq!(
        app.designer_schematic()[armour_slot]
            .fitted
            .as_ref()
            .unwrap()
            .1,
        capacity
    );
    assert!(
        !app.designer_drop_on_slot(armour, armour_slot),
        "a full slot takes no more"
    );

    // Dragging back to the parts list takes one off.
    assert!(app.designer_drop_on_list(DesignerDrag {
        count: 1,
        from_slot: Some(armour_slot),
        ..armour
    }));
    assert_eq!(
        app.designer_schematic()[armour_slot]
            .fitted
            .as_ref()
            .unwrap()
            .1,
        capacity - 1
    );
}

/// Dropping an engine back on the list removes the whole stack, whatever the
/// drag was carrying.
#[test]
fn an_engine_comes_off_all_at_once() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;
    let scout = app
        .designer_hulls()
        .iter()
        .position(|h| h.name == "Scout")
        .expect("a Scout");
    app.designer.as_mut().expect("open").selected = scout;
    app.designer_copy();

    let engine_slot = app
        .designer_schematic()
        .iter()
        .position(|s| s.allowed & slot::ENGINE != 0)
        .expect("an engine slot");
    app.designer_drop_on_slot(
        DesignerDrag {
            category: slot::ENGINE,
            item: 1,
            count: 1,
            from_slot: None,
        },
        engine_slot,
    );
    assert!(app.designer_schematic()[engine_slot].fitted.is_some());

    app.designer_drop_on_list(DesignerDrag {
        category: slot::ENGINE,
        item: 1,
        count: 1,
        from_slot: Some(engine_slot),
    });
    assert!(
        app.designer_schematic()[engine_slot].fitted.is_none(),
        "one drag takes the whole engine stack off"
    );
}

/// What Ctrl and Shift do to a drag.
#[test]
fn the_modifier_keys_change_how_many_are_dragged() {
    // From the parts list.
    assert_eq!(App::designer_drag_count(None, 1, false, false), 1);
    assert_eq!(App::designer_drag_count(None, 1, false, true), 4);
    assert_eq!(App::designer_drag_count(None, 1, true, false), 100);

    // Off a slot holding ten.
    assert_eq!(App::designer_drag_count(Some(0), 10, false, false), 1);
    assert_eq!(App::designer_drag_count(Some(0), 10, false, true), 4);
    assert_eq!(App::designer_drag_count(Some(0), 10, true, false), 10);
    // Shift never takes more than is there.
    assert_eq!(App::designer_drag_count(Some(0), 3, false, true), 3);
}

/// A design ships have been built to cannot be edited, and deleting it warns
/// first and then destroys them.
#[test]
fn deleting_a_design_destroys_its_ships() {
    let mut app = a_game();
    app.open_designer();

    // Find a starting design the player actually has ships of.
    let (index, slot) = {
        let game = app.game.as_ref().expect("game");
        let used: Vec<usize> = (0..16)
            .filter(|s| game.designs[0].get(*s).is_some_and(|d| d.hull_id >= 0))
            .collect();
        let flown = used
            .iter()
            .position(|slot| {
                let slot = u8::try_from(*slot).unwrap();
                game.fleets
                    .iter()
                    .filter(|f| f.owner == 0)
                    .any(|f| f.stacks.iter().any(|s| s.design == slot && s.count > 0))
            })
            .expect("the player starts with ships");
        (flown, used[flown])
    };
    app.designer.as_mut().expect("open").selected = index;

    // Ships exist, so Edit is refused but Delete is offered.
    assert!(!app.designer_can_edit());
    assert!(app.designer_can_delete());
    let warning = app
        .designer_delete_warning()
        .expect("it warns before destroying ships");
    assert!(warning.contains("destroyed"), "{warning}");

    let before = app.game.as_ref().expect("game").fleets.len();
    app.designer_delete();

    let game = app.game.as_ref().expect("game");
    assert_eq!(game.designs[0][slot].hull_id, -1, "the slot is free again");
    let slot = u8::try_from(slot).unwrap();
    assert!(
        !game
            .fleets
            .iter()
            .any(|f| f.owner == 0 && f.stacks.iter().any(|s| s.design == slot)),
        "every ship built to it is gone"
    );
    assert!(game.fleets.len() <= before);
    assert!(!app.designer_list().contains(&String::new()));
}

/// Copying one of the player's own designs numbers the copy, and copying is
/// refused once all sixteen slots are full.
#[test]
fn copying_stops_at_sixteen_designs() {
    let mut app = a_game();
    app.open_designer();

    let first = app.designer_list()[0].clone();
    app.designer_copy();
    let copy = app
        .designer
        .as_ref()
        .expect("open")
        .editing
        .as_ref()
        .expect("editing")
        .design
        .name
        .clone();
    assert_eq!(copy, format!("{first} (2)"));
    app.designer_cancel();

    // Fill every slot, then Copy is greyed out — which is the manual's
    // "Stars! will gray the Copy Selected Design button" (p. 9-6).
    {
        let game = app.game.as_mut().expect("game");
        let template = game.designs[0]
            .iter()
            .find(|d| d.hull_id >= 0)
            .cloned()
            .expect("a design");
        game.designs[0].resize(26, template.clone());
        for slot in 0..16 {
            game.designs[0][slot] = template.clone();
        }
    }
    assert_eq!(app.designer_list().len(), 16);
    assert!(!app.designer_can_copy());
}

/// The cost panel shows the miniaturised price, and a starbase's is half what
/// the hull table says.
#[test]
fn the_cost_panel_shows_what_it_would_really_cost() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;

    let rows = app.designer_cost_rows();
    let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Ironium", "Boranium", "Germanium", "Resources", "Mass"]
    );

    let stats = app.designer_stat_rows();
    let labels: Vec<&str> = stats.iter().map(|(l, _)| l.as_str()).collect();
    assert!(labels.contains(&"Max Fuel:"), "{labels:?}");
    assert!(labels.contains(&"Armor:"), "{labels:?}");
    assert!(labels.contains(&"Shields:"), "{labels:?}");

    // The Orbital Fort is listed at 80 resources. It requires no technology at
    // all, so miniaturisation measures a Jack of All Trades' three starting
    // levels and takes 12% off — 80 becomes 70 — and the starbase halving then
    // makes it 35.
    app.designer.as_mut().expect("open").starbase = true;
    app.designer.as_mut().expect("open").selected = 0;
    let fort = app
        .designer_hulls()
        .iter()
        .position(|h| h.name == "Orbital Fort")
        .expect("an Orbital Fort");
    app.designer.as_mut().expect("open").selected = fort;
    assert_eq!(stars_core::components::STARBASE_HULLS[0].resource_cost, 80);
    let rows = app.designer_cost_rows();
    let resources = rows
        .iter()
        .find(|(l, _)| l == "Resources")
        .map(|(_, v)| v.clone())
        .expect("a resource cost");
    assert_eq!(resources, "35");
    // and a starbase has no mass row, because it never moves.
    assert!(!rows.iter().any(|(l, _)| l == "Mass"));
}

/// A foreign design can only be copied if its hull is one the player could
/// build for themselves.
#[test]
fn a_foreign_design_needs_a_hull_you_can_build() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Enemy;

    let enemy = app.designer_enemy_designs();
    assert!(!enemy.is_empty(), "the opponent has designs of its own");
    // Each entry names its owner, so the list is not just design names.
    let list = app.designer_list();
    assert_eq!(list.len(), enemy.len());
    let opponent = app.player_name(enemy[0].0);
    assert!(list[0].starts_with(&opponent), "{:?}", list[0]);

    // Copying is offered only for hulls this player can build. Whatever the
    // answer, it must agree with the hull list.
    let buildable: Vec<&str> = app.designer_hulls().iter().map(|h| h.name).collect();
    for (index, _) in enemy.iter().enumerate() {
        app.designer.as_mut().expect("open").selected = index;
        let design = app.designer_subject().expect("a design");
        let hull = App::designer_hull(&design).expect("a hull");
        assert_eq!(
            app.designer_can_copy(),
            buildable.contains(&hull.name),
            "{} on a {}",
            design.name,
            hull.name
        );
    }
}

/// A hull owns four pictures, and the two arrows walk those four and no
/// others.
///
/// `BuildDlg` splits the index into a base and a variant, steps the variant
/// with `(iCur + 4 ± 1) & 3`, and puts the base back — so the choice wraps
/// inside the hull's own group and can never land on another hull's ship.
#[test]
fn the_arrows_spin_between_a_hull_s_own_four_pictures() {
    use stars_formats::resources::art;

    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;
    let scout = app
        .designer_hulls()
        .iter()
        .position(|h| h.name == "Scout")
        .expect("a Scout");
    app.designer.as_mut().expect("open").selected = scout;
    let base = u16::from(
        app.designer_hulls()[scout]
            .picture
            .try_into()
            .unwrap_or(u8::MAX),
    );
    assert_eq!(base % art::PICTURES_PER_HULL, 0, "a hull starts a group");

    // A fresh design starts on the first of its hull's four, not on zero —
    // zero would be the Small Freighter whatever was copied.
    app.designer_copy();
    let picture = |app: &App| {
        u16::from(
            app.designer
                .as_ref()
                .expect("open")
                .editing
                .as_ref()
                .expect("editing")
                .design
                .picture,
        )
    };
    assert_eq!(picture(&app), base);

    // Four steps forward comes back to where it started, and every one stays
    // inside the group.
    let mut seen = Vec::new();
    for _ in 0..4 {
        seen.push(picture(&app));
        app.designer_next_picture(true);
    }
    assert_eq!(seen, vec![base, base + 1, base + 2, base + 3]);
    assert_eq!(picture(&app), base, "and round again");

    // Backwards wraps the other way rather than running off the bottom.
    app.designer_next_picture(false);
    assert_eq!(picture(&app), base + 3);

    // Whatever the arrows do, the picture is always one of this hull's four,
    // and always in the same column of the same sheet.
    let column = art::ship(base, art::ShipSize::Large);
    for _ in 0..9 {
        app.designer_next_picture(true);
        let cell = art::ship(picture(&app), art::ShipSize::Large);
        assert_eq!(cell.resource, column.resource);
        assert_eq!(cell.x, column.x);
    }
}

/// The designer is laid out from its own template — resource 92,
/// `Ship & Starbase Designer`, 351 by 250 dialog units and fifteen controls.
#[test]
fn the_designer_template_is_the_games_own() {
    use stars_ui::dialog::{Class, DESIGNER, DESIGNER_BROWSER_ONLY, DESIGNER_CLOSE};

    assert_eq!(DESIGNER.caption, "Ship & Starbase Designer");
    assert_eq!(DESIGNER.size, (351, 250));
    assert_eq!(DESIGNER.controls.len(), 15);

    // The Design radios are **plural** in the resource, which is easy to get
    // wrong from the manual's prose.
    assert_eq!(DESIGNER.control(0x810).expect("Ships").text, "Ships");
    assert_eq!(
        DESIGNER.control(0x811).expect("Starbases").text,
        "Starbases"
    );
    // And the three buttons all say "Selected Design".
    assert_eq!(
        DESIGNER.control(0x817).expect("Delete").label(),
        "Delete Selected Design"
    );
    assert_eq!(
        DESIGNER.control(0x818).expect("Edit").label(),
        "Edit Selected Design"
    );

    // `mdBuild = wParam - 0x812`, so the four View radios run 0x812..0x815 in
    // the order the enum has them.
    for (index, title) in [
        "Existing Designs",
        "Available Hull Types",
        "Enemy Hulls",
        "Components",
    ]
    .into_iter()
    .enumerate()
    {
        #[allow(clippy::cast_possible_truncation)]
        let id = 0x812 + index as u16;
        assert_eq!(DESIGNER.control(id).expect("a View radio").text, title);
    }

    // `ShowMainControls` swaps nine controls between the two faces, and the
    // second button is `Done` in the browser and `Cancel` in the editor.
    assert_eq!(DESIGNER_BROWSER_ONLY.len(), 9);
    assert!(DESIGNER_BROWSER_ONLY.contains(&0x816));
    assert!(
        !DESIGNER_BROWSER_ONLY.contains(&0x80c),
        "the parts list stays"
    );
    assert_eq!(DESIGNER_CLOSE, ("Done", "Cancel"));

    assert_eq!(
        DESIGNER.control(0x80c).expect("the parts list").class,
        Class::ListBox
    );
    assert_eq!(
        DESIGNER.control(0x81b).expect("the name field").class,
        Class::Edit
    );
}

/// The parts list runs past the bottom of the dialog it is in, in the resource
/// as shipped.
#[test]
fn the_parts_list_overruns_the_dialog() {
    use stars_ui::dialog::DESIGNER;

    let list = DESIGNER.control(0x80c).expect("the parts list").at;
    assert_eq!(list, (114, 90, 135, 170));
    assert!(
        list.1 + list.3 > DESIGNER.size.1,
        "90 + 170 is ten units past a height of 250"
    );
    assert_eq!(list.1 + list.3 - DESIGNER.size.1, 10);
}

/// Dragging a part off the palette and letting go over a slot fits it —
/// through egui, as a hand does it, not through `designer_drop_on_slot`.
#[test]
fn a_part_dragged_from_the_palette_lands_in_the_slot() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;
    let scout = app
        .designer_hulls()
        .iter()
        .position(|h| h.name == "Scout")
        .expect("a Scout");
    app.designer.as_mut().expect("open").selected = scout;
    app.designer_copy();
    let engine_slot = app
        .designer_schematic()
        .iter()
        .position(|s| s.allowed & slot::ENGINE != 0)
        .expect("an engine slot");

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    if let Ok(bytes) = std::fs::read(root.join("stars.2.7j.exe")) {
        app.load_art(bytes, "the test's copy")
            .expect("the pictures load");
    }
    let mut shell = stars_ui::autopilot::Shell::new(app);
    shell.frame();
    shell.frame();
    let row = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == "Quick Jump 5")
        .map(|w| w.rect)
        .expect("the Quick Jump 5 row is drawn");
    let target = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == format!("slot {engine_slot}"))
        .map(|w| w.rect)
        .expect("the engine slot is drawn");
    shell.drag(row.center(), target.center());
    shell.frame();

    let fitted = shell.app.designer_schematic();
    let (name, _) = fitted[engine_slot]
        .fitted
        .clone()
        .expect("the engine went in by the drag");
    assert_eq!(name, "Quick Jump 5");
}

/// The same drag, the way a mouse delivers it: the press and the first
/// movement in one frame, then a pixel a frame.
#[test]
fn a_part_dragged_slowly_lands_in_the_slot() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;
    let scout = app
        .designer_hulls()
        .iter()
        .position(|h| h.name == "Scout")
        .expect("a Scout");
    app.designer.as_mut().expect("open").selected = scout;
    app.designer_copy();
    let engine_slot = app
        .designer_schematic()
        .iter()
        .position(|s| s.allowed & slot::ENGINE != 0)
        .expect("an engine slot");
    let mut shell = stars_ui::autopilot::Shell::new(app);
    shell.frame();
    shell.frame();
    let row = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == "Quick Jump 5")
        .map(|w| w.rect)
        .expect("row");
    let target = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == format!("slot {engine_slot}"))
        .map(|w| w.rect)
        .expect("slot");
    let from = row.center();
    let to = target.center();
    shell.push_event(egui::Event::PointerMoved(from));
    shell.frame();
    shell.push_event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    shell.push_event(egui::Event::PointerMoved(from + egui::vec2(3.0, 0.0)));
    shell.frame();
    let steps = 40;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        shell.push_event(egui::Event::PointerMoved(from + (to - from) * t));
        shell.frame();
    }
    shell.push_event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    shell.frame();
    shell.frame();
    let fitted = shell.app.designer_schematic();
    assert!(
        fitted[engine_slot].fitted.is_some(),
        "the engine went in by the slow drag"
    );
}

/// A copied Scout with its engine in, in a shell, with the engine slot's
/// index and the rectangles of its row and slot.
fn fitted_scout_in_a_shell() -> (stars_ui::autopilot::Shell, usize, egui::Rect) {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;
    let scout = app
        .designer_hulls()
        .iter()
        .position(|h| h.name == "Scout")
        .expect("a Scout");
    app.designer.as_mut().expect("open").selected = scout;
    app.designer_copy();
    let engine_slot = app
        .designer_schematic()
        .iter()
        .position(|s| s.allowed & slot::ENGINE != 0)
        .expect("an engine slot");
    assert!(app.designer_drop_on_slot(
        DesignerDrag {
            category: slot::ENGINE,
            item: 1,
            count: 1,
            from_slot: None,
        },
        engine_slot,
    ));
    let mut shell = stars_ui::autopilot::Shell::new(app);
    shell.frame();
    shell.frame();
    let target = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == format!("slot {engine_slot}"))
        .map(|w| w.rect)
        .expect("the engine slot is drawn");
    (shell, engine_slot, target)
}

/// Delete (or Backspace) with a slot selected empties it — a key the
/// original does not have, for a design to be cleared without a drag.
#[test]
fn delete_empties_the_selected_slot() {
    let (mut shell, engine_slot, target) = fitted_scout_in_a_shell();
    shell.click_at(target.center());
    assert_eq!(
        shell.app.designer.as_ref().expect("open").selected_slot,
        Some(engine_slot)
    );
    shell.push_event(egui::Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    shell.frame();
    shell.frame();
    assert!(
        shell.app.designer_schematic()[engine_slot].fitted.is_none(),
        "Delete emptied the slot"
    );
}

/// A stack dragged off its slot and let go over the left half of the
/// dialog — not on the parts list itself — comes off the design, as
/// `IDropPart` has it.
#[test]
fn a_stack_let_go_on_the_left_comes_off() {
    let (mut shell, engine_slot, target) = fitted_scout_in_a_shell();
    // The filter dropdown's row, left of centre and above the list.
    let dropdown = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == "Quick Jump 5")
        .map(|w| w.rect)
        .expect("the list is drawn");
    let to = egui::pos2(dropdown.left() - 4.0, dropdown.top() - 30.0);
    shell.drag(target.center(), to);
    shell.frame();
    assert!(
        shell.app.designer_schematic()[engine_slot].fitted.is_none(),
        "the engine came off"
    );
}

/// The keys are read at the drop, not the pick-up: a part dragged with
/// nothing held and Ctrl pressed before letting go fills the slot.
#[test]
fn the_keys_are_read_at_the_drop() {
    let mut app = a_game();
    app.open_designer();
    app.designer.as_mut().expect("open").view = DesignView::Hulls;
    // The first hull the player can build with an armour slot that holds
    // several — a new game starts with the Destroyer at hand.
    let hulls = app.designer_hulls();
    let (hull, weapons) = (0..hulls.len())
        .find_map(|h| {
            app.designer.as_mut().expect("open").selected = h;
            app.designer_schematic()
                .iter()
                .position(|s| s.allowed & slot::ARMOR != 0 && s.capacity > 1)
                .map(|w| (h, w))
        })
        .expect("a hull with an armour slot that holds several");
    app.designer.as_mut().expect("open").selected = hull;
    app.designer_copy();
    let schematic = app.designer_schematic();
    let capacity = schematic[weapons].capacity;
    let mut shell = stars_ui::autopilot::Shell::new(app);
    shell.frame();
    shell.frame();
    let row = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == "Tritanium")
        .map(|w| w.rect)
        .expect("a Tritanium row");
    let target = shell
        .app
        .drawn
        .iter()
        .find(|w| w.label == format!("slot {weapons}"))
        .map(|w| w.rect)
        .expect("the weapon slot");
    let from = row.center();
    let to = target.center();
    shell.push_event(egui::Event::PointerMoved(from));
    shell.frame();
    shell.push_event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    shell.frame();
    for step in 1..=6 {
        let t = step as f32 / 6.0;
        shell.push_event(egui::Event::PointerMoved(from + (to - from) * t));
        shell.frame();
    }
    // Ctrl goes down only now.
    shell.modifiers = egui::Modifiers::COMMAND;
    shell.frame();
    shell.push_event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::COMMAND,
    });
    shell.frame();
    shell.modifiers = egui::Modifiers::NONE;
    shell.frame();
    let (_, count) = shell.app.designer_schematic()[weapons]
        .fitted
        .clone()
        .expect("the armour went in");
    assert_eq!(count, capacity, "Ctrl at the drop filled the slot");
}
