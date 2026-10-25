//! The ResCAD NG window.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Key, Layout, Rect, RichText, Sense, Stroke,
    TextEdit, Vec2,
};

use crate::calc::{self, DividerError, Kind, Row};
use crate::colour_code;
use crate::eseries::SeriesMask;
use crate::units;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Tab {
    Nearest,
    Series,
    Parallel,
    Network,
    Divider,
    Trim,
    About,
}

impl Tab {
    const ALL: [(Tab, &'static str, Key); 7] = [
        (Tab::Nearest, "Nearest", Key::N),
        (Tab::Series, "Series", Key::S),
        (Tab::Parallel, "Parallel", Key::P),
        (Tab::Network, "Network", Key::W),
        (Tab::Divider, "Divider", Key::D),
        (Tab::Trim, "Trim", Key::T),
        (Tab::About, "About", Key::A),
    ];
}

/// What the Trim tab designs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TrimMode {
    /// A divider with a trimmer between R1 and R2.
    Divider,
    /// A variable resistor: a trimmer padded with fixed resistors.
    Resistor,
}

/// Alt+digit shortcuts for the series buttons (the original's keys, plus 8 for E48).
const SERIES_KEYS: [(SeriesMask, Key, &str); 6] = [
    (SeriesMask::E6, Key::Num6, "Alt+6"),
    (SeriesMask::E12, Key::Num1, "Alt+1"),
    (SeriesMask::E24, Key::Num4, "Alt+4"),
    (SeriesMask::E48, Key::Num8, "Alt+8"),
    (SeriesMask::E96, Key::Num9, "Alt+9"),
    (SeriesMask::E192, Key::Num2, "Alt+2"),
];

/// Everything the results depend on; results are recomputed only when this changes.
#[derive(Clone, PartialEq)]
struct Inputs {
    tab: Tab,
    value: String,
    ratio_num: String,
    ratio_den: String,
    ro_max: String,
    ro_min: String,
    mask: SeriesMask,
    max_err: f64,
    trim_mode: TrimMode,
    trim: f64,
}

type Outcome = Result<Vec<Row>, String>;

pub struct ResCadApp {
    tab: Tab,
    /// Shared by the Nearest, Series and Parallel tabs, as in the original.
    value: String,
    ratio_num: String,
    ratio_den: String,
    ro_max: String,
    ro_min: String,
    mask: SeriesMask,
    max_err: f64,
    max_err_auto: bool,
    trim_mode: TrimMode,
    /// Required adjustment range, ± % of the target.
    trim: f64,
    trim_auto: bool,
    selected: Option<usize>,
    cache: Option<(Inputs, Outcome)>,
    focus_input: bool,
    flash: Option<(String, f64)>,
}

impl Default for ResCadApp {
    fn default() -> Self {
        let mask = SeriesMask::E12;
        Self {
            tab: Tab::Nearest,
            value: "1k23".into(),
            ratio_num: "0.5".into(),
            ratio_den: "1".into(),
            ro_max: "10k".into(),
            ro_min: "1k".into(),
            mask,
            max_err: mask.default_tolerance(),
            max_err_auto: true,
            trim_mode: TrimMode::Divider,
            trim: 10.0,
            trim_auto: true,
            selected: Some(0),
            cache: None,
            focus_input: true,
            flash: None,
        }
    }
}

impl ResCadApp {
    fn inputs(&self) -> Inputs {
        Inputs {
            tab: self.tab,
            value: self.value.clone(),
            ratio_num: self.ratio_num.clone(),
            ratio_den: self.ratio_den.clone(),
            ro_max: self.ro_max.clone(),
            ro_min: self.ro_min.clone(),
            mask: self.mask,
            max_err: self.max_err,
            trim_mode: self.trim_mode,
            trim: self.trim,
        }
    }

    fn results(&mut self) -> &Outcome {
        let inputs = self.inputs();
        if self.cache.as_ref().is_none_or(|(old, _)| *old != inputs) {
            let outcome = compute(&inputs);
            let len = outcome.as_ref().map_or(0, Vec::len);
            self.selected = if len == 0 {
                None
            } else {
                Some(self.selected.unwrap_or(0).min(len - 1))
            };
            self.cache = Some((inputs, outcome));
        }
        &self.cache.as_ref().unwrap().1
    }

    fn set_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            self.tab = tab;
            self.selected = Some(0);
            self.focus_input = true;
        }
    }

    fn toggle_series(&mut self, s: SeriesMask) {
        let mut mask = self.mask;
        mask.toggle(s);
        // Never allow an empty selection: there would be nothing to search.
        if !mask.is_empty() {
            self.mask = mask;
            if self.max_err_auto {
                self.max_err = mask.default_tolerance();
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Alt+letter must not also type the letter into the focused field, so drop
        // text produced by an Alt key press.  The key event's own modifiers are
        // used because a quick press can be released within a single frame.
        // (AltGr reports Ctrl+Alt on some platforms, so leave that alone.)
        ctx.input_mut(|i| {
            let mut alt_key = false;
            i.events.retain(|e| match e {
                egui::Event::Key {
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    alt_key = modifiers.alt && !modifiers.ctrl;
                    true
                }
                egui::Event::Text(_) => !alt_key,
                _ => true,
            });
        });

        let alt = egui::Modifiers::ALT;
        for (tab, _, key) in Tab::ALL {
            if ctx.input_mut(|i| i.consume_key(alt, key)) {
                self.set_tab(tab);
            }
        }
        for (s, key, _) in SERIES_KEYS {
            if ctx.input_mut(|i| i.consume_key(alt, key)) {
                self.toggle_series(s);
            }
        }
        if ctx.input_mut(|i| i.consume_key(alt, Key::C)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Tab)) {
            let idx = Tab::ALL
                .iter()
                .position(|(t, _, _)| *t == self.tab)
                .unwrap_or(0);
            self.set_tab(Tab::ALL[(idx + 1) % Tab::ALL.len()].0);
        }

        // Up/Down move through the results while typing.
        let len = match &self.cache {
            Some((_, Ok(rows))) => rows.len(),
            _ => 0,
        };
        if len > 0 {
            let none = egui::Modifiers::NONE;
            let sel = self.selected.unwrap_or(0);
            if ctx.input_mut(|i| i.consume_key(none, Key::ArrowDown)) {
                self.selected = Some((sel + 1).min(len - 1));
            }
            if ctx.input_mut(|i| i.consume_key(none, Key::ArrowUp)) {
                self.selected = Some(sel.saturating_sub(1));
            }
            if ctx.input_mut(|i| i.consume_key(none, Key::PageDown)) {
                self.selected = Some((sel + 10).min(len - 1));
            }
            if ctx.input_mut(|i| i.consume_key(none, Key::PageUp)) {
                self.selected = Some(sel.saturating_sub(10));
            }
        }
    }

    fn copy_row(&mut self, ctx: &egui::Context, idx: usize) {
        if let Some((inputs, Ok(rows))) = &self.cache
            && let Some(row) = rows.get(idx)
        {
            ctx.copy_text(row_text(inputs, row));
            self.flash = Some(("Copied".into(), ctx.input(|i| i.time)));
        }
    }

    fn copy_all(&mut self, ctx: &egui::Context) {
        if let Some((inputs, Ok(rows))) = &self.cache {
            let mut text = columns(inputs).join("\t");
            text.push('\n');
            for row in rows {
                text.push_str(&row_text(inputs, row));
                text.push('\n');
            }
            ctx.copy_text(text);
            self.flash = Some((format!("Copied {} rows", rows.len()), ctx.input(|i| i.time)));
        }
    }

    // ---------------------------------------------------------------- panels

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, name, key) in Tab::ALL {
                let selected = self.tab == tab;
                let text = RichText::new(name).size(14.0);
                let resp = ui
                    .add(egui::Button::selectable(selected, text))
                    .on_hover_text(format!("Alt+{}", key.name()));
                if resp.clicked() {
                    self.set_tab(tab);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), theme_button);
        });
    }

    fn input_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);

        let first_id;
        match self.tab {
            Tab::Nearest | Tab::Series | Tab::Parallel | Tab::Network => {
                first_id = "value";
                self.value_field(ui);
                let hint = match self.tab {
                    Tab::Nearest => "Closest preferred values",
                    Tab::Series => "R1 + R2 in series",
                    Tab::Parallel => "R1 || R2 in parallel",
                    _ => "R1 + (R2 || R3)  or  R1 || (R2 + R3)",
                };
                ui.add_space(4.0);
                ui.label(RichText::new(hint).weak());
            }
            Tab::Divider => {
                first_id = "ratio_num";
                self.divider_fields(ui);
                ui.add_space(4.0);
                divider_diagram(ui, false);
            }
            Tab::Trim => {
                ui.horizontal(|ui| {
                    for (mode, name, tip) in [
                        (TrimMode::Divider, "Divider", "R1, trimmer, R2 in a chain, output on the wiper"),
                        (TrimMode::Resistor, "Resistor", "A trimmer padded with fixed resistors"),
                    ] {
                        let on = self.trim_mode == mode;
                        if ui
                            .add(egui::Button::selectable(on, name))
                            .on_hover_text(tip)
                            .clicked()
                            && !on
                        {
                            self.trim_mode = mode;
                            self.selected = Some(0);
                            self.focus_input = true;
                        }
                    }
                });
                ui.add_space(4.0);
                if self.trim_mode == TrimMode::Divider {
                    first_id = "ratio_num";
                    self.divider_fields(ui);
                    ui.add_space(4.0);
                    divider_diagram(ui, true);
                } else {
                    first_id = "value";
                    self.value_field(ui);
                    ui.add_space(4.0);
                    ui.label(RichText::new("Rs + P,  Rs + (P || Rp)  or  (Rs + P) || Rp").weak());
                }
            }
            Tab::About => return,
        }

        if std::mem::take(&mut self.focus_input) {
            let id = egui::Id::unique(first_id);
            ui.memory_mut(|m| m.request_focus(id));
            // Select the whole field so typing replaces it, like the original.
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
                let len = match first_id {
                    "value" => self.value.chars().count(),
                    _ => self.ratio_num.chars().count(),
                };
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(len),
                    )));
                state.store(ui.ctx(), id);
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label(RichText::new("Series").strong());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding.x = 4.0;
            for (s, name) in SeriesMask::ALL {
                let key = SERIES_KEYS
                    .iter()
                    .find(|(m, _, _)| *m == s)
                    .map_or("", |k| k.2);
                let on = self.mask.contains(s);
                let resp = ui
                    .add(
                        egui::Button::selectable(on, RichText::new(name).monospace())
                            .min_size(Vec2::new(40.0, 22.0)),
                    )
                    .on_hover_text(format!(
                        "Toggle {name} ({key}). Several series can be combined."
                    ));
                if resp.clicked() {
                    self.toggle_series(s);
                }
            }
        });

        ui.add_space(6.0);
        if self.tab == Tab::Trim {
            self.trim_control(ui);
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Max error ±");
            let resp = ui.add(
                egui::DragValue::new(&mut self.max_err)
                    .range(0.01..=50.0)
                    .speed(0.05)
                    .max_decimals(2)
                    .suffix(" %"),
            );
            if resp.changed() {
                self.max_err_auto = false;
            }
            if !self.max_err_auto
                && ui
                    .small_button("auto")
                    .on_hover_text("Use the default for the selected series")
                    .clicked()
            {
                self.max_err_auto = true;
                self.max_err = self.mask.default_tolerance();
            }
        });
    }

    fn value_field(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("inputs")
            .num_columns(2)
            .spacing([10.0, 8.0])
            .show(ui, |ui| {
                ui.label("Desired value");
                let r = ui.add(field(&mut self.value, "value"));
                mark_invalid(ui, &r, units::parse(&self.value).is_none());
                ui.end_row();
            });
    }

    fn divider_fields(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("inputs")
            .num_columns(2)
            .spacing([10.0, 8.0])
            .show(ui, |ui| {
                ui.label("Ratio");
                ui.horizontal(|ui| {
                    let r =
                        ui.add(field(&mut self.ratio_num, "ratio_num").desired_width(48.0));
                    mark_invalid(ui, &r, units::parse(&self.ratio_num).is_none());
                    ui.label(RichText::new("/").size(16.0));
                    let r =
                        ui.add(field(&mut self.ratio_den, "ratio_den").desired_width(48.0));
                    mark_invalid(ui, &r, units::parse(&self.ratio_den).is_none());
                });
                ui.end_row();
                ui.label("Output R max");
                let r = ui.add(field(&mut self.ro_max, "ro_max"));
                mark_invalid(ui, &r, units::parse(&self.ro_max).is_none());
                ui.end_row();
                ui.label("Output R min");
                let r = ui.add(field(&mut self.ro_min, "ro_min"));
                mark_invalid(ui, &r, units::parse(&self.ro_min).is_none());
                ui.end_row();
            });
    }

    /// The required adjustment range.  "Auto" covers the worst case of the
    /// selected series' tolerance: for a divider both resistors may drift,
    /// moving the ratio by up to 2 · (1 - ratio) · tolerance; for a resistor,
    /// by the tolerance itself.
    fn trim_control(&mut self, ui: &mut egui::Ui) {
        if self.trim_auto {
            let tol = self.mask.component_tolerance();
            self.trim = match self.trim_mode {
                TrimMode::Divider => match ratio_of(&self.ratio_num, &self.ratio_den) {
                    Some(r) if r > 0.0 && r < 1.0 => {
                        ((2.0 * (1.0 - r) * tol * 100.0).round() / 100.0).max(0.01)
                    }
                    _ => self.trim,
                },
                TrimMode::Resistor => tol,
            };
        }
        ui.horizontal(|ui| {
            ui.label("Trim range ±");
            let resp = ui.add(
                egui::DragValue::new(&mut self.trim)
                    .range(0.01..=90.0)
                    .speed(0.05)
                    .max_decimals(2)
                    .suffix(" %"),
            );
            if resp.changed() {
                self.trim_auto = false;
            }
            resp.on_hover_text(format!(
                "How far either side of the target the trimmer must reach, \
                 even with the trimmer {:.0} % low. Auto covers the tolerance \
                 of the selected series. Trimmers come from the 1-2-5 values.",
                calc::POT_TOLERANCE * 100.0
            ));
            if !self.trim_auto
                && ui
                    .small_button("auto")
                    .on_hover_text("Cover the tolerance of the selected series")
                    .clicked()
            {
                self.trim_auto = true;
            }
        });
    }

    fn colour_bands(&self, ui: &mut egui::Ui) {
        let Some((inputs, Ok(rows))) = &self.cache else {
            return;
        };
        let Some(row) = self.selected.and_then(|i| rows.get(i)) else {
            return;
        };
        ui.separator();
        let labels = row.kind.labels(row.resistors.len());
        for (i, &r) in row.resistors.iter().enumerate() {
            ui.horizontal(|ui| {
                if row.resistors.len() > 1 {
                    let label = format!("{:<2}", labels.get(i).copied().unwrap_or(""));
                    ui.label(RichText::new(label).monospace().weak());
                }
                if row.kind.pot_index() == Some(i) {
                    paint_trimmer(ui);
                    ui.label(RichText::new(units::format(r)).monospace().size(14.0));
                    ui.label(RichText::new("trimmer").weak());
                    return;
                }
                let bands = colour_code::bands(r, inputs.mask.is_precision());
                let resp = paint_resistor(ui, &bands);
                ui.label(RichText::new(units::format(r)).monospace().size(14.0));
                if !bands.is_empty() {
                    resp.on_hover_text(
                        bands.iter().map(|b| b.name).collect::<Vec<_>>().join(" – "),
                    );
                }
            });
        }
    }

    fn results_panel(&mut self, ui: &mut egui::Ui) {
        let selected = self.selected;
        let outcome = self.results().clone();
        let inputs = self.inputs();
        let rows = match outcome {
            Ok(rows) => rows,
            Err(msg) => {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(msg).color(ui.visuals().warn_fg_color))
                });
                return;
            }
        };

        let cols = columns(&inputs);
        let row_h = 20.0;
        let font = FontId::monospace(14.0);
        let width = ui.available_width();
        let col_w = (width / cols.len() as f32).min(110.0);

        // Header.
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), ui.visuals().faint_bg_color);
        for (c, name) in cols.iter().enumerate() {
            let x = rect.left() + col_w * (c as f32 + 1.0) - 8.0;
            ui.painter().text(
                egui::pos2(x, rect.center().y),
                Align2::RIGHT_CENTER,
                *name,
                FontId::proportional(13.0),
                ui.visuals().strong_text_color(),
            );
        }

        if rows.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                let msg = if inputs.tab == Tab::Trim {
                    "No combination covers the trim range"
                } else {
                    "No combinations within the error limit"
                };
                ui.label(RichText::new(msg).weak());
            });
            return;
        }

        let mut clicked = None;
        let mut copy = None;
        let mut copy_all = false;
        let scroll_to = ui.ctx().input(|i| {
            i.events.iter().any(|e| {
                matches!(e, egui::Event::Key { pressed: true, key, .. }
                if matches!(key, Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown))
            })
        });

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_h, rows.len(), |ui, range| {
                for idx in range {
                    let row = &rows[idx];
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
                    let is_sel = selected == Some(idx);
                    let visuals = ui.visuals();
                    if is_sel {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(3),
                            visuals.selection.bg_fill,
                        );
                    } else if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(3),
                            visuals.widgets.hovered.weak_bg_fill,
                        );
                    } else if idx % 2 == 1 {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::ZERO, visuals.faint_bg_color);
                    }
                    let text_color = if is_sel {
                        visuals.selection.stroke.color
                    } else {
                        visuals.text_color()
                    };
                    // For trimmer circuits, rate the range against the one asked for:
                    // a trimmer 20 % low needs 1.25× at best.
                    let rating = if inputs.tab == Tab::Trim {
                        match row.error_pct / inputs.trim {
                            x if x < 1.6 => 0.0,
                            x if x < 2.5 => 1.0,
                            _ => 5.0,
                        }
                    } else {
                        row.error_pct
                    };
                    let err_color = if is_sel {
                        text_color
                    } else {
                        error_colour(rating, visuals.dark_mode)
                    };
                    for (c, cell) in cells(&inputs, row).iter().enumerate() {
                        let x = rect.left() + col_w * (c as f32 + 1.0) - 8.0;
                        let color = if c == 0 { err_color } else { text_color };
                        // The circuit description is text, not a number.
                        let font = if cols[c] == "Circuit" {
                            FontId::proportional(13.0)
                        } else {
                            font.clone()
                        };
                        ui.painter().text(
                            egui::pos2(x, rect.center().y),
                            Align2::RIGHT_CENTER,
                            cell,
                            font,
                            color,
                        );
                    }
                    if is_sel && scroll_to {
                        resp.scroll_to_me(None);
                    }
                    if resp.clicked() {
                        clicked = Some(idx);
                    }
                    if resp.double_clicked() {
                        copy = Some(idx);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Copy row").clicked() {
                            clicked = Some(idx);
                            copy = Some(idx);
                            ui.close();
                        }
                        if ui.button("Copy all rows").clicked() {
                            copy_all = true;
                            ui.close();
                        }
                    });
                }
            });

        if let Some(idx) = clicked {
            self.selected = Some(idx);
        }
        if let Some(idx) = copy {
            self.copy_row(ui.ctx(), idx);
        }
        if copy_all {
            self.copy_all(ui.ctx());
        }
    }

    fn about(&self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            ui.heading(RichText::new("ResCAD NG").size(26.0).strong());
            ui.label(format!("version {}", env!("CARGO_PKG_VERSION")));
            ui.add_space(6.0);
            ui.label("Standard-value resistor calculator: nearest value, pairs, three-resistor networks, dividers and trimmer circuits.");
            ui.add_space(6.0);
            ui.label(
                RichText::new("A tribute to Resistor CAD 2.0 © 1996–2006 Terry Harris, Vader Systems").weak(),
            );
        });
        ui.add_space(12.0);
        ui.separator();
        egui::Grid::new("keys")
            .num_columns(2)
            .spacing([24.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                let rows = [
                    ("Alt+N / S / P / W / D / T / A", "Switch tab"),
                    ("Ctrl+Tab", "Next tab"),
                    (
                        "Alt+6 / 1 / 4 / 8 / 9 / 2",
                        "Toggle E6 / E12 / E24 / E48 / E96 / E192",
                    ),
                    ("↑ ↓ PgUp PgDn", "Move through results"),
                    ("Ctrl+C, double-click", "Copy selected result"),
                    ("Right-click", "Copy row / all rows"),
                    ("Alt+C", "Close"),
                ];
                for (k, v) in rows {
                    ui.label(RichText::new(k).monospace());
                    ui.label(v);
                    ui.end_row();
                }
            });
        ui.add_space(8.0);
        ui.label(
            RichText::new("Values: 4k7, 4.7k, 2R2, 1M5, 470, 33 kΩ.  Prefixes p n u m R k M G T (m = milli, M = mega).")
                .weak(),
        );
    }
}

impl eframe::App for ResCadApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(2.0);
            self.tab_bar(ui);
            ui.add_space(1.0);
        });

        if self.tab == Tab::About {
            egui::CentralPanel::default().show(ui, |ui| self.about(ui));
            return;
        }

        let now = ctx.input(|i| i.time);
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let count = match &self.cache {
                    Some((_, Ok(rows))) => format!(
                        "{} result{}",
                        rows.len(),
                        if rows.len() == 1 { "" } else { "s" }
                    ),
                    _ => String::new(),
                };
                ui.label(RichText::new(count).weak());
                if let Some((msg, t)) = &self.flash {
                    if now - t < 1.5 {
                        ui.label(RichText::new(msg.as_str()).color(ui.visuals().hyperlink_color));
                        ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    } else {
                        self.flash = None;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("Ctrl+C copies the selected row").weak());
                });
            });
        });

        egui::Panel::left("inputs")
            .resizable(false)
            .exact_size(272.0)
            .show(ui, |ui| {
                self.input_panel(ui);
                // Make sure the band display reflects the current inputs.
                let _ = self.results();
                self.colour_bands(ui);
            });

        egui::CentralPanel::default().show(ui, |ui| self.results_panel(ui));

        // Ctrl+C copies the selected row unless a text field has a selection to copy.
        let copy_requested = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if copy_requested
            && !text_selection_active(&ctx)
            && let Some(idx) = self.selected
        {
            self.copy_row(&ctx, idx);
        }
    }
}

/// Cycles the theme preference: follow the system, light, dark.
fn theme_button(ui: &mut egui::Ui) {
    use egui::ThemePreference;
    let current = ui.ctx().options(|o| o.theme_preference);
    let (label, next) = match current {
        ThemePreference::System => ("Theme: auto", ThemePreference::Light),
        ThemePreference::Light => ("Theme: light", ThemePreference::Dark),
        ThemePreference::Dark => ("Theme: dark", ThemePreference::System),
    };
    if ui
        .small_button(label)
        .on_hover_text("Switch between system, light and dark themes")
        .clicked()
    {
        ui.ctx().set_theme(next);
    }
}

fn field<'a>(text: &'a mut String, id: &str) -> TextEdit<'a> {
    TextEdit::singleline(text)
        .id(egui::Id::unique(id))
        .desired_width(110.0)
        .font(FontId::monospace(15.0))
}

fn text_selection_active(ctx: &egui::Context) -> bool {
    let Some(id) = ctx.memory(|m| m.focused()) else {
        return false;
    };
    TextEdit::load_state(ctx, id)
        .and_then(|s| s.cursor.char_range())
        .is_some_and(|r| !r.is_empty())
}

fn compute(inputs: &Inputs) -> Outcome {
    let need =
        |text: &str, what: &str| units::parse(text).ok_or_else(|| format!("Enter a valid {what}"));
    match inputs.tab {
        Tab::Nearest => Ok(calc::nearest(inputs.mask, need(&inputs.value, "value")?)),
        Tab::Series => Ok(calc::series(
            inputs.mask,
            need(&inputs.value, "value")?,
            inputs.max_err,
        )),
        Tab::Parallel => Ok(calc::parallel(
            inputs.mask,
            need(&inputs.value, "value")?,
            inputs.max_err,
        )),
        Tab::Divider => {
            let num = need(&inputs.ratio_num, "ratio")?;
            let den = need(&inputs.ratio_den, "ratio")?;
            let ro_max = need(&inputs.ro_max, "maximum output resistance")?;
            let ro_min = need(&inputs.ro_min, "minimum output resistance")?;
            calc::divider(inputs.mask, num / den, ro_min, ro_max, inputs.max_err).map_err(|e| {
                match e {
                    DividerError::RatioOutOfRange => "Ratio must be between 0 and 1".to_string(),
                    DividerError::BadInput => {
                        "Output resistance limits must be positive".to_string()
                    }
                    DividerError::TrimOutOfRange => unreachable!(),
                }
            })
        }
        Tab::Network => Ok(calc::network(
            inputs.mask,
            need(&inputs.value, "value")?,
            inputs.max_err,
        )),
        Tab::Trim => match inputs.trim_mode {
            TrimMode::Divider => {
                let num = need(&inputs.ratio_num, "ratio")?;
                let den = need(&inputs.ratio_den, "ratio")?;
                let ro_max = need(&inputs.ro_max, "maximum output resistance")?;
                let ro_min = need(&inputs.ro_min, "minimum output resistance")?;
                calc::trim_divider(inputs.mask, num / den, ro_min, ro_max, inputs.trim).map_err(
                    |e| match e {
                        DividerError::RatioOutOfRange => {
                            "Ratio must be between 0 and 1".to_string()
                        }
                        DividerError::BadInput => {
                            "Output resistance limits must be positive".to_string()
                        }
                        DividerError::TrimOutOfRange => {
                            "The trim range would take the ratio past 1".to_string()
                        }
                    },
                )
            }
            TrimMode::Resistor => {
                calc::trim_resistor(inputs.mask, need(&inputs.value, "value")?, inputs.trim)
                    .ok_or_else(|| "The trim range must be between 0 and 90 %".to_string())
            }
        },
        Tab::About => Ok(Vec::new()),
    }
}

fn columns(inputs: &Inputs) -> &'static [&'static str] {
    match inputs.tab {
        Tab::Nearest => &["% err", "R"],
        Tab::Series => &["% err", "R1", "R2", "R1+R2"],
        Tab::Parallel => &["% err", "R1", "R2", "R1||R2"],
        Tab::Network => &["% err", "R1", "R2", "R3", "Total", "Circuit"],
        Tab::Divider => &["% err", "R1", "R2", "Ratio", "RO"],
        Tab::Trim => match inputs.trim_mode {
            TrimMode::Divider => &["±adj %", "R1", "P", "R2", "Wiper", "RO"],
            TrimMode::Resistor => &["±adj %", "Rs", "P", "Rp", "Wiper", "Circuit"],
        },
        Tab::About => &[],
    }
}

/// The divider ratio typed in, if valid.
fn ratio_of(num: &str, den: &str) -> Option<f64> {
    Some(units::parse(num)? / units::parse(den)?)
}

fn cells(inputs: &Inputs, row: &Row) -> Vec<String> {
    let r = &row.resistors;
    let ohms = |v: f64| units::format(v);
    match (inputs.tab, row.kind) {
        (Tab::Trim, kind) => {
            let target = match inputs.trim_mode {
                TrimMode::Divider => ratio_of(&inputs.ratio_num, &inputs.ratio_den),
                TrimMode::Resistor => units::parse(&inputs.value),
            }
            .unwrap_or(f64::NAN);
            let wiper = format!("{:.0}%", calc::wiper(row, target) * 100.0);
            let mut out = vec![format!("±{:.2}", row.error_pct), ohms(r[0]), ohms(r[1])];
            out.push(r.get(2).map_or("–".into(), |&v| ohms(v)));
            out.push(wiper);
            out.push(if kind == Kind::PotDivider {
                ohms((r[0] + r[1] + r[2]) * target * (1.0 - target))
            } else {
                kind.circuit().into()
            });
            out
        }
        (_, Kind::SeriesParallel | Kind::ParallelSeries) => {
            let total = if row.kind == Kind::SeriesParallel {
                r[0] + r[1] * r[2] / (r[1] + r[2])
            } else {
                r[0] * (r[1] + r[2]) / (r[0] + r[1] + r[2])
            };
            let mut out = vec![units::format_error(row.error_pct)];
            out.extend(r.iter().map(|&v| ohms(v)));
            out.push(units::format_digits(total, 5));
            out.push(row.kind.circuit().into());
            out
        }
        _ => {
            let mut out = vec![units::format_error(row.error_pct)];
            out.extend(r.iter().map(|&v| ohms(v)));
            if let [r1, r2] = r[..] {
                match inputs.tab {
                    Tab::Series => out.push(ohms(r1 + r2)),
                    Tab::Parallel => out.push(ohms(r1 * r2 / (r1 + r2))),
                    Tab::Divider => {
                        out.push(format!("{:.4}", r2 / (r1 + r2)));
                        out.push(ohms(r1 * r2 / (r1 + r2)));
                    }
                    _ => {}
                }
            }
            out
        }
    }
}

fn row_text(inputs: &Inputs, row: &Row) -> String {
    cells(inputs, row).join("\t")
}

fn error_colour(pct: f64, dark: bool) -> Color32 {
    let a = pct.abs();
    match (a, dark) {
        (a, true) if a < 0.5 => Color32::from_rgb(110, 210, 120),
        (a, false) if a < 0.5 => Color32::from_rgb(20, 130, 40),
        (a, true) if a < 2.0 => Color32::from_rgb(220, 200, 90),
        (a, false) if a < 2.0 => Color32::from_rgb(150, 120, 0),
        (_, true) => Color32::from_rgb(235, 130, 100),
        (_, false) => Color32::from_rgb(190, 60, 30),
    }
}

fn mark_invalid(ui: &egui::Ui, resp: &egui::Response, invalid: bool) {
    if invalid {
        ui.painter().rect_stroke(
            resp.rect.expand(1.0),
            CornerRadius::same(3),
            Stroke::new(1.5, ui.visuals().error_fg_color),
            egui::StrokeKind::Outside,
        );
    }
}

/// The divider schematic; with `trimmer`, R1, P and R2 in a chain with the
/// output on the wiper.
fn divider_diagram(ui: &mut egui::Ui, trimmer: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 70.0), Sense::hover());
    let p = ui.painter();
    let color = ui.visuals().weak_text_color();
    let stroke = Stroke::new(1.2, color);
    let font = FontId::proportional(12.0);
    let small = FontId::proportional(10.0);
    let x0 = rect.left() + 4.0;
    let pos = egui::pos2;
    let label = |r: Rect, text: &str| {
        p.text(r.center(), Align2::CENTER_CENTER, text, small.clone(), color);
    };
    let ground = |x: f32, y: f32| {
        p.line_segment([pos(x - 6.0, y), pos(x + 6.0, y)], stroke);
        p.line_segment([pos(x - 3.5, y + 3.0), pos(x + 3.5, y + 3.0)], stroke);
    };

    let formula = if trimmer {
        // Vin ─[R1]─[ P ]─[R2]─⏚, Vout on the wiper.
        let y = rect.top() + 30.0;
        p.text(pos(x0, y), Align2::LEFT_CENTER, "Vin", font.clone(), color);
        let r1 = Rect::from_min_size(pos(x0 + 44.0, y - 6.0), Vec2::new(30.0, 12.0));
        let pot = Rect::from_min_size(pos(r1.right() + 18.0, y - 7.0), Vec2::new(40.0, 14.0));
        let r2 = Rect::from_min_size(pos(pot.right() + 18.0, y - 6.0), Vec2::new(30.0, 12.0));
        p.line_segment([pos(x0 + 26.0, y), pos(r2.right() + 16.0, y)], stroke);
        for (r, name) in [(r1, "R1"), (pot, "P"), (r2, "R2")] {
            p.rect_filled(r, CornerRadius::ZERO, ui.visuals().panel_fill);
            p.rect_stroke(r, CornerRadius::ZERO, stroke, egui::StrokeKind::Middle);
            label(r, name);
        }
        let gx = r2.right() + 16.0;
        p.line_segment([pos(gx, y), pos(gx, y + 10.0)], stroke);
        ground(gx, y + 10.0);
        // Wiper arrow from Vout down onto the trimmer.
        let wx = pot.center().x + 8.0;
        p.line_segment([pos(wx, pot.top() - 14.0), pos(wx, pot.top())], stroke);
        p.line_segment([pos(wx - 3.0, pot.top() - 4.0), pos(wx, pot.top())], stroke);
        p.line_segment([pos(wx + 3.0, pot.top() - 4.0), pos(wx, pot.top())], stroke);
        p.line_segment([pos(wx, pot.top() - 14.0), pos(wx + 18.0, pot.top() - 14.0)], stroke);
        p.text(
            pos(wx + 22.0, pot.top() - 14.0),
            Align2::LEFT_CENTER,
            "Vout",
            font.clone(),
            color,
        );
        "ratio = R2/Rt … (R2+P)/Rt,  Rt = R1+P+R2"
    } else {
        // Vin ─[R1]─┬─ Vout
        //          [R2]
        //           ⏚
        let y = rect.top() + 12.0;
        p.text(pos(x0, y), Align2::LEFT_CENTER, "Vin", font.clone(), color);
        let r1 = Rect::from_min_size(pos(x0 + 50.0, y - 6.0), Vec2::new(34.0, 12.0));
        p.line_segment([pos(x0 + 26.0, y), pos(r1.left(), y)], stroke);
        p.rect_stroke(r1, CornerRadius::ZERO, stroke, egui::StrokeKind::Middle);
        label(r1, "R1");
        let node = pos(r1.right() + 26.0, y);
        p.line_segment([pos(r1.right(), y), pos(node.x + 30.0, y)], stroke);
        p.circle_filled(node, 2.2, color);
        p.text(pos(node.x + 34.0, y), Align2::LEFT_CENTER, "Vout", font.clone(), color);
        let r2 = Rect::from_center_size(pos(node.x, y + 22.0), Vec2::new(12.0, 22.0));
        p.line_segment([node, pos(node.x, r2.top())], stroke);
        p.rect_stroke(r2, CornerRadius::ZERO, stroke, egui::StrokeKind::Middle);
        p.text(
            pos(r2.right() + 4.0, r2.center().y),
            Align2::LEFT_CENTER,
            "R2",
            small.clone(),
            color,
        );
        let gy = r2.bottom() + 4.0;
        p.line_segment([pos(node.x, r2.bottom()), pos(node.x, gy)], stroke);
        p.line_segment([pos(node.x - 6.0, gy), pos(node.x + 6.0, gy)], stroke);
        "ratio = R2 / (R1+R2)    RO = R1 || R2"
    };
    p.text(
        pos(rect.left() + 4.0, rect.bottom() - 2.0),
        Align2::LEFT_BOTTOM,
        formula,
        FontId::proportional(11.0),
        color,
    );
}

/// A trimmer: a square body with an adjustment slot.
fn paint_trimmer(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 26.0), Sense::hover());
    let p = ui.painter();
    p.line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(2.0, Color32::from_gray(150)),
    );
    let body = Rect::from_center_size(rect.center(), Vec2::new(26.0, 24.0));
    p.rect_filled(body, CornerRadius::same(3), Color32::from_rgb(40, 90, 170));
    let c = body.center();
    p.circle_filled(c, 8.0, Color32::from_gray(225));
    p.line_segment(
        [c + Vec2::new(-5.0, 5.0), c + Vec2::new(5.0, -5.0)],
        Stroke::new(2.0, Color32::from_gray(90)),
    );
}

fn paint_resistor(ui: &mut egui::Ui, bands: &[colour_code::Band]) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(120.0, 26.0), Sense::hover());
    let p = ui.painter();
    let lead = Stroke::new(2.0, Color32::from_gray(150));
    p.line_segment([rect.left_center(), rect.right_center()], lead);
    let body = Rect::from_center_size(rect.center(), Vec2::new(82.0, 22.0));
    let body_colour = if bands.len() == 5 {
        Color32::from_rgb(90, 150, 210)
    } else {
        Color32::from_rgb(222, 196, 150)
    };
    p.rect_filled(body, CornerRadius::same(8), body_colour);
    if bands.is_empty() {
        return resp;
    }
    // Value bands grouped on the left, tolerance band set apart on the right.
    let n = bands.len();
    let band_w = 6.0;
    let gap = 5.0;
    let start = body.left() + 12.0;
    for (i, band) in bands.iter().enumerate() {
        let x = if i == n - 1 {
            body.right() - 12.0 - band_w
        } else {
            start + i as f32 * (band_w + gap)
        };
        let r = Rect::from_min_size(egui::pos2(x, body.top()), Vec2::new(band_w, body.height()));
        p.rect_filled(r, CornerRadius::ZERO, band.colour);
    }
    resp
}
