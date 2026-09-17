//! Export the player's guide as a set of HTML pages.
//!
//! `cargo run -p stars-formats --example help_export -- binary/STARS!.HLP out/`
//!
//! Every topic of the file — the 414 titled ones, and any untitled one a
//! hotspot or a context number reaches — becomes `t<offset>.html`, with
//! its non-scrolling band above a rule, its text in the file's own fonts
//! (a CSS class per `|FONT` descriptor), its tables as tables, its
//! pictures as bitmaps with their hotspots as image maps, and every
//! hotspot as a link: a jump is a plain link, a popup a link marked as
//! one. Each page carries the browse sequence's previous and next, the
//! Contents, and the context numbers that reach it. `index.html` lists the
//! topics, `keywords.html` the Search dialog's keywords, and
//! `contexts.html` the dialogs' Help buttons by number.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use stars_formats::help::{Align, Block, Jump, Paragraph, Placement, Run, Table, Topic};
use stars_formats::resources::write_bmp;
use stars_formats::HelpFile;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("a help file");
    let out = PathBuf::from(args.next().unwrap_or_else(|| "help-html".to_string()));
    let help = HelpFile::read(std::fs::read(&path).expect("readable")).expect("a help file");
    std::fs::create_dir_all(&out).expect("the output directory");

    // Every topic there is: the titled ones, the contents page, every
    // context number's target, and whatever the hotspots reach — walked
    // until nothing new turns up.
    let mut wanted: BTreeSet<i32> = help.titled_topics().iter().map(|(o, _)| *o).collect();
    wanted.insert(help.contents());
    let mut contexts: BTreeMap<i32, Vec<u32>> = BTreeMap::new();
    for id in 0..0x10000u32 {
        if let Some(offset) = help.topic_for_context(id) {
            wanted.insert(offset);
            contexts.entry(offset).or_default().push(id);
        }
    }
    let mut topics: BTreeMap<i32, Topic> = BTreeMap::new();
    let mut pictures: BTreeSet<u16> = BTreeSet::new();
    let mut queue: Vec<i32> = wanted.iter().copied().collect();
    while let Some(offset) = queue.pop() {
        if topics.contains_key(&offset) {
            continue;
        }
        let Ok(topic) = help.topic(offset) else {
            eprintln!("topic {offset:#x} does not read; skipped");
            continue;
        };
        for paragraph in topic.paragraphs() {
            for run in &paragraph.runs {
                match run {
                    Run::Text {
                        jump: Some(Jump::Topic(t) | Jump::Popup(t)),
                        ..
                    } => queue.push(*t),
                    Run::Picture { number, .. } => {
                        pictures.insert(*number);
                    }
                    _ => {}
                }
            }
        }
        for next in [topic.browse_back, topic.browse_forward]
            .into_iter()
            .flatten()
        {
            queue.push(next);
        }
        topics.insert(offset, topic);
    }

    // The pictures, as bitmaps, and their hotspots' targets too.
    let mut maps: Maps = BTreeMap::new();
    for number in &pictures {
        match help.picture(*number) {
            Ok(picture) => {
                std::fs::write(
                    out.join(format!("bm{number}.bmp")),
                    write_bmp(&picture.image),
                )
                .expect("the picture");
                let spots: Vec<_> = picture
                    .hotspots
                    .iter()
                    .map(|h| (h.rect.0, h.rect.1, h.rect.2, h.rect.3, h.jump.clone()))
                    .collect();
                for (_, _, _, _, jump) in &spots {
                    if let Jump::Topic(t) | Jump::Popup(t) = jump {
                        if !topics.contains_key(t) {
                            if let Ok(topic) = help.topic(*t) {
                                topics.insert(*t, topic);
                            }
                        }
                    }
                }
                maps.insert(*number, spots);
            }
            Err(e) => eprintln!("picture {number}: {e}"),
        }
    }

    std::fs::write(out.join("help.css"), stylesheet(&help)).expect("the stylesheet");
    for (offset, topic) in &topics {
        let page = topic_page(&help, topic, &topics, &maps, contexts.get(offset));
        std::fs::write(out.join(file_name(*offset)), page).expect("the page");
    }
    std::fs::write(out.join("index.html"), index_page(&help, &topics)).expect("the index");
    std::fs::write(out.join("keywords.html"), keywords_page(&help, &topics)).expect("the keywords");
    std::fs::write(out.join("contexts.html"), contexts_page(&help, &contexts))
        .expect("the contexts");
    println!(
        "{}: {} topics, {} pictures, {} context numbers",
        out.display(),
        topics.len(),
        pictures.len(),
        contexts.values().map(Vec::len).sum::<usize>()
    );
}

fn file_name(offset: i32) -> String {
    format!("t{offset:x}.html")
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// One CSS class per font: the face, the size in points, the weight,
/// the slant, the lines, the colour — with the file's (1, 1, 0) as black,
/// which is what WinHelp makes of it.
fn stylesheet(help: &HelpFile) -> String {
    let mut css = String::from(
        "body { background: #fff; color: #000; font-family: Arial, Helvetica, sans-serif; \
         margin: 0; }\n\
         .band { background: #fff; padding: 8px 16px; border-bottom: 1px solid #808080; }\n\
         .body { padding: 8px 16px 32px; }\n\
         nav { background: #c0c0c0; padding: 4px 16px; font: 12px Arial, sans-serif; }\n\
         nav a { margin-right: 12px; }\n\
         nav span.dead { color: #808080; margin-right: 12px; }\n\
         p { margin: 0; }\n\
         a { color: #008000; }\n\
         a.popup { text-decoration: underline dotted; }\n\
         table { border-collapse: collapse; }\n\
         td { vertical-align: top; padding: 0 4px 0 0; }\n\
         img { vertical-align: top; }\n\
         .left { float: left; margin-right: 8px; }\n\
         .right { float: right; margin-left: 8px; }\n\
         .ctx { color: #808080; font: 11px Arial, sans-serif; margin-top: 24px; }\n\
         ul.index { columns: 2; }\n",
    );
    for (number, font) in help.fonts().iter().enumerate() {
        let family = if font.face.starts_with("Courier") || font.face == "LinePrinter" {
            "\"Courier New\", Courier, monospace"
        } else if font.face == "Symbol" {
            "Symbol, serif"
        } else {
            "Arial, Helvetica, sans-serif"
        };
        let colour = match font.colour {
            [1, 1, 0] => [0, 0, 0],
            c => c,
        };
        let _ = writeln!(
            css,
            ".f{number} {{ font-family: {family}; font-size: {}pt; font-weight: {}; \
             font-style: {}; text-decoration: {}; color: #{:02x}{:02x}{:02x}; }}",
            font.points(),
            if font.bold { "bold" } else { "normal" },
            if font.italic { "italic" } else { "normal" },
            match (font.underline, font.strike_out) {
                (true, true) => "underline line-through",
                (true, false) => "underline",
                (false, true) => "line-through",
                (false, false) => "none",
            },
            colour[0],
            colour[1],
            colour[2],
        );
    }
    css
}

fn head(title: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{}</title>\n<link rel=\"stylesheet\" href=\"help.css\">\n</head>\n<body>\n",
        escape(title)
    )
}

fn nav(help: &HelpFile, topic: Option<&Topic>, topics: &BTreeMap<i32, Topic>) -> String {
    let title_of = |offset: i32| -> String {
        topics
            .get(&offset)
            .map(|t| t.title.clone())
            .or_else(|| help.title_of(offset).map(str::to_string))
            .unwrap_or_else(|| format!("topic {offset:#x}"))
    };
    let mut out = String::from("<nav>");
    let _ = write!(
        out,
        "<a href=\"{}\">Contents</a><a href=\"index.html\">Index</a>\
         <a href=\"keywords.html\">Search</a><a href=\"contexts.html\">Help buttons</a>",
        file_name(help.contents())
    );
    if let Some(topic) = topic {
        match topic.browse_back.filter(|b| topics.contains_key(b)) {
            Some(back) => {
                let _ = write!(
                    out,
                    "<a href=\"{}\" title=\"{}\">&lt;&lt;</a>",
                    file_name(back),
                    escape(&title_of(back))
                );
            }
            None => out.push_str("<span class=\"dead\">&lt;&lt;</span>"),
        }
        match topic.browse_forward.filter(|f| topics.contains_key(f)) {
            Some(next) => {
                let _ = write!(
                    out,
                    "<a href=\"{}\" title=\"{}\">&gt;&gt;</a>",
                    file_name(next),
                    escape(&title_of(next))
                );
            }
            None => out.push_str("<span class=\"dead\">&gt;&gt;</span>"),
        }
    }
    out.push_str("</nav>\n");
    out
}

type Maps = BTreeMap<u16, Vec<(u16, u16, u16, u16, Jump)>>;

fn topic_page(
    help: &HelpFile,
    topic: &Topic,
    topics: &BTreeMap<i32, Topic>,
    maps: &Maps,
    contexts: Option<&Vec<u32>>,
) -> String {
    let title = if topic.title.is_empty() {
        format!("Topic {:#x}", topic.offset)
    } else {
        topic.title.clone()
    };
    let mut html = head(&title);
    html.push_str(&nav(help, Some(topic), topics));
    if !topic.band.is_empty() {
        html.push_str("<div class=\"band\">\n");
        for block in &topic.band {
            html.push_str(&block_html(block, topics, maps));
        }
        html.push_str("</div>\n");
    }
    html.push_str("<div class=\"body\">\n");
    for block in &topic.body {
        html.push_str(&block_html(block, topics, maps));
    }
    if let Some(ids) = contexts {
        let list: Vec<String> = ids.iter().map(|id| format!("{id:#x}")).collect();
        let _ = write!(
            html,
            "<p class=\"ctx\">Help context {}; topic offset {:#x}</p>",
            list.join(", "),
            topic.offset
        );
    } else {
        let _ = write!(
            html,
            "<p class=\"ctx\">Topic offset {:#x}</p>",
            topic.offset
        );
    }
    html.push_str("\n</div>\n</body>\n</html>\n");
    html
}

fn block_html(block: &Block, topics: &BTreeMap<i32, Topic>, maps: &Maps) -> String {
    match block {
        Block::Text(paragraphs) => paragraphs
            .iter()
            .map(|p| paragraph_html(p, topics, maps))
            .collect(),
        Block::Table(table) => table_html(table, topics, maps),
    }
}

fn table_html(table: &Table, topics: &BTreeMap<i32, Topic>, maps: &Maps) -> String {
    let mut html = String::from("<table>\n");
    for row in &table.rows {
        html.push_str("<tr>");
        for cell in row {
            let width = table
                .columns
                .get(cell.column)
                .map(|c| f32::from(c.width) / 2.0)
                .unwrap_or(0.0);
            let _ = write!(html, "<td style=\"width: {width}pt\">");
            for paragraph in &cell.paragraphs {
                html.push_str(&paragraph_html(paragraph, topics, maps));
            }
            html.push_str("</td>");
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</table>\n");
    html
}

fn paragraph_html(paragraph: &Paragraph, topics: &BTreeMap<i32, Topic>, maps: &Maps) -> String {
    let style = &paragraph.style;
    let pt = |half: i16| f32::from(half) / 2.0;
    let mut css = String::new();
    match style.align {
        Align::Left => {}
        Align::Centre => css.push_str("text-align: center; "),
        Align::Right => css.push_str("text-align: right; "),
    }
    if style.above != 0 {
        let _ = write!(css, "margin-top: {}pt; ", pt(style.above));
    }
    if style.below != 0 {
        let _ = write!(css, "margin-bottom: {}pt; ", pt(style.below));
    }
    if style.left != 0 {
        let _ = write!(css, "margin-left: {}pt; ", pt(style.left));
    }
    if style.right != 0 {
        let _ = write!(css, "margin-right: {}pt; ", pt(style.right));
    }
    if style.first != 0 {
        let _ = write!(css, "text-indent: {}pt; ", pt(style.first));
    }
    // The border byte: a box, or rules on any of the four sides.
    let border = style.border;
    if border & 0x01 != 0 {
        css.push_str("border: 1px solid #000; padding: 2px; ");
    } else {
        for (bit, side) in [
            (0x02, "top"),
            (0x04, "left"),
            (0x08, "bottom"),
            (0x10, "right"),
        ] {
            if border & bit != 0 {
                let _ = write!(css, "border-{side}: 1px solid #000; padding-{side}: 2px; ");
            }
        }
    }
    let mut html = if css.is_empty() {
        String::from("<p>")
    } else {
        format!("<p style=\"{}\">", css.trim_end())
    };
    for run in &paragraph.runs {
        match run {
            Run::Text { font, text, jump } => {
                let text = escape(text).replace('\u{a0}', "&nbsp;");
                match jump {
                    Some(Jump::Topic(t)) if topics.contains_key(t) => {
                        let _ = write!(
                            html,
                            "<a class=\"f{font}\" href=\"{}\">{text}</a>",
                            file_name(*t)
                        );
                    }
                    Some(Jump::Popup(t)) if topics.contains_key(t) => {
                        let _ = write!(
                            html,
                            "<a class=\"f{font} popup\" href=\"{}\" title=\"popup\">{text}</a>",
                            file_name(*t)
                        );
                    }
                    Some(Jump::Macro(m)) => {
                        let _ = write!(
                            html,
                            "<span class=\"f{font}\" title=\"macro: {}\">{text}</span>",
                            escape(m)
                        );
                    }
                    _ => {
                        let _ = write!(html, "<span class=\"f{font}\">{text}</span>");
                    }
                }
            }
            Run::Picture { number, placement } => {
                let class = match placement {
                    Placement::Inline => "",
                    Placement::Left => " class=\"left\"",
                    Placement::Right => " class=\"right\"",
                };
                match maps.get(number) {
                    Some(spots) if !spots.is_empty() => {
                        let _ = write!(
                            html,
                            "<img src=\"bm{number}.bmp\" usemap=\"#m{number}\" alt=\"\"{class}>\
                             <map name=\"m{number}\">"
                        );
                        for (x, y, w, h, jump) in spots {
                            if let Jump::Topic(t) | Jump::Popup(t) = jump {
                                if topics.contains_key(t) {
                                    let _ = write!(
                                        html,
                                        "<area shape=\"rect\" coords=\"{x},{y},{},{}\" href=\"{}\">",
                                        x + w,
                                        y + h,
                                        file_name(*t)
                                    );
                                }
                            }
                        }
                        html.push_str("</map>");
                    }
                    Some(_) => {
                        let _ = write!(html, "<img src=\"bm{number}.bmp\" alt=\"\"{class}>");
                    }
                    // A metafile, which this reader does not decode.
                    None => {
                        let _ = write!(html, "<span title=\"picture {number}\">[picture]</span>");
                    }
                }
            }
            Run::Tab => html.push_str("&nbsp;&nbsp;&nbsp;&nbsp;"),
            Run::Break => html.push_str("<br>"),
        }
    }
    html.push_str("</p>\n");
    html
}

fn index_page(help: &HelpFile, topics: &BTreeMap<i32, Topic>) -> String {
    let mut html = head(help.title());
    html.push_str(&nav(help, None, topics));
    let _ = write!(
        html,
        "<div class=\"body\"><h1>{}</h1>\n<p>{} topics. Start at the \
         <a href=\"{}\">contents</a>.</p>\n<ul class=\"index\">\n",
        escape(help.title()),
        topics.len(),
        file_name(help.contents())
    );
    for (offset, topic) in topics {
        let title = if topic.title.is_empty() {
            format!("(untitled, {offset:#x})")
        } else {
            topic.title.clone()
        };
        let _ = writeln!(
            html,
            "<li><a href=\"{}\">{}</a></li>",
            file_name(*offset),
            escape(&title)
        );
    }
    html.push_str("</ul></div>\n</body>\n</html>\n");
    html
}

fn keywords_page(help: &HelpFile, topics: &BTreeMap<i32, Topic>) -> String {
    let mut html = head("Search");
    html.push_str(&nav(help, None, topics));
    html.push_str("<div class=\"body\"><h1>Search</h1>\n<dl>\n");
    for keyword in help.keywords() {
        let _ = write!(html, "<dt>{}</dt><dd>", escape(&keyword.word));
        let links: Vec<String> = keyword
            .topics
            .iter()
            .filter_map(|offset| {
                let title = topics.get(offset)?;
                Some(format!(
                    "<a href=\"{}\">{}</a>",
                    file_name(*offset),
                    escape(&title.title)
                ))
            })
            .collect();
        html.push_str(&links.join(" · "));
        html.push_str("</dd>\n");
    }
    html.push_str("</dl></div>\n</body>\n</html>\n");
    html
}

fn contexts_page(help: &HelpFile, contexts: &BTreeMap<i32, Vec<u32>>) -> String {
    let mut html = head("Help buttons");
    html.push_str(&nav(help, None, &BTreeMap::new()));
    html.push_str(
        "<div class=\"body\"><h1>Help buttons</h1>\n<p>Every context number the file maps, \
         as the game's dialogs ask for them.</p>\n<table>\n",
    );
    let mut rows: Vec<(u32, i32)> = contexts
        .iter()
        .flat_map(|(offset, ids)| ids.iter().map(move |id| (*id, *offset)))
        .collect();
    rows.sort_unstable();
    for (id, offset) in rows {
        let _ = writeln!(
            html,
            "<tr><td>{id:#x}</td><td><a href=\"{}\">{}</a></td></tr>",
            file_name(offset),
            escape(help.title_of(offset).unwrap_or("(untitled)"))
        );
    }
    html.push_str("</table></div>\n</body>\n</html>\n");
    html
}
