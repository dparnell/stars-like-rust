//! Watch the tutorial play itself.
//!
//! The tutorial's walkthrough (`stars_ui::autopilot::script::tutorial`)
//! is run on a thread, one press at a time at a human pace, on a headless
//! shell whose every frame is rasterised and shown here as it happens —
//! the same run `cargo test -p stars-ui --test tutorial_ui` makes and the
//! film `STARS_TUTORIAL_VIDEO` records, but in a window, with nothing to
//! click.
//!
//! ```sh
//! cargo run --release -p stars-desktop --example autoplay_tutorial
//! STARS_AUTOPLAY_DELAY_MS=400 cargo run --release -p stars-desktop --example autoplay_tutorial
//! ```
//!
//! The game's own pictures and text are used when a copy of the original
//! is in `binary/`. The run stops where the walkthrough stops (see
//! `docs/ui/tutorial.md`, *Where the run stops*) and says so.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use stars_ui::autopilot::{script, Raster, Shell, Sink};

/// What the thread playing the tutorial hands the window.
#[derive(Default)]
struct Shared {
    /// The latest frame, when it has changed since the window last took it.
    image: Option<egui::ColorImage>,
    /// The last step the script took.
    note: String,
    /// Frames drawn so far.
    frames: usize,
    /// How the run ended, once it has.
    ended: Option<String>,
}

/// The sink: rasterise each frame into the shared image, and wait at
/// every step so the run reads at a human pace.
struct Live {
    shared: Arc<Mutex<Shared>>,
    raster: Raster,
    delay: Duration,
}

impl Sink for Live {
    fn frame(
        &mut self,
        delta: &egui::TexturesDelta,
        primitives: &[egui::ClippedPrimitive],
        page_turned: bool,
    ) {
        self.raster.draw(delta, primitives);
        let image = egui::ColorImage::from_rgba_premultiplied(
            [self.raster.width, self.raster.height],
            &self.raster.pixels,
        );
        {
            let mut shared = self.shared.lock().expect("the shared frame");
            shared.image = Some(image);
            shared.frames += 1;
        }
        if page_turned {
            // A new page: time to read it.
            std::thread::sleep(self.delay * 3);
        }
    }

    fn step(&mut self, what: &str) {
        {
            let mut shared = self.shared.lock().expect("the shared frame");
            shared.note = what.to_string();
        }
        std::thread::sleep(self.delay);
    }
}

/// The window: the latest frame, scaled to fit, and a line about the run.
struct Watcher {
    shared: Arc<Mutex<Shared>>,
    texture: Option<egui::TextureHandle>,
    note: String,
    frames: usize,
    ended: Option<String>,
}

impl eframe::App for Watcher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        {
            let mut shared = self.shared.lock().expect("the shared frame");
            if let Some(image) = shared.image.take() {
                match self.texture.as_mut() {
                    Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                    None => {
                        self.texture =
                            Some(ctx.load_texture("tutorial", image, egui::TextureOptions::LINEAR));
                    }
                }
            }
            self.note.clone_from(&shared.note);
            self.frames = shared.frames;
            self.ended.clone_from(&shared.ended);
        }
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                match &self.ended {
                    Some(how) => ui.label(how),
                    None => ui.label(format!("playing — {} (frame {})", self.note, self.frames)),
                };
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(0x1b, 0x1b, 0x1b)))
            .show(ctx, |ui| {
                let Some(texture) = self.texture.as_ref() else {
                    ui.centered_and_justified(|ui| ui.label("setting the tutorial up…"));
                    return;
                };
                let avail = ui.available_size();
                let size = texture.size_vec2();
                let scale = (avail.x / size.x).min(avail.y / size.y).max(0.05);
                let shown = size * scale;
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Image::new((texture.id(), shown)));
                });
            });
        ctx.request_repaint_after(Duration::from_millis(40));
    }
}

fn main() -> eframe::Result<()> {
    let delay = std::env::var("STARS_AUTOPLAY_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map_or(Duration::from_millis(800), Duration::from_millis);
    let shared = Arc::new(Mutex::new(Shared::default()));

    let player = Arc::clone(&shared);
    std::thread::spawn(move || {
        let sink = Live {
            shared: Arc::clone(&player),
            raster: Raster::new(1920, 1080),
            delay,
        };
        let outcome = std::panic::catch_unwind(move || {
            let mut shell = Shell::with_sink(script::tutorial_app(), Some(Box::new(sink)));
            script::tutorial(&mut shell);
            shell
                .app
                .tutor
                .as_ref()
                .map(stars_ui::tutorial::Tutor::page)
        });
        let ended = match outcome {
            Ok(page) => format!(
                "the walkthrough is done — it stops on page {} (docs/ui/tutorial.md, Where the run stops)",
                page.unwrap_or(0)
            ),
            Err(e) => {
                let why = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_else(|| "a panic".to_string());
                format!("the walkthrough stopped: {why}")
            }
        };
        player.lock().expect("the shared frame").ended = Some(ended);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 760.0])
            .with_title("Stars! tutorial, playing itself"),
        ..Default::default()
    };
    eframe::run_native(
        "Stars! tutorial",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Watcher {
                shared,
                texture: None,
                note: String::new(),
                frames: 0,
                ended: None,
            }))
        }),
    )
}
