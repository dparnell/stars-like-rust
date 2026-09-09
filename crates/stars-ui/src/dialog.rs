//! Dialog templates read out of the game's own resources.
//!
//! The original's dialogs are Windows `DIALOG` resources, so their layout is
//! data rather than code: a size and a list of controls, each with a class, a
//! position and a caption, all in **dialog units**. Reproducing one properly
//! means laying the controls out where the template puts them rather than
//! stacking them in whatever order they are written.
//!
//! A dialog unit is a quarter of the font's average character width across and
//! an eighth of its height down, so the same template comes out at different
//! pixel sizes under different fonts. Here the whole dialog is scaled to the
//! room it is given, which keeps the proportions the template asks for.

/// What one horizontal dialog unit is worth in pixels, for MS Sans Serif 8pt
/// at 96 dpi — the font every one of these dialogs asks for.
pub const DLU_X: f32 = 1.5;
/// And one vertical unit: the font's height is thirteen, over eight.
pub const DLU_Y: f32 = 13.0 / 8.0;

/// What a control is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A push button, a checkbox or a radio button, by its style.
    Button,
    /// A list box.
    ListBox,
    /// A label.
    Static,
    /// A combo box.
    ComboBox,
    /// A text field.
    Edit,
}

/// One control of a dialog template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// The control id `WM_COMMAND` names it by.
    pub id: u16,
    /// What it is.
    pub class: Class,
    /// Where it goes and how big it is, in dialog units.
    pub at: (i16, i16, i16, i16),
    /// Its caption, `&` and all — the ampersand marks the accelerator.
    pub text: &'static str,
}

impl Control {
    /// The caption with the accelerator marker taken out.
    #[must_use]
    pub fn label(&self) -> String {
        self.text.replace('&', "")
    }
}

/// Where one control goes, given the dialog's own origin and scale.
#[must_use]
pub fn place(origin: egui::Pos2, scale: f32, at: (i16, i16, i16, i16)) -> egui::Rect {
    let (x, y, w, h) = at;
    egui::Rect::from_min_size(
        origin + egui::vec2(f32::from(x) * DLU_X * scale, f32::from(y) * DLU_Y * scale),
        egui::vec2(f32::from(w) * DLU_X * scale, f32::from(h) * DLU_Y * scale),
    )
}

/// A dialog template.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    /// The title bar's text.
    pub caption: &'static str,
    /// How big the dialog is in dialog units.
    pub size: (i16, i16),
    /// Its controls, in the template's own order.
    pub controls: &'static [Control],
}

impl Template {
    /// Where a control sits, scaled into `rect`.
    ///
    /// The whole dialog is scaled by the smaller of the two ratios so the
    /// template's proportions survive being squeezed.
    #[must_use]
    pub fn place(&self, rect: egui::Rect, control: &Control) -> egui::Rect {
        let scale = self.scale(rect);
        let (x, y, w, h) = control.at;
        egui::Rect::from_min_size(
            rect.min + egui::vec2(f32::from(x) * DLU_X * scale, f32::from(y) * DLU_Y * scale),
            egui::vec2(f32::from(w) * DLU_X * scale, f32::from(h) * DLU_Y * scale),
        )
    }

    /// How much the template has to be squeezed to fit `rect`.
    #[must_use]
    pub fn scale(&self, rect: egui::Rect) -> f32 {
        let want = self.pixels();
        (rect.width() / want.x)
            .min(rect.height() / want.y)
            .clamp(0.4, 1.0)
    }

    /// How big the dialog wants to be, in pixels.
    #[must_use]
    pub fn pixels(&self) -> egui::Vec2 {
        egui::vec2(
            f32::from(self.size.0) * DLU_X,
            f32::from(self.size.1) * DLU_Y,
        )
    }

    /// One control, by id.
    #[must_use]
    pub fn control(&self, id: u16) -> Option<&'static Control> {
        self.controls.iter().find(|control| control.id == id)
    }
}

/// Where the buttons in the production dialog's middle column go, once the
/// two overlaps in the shipped resource are opened out.
///
/// The template really does place `Item Up` at `y = 0` and `Add ->` at
/// `y = 10`, and `Clear` at `y = 60` and `Help` at `y = 70`, with every button
/// fourteen units tall — so each pair overlaps by four. That is what is in the
/// file (resource 93, thirteen controls, no extra data on any of them), and it
/// is reproduced in [`PRODUCTION`] unchanged. This is the same column with
/// each overlap pushed apart, which is what gets drawn, because two buttons
/// on top of each other would leave one of them unclickable.
#[must_use]
pub fn unoverlapped(template: &Template) -> Vec<(u16, (i16, i16, i16, i16))> {
    let mut out: Vec<(u16, (i16, i16, i16, i16))> = template
        .controls
        .iter()
        .map(|control| (control.id, control.at))
        .collect();
    // Only the middle column, and only downwards, so the row along the foot
    // and the two lists stay exactly where the template puts them.
    let mut column: Vec<usize> = (0..out.len()).filter(|&i| out[i].1 .0 == 125).collect();
    column.sort_by_key(|&i| out[i].1 .1);
    for pair in column.windows(2) {
        let (above, below) = (pair[0], pair[1]);
        let bottom = out[above].1 .1 + out[above].1 .3;
        if out[below].1 .1 < bottom {
            out[below].1 .1 = bottom;
        }
    }
    out
}

/// `Planet Production`, dialog resource 93.
///
/// Thirteen controls: the inventory and the queue side by side, the buttons
/// that move items between them in a column down the middle, and a row along
/// the foot. Everything between the lists and that row is drawn by
/// `DrawProductionDlg` (`10d0:35dc`) rather than being controls — the cost
/// panel under each list, and the blue diamond.
pub const PRODUCTION: Template = Template {
    caption: "Planet Production",
    size: (294, 191),
    controls: &[
        Control {
            id: 0x416,
            class: Class::ListBox,
            at: (8, 12, 109, 84),
            text: "",
        },
        Control {
            id: 0x417,
            class: Class::ListBox,
            at: (174, 12, 111, 84),
            text: "",
        },
        Control {
            id: 0x439,
            class: Class::Button,
            at: (125, 0, 40, 14),
            text: "Item &Up",
        },
        Control {
            id: 0x418,
            class: Class::Button,
            at: (125, 10, 40, 14),
            text: "&Add ->",
        },
        Control {
            id: 0x419,
            class: Class::Button,
            at: (125, 35, 40, 14),
            text: "<- &Remove",
        },
        Control {
            id: 0x42d,
            class: Class::Button,
            at: (125, 60, 40, 14),
            text: "&Clear",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (125, 70, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x43a,
            class: Class::Button,
            at: (125, 90, 40, 14),
            text: "Item &Down",
        },
        Control {
            id: 0x8b,
            class: Class::Button,
            at: (6, 168, 90, 14),
            text: "Contribute only &leftover resources to research",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (100, 168, 40, 14),
            text: "&Prev",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (147, 168, 40, 14),
            text: "&Next",
        },
        Control {
            id: 0x1,
            class: Class::Button,
            at: (198, 168, 40, 14),
            text: "OK",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (245, 168, 40, 14),
            text: "Cancel",
        },
    ],
};

/// `Required Minerals:` — the heading of the panel `DrawProductionDlg` draws
/// under each list.
pub const REQUIRED_MINERALS: &str = "Required Minerals:";

/// The four rows of that panel, which are the first three of `rgpszMin` and
/// then its **sixth**: the three minerals and then resources.
pub const COST_ROWS: [(&str, [u8; 3]); 4] = [
    ("Ironium", [0x00, 0x00, 0xff]),
    ("Boranium", [0x00, 0x7f, 0x00]),
    ("Germanium", [0xff, 0xff, 0x00]),
    // `rgcrMin[5]` is black, and the row carries no `kT` after it.
    ("Resources", [0x00, 0x00, 0x00]),
];

/// What follows the first three rows' figures (`idsKt`).
pub const KT: &str = "kT";

/// The completion line under the queue's own cost panel
/// (`idsDDoneCompletion`), with `PszProductionETA`'s wording after it.
pub const COMPLETION: &str = "% Done,   Completion ";

/// What is written beside the blue diamond
/// (`idsApplyDefineProductionTemplate`).
pub const TEMPLATE_HINT: &str = "Apply or define a production template";

/// The diamond's own colour, `hbrBBlue`.
pub const DIAMOND: [u8; 3] = [0x00, 0x00, 0xff];

/// Where the diamond goes, given the dialog's client rectangle and a line of
/// its font.
///
/// `DrawProductionDlg` puts it `dyArial8 * 5 / 2 + 12` up from the bottom, six
/// pixels in, `dyArial8` wide and `dyArial8 | 1` tall — odd, so it has a
/// middle row to be pointed at.
#[must_use]
pub fn diamond(rect: egui::Rect, line: f32) -> egui::Rect {
    #[allow(clippy::cast_possible_truncation)]
    let odd = ((line as i32) | 1) as f32;
    let top = rect.bottom() - (line * 2.5 + 12.0);
    egui::Rect::from_min_size(egui::pos2(rect.left() + 6.0, top), egui::vec2(line, odd))
}

/// `Ship & Starbase Designer`, dialog resource 92 (`0x5c`).
///
/// One window with two faces. `ShowMainControls` (`10c8:0160`) swaps them by
/// hiding or showing nine of these fifteen controls — the two Design radios,
/// the four View radios and the three buttons — and doing two more things that
/// read backwards at first glance: **OK is hidden in the browser and shown in
/// the editor**, and the button beside it is relabelled `Done` for the browser
/// and `Cancel` for the editor. So the browser's only way out is the button
/// the template calls `Cancel`.
///
/// The template places the dialog at 11, 52 rather than centring it, and its
/// parts list runs ten units past the bottom of the dialog it is in — 90 plus
/// 170 against a height of 250. That is what the resource says.
pub const DESIGNER: Template = Template {
    caption: "Ship & Starbase Designer",
    size: (351, 250),
    controls: &[
        Control {
            id: 0x810,
            class: Class::Button,
            at: (14, 8, 91, 13),
            text: "Ships",
        },
        Control {
            id: 0x811,
            class: Class::Button,
            at: (14, 21, 91, 13),
            text: "Starbases",
        },
        Control {
            id: 0x812,
            class: Class::Button,
            at: (14, 48, 91, 13),
            text: "Existing Designs",
        },
        Control {
            id: 0x813,
            class: Class::Button,
            at: (14, 62, 91, 13),
            text: "Available Hull Types",
        },
        Control {
            id: 0x814,
            class: Class::Button,
            at: (14, 76, 91, 13),
            text: "Enemy Hulls",
        },
        Control {
            id: 0x815,
            class: Class::Button,
            at: (14, 90, 91, 13),
            text: "Components",
        },
        Control {
            id: 0x816,
            class: Class::Button,
            at: (13, 114, 86, 16),
            text: "&Copy Selected Design",
        },
        Control {
            id: 0x817,
            class: Class::Button,
            at: (13, 134, 86, 16),
            text: "&Delete Selected Design",
        },
        Control {
            id: 0x818,
            class: Class::Button,
            at: (13, 154, 86, 16),
            text: "&Edit Selected Design",
        },
        Control {
            id: 0x81a,
            class: Class::ComboBox,
            at: (180, 20, 142, 78),
            text: "",
        },
        Control {
            id: 0x81b,
            class: Class::Edit,
            at: (186, 52, 132, 15),
            text: "",
        },
        Control {
            id: 0x80c,
            class: Class::ListBox,
            at: (114, 90, 135, 170),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (281, 134, 33, 13),
            text: "&Help",
        },
        Control {
            id: 0x1,
            class: Class::Button,
            at: (281, 222, 33, 13),
            text: "OK",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (281, 238, 33, 13),
            text: "Cancel",
        },
    ],
};

/// What the designer's second button reads in each face (`idsDone` and
/// `idsCancel`).
pub const DESIGNER_CLOSE: (&str, &str) = ("Done", "Cancel");

/// Which of the designer's controls `ShowMainControls` hides for the editor
/// and shows for the browser.
pub const DESIGNER_BROWSER_ONLY: [u16; 9] = [
    0x816, 0x818, 0x817, 0x810, 0x811, 0x812, 0x813, 0x814, 0x815,
];

/// Where a hull's schematic grid starts, and where the plaque under it goes.
///
/// `UpdateSlotGlobals` (`10c8:6528`) works in half-cells of 32 pixels — a slot
/// is two of them square — and puts the origin in one of two places depending
/// on which window is asking:
///
/// * the **designer** (`hwndSlotDlg != 0`): `ptslotGlob.x - 0x14a` across and
///   32 down;
/// * the `grPopupShdef` **pop-up**: 12 across and `dyArial8 + 12` down.
///
/// The plaque — the `n of m` under the picture — is a fixed offset from
/// whichever origin was used, and the hull's cargo bay comes out of
/// `HULDEF.wrcCargo`: its high byte is the top-left cell and its low byte the
/// bottom-right, each nibble a half-cell like `rgbrc`.
pub const SLOT_CELL: f32 = 32.0;
/// How far the designer's own grid is left of `ptslotGlob.x`.
pub const SLOT_ORIGIN_BACK: f32 = 0x14a as f32;
/// How far down the designer's grid starts.
pub const SLOT_ORIGIN_TOP: f32 = 32.0;
/// Where the plaque sits from the grid's origin.
pub const PLAQUE_OFFSET: (f32, f32) = (0x102 as f32, 0x111 as f32);

// --- The race wizard ------------------------------------------------------
//
// Six dialogs, `IDD_RACE_WIZARD_1` (146) through `_6` (151), all **261 by 209**
// dialog units and all carrying the same five-button footer at `y = 190`. The
// trap the spec already records is visible here: pages 2 and 3 hold almost no
// controls, because the habitability sliders and the economy bars are painted
// in `WM_PAINT` rather than laid out.

/// The five buttons every page of the wizard carries, in the template's own
/// left-to-right order.
pub const WIZARD_FOOTER: [u16; 5] = [0x76, 0x2, 0x42e, 0x42f, 0x430];

/// Where that footer sits — every page puts it at the same `y`.
pub const WIZARD_FOOTER_Y: i16 = 190;
/// `IDD_RACE_WIZARD_1` (resource 146) — the Race page.
///
/// 21 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_1: Template = Template {
    caption: "Race",
    size: (261, 209),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 7, 68, 10),
            text: "Race Name:",
        },
        Control {
            id: 0x10c,
            class: Class::Edit,
            at: (76, 6, 90, 12),
            text: "",
        },
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 23, 68, 10),
            text: "Plural Race Name:",
        },
        Control {
            id: 0x81b,
            class: Class::Edit,
            at: (76, 22, 90, 12),
            text: "",
        },
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 39, 68, 10),
            text: "Password:",
        },
        Control {
            id: 0x10d,
            class: Class::Edit,
            at: (76, 38, 60, 12),
            text: "",
        },
        Control {
            id: 0x10f,
            class: Class::Button,
            at: (26, 64, 70, 14),
            text: "Humanoid",
        },
        Control {
            id: 0x110,
            class: Class::Button,
            at: (26, 80, 70, 14),
            text: "Rabbitoid",
        },
        Control {
            id: 0x111,
            class: Class::Button,
            at: (26, 96, 70, 14),
            text: "Insectoid",
        },
        Control {
            id: 0x112,
            class: Class::Button,
            at: (26, 112, 70, 14),
            text: "Nucleotid",
        },
        Control {
            id: 0x113,
            class: Class::Button,
            at: (112, 64, 70, 14),
            text: "Silicanoid",
        },
        Control {
            id: 0x114,
            class: Class::Button,
            at: (112, 80, 70, 14),
            text: "Antetheral",
        },
        Control {
            id: 0x115,
            class: Class::Button,
            at: (112, 96, 70, 14),
            text: "Random",
        },
        Control {
            id: 0x116,
            class: Class::Button,
            at: (112, 112, 70, 14),
            text: "Custom",
        },
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (26, 144, 170, 12),
            text: "Spend up to 50 leftover advantage points on:",
        },
        Control {
            id: 0x81a,
            class: Class::ComboBox,
            at: (26, 156, 156, 74),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_2` (resource 147) — the Habitability page.
///
/// 8 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_2: Template = Template {
    caption: "Habitability",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x123,
            class: Class::Button,
            at: (86, 110, 70, 12),
            text: "Immune to &Gravity",
        },
        Control {
            id: 0x124,
            class: Class::Button,
            at: (86, 110, 70, 12),
            text: "Immune to &Temperature",
        },
        Control {
            id: 0x125,
            class: Class::Button,
            at: (86, 110, 70, 12),
            text: "Immune to &Radiation",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_3` (resource 148) — the Economy page.
///
/// 6 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_3: Template = Template {
    caption: "Economy",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x123,
            class: Class::Button,
            at: (135, 126, 115, 14),
            text: "Factories cost 1kT less of Germanium to build",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_4` (resource 149) — the Primary Racial Trait page.
///
/// 15 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_4: Template = Template {
    caption: "Primary Racial Trait",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x10f,
            class: Class::Button,
            at: (16, 36, 84, 10),
            text: "Hyper-Expansion",
        },
        Control {
            id: 0x110,
            class: Class::Button,
            at: (16, 48, 84, 10),
            text: "Super Stealth",
        },
        Control {
            id: 0x111,
            class: Class::Button,
            at: (16, 60, 84, 10),
            text: "War Monger",
        },
        Control {
            id: 0x112,
            class: Class::Button,
            at: (16, 72, 84, 10),
            text: "Claim Adjuster",
        },
        Control {
            id: 0x113,
            class: Class::Button,
            at: (16, 84, 84, 10),
            text: "Inner-Strength",
        },
        Control {
            id: 0x114,
            class: Class::Button,
            at: (136, 36, 84, 10),
            text: "Space Demolition",
        },
        Control {
            id: 0x115,
            class: Class::Button,
            at: (136, 48, 84, 10),
            text: "Packet Physics",
        },
        Control {
            id: 0x116,
            class: Class::Button,
            at: (136, 60, 84, 10),
            text: "Interstellar Traveler",
        },
        Control {
            id: 0x117,
            class: Class::Button,
            at: (136, 72, 84, 10),
            text: "Alternate Reality",
        },
        Control {
            id: 0x118,
            class: Class::Button,
            at: (136, 84, 84, 10),
            text: "Jack of All Trades",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_5` (resource 150) — the Lesser Racial Traits page.
///
/// 20 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_5: Template = Template {
    caption: "Lesser Racial Traits",
    size: (261, 209),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (80, 14, 80, 12),
            text: "Lesser Racial Traits",
        },
        Control {
            id: 0x123,
            class: Class::Button,
            at: (10, 26, 115, 14),
            text: "",
        },
        Control {
            id: 0x124,
            class: Class::Button,
            at: (10, 40, 115, 14),
            text: "",
        },
        Control {
            id: 0x125,
            class: Class::Button,
            at: (10, 54, 115, 14),
            text: "",
        },
        Control {
            id: 0x126,
            class: Class::Button,
            at: (10, 68, 115, 14),
            text: "",
        },
        Control {
            id: 0x127,
            class: Class::Button,
            at: (10, 82, 115, 14),
            text: "",
        },
        Control {
            id: 0x128,
            class: Class::Button,
            at: (10, 96, 115, 14),
            text: "",
        },
        Control {
            id: 0x129,
            class: Class::Button,
            at: (10, 110, 115, 14),
            text: "",
        },
        Control {
            id: 0x12a,
            class: Class::Button,
            at: (135, 26, 115, 14),
            text: "",
        },
        Control {
            id: 0x12b,
            class: Class::Button,
            at: (135, 40, 115, 14),
            text: "",
        },
        Control {
            id: 0x12c,
            class: Class::Button,
            at: (135, 54, 115, 14),
            text: "",
        },
        Control {
            id: 0x12d,
            class: Class::Button,
            at: (135, 68, 115, 14),
            text: "",
        },
        Control {
            id: 0x12e,
            class: Class::Button,
            at: (135, 82, 115, 14),
            text: "",
        },
        Control {
            id: 0x12f,
            class: Class::Button,
            at: (135, 96, 115, 14),
            text: "",
        },
        Control {
            id: 0x130,
            class: Class::Button,
            at: (135, 110, 115, 14),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// `IDD_RACE_WIZARD_6` (resource 151) — the Research Costs page.
///
/// 24 controls in 261 by 209 dialog units, the five-button footer
/// among them.
pub const RACE_WIZARD_6: Template = Template {
    caption: "Research Costs",
    size: (261, 209),
    controls: &[
        Control {
            id: 0x10f,
            class: Class::Button,
            at: (16, 36, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x110,
            class: Class::Button,
            at: (16, 48, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x111,
            class: Class::Button,
            at: (16, 60, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x112,
            class: Class::Button,
            at: (16, 84, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x113,
            class: Class::Button,
            at: (16, 96, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x114,
            class: Class::Button,
            at: (16, 108, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x115,
            class: Class::Button,
            at: (16, 132, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x116,
            class: Class::Button,
            at: (16, 144, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x117,
            class: Class::Button,
            at: (16, 156, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x118,
            class: Class::Button,
            at: (140, 36, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x119,
            class: Class::Button,
            at: (140, 48, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x11a,
            class: Class::Button,
            at: (140, 60, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x11b,
            class: Class::Button,
            at: (140, 84, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x11c,
            class: Class::Button,
            at: (140, 96, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x11d,
            class: Class::Button,
            at: (140, 108, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x11e,
            class: Class::Button,
            at: (140, 132, 90, 10),
            text: "Costs 75% extra",
        },
        Control {
            id: 0x11f,
            class: Class::Button,
            at: (140, 144, 90, 10),
            text: "Costs standard amount",
        },
        Control {
            id: 0x120,
            class: Class::Button,
            at: (140, 156, 90, 10),
            text: "Costs 50% less",
        },
        Control {
            id: 0x123,
            class: Class::Button,
            at: (12, 174, 210, 14),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// The wizard's six pages in order.
pub const RACE_WIZARD: [&Template; 6] = [
    &RACE_WIZARD_1,
    &RACE_WIZARD_2,
    &RACE_WIZARD_3,
    &RACE_WIZARD_4,
    &RACE_WIZARD_5,
    &RACE_WIZARD_6,
];

/// The wizard's caption, string `0x010e`: `"Custom Race Wizard - Step %d of 6"`.
pub const WIZARD_CAPTION: &str = "Custom Race Wizard - Step {} of 6";

// --- Game Parameters ------------------------------------------------------
//
// The **Advanced Game** wizard, which View (Game Parameters) shows read-only:
// three dialogs, 390, 391 and 392, all 261 by 210 dialog units and carrying
// the same five-button footer as the race wizard, at the same `y = 190`.
//
// Page 2 holds nothing but that footer — the player list is built at run time —
// and page 3 holds only its seven **checkboxes**, twelve units square: the
// condition each stands for and the control that sets its value are made
// alongside. That is why the templates alone describe an almost empty dialog,
// and it is the same trap the race wizard's pages 2 and 3 set.
/// Game Parameters page 1 (resource 390) — Universe.
///
/// 27 controls in 261 by 210 dialog units, the five-button footer
/// among them.
pub const GAME_PARAMS_1: Template = Template {
    caption: "Universe",
    size: (261, 210),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (10, 10, 48, 10),
            text: "&Game Name:",
        },
        Control {
            id: 0x406,
            class: Class::Edit,
            at: (64, 8, 150, 12),
            text: "",
        },
        Control {
            id: 0x3e8,
            class: Class::Button,
            at: (24, 40, 44, 12),
            text: "Tiny",
        },
        Control {
            id: 0x3e9,
            class: Class::Button,
            at: (24, 52, 44, 12),
            text: "Small",
        },
        Control {
            id: 0x3ea,
            class: Class::Button,
            at: (24, 64, 44, 12),
            text: "Medium",
        },
        Control {
            id: 0x3eb,
            class: Class::Button,
            at: (24, 76, 44, 12),
            text: "Large",
        },
        Control {
            id: 0x3ec,
            class: Class::Button,
            at: (24, 88, 44, 12),
            text: "Huge",
        },
        Control {
            id: 0x3ed,
            class: Class::Button,
            at: (24, 124, 44, 12),
            text: "Sparse",
        },
        Control {
            id: 0x3ee,
            class: Class::Button,
            at: (24, 136, 44, 12),
            text: "Normal",
        },
        Control {
            id: 0x3ef,
            class: Class::Button,
            at: (24, 148, 44, 12),
            text: "Dense",
        },
        Control {
            id: 0x3f0,
            class: Class::Button,
            at: (24, 160, 44, 12),
            text: "Packed",
        },
        Control {
            id: 0x3f1,
            class: Class::Button,
            at: (96, 40, 48, 12),
            text: "Close",
        },
        Control {
            id: 0x3f2,
            class: Class::Button,
            at: (96, 52, 48, 12),
            text: "Moderate",
        },
        Control {
            id: 0x3f3,
            class: Class::Button,
            at: (96, 64, 48, 12),
            text: "Farther",
        },
        Control {
            id: 0x3f4,
            class: Class::Button,
            at: (96, 76, 48, 12),
            text: "Distant",
        },
        Control {
            id: 0x3f8,
            class: Class::Button,
            at: (90, 94, 140, 12),
            text: "Beginner:  &Maximum Minerals",
        },
        Control {
            id: 0x3f9,
            class: Class::Button,
            at: (90, 107, 140, 12),
            text: "&Slower Tech Advances",
        },
        Control {
            id: 0x3fa,
            class: Class::Button,
            at: (90, 120, 140, 12),
            text: "&Accelerated BBS Play",
        },
        Control {
            id: 0x3fb,
            class: Class::Button,
            at: (90, 133, 140, 12),
            text: "No &Random Events",
        },
        Control {
            id: 0x3fc,
            class: Class::Button,
            at: (90, 146, 140, 12),
            text: "&Computer Players Form Alliances",
        },
        Control {
            id: 0x3fd,
            class: Class::Button,
            at: (90, 159, 140, 12),
            text: "&Public Player Scores",
        },
        Control {
            id: 0x41a,
            class: Class::Button,
            at: (90, 172, 140, 12),
            text: "&Galaxy Clumping",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// Game Parameters page 2 (resource 391) — Players.
///
/// 5 controls in 261 by 210 dialog units, the five-button footer
/// among them.
pub const GAME_PARAMS_2: Template = Template {
    caption: "Players",
    size: (261, 210),
    controls: &[
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
    ],
};

/// Game Parameters page 3 (resource 392) — Victory Conditions.
///
/// 13 controls in 261 by 210 dialog units, the five-button footer
/// among them.
pub const GAME_PARAMS_3: Template = Template {
    caption: "Victory Conditions",
    size: (261, 210),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (22, 10, 200, 12),
            text: "Victory is declared when a player:",
        },
        Control {
            id: 0x123,
            class: Class::Button,
            at: (10, 26, 12, 12),
            text: "",
        },
        Control {
            id: 0x124,
            class: Class::Button,
            at: (10, 42, 12, 12),
            text: "",
        },
        Control {
            id: 0x125,
            class: Class::Button,
            at: (10, 58, 12, 12),
            text: "",
        },
        Control {
            id: 0x126,
            class: Class::Button,
            at: (10, 74, 12, 12),
            text: "",
        },
        Control {
            id: 0x127,
            class: Class::Button,
            at: (10, 90, 12, 12),
            text: "",
        },
        Control {
            id: 0x128,
            class: Class::Button,
            at: (10, 106, 12, 12),
            text: "",
        },
        Control {
            id: 0x129,
            class: Class::Button,
            at: (10, 122, 12, 12),
            text: "",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (10, 190, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (60, 190, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (110, 190, 40, 14),
            text: "< &Back",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (160, 190, 40, 14),
            text: "&Next >",
        },
        Control {
            id: 0x430,
            class: Class::Button,
            at: (210, 190, 40, 14),
            text: "&Finish",
        },
    ],
};

/// The Game Parameters wizard's three pages in order.
pub const GAME_PARAMS: [&Template; 3] = [&GAME_PARAMS_1, &GAME_PARAMS_2, &GAME_PARAMS_3];

/// What page 3's heading says (`Victory is declared when a player:`), which
/// `MANUAL.PDF` p. 2-3 sends the player to page 3 to read.
pub const VICTORY_HEADING: &str = "Victory is declared when a player:";

/// `Technology Browser`, dialog resource 128.
///
/// Five controls and nothing else: Prev, the category dropdown and Next in a
/// row along the top, and the filter checkbox and Close along the foot.
/// Everything between them is a **child window** of the class `starsbrowser`
/// (`DS:0x21c`) that `DisplayComponentInfo` (`10d8:2ac6`) paints, created by
/// `BrowserDlg` at [`BROWSER_PANEL`] rather than placed by the template.
///
/// The dialog takes its size from the resource — 349 by 247 — and nothing
/// resizes it afterwards.
pub const BROWSER: Template = Template {
    caption: "Technology Browser",
    size: (349, 247),
    controls: &[
        Control {
            id: 0x42e,
            class: Class::Button,
            at: (7, 7, 50, 14),
            text: "<- Prev",
        },
        Control {
            id: 0x10b,
            class: Class::ComboBox,
            at: (68, 7, 83, 12),
            text: "",
        },
        Control {
            id: 0x42f,
            class: Class::Button,
            at: (158, 7, 50, 14),
            text: "Next ->",
        },
        Control {
            id: 0x10a,
            class: Class::Button,
            at: (6, 232, 130, 11),
            text: "Show Only Available Technology",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (157, 230, 50, 14),
            text: "Close",
        },
    ],
};

/// Where the browser's panel child goes, in **pixels** — `BrowserDlg`
/// (`10d8:21ce`) creates it rather than the template placing it, so this is
/// not in dialog units:
///
/// ```text
/// x      = 6
/// y      = dyArial8 * 3 / 2 + 12
/// width  = 0x158, and 0x28 wider again when dyArial8 > 14
/// height = dyArial8 * 12 + dyArial10 + 0x4e
/// ```
///
/// So the panel is sized from the **font**, not from the widest category name.
/// The global the height adds is `dyArial10` (`DS:0x530a`), which is how the
/// same size turns up again as the `grPopupComponent` pop-up's — the two are
/// the same panel, painted by the same routine. See
/// [`crate::popup::component_size`].
#[must_use]
pub fn browser_panel(line: f32, line10: f32) -> (egui::Pos2, egui::Vec2) {
    let (width, height) = crate::popup::component_size(line, line10);
    (
        egui::pos2(6.0, line * 1.5 + 12.0),
        egui::vec2(width, height),
    )
}

/// `Battle VCR`, dialog resource 160.
///
/// Seven buttons in a row along the foot, each 32 by 13 at `y = 244`, and
/// nothing else: the whole 260 by 244 above them is the board and the two
/// token panels, painted by `DrawVCR` (`10e8:1c62`).
///
/// The five transport buttons are `0xa1`…`0xa5` and carry icons at run time —
/// `rghiconVCR`, seven of them loaded in `FCreateStuff` — so the captions here
/// are what shows without a copy of the game to read them out of. They are the
/// resource's own text either way.
pub const BATTLE_VCR: Template = Template {
    caption: "Battle VCR",
    size: (260, 270),
    controls: &[
        Control {
            id: 0xa1,
            class: Class::Button,
            at: (4, 244, 32, 13),
            text: "|<<",
        },
        Control {
            id: 0xa2,
            class: Class::Button,
            at: (39, 244, 32, 13),
            text: "<",
        },
        Control {
            id: 0xa3,
            class: Class::Button,
            at: (76, 244, 32, 13),
            text: ">/||",
        },
        Control {
            id: 0xa4,
            class: Class::Button,
            at: (114, 244, 32, 13),
            text: ">",
        },
        Control {
            id: 0xa5,
            class: Class::Button,
            at: (150, 244, 32, 13),
            text: ">>|",
        },
        Control {
            id: 0x1,
            class: Class::Button,
            at: (187, 244, 32, 13),
            text: "&Done",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (224, 244, 32, 13),
            text: "&Help",
        },
    ],
};

/// The five transport buttons, in the order the template has them.
pub const VCR_TRANSPORT: [u16; 5] = [0xa1, 0xa2, 0xa3, 0xa4, 0xa5];

/// `Stars! Host Mode`, dialog resource 115.
///
/// Fifteen controls down the **right**, and a large empty area on the left
/// that the template says nothing about: everything from `y = 28` down is the
/// player list, painted by `DrawHostDialog2` (`1020:6240`).
pub const HOST_MODE: Template = Template {
    caption: "Stars! Host Mode",
    size: (260, 220),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (4, 4, 24, 8),
            text: "Game:",
        },
        Control {
            id: 0x409,
            class: Class::Static,
            at: (30, 4, 120, 8),
            text: "",
        },
        Control {
            id: 0xfffe,
            class: Class::Static,
            at: (4, 14, 24, 8),
            text: "File:",
        },
        Control {
            id: 0x40a,
            class: Class::Static,
            at: (30, 14, 175, 8),
            text: "",
        },
        Control {
            id: 0xfffd,
            class: Class::Static,
            at: (190, 4, 64, 8),
            text: "Next year is:",
        },
        Control {
            id: 0x7e0,
            class: Class::Static,
            at: (200, 14, 44, 8),
            text: "",
        },
        Control {
            id: 0x407,
            class: Class::Button,
            at: (190, 54, 65, 14),
            text: "&Generate Now",
        },
        Control {
            id: 0x408,
            class: Class::Button,
            at: (190, 74, 65, 14),
            text: "&Auto Generate",
        },
        Control {
            id: 0x7df,
            class: Class::Button,
            at: (190, 94, 65, 14),
            text: "&Password...",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (190, 114, 65, 14),
            text: "&Close",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (190, 134, 65, 14),
            text: "&Help",
        },
        Control {
            id: 0xfffc,
            class: Class::Static,
            at: (190, 160, 64, 8),
            text: "Time since",
        },
        Control {
            id: 0xfffb,
            class: Class::Static,
            at: (190, 170, 64, 8),
            text: "last change:",
        },
        Control {
            id: 0x7e1,
            class: Class::Static,
            at: (190, 180, 64, 8),
            text: "",
        },
    ],
};

/// Where the host dialog's player list goes, in **pixels** —
/// `DrawHostDialog2` (`1020:6240`) paints it rather than the template placing
/// it.
///
/// The first row's top is a flat `0x30`, the rows are `dyArial8 + 4` apart, and
/// each carries a blue diamond `dyArial8 + 1` square at `x = 6`. The player's
/// number is right-aligned at a column measured from the literal `#16:` —
/// `dyArial8 + 10 + extent("#16:")` — and the sentence beside it starts four
/// pixels past that.
pub const HOST_LIST_TOP: f32 = 0x30 as f32;
/// What one row of the list adds to a line.
pub const HOST_ROW_GAP: f32 = 4.0;
/// Where the diamond sits.
pub const HOST_DIAMOND_LEFT: f32 = 6.0;
/// The sample the number column is measured from (`idsN16`).
pub const HOST_NUMBER_SAMPLE: &str = "#16:";
/// What that column adds to the sample.
pub const HOST_NUMBER_PAD: f32 = 10.0;
/// What the sentence adds past the number column.
pub const HOST_TEXT_PAD: f32 = 4.0;

/// `Change Password`, dialog resource 141.
///
/// Two `ES_PASSWORD` edits with their labels, three buttons down the right,
/// and a static across the foot that is filled in at run time — the note
/// saying **when** the new password starts to bind, which differs for a player
/// and for the host. In host mode the caption changes too, to
/// `Change Host Password` (string `0x35e`).
///
/// There is no box for the old password, and that is not an oversight: the
/// game stores a salt rather than the password, and anyone who can open the
/// file can change it.
pub const CHANGE_PASSWORD: Template = Template {
    caption: "Change Password",
    size: (199, 70),
    controls: &[
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 6, 60, 12),
            text: "New Password:",
        },
        Control {
            id: 0x10c,
            class: Class::Edit,
            at: (68, 4, 73, 14),
            text: "",
        },
        Control {
            id: 0xfffe,
            class: Class::Static,
            at: (6, 24, 60, 12),
            text: "Retype Password:",
        },
        Control {
            id: 0x10d,
            class: Class::Edit,
            at: (68, 22, 73, 14),
            text: "",
        },
        Control {
            id: 0x1,
            class: Class::Button,
            at: (155, 4, 40, 14),
            text: "OK",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (155, 22, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (155, 40, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0x7e2,
            class: Class::Static,
            at: (8, 47, 135, 23),
            text: "",
        },
    ],
};

/// `Stars!`, dialog resource 140 — the prompt that **asks** for a password.
///
/// A different dialog from the one that sets it, and a smaller one: one edit,
/// three buttons, and a static filled at run time with *Enter the password:*
/// (string `0x35f`). Its edit is limited to **fifteen** characters where the
/// other allows sixteen, which is the original's own inconsistency.
pub const PASSWORD_PROMPT: Template = Template {
    caption: "Stars!",
    size: (145, 60),
    controls: &[
        Control {
            id: 0x7e2,
            class: Class::Static,
            at: (6, 6, 83, 12),
            text: "",
        },
        Control {
            id: 0x10c,
            class: Class::Edit,
            at: (6, 22, 73, 14),
            text: "",
        },
        Control {
            id: 0x1,
            class: Class::Button,
            at: (96, 6, 40, 14),
            text: "OK",
        },
        Control {
            id: 0x2,
            class: Class::Button,
            at: (96, 24, 40, 14),
            text: "Cancel",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (96, 42, 40, 14),
            text: "&Help",
        },
    ],
};

/// The caption the Change Password dialog takes in host mode (string `0x35e`).
pub const CHANGE_HOST_PASSWORD: &str = "Change Host Password";

/// What the prompt's static says (string `0x35f`).
pub const PASSWORD_PROMPT_LABEL: &str = "Enter the password:";

// --- Player Relations -----------------------------------------------------

/// `Player Relations`, dialog resource 2008.
///
/// `RelationsDlg` (`10f0:0088`), reached from **Commands (Player Relations)**
/// or **F7**. A list of the other players, three radio buttons saying how this
/// player regards whichever is selected, and Close.
///
/// The listbox has `LBS_NOTIFY` but **not** `LBS_SORT` (style `0x50a10001`),
/// so the players come out in index order, and it is not owner-drawn, so the
/// names are plain black on white — the dialog's `WM_CTLCOLOR` hands back the
/// button-face brush for every control **except** this one, which is what
/// leaves it the only white thing on the dialog.
///
/// The three radios are stacked **Friend, Neutral, Enemy**, which is the order
/// of their `y` coordinates and not the order of their values: the handler
/// stores `wParam - 0x7d4`, so Neutral is 0, Friend 1 and Enemy 2. There is no
/// Cancel — Close commits.
pub const RELATIONS: Template = Template {
    caption: "Player Relations",
    size: (198, 80),
    controls: &[
        Control {
            id: 0x2,
            class: Class::Button,
            at: (152, 16, 40, 14),
            text: "Close",
        },
        Control {
            id: 0x76,
            class: Class::Button,
            at: (152, 36, 40, 14),
            text: "&Help",
        },
        Control {
            id: 0xffff,
            class: Class::Static,
            at: (6, 4, 64, 10),
            text: "&Player:",
        },
        Control {
            id: 0x7d3,
            class: Class::ListBox,
            at: (6, 16, 75, 65),
            text: "",
        },
        Control {
            id: 0x7d5,
            class: Class::Button,
            at: (96, 20, 42, 12),
            text: "&Friend",
        },
        Control {
            id: 0x7d4,
            class: Class::Button,
            at: (96, 38, 42, 12),
            text: "&Neutral",
        },
        Control {
            id: 0x7d6,
            class: Class::Button,
            at: (96, 56, 42, 12),
            text: "&Enemy",
        },
    ],
};

/// What the Player Relations group frame is captioned (string 904).
pub const RELATION_GROUP: &str = "Relation";

/// The frame the Player Relations dialog draws around its three radios.
///
/// It is **not** a control: `RelationsDlg`'s `WM_PAINT` (`10f0:019f`) asks
/// Windows where `&Friend` and `&Enemy` ended up, takes the first's top-left
/// and the second's bottom-right, and grows that rectangle by `dyArial8`
/// across and `dyArial8 / 2` down (`ExpandRc`, `1040:2f0c`) before drawing a
/// 3-D frame in it. `line` stands in for `dyArial8`.
#[must_use]
pub fn relation_group(friend: egui::Rect, enemy: egui::Rect, line: f32) -> egui::Rect {
    egui::Rect::from_min_max(friend.min, enemy.max).expand2(egui::vec2(line, (line / 2.0).floor()))
}

/// Where the frame's caption goes: eight pixels in from its left edge, and
/// half a line **above** its top, so the text straddles the border
/// (`TextOut` at `10f0:0284`).
#[must_use]
pub fn relation_group_caption(frame: egui::Rect, line: f32) -> egui::Pos2 {
    egui::pos2(frame.left() + 8.0, frame.top() - (line / 2.0).floor())
}
