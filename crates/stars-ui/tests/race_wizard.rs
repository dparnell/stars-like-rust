//! The Custom Race Wizard — File (Custom Race Wizard).
//!
//! The wizard's job is the advantage-points budget and a `.rN` file at the end
//! of it, so that is what these check: that editing moves the counter the way
//! `CAdvantagePoints` says it should, that the pages hold their ends, and that
//! what comes out is a race file the reader gets the same race back from.
//!
//! See `docs/ui/race-wizard.md`.

use stars_core::race::{lrt, Prt, RaceStat};
use stars_ui::App;

/// The wizard, open on its first page.
fn wizard() -> App {
    let mut app = App::new();
    app.open_race_wizard();
    app
}

#[test]
fn it_opens_on_the_first_predefined_race() {
    let app = wizard();
    let open = app.race_wizard.as_ref().expect("the wizard is open");
    assert_eq!(open.name, "Humanoid");
    assert_eq!(open.plural, "Humanoids");
    assert_eq!(open.page, 0);
    // The stock Humanoid prices at 25 — the same figure the turn-0 fixture's
    // human player is stocked from, see `stars-core/tests/new_game.rs`.
    assert_eq!(app.race_wizard_points(), 25);
    assert!(app.race_wizard_is_legal());
}

#[test]
fn the_caption_counts_the_steps() {
    let mut app = wizard();
    assert_eq!(app.race_wizard_title(), "Custom Race Wizard - Step 1 of 6");
    app.race_wizard_page(true);
    assert_eq!(app.race_wizard_title(), "Custom Race Wizard - Step 2 of 6");
}

#[test]
fn the_pages_stop_at_the_ends() {
    let mut app = wizard();
    app.race_wizard_page(false);
    assert_eq!(app.race_wizard.as_ref().expect("open").page, 0);
    for _ in 0..10 {
        app.race_wizard_page(true);
    }
    assert_eq!(
        app.race_wizard.as_ref().expect("open").page,
        stars_ui::RACE_WIZARD_PAGES - 1
    );
}

#[test]
fn each_of_the_seven_predefined_races_can_be_loaded() {
    let mut app = wizard();
    for (index, preset) in stars_core::presets::ALL.iter().enumerate() {
        app.race_wizard_load_preset(index);
        let open = app.race_wizard.as_ref().expect("open");
        assert_eq!(open.name, preset.name);
        assert_eq!(open.plural, preset.plural);
        assert_eq!(open.emblem, preset.emblem);
        assert_eq!(open.race, preset.race);
    }
}

#[test]
fn the_buttons_check_the_race_that_is_loaded() {
    let mut app = wizard();
    // A freshly opened wizard is on the Humanoid, so its button is the checked
    // one and `Custom` is not.
    assert_eq!(app.race_wizard_selected_preset(), Some(0));
    app.race_wizard_load_preset(3);
    assert_eq!(app.race_wizard_selected_preset(), Some(3));
    // Change anything and it is a custom race, which is what `Custom` means.
    app.race_wizard_set_growth(19);
    assert_eq!(app.race_wizard_selected_preset(), None);
}

#[test]
fn a_name_the_player_typed_survives_the_buttons() {
    let mut app = wizard();
    if let Some(wizard) = app.race_wizard.as_mut() {
        wizard.name = "Testoid".to_string();
        wizard.plural = "Testoids".to_string();
    }
    app.race_wizard_load_preset(1);
    let open = app.race_wizard.as_ref().expect("open");
    assert_eq!(open.name, "Testoid", "a typed name is the player's to keep");
    assert_eq!(open.race, stars_core::presets::ALL[1].race);

    // A name still one of the seven is replaced along with the race — and
    // once a name has been typed it stays typed, whichever button is pressed.
    let mut fresh = wizard();
    fresh.race_wizard_load_preset(2);
    let open = fresh.race_wizard.as_ref().expect("open");
    assert_eq!(open.name, "Insectoid");
    assert_eq!(open.plural, "Insectoids");
}

#[test]
fn the_random_race_cannot_be_edited_page_by_page() {
    let mut app = wizard();
    assert!(app.race_wizard_can_go_on());
    app.race_wizard_load_preset(stars_core::presets::ALL.len() - 1);
    assert_eq!(app.race_wizard.as_ref().expect("open").name, "Random");
    assert!(!app.race_wizard_can_go_on());
}

#[test]
fn a_cheaper_race_is_worth_more_points() {
    let mut app = wizard();
    let before = app.race_wizard_points();
    // A worse factory build cost is a cheaper race, so the budget grows.
    app.race_wizard_set_stat(RaceStat::FactBuild, 15);
    assert!(
        app.race_wizard_points() > before,
        "a worse economy should refund points, went from {before} to {}",
        app.race_wizard_points()
    );
}

#[test]
fn spending_past_the_budget_is_illegal_and_says_so() {
    let mut app = wizard();
    // Wide habitability on all three axes, an expensive primary trait and
    // every research field at half price: well past the budget.
    for axis in 0..3 {
        app.race_wizard_set_env(axis, 0, 0);
        app.race_wizard_set_env(axis, 2, 100);
    }
    app.race_wizard_set_growth(20);
    for field in 0..6 {
        app.race_wizard_set_research(field, 2);
    }
    assert!(app.race_wizard_points() < 0);
    assert!(!app.race_wizard_is_legal());
    let refusal = app.race_wizard_refusal().expect("a refusal");
    assert!(refusal.starts_with("Your advantage points are currently in the hole by"));
}

#[test]
fn immunity_is_a_negative_upper_bound_and_comes_back_as_a_range() {
    let mut app = wizard();
    app.race_wizard_set_immune(1, true);
    assert!(app.race_wizard.as_ref().expect("open").race.is_immune(1));
    app.race_wizard_set_immune(1, false);
    let race = &app.race_wizard.as_ref().expect("open").race;
    assert!(!race.is_immune(1));
    assert!(race.env_min[1] <= race.env_center[1] && race.env_center[1] <= race.env_max[1]);
}

#[test]
fn the_bounds_of_an_axis_stay_in_order() {
    let mut app = wizard();
    // Drag the low bound past the ideal and the high bound below it.
    app.race_wizard_set_env(0, 0, 90);
    app.race_wizard_set_env(0, 2, 10);
    let race = &app.race_wizard.as_ref().expect("open").race;
    assert!(race.env_min[0] <= race.env_center[0]);
    assert!(race.env_center[0] <= race.env_max[0]);
}

#[test]
fn the_traits_are_the_ones_the_pages_offer() {
    let mut app = wizard();
    app.race_wizard_set_prt(Prt::Wm);
    assert_eq!(
        app.race_wizard.as_ref().expect("open").race.prt(),
        Some(Prt::Wm)
    );
    app.race_wizard_toggle_lrt(lrt::IFE);
    assert!(app
        .race_wizard
        .as_ref()
        .expect("open")
        .race
        .has_lrt(lrt::IFE));
    app.race_wizard_toggle_lrt(lrt::IFE);
    assert!(!app
        .race_wizard
        .as_ref()
        .expect("open")
        .race
        .has_lrt(lrt::IFE));
    // The two that live far up the same word are set, not toggled.
    app.race_wizard_set_flag(lrt::TECH3, true);
    app.race_wizard_set_flag(lrt::CHEAP_FACT, true);
    let race = &app.race_wizard.as_ref().expect("open").race;
    assert!(race.has_lrt(lrt::TECH3) && race.has_lrt(lrt::CHEAP_FACT));
}

#[test]
fn what_it_writes_reads_back_as_the_same_race() {
    let mut app = wizard();
    app.race_wizard_load_preset(2); // Insectoid: immune to gravity, WM, LRTs.
    if let Some(wizard) = app.race_wizard.as_mut() {
        wizard.name = "Testoid".to_string();
        wizard.plural = "Testoids".to_string();
    }
    app.race_wizard_set_emblem(7);

    let bytes = app
        .race_wizard_file()
        .expect("the wizard is open")
        .expect("writes");
    let file = stars_formats::StarsFile::decode(&bytes).expect("decodes");
    let record = stars_formats::RaceRecord::from_file(&file).expect("a race record");

    assert_eq!(file.header.file_type, stars_formats::FileType::Race);
    assert_eq!(record.singular_name, "Testoid");
    assert_eq!(record.plural_name, "Testoids");
    assert_eq!(record.prt, stars_formats::Prt::WM);
    // Immunity survives as the three `0xFF`s the reader gives back as `None`.
    assert_eq!(record.gravity.center, None);

    // Everything the file carries comes back. The immune axis is the one
    // thing that does not come back byte for byte: it is stored as three
    // `0xFF`s, and the reader rebuilds it as "no range, negative bound",
    // which is the same race by every question the simulation asks.
    let race = stars_core::load::race_from_record(&record);
    let designed = &app.race_wizard.as_ref().expect("open").race;
    assert_eq!(race.attrs, designed.attrs);
    assert_eq!(race.lrt_bits, designed.lrt_bits);
    assert_eq!(race.pct_ideal_growth, designed.pct_ideal_growth);
    for axis in 0..3 {
        assert_eq!(race.is_immune(axis), designed.is_immune(axis));
        if !designed.is_immune(axis) {
            assert_eq!(race.env_center[axis], designed.env_center[axis]);
            assert_eq!(race.env_min[axis], designed.env_min[axis]);
            assert_eq!(race.env_max[axis], designed.env_max[axis]);
        }
    }
}

#[test]
fn cancelling_throws_the_race_away() {
    let mut app = wizard();
    app.race_wizard_set_prt(Prt::Ar);
    app.close_race_wizard();
    assert!(app.race_wizard.is_none());
    assert!(app.race_wizard_file().is_none());
}
