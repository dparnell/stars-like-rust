//! The help viewer — `WINHELP.EXE` with the player's guide in it.
//!
//! The original opens Windows' own help viewer on `STARS!.HLP`; there is no
//! help code in the game. What is reproduced here is that viewer as
//! Windows 3.1 shipped it: a menu bar (File, Edit, Bookmark, Help), the
//! button bar — Contents, Search, Back, History, and the `<<` `>>` browse
//! pair the file's `BrowseButtons()` macro adds — a white non-scrolling
//! band at the top of a topic and the scrolling text under it, green
//! underlined hotspots that jump or raise a popup, and the Search and
//! History windows. The text is laid out from the file's own fonts and
//! paragraph styles (`docs/formats/help.md`).
//!
//! What egui's bundled faces cannot do — bold and italic — is drawn
//! regular; the sizes and colours are the file's.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Stroke, Vec2};
use stars_formats::help::{Align, Block, Jump, Paragraph, Placement, Run, Table, Topic};

use crate::help::Notice;
use crate::App;

/// Half points to pixels at ninety-six dots an inch: a point is four
/// thirds of a pixel.
fn px(half_points: i16) -> f32 {
    f32::from(half_points) * 2.0 / 3.0
}

/// The window's background — `COLOR_WINDOW`, white.
const PAPER: Color32 = Color32::WHITE;
/// The button bar — `COLOR_BTNFACE`.
const BAR: Color32 = Color32::from_rgb(0xc0, 0xc0, 0xc0);
/// The green WinHelp draws hotspots in.
const HOTSPOT: Color32 = Color32::from_rgb(0, 0x80, 0);
/// The largest popup, as a share of the screen.
const POPUP_WIDTH: f32 = 360.0;

/// Draw whatever of the viewer is up: the window, its popup, the Search
/// and History windows, or a notice.
pub fn windows(app: &mut App, ctx: &egui::Context) {
    notice(app, ctx);
    if !app.help.open {
        return;
    }
    let caption = app.help.caption();
    let screen = ctx.screen_rect();
    // The file's window record: 710 by 909 thousandths of the screen, at
    // the top left.
    let (w, h) = app
        .help
        .file()
        .and_then(|f| f.window())
        .map_or((0.71, 0.909), |w| {
            (f32::from(w.width) / 1000.0, f32::from(w.height) / 1000.0)
        });
    let size = Vec2::new(screen.width() * w, screen.height() * h);
    let mut open = true;
    let mut jump: Option<(Jump, [f32; 2])> = None;
    egui::Window::new(caption)
        .id(egui::Id::new("help-viewer"))
        .open(&mut open)
        .resizable(true)
        .default_size(size)
        .frame(egui::Frame::window(&ctx.style()).inner_margin(0.0))
        .show(ctx, |ui| {
            jump = viewer(app, ui);
        });
    if !open {
        app.help_close();
        return;
    }
    if let Some((jump, at)) = jump {
        match jump {
            Jump::Popup(offset) => app.help_popup(offset, at),
            other => app.help_jump(&other),
        }
    }
    popup(app, ctx);
    search(app, ctx);
    history(app, ctx);
}

/// The viewer's contents: menu bar, button bar, band and text.
fn viewer(app: &mut App, ui: &mut egui::Ui) -> Option<(Jump, [f32; 2])> {
    let mut jump = None;
    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Exit").clicked() {
                ui.close_menu();
                app.help_close();
            }
        });
        ui.menu_button("Edit", |ui| {
            let text = app.help.topic().map(Topic::plain_text);
            if ui
                .add_enabled(text.is_some(), egui::Button::new("Copy"))
                .clicked()
            {
                ui.close_menu();
                if let Some(text) = text {
                    ui.output_mut(|o| o.copied_text = text);
                }
            }
        });
        ui.menu_button("Bookmark", |ui| {
            ui.add_enabled(false, egui::Button::new("Define…"));
        });
        ui.menu_button("Help", |ui| {
            if ui.button("About Help…").clicked() {
                ui.close_menu();
                app.help.notice = Some(Notice::About);
            }
        });
    });
    if !app.help.open {
        return None;
    }

    // The button bar: raised buttons on a grey band.
    egui::Frame::none()
        .fill(BAR)
        .inner_margin(egui::Margin::symmetric(6.0, 3.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if crate::views::flow_button(app, ui, "Contents", true).clicked() {
                    app.help_contents();
                }
                if crate::views::flow_button(app, ui, "Search", true).clicked() {
                    app.help_search_open();
                }
                let back = app.help.can_go_back();
                if crate::views::flow_button(app, ui, "Back", back).clicked() {
                    app.help_back();
                }
                if crate::views::flow_button(app, ui, "History", true).clicked() {
                    app.help.history_open = true;
                }
                let can = app.help_can_browse(false);
                if crate::views::flow_button(app, ui, "<<", can).clicked() {
                    app.help_browse(false);
                }
                let can = app.help_can_browse(true);
                if crate::views::flow_button(app, ui, ">>", can).clicked() {
                    app.help_browse(true);
                }
            });
        });

    let topic = app.help.topic().cloned()?;
    // The non-scrolling region, white with a rule under it.
    if !topic.band.is_empty() {
        egui::Frame::none()
            .fill(PAPER)
            .inner_margin(egui::Margin::symmetric(10.0, 4.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for block in &topic.band {
                    if let Some(j) = draw_block(app, ui, block) {
                        jump = Some(j);
                    }
                }
            });
        let rect = ui.available_rect_before_wrap();
        let y = rect.top();
        ui.painter().hline(
            rect.x_range(),
            y,
            Stroke::new(1.0_f32, Color32::from_rgb(0x80, 0x80, 0x80)),
        );
        ui.add_space(1.0);
    }
    // The scrolling region.
    egui::Frame::none()
        .fill(PAPER)
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical()
                .id_source("help-text")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for block in &topic.body {
                        if let Some(j) = draw_block(app, ui, block) {
                            jump = Some(j);
                        }
                    }
                });
        });
    jump
}

/// One block of a topic.
fn draw_block(app: &mut App, ui: &mut egui::Ui, block: &Block) -> Option<(Jump, [f32; 2])> {
    match block {
        Block::Text(paragraphs) => {
            let mut jump = None;
            for paragraph in paragraphs {
                if let Some(j) = draw_paragraph(app, ui, paragraph, None) {
                    jump = Some(j);
                }
            }
            jump
        }
        Block::Table(table) => draw_table(app, ui, table),
    }
}

/// A table: its columns side by side at the widths the file gives them.
fn draw_table(app: &mut App, ui: &mut egui::Ui, table: &Table) -> Option<(Jump, [f32; 2])> {
    let mut jump = None;
    let available = ui.available_width();
    let total: f32 = table.columns.iter().map(|c| px(c.width) + px(c.gap)).sum();
    // Squeeze the columns when the window is narrower than the file
    // expected.
    let k = if total > available && total > 0.0 {
        available / total
    } else {
        1.0
    };
    for row in &table.rows {
        ui.horizontal_top(|ui| {
            for cell in row {
                let Some(column) = table.columns.get(cell.column) else {
                    continue;
                };
                ui.add_space(px(column.gap) * k);
                let width = px(column.width) * k;
                ui.allocate_ui_with_layout(
                    Vec2::new(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_max_width(width);
                        for paragraph in &cell.paragraphs {
                            if let Some(j) = draw_paragraph(app, ui, paragraph, Some(width)) {
                                jump = Some(j);
                            }
                        }
                    },
                );
            }
        });
    }
    jump
}

/// One paragraph: its pictures, then its text laid out in the file's fonts
/// with the hotspots underlined in green, wrapped to the width left.
fn draw_paragraph(
    app: &mut App,
    ui: &mut egui::Ui,
    paragraph: &Paragraph,
    width: Option<f32>,
) -> Option<(Jump, [f32; 2])> {
    let style = &paragraph.style;
    let mut jump = None;
    ui.add_space(px(style.above));
    let left = px(style.left).max(0.0);
    let right = px(style.right).max(0.0);
    let width = width.unwrap_or_else(|| ui.available_width());
    let text_width = (width - left - right).max(40.0);
    ui.horizontal_top(|ui| {
        ui.add_space(left);
        // A picture at the left has the text alongside; one in the line
        // stands on its own line above the text, which is how this file
        // uses them.
        let pictures: Vec<(u16, Placement)> = paragraph
            .runs
            .iter()
            .filter_map(|run| match run {
                Run::Picture { number, placement } => Some((*number, *placement)),
                _ => None,
            })
            .collect();
        let has_text = paragraph
            .runs
            .iter()
            .any(|run| matches!(run, Run::Text { text, .. } if !text.trim().is_empty()));
        let beside = pictures
            .iter()
            .any(|(_, p)| matches!(p, Placement::Left | Placement::Right));
        ui.vertical(|ui| {
            ui.set_max_width(text_width);
            if beside && has_text {
                ui.horizontal_top(|ui| {
                    let mut used = 0.0;
                    for (number, placement) in &pictures {
                        if *placement == Placement::Left {
                            if let Some((j, w)) = draw_picture(app, ui, *number) {
                                used += w + 8.0;
                                if let Some(j) = j {
                                    jump = Some(j);
                                }
                                ui.add_space(8.0);
                            }
                        }
                    }
                    if let Some(j) = draw_text(app, ui, paragraph, (text_width - used).max(40.0)) {
                        jump = Some(j);
                    }
                    for (number, placement) in &pictures {
                        if *placement == Placement::Right {
                            ui.add_space(8.0);
                            if let Some((Some(j), _)) = draw_picture(app, ui, *number) {
                                jump = Some(j);
                            }
                        }
                    }
                });
            } else {
                for (number, _) in &pictures {
                    if let Some((Some(j), _)) = draw_picture(app, ui, *number) {
                        jump = Some(j);
                    }
                }
                if has_text {
                    if let Some(j) = draw_text(app, ui, paragraph, text_width) {
                        jump = Some(j);
                    }
                }
            }
        });
    });
    ui.add_space(px(style.below));
    jump
}

/// The text of a paragraph as one wrapped galley, with a hand over the
/// hotspots and a click on one reported.
fn draw_text(
    app: &mut App,
    ui: &mut egui::Ui,
    paragraph: &Paragraph,
    width: f32,
) -> Option<(Jump, [f32; 2])> {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    // Each section's character range and the hotspot it belongs to.
    let mut sections: Vec<(usize, usize, Option<Jump>)> = Vec::new();
    let mut chars = 0usize;
    let mut last_font = 4u16;
    for run in &paragraph.runs {
        let (text, font, jump) = match run {
            Run::Text { font, text, jump } => {
                last_font = *font;
                (text.clone(), *font, jump.clone())
            }
            Run::Tab => ("    ".to_string(), last_font, None),
            Run::Break => ("\n".to_string(), last_font, None),
            Run::Picture { .. } => continue,
        };
        if text.is_empty() {
            continue;
        }
        let format = text_format(app, font, jump.is_some());
        let count = text.chars().count();
        sections.push((chars, chars + count, jump));
        chars += count;
        job.append(&text, 0.0, format);
    }
    if chars == 0 {
        return None;
    }
    let galley = ui.fonts(|f| f.layout_job(job));
    let size = galley.size();
    let offset = match paragraph.style.align {
        Align::Left => 0.0,
        Align::Centre => ((width - size.x) / 2.0).max(0.0),
        Align::Right => (width - size.x).max(0.0),
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, size.y), egui::Sense::click());
    let origin = rect.min + Vec2::new(offset, 0.0);
    ui.painter().galley(origin, galley.clone(), Color32::BLACK);
    let pos = response.hover_pos()?;
    let cursor = galley.cursor_from_pos(pos - origin);
    let index = cursor.ccursor.index;
    let hit = sections
        .iter()
        .find(|(start, end, jump)| jump.is_some() && index >= *start && index < *end)
        .and_then(|(_, _, jump)| jump.clone())?;
    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
    response.clicked().then_some((hit, [pos.x, pos.y]))
}

/// How one of the file's fonts is drawn.
fn text_format(app: &App, font: u16, hotspot: bool) -> TextFormat {
    let description = app.help.file().and_then(|f| f.font(font));
    let size = description.map_or(20, |f| f.half_points);
    let family = match description.map(|f| f.face.as_str()) {
        Some(face) if face.starts_with("Courier") || face == "LinePrinter" => FontFamily::Monospace,
        _ => FontFamily::Proportional,
    };
    // The file writes black as (1, 1, 0), the colour WinHelp treats as
    // "the window's text colour".
    let colour = match description.map(|f| f.colour) {
        Some([1, 1, 0]) | None => Color32::BLACK,
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
    };
    let colour = if hotspot { HOTSPOT } else { colour };
    let underline = if hotspot || description.is_some_and(|f| f.underline) {
        Stroke::new(1.0_f32, colour)
    } else {
        Stroke::NONE
    };
    TextFormat {
        font_id: FontId::new(px(i16::from(size)).max(8.0), family),
        color: colour,
        underline,
        strikethrough: if description.is_some_and(|f| f.strike_out) {
            Stroke::new(1.0_f32, colour)
        } else {
            Stroke::NONE
        },
        ..TextFormat::default()
    }
}

/// A hotspot pressed, and where.
type Pressed = Option<(Jump, [f32; 2])>;

/// A picture at its own size, with its hotspots live.
///
/// Returns the hotspot pressed, if one was, and the width drawn; `None`
/// when the picture cannot be shown (a metafile, or none such).
fn draw_picture(app: &mut App, ui: &mut egui::Ui, number: u16) -> Option<(Pressed, f32)> {
    if app.help.undecodable.contains(&number) {
        return None;
    }
    // Decoded once, then kept with its hotspots.
    let missing = !app.help.textures.contains_key(&number);
    if missing {
        let decoded = app.help.file()?.picture(number);
        match decoded {
            Ok(picture) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [picture.image.width as usize, picture.image.height as usize],
                    &picture.image.pixels,
                );
                let handle = ui.ctx().load_texture(
                    format!("help-bm{number}"),
                    image,
                    egui::TextureOptions::NEAREST,
                );
                app.help.textures.insert(number, (handle, picture.hotspots));
            }
            Err(_) => {
                app.help.undecodable.insert(number);
                return None;
            }
        }
    }
    let (texture, hotspots) = app.help.textures.get(&number)?;
    let size = texture.size_vec2();
    let response = ui.add(egui::Image::new((texture.id(), size)).sense(egui::Sense::click()));
    let rect = response.rect;
    let mut pressed = None;
    if let Some(pos) = response.hover_pos() {
        let at = pos - rect.min;
        for hotspot in hotspots {
            let (x, y, w, h) = hotspot.rect;
            let inside = at.x >= f32::from(x)
                && at.x < f32::from(x + w)
                && at.y >= f32::from(y)
                && at.y < f32::from(y + h);
            if inside {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                if response.clicked() {
                    pressed = Some((hotspot.jump.clone(), [pos.x, pos.y]));
                }
                break;
            }
        }
    }
    Some((pressed, size.x))
}

/// A popup topic, where the hotspot was, gone at the next click.
fn popup(app: &mut App, ctx: &egui::Context) {
    let Some((topic, at)) = app.help.popup.clone() else {
        app.help.popup_age = 0;
        return;
    };
    // The click that raised it must not also dismiss it.
    if app.help.popup_age > 0
        && ctx.input(|i| i.pointer.any_click() || i.key_pressed(egui::Key::Escape))
    {
        app.help.popup = None;
        app.help.popup_age = 0;
        return;
    }
    app.help.popup_age += 1;
    let screen = ctx.screen_rect();
    let mut pos = egui::pos2(at[0], at[1] + 12.0);
    pos.x = pos
        .x
        .min(screen.right() - POPUP_WIDTH - 8.0)
        .max(screen.left());
    let mut jump = None;
    egui::Area::new(egui::Id::new("help-popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(PAPER)
                .stroke(Stroke::new(1.0_f32, Color32::BLACK))
                .shadow(egui::epaint::Shadow {
                    offset: Vec2::new(3.0, 3.0),
                    blur: 0.0,
                    spread: 0.0,
                    color: Color32::from_black_alpha(0x80),
                })
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_max_width(POPUP_WIDTH);
                    for block in topic.band.iter().chain(topic.body.iter()) {
                        if let Some(j) = draw_block(app, ui, block) {
                            jump = Some(j);
                        }
                    }
                });
        });
    if let Some((jump, _)) = jump {
        app.help.popup = None;
        app.help_jump(&jump);
    }
}

/// The Search window: a word, the keywords, and the topics under one.
fn search(app: &mut App, ctx: &egui::Context) {
    if app.help.search.is_none() {
        return;
    }
    let mut open = true;
    egui::Window::new("Search")
        .id(egui::Id::new("help-search"))
        .open(&mut open)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new("Type a word, or select one from the list.").small());
            let mut text = app
                .help
                .search
                .as_ref()
                .map(|s| s.text.clone())
                .unwrap_or_default();
            let field = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
            crate::views::record(app, ui, "search word", &field);
            if field.changed() {
                if let Some(search) = app.help.search.as_mut() {
                    search.text = text;
                    search.keyword = None;
                }
            }
            let matches = app.help_keyword_matches();
            let picked = app.help.search.as_ref().and_then(|s| s.keyword);
            let mut choose = None;
            egui::ScrollArea::vertical()
                .id_source("help-keywords")
                .max_height(140.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let file = app.help.file();
                    for index in matches {
                        let Some(word) = file.and_then(|f| f.keywords().get(index)) else {
                            continue;
                        };
                        if ui
                            .selectable_label(
                                picked == Some(index),
                                egui::RichText::new(&word.word).small(),
                            )
                            .clicked()
                        {
                            choose = Some(index);
                        }
                    }
                });
            let mut show = false;
            if let Some(index) = choose {
                if let Some(search) = app.help.search.as_mut() {
                    search.keyword = Some(index);
                    // Picking a word is as good as pressing Show Topics, as
                    // it is in WinHelp when the list is double-clicked; one
                    // click fills the word in and shows its topics.
                    show = true;
                }
            }
            ui.horizontal(|ui| {
                let can = picked.is_some() || choose.is_some();
                if crate::views::flow_button(app, ui, "Show Topics", can).clicked() {
                    show = true;
                }
                if crate::views::flow_button(app, ui, "Cancel", true).clicked() {
                    app.help.search = None;
                }
            });
            if show {
                app.help_show_topics();
            }
            let topics = app
                .help
                .search
                .as_ref()
                .map(|s| s.topics.clone())
                .unwrap_or_default();
            let chosen = app.help.search.as_ref().and_then(|s| s.topic);
            let mut go = false;
            egui::ScrollArea::vertical()
                .id_source("help-search-topics")
                .max_height(120.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (index, (_, title)) in topics.iter().enumerate() {
                        let response = ui.selectable_label(
                            chosen == Some(index),
                            egui::RichText::new(title).small(),
                        );
                        if response.clicked() {
                            if let Some(search) = app.help.search.as_mut() {
                                search.topic = Some(index);
                            }
                        }
                        if response.double_clicked() {
                            go = true;
                        }
                    }
                });
            if crate::views::flow_button(app, ui, "Go To", chosen.is_some()).clicked() {
                go = true;
            }
            if go {
                app.help_search_go();
            }
        });
    if !open {
        app.help.search = None;
    }
}

/// The History window: every topic shown, most recent first.
fn history(app: &mut App, ctx: &egui::Context) {
    if !app.help.history_open {
        return;
    }
    let mut open = true;
    let mut goto = None;
    egui::Window::new("Windows Help History")
        .id(egui::Id::new("help-history"))
        .open(&mut open)
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let visited = app.help.visited.clone();
                    let current = app.help.topic().map(|t| t.offset);
                    for offset in visited {
                        let title = app
                            .help
                            .file()
                            .and_then(|f| f.title_of(offset))
                            .unwrap_or("(untitled)")
                            .to_string();
                        if ui
                            .selectable_label(
                                current == Some(offset),
                                egui::RichText::new(title).small(),
                            )
                            .clicked()
                        {
                            goto = Some(offset);
                        }
                    }
                });
        });
    if let Some(offset) = goto {
        app.help_goto(offset);
    }
    if !open {
        app.help.history_open = false;
    }
}

/// A notice in place of a topic, with an OK to take it down.
fn notice(app: &mut App, ctx: &egui::Context) {
    let Some(notice) = app.help.notice.clone() else {
        return;
    };
    let (title, text) = match notice {
        Notice::NoFile => (
            "Help",
            "The player's guide could not be found. Put STARS!.HLP beside the \
             original executable, or in the folder the game was opened from."
                .to_string(),
        ),
        Notice::NoTopic(id) => (
            "Help",
            format!("The player's guide has no page for this dialog (topic {id:#x})."),
        ),
        Notice::About => (
            "About Help",
            format!(
                "{}\n\nA reader for the Windows 3.1 help file the original ships, \
                 built for this project.",
                app.help.caption()
            ),
        ),
    };
    let mut open = true;
    let mut done = false;
    egui::Window::new(title)
        .id(egui::Id::new("help-notice"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.label(text);
            ui.add_space(6.0);
            if crate::views::flow_button(app, ui, "OK", true).clicked() {
                done = true;
            }
        });
    if done || !open {
        app.help.notice = None;
    }
}
