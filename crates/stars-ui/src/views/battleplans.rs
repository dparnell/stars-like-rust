//! The Battle Plans dialog.
//!
//! `BattlePlansDlg`, `IDD_BATTLE_PLANS` (2013) — Commands (Battle Plans...),
//! **F6**. One plan at a time: the list picks it, and the four combos and the
//! checkbox below are that plan.
//!
//! ```text
//!   Plan:              [ Default            v ]      [ Rename... ]
//!   Primary Target:    [ Armed Ships        v ]
//!   Secondary Target:  [ Any                v ]
//!   Tactic:            [ Maximize damage r. v ]
//!   Attack Who:        [ Neutrals & Enemies v ]      [x] Dump Cargo
//!   [ Delete ]  [ Copy ]  [ Close ]  [ Help ]
//! ```
//!
//! See `docs/ui/battle-plans.md`.

use crate::App;
use stars_core::battle::{Tactic, TargetClass};

/// Draw the dialog.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(dialog) = app.battle_plans.as_ref() else {
        return;
    };
    let slot = dialog.selected;
    let renaming = dialog.rename.is_some();
    let confirming = dialog.confirm_delete;
    let Some(plan) = app.selected_battle_plan().cloned() else {
        ui.label("This player has no battle plans.");
        return;
    };
    let names: Vec<String> = app
        .battle_plan_list()
        .iter()
        .map(|p| p.name.clone())
        .collect();
    // Plan 0 is the one every fleet falls back on: the original disables both
    // buttons that would take it away.
    let editable_name = slot > 0;

    let mut select: Option<usize> = None;
    let mut command: Option<Command> = None;

    egui::Grid::new("battle-plan-fields")
        .num_columns(3)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Plan:");
            egui::ComboBox::from_id_source("battle-plan-list")
                .width(160.0)
                .selected_text(names.get(slot).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        if ui.selectable_label(index == slot, name).clicked() {
                            select = Some(index);
                        }
                    }
                });
            if ui
                .add_enabled(editable_name, egui::Button::new("Rename..."))
                .on_disabled_hover_text("The first plan cannot be renamed or deleted.")
                .clicked()
            {
                command = Some(Command::Rename);
            }
            ui.end_row();

            for (primary, label) in [(true, "Primary Target:"), (false, "Secondary Target:")] {
                let current = TargetClass::from_raw(if primary {
                    plan.primary_target
                } else {
                    plan.secondary_target
                });
                ui.label(label);
                egui::ComboBox::from_id_source(("battle-plan-target", primary))
                    .width(160.0)
                    .selected_text(current.name())
                    .show_ui(ui, |ui| {
                        for class in TargetClass::ALL {
                            if ui
                                .selectable_label(class == current, class.name())
                                .clicked()
                            {
                                command = Some(Command::Target(primary, class));
                            }
                        }
                    });
                ui.end_row();
            }

            // The tactic is the low nibble of a byte that also carries the
            // deleted and dump-cargo flags.
            let tactic = Tactic::from_raw(plan.tactic_nibble()).unwrap_or(Tactic::Disengage);
            ui.label("Tactic:");
            egui::ComboBox::from_id_source("battle-plan-tactic")
                .width(160.0)
                .selected_text(tactic.name())
                .show_ui(ui, |ui| {
                    for choice in Tactic::ALL {
                        if ui
                            .selectable_label(choice == tactic, choice.name())
                            .clicked()
                        {
                            command = Some(Command::Tactic(choice));
                        }
                    }
                });
            ui.end_row();

            let options = app.battle_plan_attack_options();
            let current = options
                .iter()
                .find(|(value, _)| *value == plan.attack_who)
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| format!("player {}", plan.attack_who.saturating_sub(4)));
            ui.label("Attack Who:");
            // A single-player game has nobody to choose between, so the
            // original leaves `Everyone` in the combo and disables it.
            ui.add_enabled_ui(options.len() > 1, |ui| {
                egui::ComboBox::from_id_source("battle-plan-attack")
                    .width(160.0)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (value, name) in &options {
                            if ui
                                .selectable_label(*value == plan.attack_who, name)
                                .clicked()
                            {
                                command = Some(Command::AttackWho(*value));
                            }
                        }
                    });
            });
            let mut dump = plan.dump_cargo();
            if ui
                .checkbox(&mut dump, "Dump Cargo")
                .on_hover_text("Jettison cargo at the start of battle.")
                .changed()
            {
                command = Some(Command::DumpCargo(dump));
            }
            ui.end_row();
        });

    ui.separator();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(editable_name, egui::Button::new("Delete"))
            .clicked()
        {
            command = Some(Command::Delete);
        }
        // The original stops at fifteen plans, though a sixteenth arriving in
        // a log record is accepted.
        let room = app.battle_plan_count() < crate::app::MAX_BATTLE_PLANS;
        if ui
            .add_enabled(room, egui::Button::new("Copy"))
            .on_disabled_hover_text("Fifteen battle plans is as many as there can be.")
            .clicked()
        {
            command = Some(Command::Copy);
        }
        if ui.button("Close").clicked() {
            command = Some(Command::Close);
        }
    });

    // The rename box, which the original puts up as a modal dialog of its own
    // and which `Copy` also lands in.
    if renaming {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Plan name:");
            let mut text = app
                .battle_plans
                .as_ref()
                .and_then(|d| d.rename.clone())
                .unwrap_or_default();
            let field = ui.add(egui::TextEdit::singleline(&mut text).desired_width(160.0));
            if ui.memory(|m| m.focused().is_none()) {
                field.request_focus();
            }
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(dialog) = app.battle_plans.as_mut() {
                dialog.rename = Some(text.clone());
            }
            if entered || ui.button("OK").clicked() {
                command = Some(Command::Renamed(text));
            } else if ui.button("Cancel").clicked() {
                command = Some(Command::RenameCancelled);
            }
        });
    }

    // Deleting a plan fleets are using moves them to the plan before it, so
    // the original asks first (string `0x035b`). The warning is worded here
    // rather than copied from the game.
    if confirming {
        let users = app.fleets_using_battle_plan(slot);
        ui.separator();
        ui.label(format!(
            "{users} fleet{} using this plan. Deleting it moves {} to the plan \
             before this one.",
            if users == 1 { " is" } else { "s are" },
            if users == 1 { "it" } else { "them" }
        ));
        ui.horizontal(|ui| {
            if ui.button("Delete it").clicked() {
                command = Some(Command::DeleteConfirmed);
            }
            if ui.button("Keep it").clicked() {
                command = Some(Command::DeleteCancelled);
            }
        });
    }

    if let Some(index) = select {
        app.select_battle_plan(index);
    }
    if let Some(command) = command {
        act(app, command, slot);
    }
}

/// What the dialog was asked to do this frame.
enum Command {
    Target(bool, TargetClass),
    Tactic(Tactic),
    AttackWho(u8),
    DumpCargo(bool),
    Rename,
    Renamed(String),
    RenameCancelled,
    Copy,
    Delete,
    DeleteConfirmed,
    DeleteCancelled,
    Close,
}

/// Carry it out.
fn act(app: &mut App, command: Command, slot: usize) {
    match command {
        Command::Target(primary, class) => {
            app.set_battle_plan_target(primary, class);
        }
        Command::Tactic(tactic) => {
            app.set_battle_plan_tactic(tactic);
        }
        Command::AttackWho(who) => {
            app.set_battle_plan_attack_who(who);
        }
        Command::DumpCargo(on) => {
            app.set_battle_plan_dump_cargo(on);
        }
        Command::Rename => {
            let name = app.selected_battle_plan().map(|p| p.name.clone());
            if let Some(dialog) = app.battle_plans.as_mut() {
                dialog.rename = name;
            }
        }
        Command::Renamed(name) => {
            app.rename_battle_plan(&name);
            if let Some(dialog) = app.battle_plans.as_mut() {
                dialog.rename = None;
            }
        }
        Command::RenameCancelled => {
            if let Some(dialog) = app.battle_plans.as_mut() {
                dialog.rename = None;
            }
        }
        Command::Copy => {
            app.copy_battle_plan();
        }
        Command::Delete => {
            // Only worth asking when something is using it.
            if app.fleets_using_battle_plan(slot) > 0 {
                if let Some(dialog) = app.battle_plans.as_mut() {
                    dialog.confirm_delete = true;
                }
            } else {
                app.delete_selected_battle_plan();
            }
        }
        Command::DeleteConfirmed => {
            app.delete_selected_battle_plan();
        }
        Command::DeleteCancelled => {
            if let Some(dialog) = app.battle_plans.as_mut() {
                dialog.confirm_delete = false;
            }
        }
        Command::Close => app.close_battle_plans(),
    }
}
