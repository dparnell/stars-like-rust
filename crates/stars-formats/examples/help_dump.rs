//! Print a help topic as text, by context number or topic offset.
//!
//! `cargo run -p stars-formats --example help_dump -- binary/STARS!.HLP 0x433`

use stars_formats::help::{Block, Jump, Run};
use stars_formats::HelpFile;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("a help file");
    let what = args.next().unwrap_or_else(|| "contents".to_string());
    let help = HelpFile::read(std::fs::read(path).expect("readable")).expect("a help file");
    let offset = match what.as_str() {
        "contents" => help.contents(),
        "pictures" => {
            for n in 0..help.picture_count() {
                match help.picture(n as u16) {
                    Ok(p) => println!(
                        "|bm{n}: {}x{} {} hotspots",
                        p.image.width,
                        p.image.height,
                        p.hotspots.len()
                    ),
                    Err(e) => println!("|bm{n}: {e}"),
                }
            }
            return;
        }
        s if s.starts_with("0x") => {
            let id = u32::from_str_radix(&s[2..], 16).expect("hex");
            help.topic_for_context(id).unwrap_or_else(|| {
                println!("no topic for context {id:#x}");
                std::process::exit(1)
            })
        }
        s => s.parse().expect("a topic offset"),
    };
    let topic = help.topic(offset).expect("a topic");
    println!(
        "== {} ({:#x}) back {:?} fwd {:?}",
        topic.title, topic.offset, topic.browse_back, topic.browse_forward
    );
    for (region, blocks) in [("band", &topic.band), ("body", &topic.body)] {
        println!("-- {region}");
        for block in blocks {
            match block {
                Block::Text(paragraphs) => {
                    for p in paragraphs {
                        print!(
                            "[{:?} l{} f{} a{} b{}] ",
                            p.style.align,
                            p.style.left,
                            p.style.first,
                            p.style.above,
                            p.style.below
                        );
                        for run in &p.runs {
                            match run {
                                Run::Text { font, text, jump } => match jump {
                                    Some(Jump::Topic(t)) => print!("{{f{font} >{t:#x} {text:?}}}"),
                                    Some(Jump::Popup(t)) => print!("{{f{font} ^{t:#x} {text:?}}}"),
                                    Some(j) => print!("{{f{font} {j:?} {text:?}}}"),
                                    None => print!("{{f{font} {text:?}}}"),
                                },
                                Run::Picture { number, placement } => {
                                    print!("{{bm{number} {placement:?}}}")
                                }
                                Run::Tab => print!("{{tab}}"),
                                Run::Break => print!("{{br}}"),
                            }
                        }
                        println!();
                    }
                }
                Block::Table(table) => {
                    println!("table {:?}", table.columns);
                    for row in &table.rows {
                        for cell in row {
                            println!("  col {}: {:?}", cell.column, cell.plain());
                        }
                        println!("  --");
                    }
                }
            }
        }
    }
}
