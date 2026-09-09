//! The Custom Race Wizard.
//!
//! `RaceCreationWizard` (`10e0:0000`), File (Custom Race Wizard). The same six
//! pages the viewer shows — `IDD_RACE_WIZARD_1` (146) through `_6` (151) — with
//! the settings editable and the **advantage points** counted as they change.
//!
//! That counter is what the wizard is for. Every choice spends from one budget
//! or refunds into it, and a race is only legal while what is left is not
//! negative; `DrawRaceAdvantagePoints` (`10e0:55c6`) draws the figure in red
//! when it is.
//!
//! See `docs/ui/race-wizard.md`.

use crate::App;
use stars_core::race::{lrt, Prt, RaceStat};

/// Draw the wizard.
///
/// `finish` comes back true when the Finish button was pressed on a legal
/// race: the caller writes the file, since this crate does no I/O.
pub fn view(app: &mut App, ui: &mut egui::Ui) -> bool {
    let Some(wizard) = app.race_wizard.as_ref() else {
        return false;
    };
    let page = wizard.page;
    let template = crate::dialog::RACE_WIZARD[page.min(5)];
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);

    // The caption counts the steps: `"Custom Race Wizard - Step %d of 6"`.
    let painter = ui.painter_at(rect);
    let font = egui::TextStyle::Small.resolve(ui.style());
    painter.text(
        rect.min + egui::vec2(6.0, 2.0),
        egui::Align2::LEFT_TOP,
        crate::dialog::WIZARD_CAPTION.replace("{}", &(page + 1).to_string()),
        font.clone(),
        ui.visuals().text_color(),
    );

    // The points counter, which the original paints on every page — two lines,
    // `"Advantage"` over `"Points Left"` (strings `0x052f`, `0x0530`), with the
    // figure in red when the race is over budget.
    let points = app.race_wizard_points();
    painter.text(
        egui::pos2(rect.right() - 6.0, rect.top() + 2.0),
        egui::Align2::RIGHT_TOP,
        format!("{points}  Advantage Points Left"),
        font,
        if points < 0 {
            egui::Color32::from_rgb(0xff, 0x6b, 0x6b)
        } else {
            ui.visuals().text_color()
        },
    );

    // The page's own body, between the caption and the footer.
    let body = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 4.0, rect.top() + 16.0),
        egui::pos2(
            rect.right() - 4.0,
            at(crate::dialog::WIZARD_FOOTER[0]).top() - 4.0,
        ),
    );
    {
        let mut child = ui.child_ui(body, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(body);
        match page {
            0 => names(app, &mut child),
            1 => habitability(app, &mut child),
            2 => economy(app, &mut child),
            3 => primary_trait(app, &mut child),
            4 => lesser_traits(app, &mut child),
            _ => research(app, &mut child),
        }
    }

    // The five buttons every page carries, each where its own template puts
    // it: Help, Cancel, `< Back`, `Next >`, Finish. Help is the one this
    // project has nothing to put behind it, and the first page's `< Back` and
    // the last's `Next >` are disabled in the templates themselves.
    let mut finish = false;
    crate::views::dialog_button(ui, at(0x76), &caption(0x76), false);
    if crate::views::dialog_button(ui, at(0x2), &caption(0x2), true).clicked() {
        app.close_race_wizard();
    }
    if crate::views::dialog_button(ui, at(0x42e), &caption(0x42e), page > 0).clicked() {
        app.race_wizard_page(false);
    }
    // `Next >` is also disabled while `Random` is the chosen race, as the
    // original disables it: there is nothing to edit on the other pages.
    let can_go_on = page + 1 < crate::dialog::RACE_WIZARD.len() && app.race_wizard_can_go_on();
    if crate::views::dialog_button(ui, at(0x42f), &caption(0x42f), can_go_on).clicked() {
        app.race_wizard_page(true);
    }
    // The original accepts the press and puts up a message box; the refusal is
    // the same either way, and it is worth reading before the button is
    // pressed rather than after.
    let refusal = app.race_wizard_refusal();
    let button = crate::views::dialog_button(ui, at(0x430), &caption(0x430), refusal.is_none());
    if let Some(text) = &refusal {
        button.on_hover_text(text);
    } else if button.clicked() {
        finish = true;
    }
    finish
}

/// What the leftover advantage points can be spent on (strings `0x0106` to
/// `0x010a`), in the order the combo lists them — which is the order
/// `rsUseLeftover` stores.
const LEFTOVER: [&str; 5] = [
    "Surface minerals",
    "Mineral concentrations",
    "Mines",
    "Factories",
    "Defenses",
];

/// Page 1: what the race is called, and which race to start from.
fn names(app: &mut App, ui: &mut egui::Ui) {
    let Some(wizard) = app.race_wizard.as_mut() else {
        return;
    };
    egui::Grid::new("wiz-names")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Race Name:");
            ui.add(egui::TextEdit::singleline(&mut wizard.name).desired_width(180.0));
            ui.end_row();
            ui.label("Plural Race Name:");
            ui.add(egui::TextEdit::singleline(&mut wizard.plural).desired_width(180.0));
            ui.end_row();
            ui.label("Password:");
            ui.add(
                egui::TextEdit::singleline(&mut wizard.password)
                    .password(true)
                    .desired_width(120.0),
            );
            ui.end_row();
        });

    // The seven predefined races, as two columns of four with `Custom` in the
    // eighth place — the template's own arrangement.
    let emblem = wizard.emblem;
    ui.add_space(4.0);
    let mut load = None;
    let mut step: Option<i16> = None;
    // Radio buttons rather than commands: the one that is checked is whichever
    // predefined race this one still matches, and `Custom` when none does.
    let chosen = app.race_wizard_selected_preset();
    ui.horizontal_top(|ui| {
        for column in 0..2 {
            ui.vertical(|ui| {
                for row in 0..4 {
                    let index = column * 4 + row;
                    match stars_core::presets::preset(index) {
                        Some(preset) => {
                            if ui.radio(chosen == Some(index), preset.name).clicked() {
                                load = Some(index);
                            }
                        }
                        // The eighth button is `Custom`, which is what a race
                        // that matches none of the seven is. Pressing it keeps
                        // whatever the wizard is holding.
                        None => {
                            let _ = ui.radio(chosen.is_none(), "Custom");
                        }
                    }
                }
            });
        }
        // The emblem is not on this page in the original — it is picked in the
        // New Game dialog — but it is part of what a race file carries, so it
        // is shown here with the race it belongs to.
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Emblem").small());
            if let Some(cell) = stars_formats::resources::art::emblem(
                emblem,
                stars_formats::resources::art::EmblemSize::Large,
            ) {
                let mut art = app.art.take();
                crate::art::draw_with(art.as_mut(), ui, cell, 32.0);
                app.art = art;
            }
            ui.horizontal(|ui| {
                if ui.small_button("<").clicked() {
                    step = Some(-1);
                }
                if ui.small_button(">").clicked() {
                    step = Some(1);
                }
            });
        });
    });
    if let Some(delta) = step {
        app.race_wizard_set_emblem((i16::from(emblem) + delta).rem_euclid(32) as u8);
    }
    if let Some(index) = load {
        app.race_wizard_load_preset(index);
    }

    ui.add_space(4.0);
    ui.label("Spend up to 50 leftover advantage points on:");
    let mut choice = app
        .race_wizard
        .as_ref()
        .map_or(0, |w| w.race.stat(RaceStat::UseLeftover))
        .clamp(0, 4) as usize;
    let before = choice;
    egui::ComboBox::from_id_source("wiz-leftover")
        .selected_text(LEFTOVER[choice])
        .show_ui(ui, |ui| {
            for (index, label) in LEFTOVER.iter().enumerate() {
                ui.selectable_value(&mut choice, index, *label);
            }
        });
    if choice != before {
        app.race_wizard_set_leftover(choice as i16);
    }
}

/// Page 2: where the race can live, and how fast it breeds.
///
/// The three sliders are painted rather than laid out, so the template holds
/// only the `Immune to …` checkboxes — the captions here are its own. The
/// bounds are clicks, `0..=100`, shown alongside in the units the game reads
/// them out in.
fn habitability(app: &mut App, ui: &mut egui::Ui) {
    let Some(wizard) = app.race_wizard.as_ref() else {
        return;
    };
    let mut edits: Vec<(usize, usize, i8)> = Vec::new();
    let mut immunity: Vec<(usize, bool)> = Vec::new();
    let mut growth = wizard.race.pct_ideal_growth;

    for (axis, caption) in [
        "Immune to Gravity",
        "Immune to Temperature",
        "Immune to Radiation",
    ]
    .iter()
    .enumerate()
    {
        let immune = wizard.race.is_immune(axis);
        let mut on = immune;
        if ui.checkbox(&mut on, *caption).changed() {
            immunity.push((axis, on));
        }
        if immune {
            continue;
        }
        ui.horizontal(|ui| {
            for (which, label) in ["Low", "Ideal", "High"].iter().enumerate() {
                let mut value = match which {
                    0 => wizard.race.env_min[axis],
                    1 => wizard.race.env_center[axis],
                    _ => wizard.race.env_max[axis],
                };
                ui.label(egui::RichText::new(*label).small());
                if ui
                    .add(egui::DragValue::new(&mut value).range(0..=100))
                    .changed()
                {
                    edits.push((axis, which, value));
                }
                ui.label(
                    egui::RichText::new(crate::app::env_text(axis, value))
                        .small()
                        .weak(),
                );
            }
        });
        ui.add_space(2.0);
    }

    ui.horizontal(|ui| {
        // String `0x00f5`.
        ui.label("Maximum colonist growth rate per year:");
        if ui
            .add(egui::DragValue::new(&mut growth).range(1..=20).suffix("%"))
            .changed()
        {
            edits.push((usize::MAX, 0, growth));
        }
    });

    for (axis, on) in immunity {
        app.race_wizard_set_immune(axis, on);
    }
    for (axis, which, value) in edits {
        if axis == usize::MAX {
            app.race_wizard_set_growth(value);
        } else {
            app.race_wizard_set_env(axis, which, value);
        }
    }
}

/// Page 3: what the race builds with.
///
/// The seven rows are painted rather than laid out, so the template says
/// nothing about them; the sentences they are painted from are strings
/// `0x00f6` to `0x0103`, each split either side of the number it frames.
fn economy(app: &mut App, ui: &mut egui::Ui) {
    const ROWS: [(RaceStat, &str, &str); 7] = [
        (
            RaceStat::ResGen,
            "One resource is generated each year for every",
            "colonists.",
        ),
        (
            RaceStat::FactProd,
            "Every 10 factories produce",
            "resources each year.",
        ),
        (
            RaceStat::FactBuild,
            "Factories require",
            "resources to build.",
        ),
        (
            RaceStat::FactOperate,
            "Every 10,000 colonists may operate up to",
            "factories.",
        ),
        (
            RaceStat::MineProd,
            "Every 10 mines produce up to",
            "of each mineral every year.",
        ),
        (RaceStat::MineBuild, "Mines require", "resources to build."),
        (
            RaceStat::MineOperate,
            "Every 10,000 colonists may operate up to",
            "mines.",
        ),
    ];
    let Some(wizard) = app.race_wizard.as_ref() else {
        return;
    };
    let mut edits: Vec<(RaceStat, i16)> = Vec::new();
    let mut cheap = wizard.race.has_lrt(lrt::CHEAP_FACT);

    for (stat, before, after) in ROWS {
        let mut value = wizard.race.stat(stat);
        ui.horizontal(|ui| {
            ui.label(before);
            if ui
                .add(egui::DragValue::new(&mut value).range(1..=100))
                .changed()
            {
                edits.push((stat, value));
            }
            ui.label(after);
        });
    }
    ui.add_space(4.0);
    let toggled = ui
        .checkbox(&mut cheap, "Factories cost 1kT less of Germanium to build")
        .changed();

    for (stat, value) in edits {
        app.race_wizard_set_stat(stat, value);
    }
    if toggled {
        app.race_wizard_set_flag(lrt::CHEAP_FACT, cheap);
    }
}

/// Page 4: the one primary trait.
fn primary_trait(app: &mut App, ui: &mut egui::Ui) {
    let Some(wizard) = app.race_wizard.as_ref() else {
        return;
    };
    let current = wizard.race.prt();
    let mut chosen = None;
    for prt in Prt::ALL {
        if ui
            .selectable_label(current == Some(prt), prt.name())
            .clicked()
        {
            chosen = Some(prt);
        }
    }
    if let Some(prt) = chosen {
        app.race_wizard_set_prt(prt);
    }
}

/// Page 5: the fourteen lesser traits.
fn lesser_traits(app: &mut App, ui: &mut egui::Ui) {
    let Some(wizard) = app.race_wizard.as_ref() else {
        return;
    };
    let mut toggled = None;
    for bit in lrt::ALL {
        let mut on = wizard.race.has_lrt(bit);
        if ui
            .checkbox(&mut on, lrt::name(bit).unwrap_or("?"))
            .changed()
        {
            toggled = Some(bit);
        }
    }
    if let Some(bit) = toggled {
        app.race_wizard_toggle_lrt(bit);
    }
}

/// Page 6: what research costs, field by field.
fn research(app: &mut App, ui: &mut egui::Ui) {
    use stars_core::research::TechField;
    let Some(wizard) = app.race_wizard.as_ref() else {
        return;
    };
    let mut edits: Vec<(usize, i16)> = Vec::new();
    let mut tech3 = wizard.race.has_lrt(lrt::TECH3);

    egui::Grid::new("wiz-research")
        .num_columns(4)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            for (field, name) in TechField::ALL.iter().enumerate() {
                let setting = wizard.race.attrs[RaceStat::TechBonus1 as usize + field];
                ui.label(egui::RichText::new(name.name()).small());
                // The wizard's own three, in its own words.
                for (value, label) in [
                    (0i16, "Costs 75% extra"),
                    (1, "Costs standard amount"),
                    (2, "Costs 50% less"),
                ] {
                    if ui
                        .selectable_label(setting == value, egui::RichText::new(label).small())
                        .clicked()
                    {
                        edits.push((field, value));
                    }
                }
                ui.end_row();
            }
        });
    ui.add_space(4.0);
    let toggled = ui
        .checkbox(
            &mut tech3,
            "All 'Costs 75% extra' research fields start at Tech 3",
        )
        .changed();

    for (field, value) in edits {
        app.race_wizard_set_research(field, value);
    }
    if toggled {
        app.race_wizard_set_flag(lrt::TECH3, tech3);
    }
}
