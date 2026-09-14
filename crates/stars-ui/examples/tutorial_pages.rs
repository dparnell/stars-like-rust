//! Print the tutorial's pages, for reading while transcribing the machine.
fn main() {
    let from: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1);
    let to: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(from);
    for n in from..=to {
        println!("=== page {n}");
        if let Some(paragraphs) = stars_ui::tutorial_text::page(n) {
            for (i, p) in paragraphs.iter().enumerate() {
                println!("[{}] {}", (n - 1) * 8 + i, p);
            }
        }
    }
}
