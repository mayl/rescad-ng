//! ResCAD NG in the browser: the same calculators as the desktop app, with a
//! Leptos front end compiled to WebAssembly.

use std::time::Duration;

use leptos::ev;
use leptos::prelude::*;
use rescad_core::calc::{self, Row};
use rescad_core::colour_code::{self, Band};
use rescad_core::eseries::SeriesMask;
use rescad_core::sketch::{self, Shape, Sketch};
use rescad_core::table::{self, Mode, Params};
use rescad_core::units;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, KeyboardEvent};

/// Rows rendered into the table; the rest are still counted and copied.
const MAX_SHOWN: usize = 500;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Nearest,
    Series,
    Parallel,
    Network,
    Divider,
    Trim,
    About,
}

/// Tabs with their Alt+key shortcut (as a `KeyboardEvent.code`).
const TABS: [(Tab, &str, &str); 7] = [
    (Tab::Nearest, "Nearest", "KeyN"),
    (Tab::Series, "Series", "KeyS"),
    (Tab::Parallel, "Parallel", "KeyP"),
    (Tab::Network, "Network", "KeyW"),
    (Tab::Divider, "Divider", "KeyD"),
    (Tab::Trim, "Trim", "KeyT"),
    (Tab::About, "About", "KeyA"),
];

/// Alt+digit shortcuts for the series buttons, as on the desktop.
const SERIES_KEYS: [(SeriesMask, &str); 6] = [
    (SeriesMask::E6, "Digit6"),
    (SeriesMask::E12, "Digit1"),
    (SeriesMask::E24, "Digit4"),
    (SeriesMask::E48, "Digit8"),
    (SeriesMask::E96, "Digit9"),
    (SeriesMask::E192, "Digit2"),
];

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

/// All the app's state, shared by the components.
#[derive(Clone, Copy)]
struct State {
    tab: RwSignal<Tab>,
    trim_divider: RwSignal<bool>,
    /// The Divider tab searches three-resistor dividers.
    divider3: RwSignal<bool>,
    value: RwSignal<String>,
    ratio_num: RwSignal<String>,
    ratio_den: RwSignal<String>,
    ro_max: RwSignal<String>,
    ro_min: RwSignal<String>,
    mask: RwSignal<SeriesMask>,
    max_err: RwSignal<f64>,
    max_err_auto: RwSignal<bool>,
    trim: RwSignal<f64>,
    trim_auto: RwSignal<bool>,
    selected: RwSignal<Option<usize>>,
    flash: RwSignal<Option<String>>,
    /// Inputs with the "auto" values filled in.
    params: Memo<Params>,
    results: Memo<Result<Vec<Row>, String>>,
}

impl State {
    fn new() -> Self {
        let defaults = Params::default();
        let tab = RwSignal::new(Tab::Nearest);
        let trim_divider = RwSignal::new(true);
        let divider3 = RwSignal::new(false);
        let value = RwSignal::new(defaults.value.clone());
        let ratio_num = RwSignal::new(defaults.ratio_num.clone());
        let ratio_den = RwSignal::new(defaults.ratio_den.clone());
        let ro_max = RwSignal::new(defaults.ro_max.clone());
        let ro_min = RwSignal::new(defaults.ro_min.clone());
        let mask = RwSignal::new(defaults.mask);
        let max_err = RwSignal::new(defaults.max_err);
        let max_err_auto = RwSignal::new(true);
        let trim = RwSignal::new(defaults.trim);
        let trim_auto = RwSignal::new(true);

        let params = Memo::new(move |_| {
            let mode = match (tab.get(), trim_divider.get()) {
                (Tab::Divider, _) if divider3.get() => Mode::Divider3,
                (Tab::Series, _) => Mode::Series,
                (Tab::Parallel, _) => Mode::Parallel,
                (Tab::Network, _) => Mode::Network,
                (Tab::Divider, _) => Mode::Divider,
                (Tab::Trim, true) => Mode::TrimDivider,
                (Tab::Trim, false) => Mode::TrimResistor,
                (Tab::Nearest | Tab::About, _) => Mode::Nearest,
            };
            let mut p = Params {
                mode,
                value: value.get(),
                ratio_num: ratio_num.get(),
                ratio_den: ratio_den.get(),
                ro_max: ro_max.get(),
                ro_min: ro_min.get(),
                mask: mask.get(),
                max_err: max_err.get(),
                trim: trim.get(),
            };
            if max_err_auto.get() {
                p.max_err = p.mask.default_tolerance();
            }
            if trim_auto.get() {
                p.trim = p.auto_trim();
            }
            p
        });
        let results = Memo::new(move |_| table::compute(&params.get()));

        let state = Self {
            tab,
            trim_divider,
            divider3,
            value,
            ratio_num,
            ratio_den,
            ro_max,
            ro_min,
            mask,
            max_err,
            max_err_auto,
            trim,
            trim_auto,
            selected: RwSignal::new(Some(0)),
            flash: RwSignal::new(None),
            params,
            results,
        };

        // Keep the selection on a row that exists.
        Effect::new(move |_| {
            let len = results.with(|r| r.as_ref().map_or(0, Vec::len));
            let sel = state.selected.get_untracked();
            let new = (len > 0).then(|| sel.unwrap_or(0).min(len - 1));
            if new != sel {
                state.selected.set(new);
            }
        });
        state
    }

    fn set_tab(self, tab: Tab) {
        if self.tab.get_untracked() != tab {
            self.tab.set(tab);
            self.selected.set(Some(0));
        }
    }

    fn toggle_series(self, s: SeriesMask) {
        let mut mask = self.mask.get_untracked();
        mask.toggle(s);
        // Never allow an empty selection: there would be nothing to search.
        if !mask.is_empty() {
            self.mask.set(mask);
        }
    }

    fn row_count(self) -> usize {
        self.results
            .with_untracked(|r| r.as_ref().map_or(0, Vec::len))
    }

    fn copy_row(self, idx: usize) {
        let text = self.results.with_untracked(|r| {
            let row = r.as_ref().ok()?.get(idx)?;
            Some(table::row_text(&self.params.get_untracked(), row))
        });
        if let Some(text) = text {
            copy_text(&text);
            self.show_flash("Copied".into());
        }
    }

    fn copy_all(self) {
        let copied = self.results.with_untracked(|r| {
            let rows = r.as_ref().ok()?;
            copy_text(&table::all_rows_text(&self.params.get_untracked(), rows));
            Some(rows.len())
        });
        if let Some(n) = copied {
            self.show_flash(format!("Copied {n} rows"));
        }
    }

    fn show_flash(self, msg: String) {
        self.flash.set(Some(msg.clone()));
        let flash = self.flash;
        set_timeout(
            move || {
                if flash.get_untracked().as_ref() == Some(&msg) {
                    flash.set(None);
                }
            },
            Duration::from_millis(1500),
        );
    }

    /// Keyboard shortcuts, matching the desktop app.
    fn on_key(self, e: &KeyboardEvent) {
        let code = e.code();
        if e.alt_key() && !e.ctrl_key() && !e.meta_key() {
            if let Some((tab, _, _)) = TABS.iter().find(|t| t.2 == code) {
                e.prevent_default();
                self.set_tab(*tab);
            } else if let Some((s, _)) = SERIES_KEYS.iter().find(|k| k.1 == code) {
                e.prevent_default();
                self.toggle_series(*s);
            }
            return;
        }
        let len = self.row_count();
        let plain = !e.alt_key() && !e.ctrl_key() && !e.meta_key();
        let in_number = active_input().is_some_and(|i| i.type_() == "number");
        if plain && len > 0 && !in_number {
            let sel = self.selected.get_untracked().unwrap_or(0);
            let new = match code.as_str() {
                "ArrowDown" => Some((sel + 1).min(len - 1)),
                "ArrowUp" => Some(sel.saturating_sub(1)),
                "PageDown" => Some((sel + 10).min(len - 1)),
                "PageUp" => Some(sel.saturating_sub(10)),
                _ => None,
            };
            if let Some(new) = new {
                e.prevent_default();
                self.selected.set(Some(new));
                scroll_selection_into_view();
            }
        }
        // Ctrl+C copies the selected row unless there is text selected to copy.
        if (e.ctrl_key() || e.meta_key())
            && code == "KeyC"
            && !text_selected()
            && let Some(idx) = self.selected.get_untracked()
        {
            e.prevent_default();
            self.copy_row(idx);
        }
    }
}

fn copy_text(text: &str) {
    let _ = window().navigator().clipboard().write_text(text);
}

fn active_input() -> Option<HtmlInputElement> {
    document().active_element()?.dyn_into().ok()
}

fn text_selected() -> bool {
    if let Some(input) = active_input() {
        let start = input.selection_start().ok().flatten();
        let end = input.selection_end().ok().flatten();
        return start != end;
    }
    window()
        .get_selection()
        .ok()
        .flatten()
        .is_some_and(|s| !s.is_collapsed())
}

fn scroll_selection_into_view() {
    request_animation_frame(|| {
        if let Ok(Some(row)) = document().query_selector("tr.sel") {
            let opts = web_sys::ScrollIntoViewOptions::new();
            opts.set_block(web_sys::ScrollLogicalPosition::Nearest);
            row.scroll_into_view_with_scroll_into_view_options(&opts);
        }
    });
}

/// Focus the first input of the current tab and select its text, so typing
/// replaces it, like the original.
fn focus_first_input() {
    request_animation_frame(|| {
        if let Some(el) = document().get_element_by_id("first")
            && let Ok(input) = el.dyn_into::<HtmlInputElement>()
        {
            let _ = input.focus();
            input.select();
        }
    });
}

#[component]
fn App() -> impl IntoView {
    let state = State::new();
    let handle = window_event_listener(ev::keydown, move |e| state.on_key(&e));
    on_cleanup(move || handle.remove());
    Effect::new(move |_| {
        state.tab.track();
        state.trim_divider.track();
        state.divider3.track();
        focus_first_input();
    });

    view! {
        <div class="app">
            <header class="tabs">
                {TABS
                    .iter()
                    .map(|&(tab, name, code)| {
                        view! {
                            <button
                                class="tab"
                                class:on=move || state.tab.get() == tab
                                title=format!("Alt+{}", &code[3..])
                                on:click=move |_| state.set_tab(tab)
                            >
                                {name}
                            </button>
                        }
                    })
                    .collect_view()}
                <span class="spacer"></span>
                <ThemeButton />
            </header>
            {move || {
                if state.tab.get() == Tab::About {
                    view! { <About /> }.into_any()
                } else {
                    view! {
                        <div class="body">
                            <aside>
                                <Inputs state />
                                <SeriesPicker state />
                                <Limit state />
                                <Parts state />
                            </aside>
                            <main>
                                <Results state />
                            </main>
                        </div>
                        <StatusBar state />
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

/// Cycles the theme: follow the system, light, dark.
#[component]
fn ThemeButton() -> impl IntoView {
    let theme = RwSignal::new(0u8);
    Effect::new(move |_| {
        let root = document().document_element().unwrap();
        let _ = match theme.get() {
            1 => root.set_attribute("data-theme", "light"),
            2 => root.set_attribute("data-theme", "dark"),
            _ => root.remove_attribute("data-theme"),
        };
    });
    view! {
        <button
            class="theme"
            title="Switch between system, light and dark themes"
            on:click=move |_| theme.update(|t| *t = (*t + 1) % 3)
        >
            {move || ["Theme: auto", "Theme: light", "Theme: dark"][theme.get() as usize]}
        </button>
    }
}

/// A value field; `first` marks the one focused on switching tabs.
fn field(sig: RwSignal<String>, first: bool, narrow: bool) -> impl IntoView {
    view! {
        <input
            type="text"
            class="field"
            class:narrow=narrow
            id=first.then_some("first")
            class:invalid=move || units::parse(&sig.get()).is_none()
            spellcheck="false"
            autocomplete="off"
            prop:value=sig.get_untracked()
            on:input=move |ev| sig.set(event_target_value(&ev))
        />
    }
}

#[component]
fn Inputs(state: State) -> impl IntoView {
    let value_field = move || {
        view! {
            <div class="grid">
                <label>"Desired value"</label>
                {field(state.value, true, false)}
            </div>
        }
    };
    let divider_fields = move || {
        view! {
            <div class="grid">
                <label>"Ratio"</label>
                <span class="ratio">
                    {field(state.ratio_num, true, true)}
                    <span class="slash">"/"</span>
                    {field(state.ratio_den, false, true)}
                </span>
                <label>"Output R max"</label>
                {field(state.ro_max, false, false)}
                <label>"Output R min"</label>
                {field(state.ro_min, false, false)}
            </div>
        }
    };
    let hint = move || state.params.with(|p| p.mode.hint());
    // The selected row's circuit, drawn large with its parts named.
    let selected_sketch = move || {
        let kind = state.results.with(|r| {
            let rows = r.as_ref().ok()?;
            Some(rows.get(state.selected.get()?)?.kind)
        });
        let sk = sketch::sketch(kind?)?;
        Some(sketch_svg(&sk, (260.0 / sk.width).min(4.5), true))
    };

    // A pair of chips choosing between two variants of a tab.
    let mode_button =
        move |sig: RwSignal<bool>, on: bool, name: &'static str, tip: &'static str| {
            view! {
                <button
                    class="chip"
                    class:on=move || sig.get() == on
                    title=tip
                    on:click=move |_| {
                        if sig.get_untracked() != on {
                            sig.set(on);
                            state.selected.set(Some(0));
                        }
                    }
                >
                    {name}
                </button>
            }
        };

    move || match state.tab.get() {
        Tab::Divider => view! {
            <div class="modes">
                {mode_button(state.divider3, false, "2 resistors", "R1 on top, R2 at the bottom")}
                {mode_button(
                    state.divider3,
                    true,
                    "3 resistors",
                    "One leg made of two resistors in series or parallel, for more precision",
                )}
            </div>
            {divider_fields()}
            {move || {
                if state.divider3.get() {
                    selected_sketch()
                        .unwrap_or_else(|| {
                            view! { <DividerDiagram trimmer=false top="top" bottom="bottom" /> }
                                .into_any()
                        })
                } else {
                    view! { <DividerDiagram trimmer=false /> }.into_any()
                }
            }}
            <p class="hint">{hint}</p>
        }
        .into_any(),
        Tab::Trim => {
            let td = state.trim_divider;
            view! {
                <div class="modes">
                    {mode_button(td, true, "Divider", "R1, trimmer, R2 in a chain, output on the wiper")}
                    {mode_button(td, false, "Resistor", "A trimmer padded with fixed resistors")}
                </div>
                {move || {
                    if state.trim_divider.get() {
                        view! {
                            {divider_fields()}
                            <DividerDiagram trimmer=true />
                        }
                            .into_any()
                    } else {
                        value_field().into_any()
                    }
                }}
                <p class="hint">{hint}</p>
                {selected_sketch}
            }
                .into_any()
        }
        _ => view! {
            {value_field()}
            <p class="hint">{hint}</p>
            {selected_sketch}
        }
        .into_any(),
    }
}

#[component]
fn SeriesPicker(state: State) -> impl IntoView {
    view! {
        <h3>"Series"</h3>
        <div class="series">
            {SeriesMask::ALL
                .iter()
                .map(|&(s, name)| {
                    let key = SERIES_KEYS
                        .iter()
                        .find(|k| k.0 == s)
                        .map_or("", |k| &k.1[5..]);
                    view! {
                        <button
                            class="chip mono"
                            class:on=move || state.mask.get().contains(s)
                            title=format!("Toggle {name} (Alt+{key}). Several series can be combined.")
                            on:click=move |_| state.toggle_series(s)
                        >
                            {name}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// The max error, or for the trimmer modes the trim range, with "auto".
#[component]
fn Limit(state: State) -> impl IntoView {
    move || {
        let trim = state.tab.get() == Tab::Trim;
        let (label, signal, auto, tip) = if trim {
            (
                "Trim range ±",
                state.trim,
                state.trim_auto,
                format!(
                    "How far either side of the target the trimmer must reach, even with the \
                     trimmer {:.0} % low. Auto covers the tolerance of the selected series. \
                     Trimmers come from the 1-2-5 values.",
                    calc::POT_TOLERANCE * 100.0
                ),
            )
        } else {
            (
                "Max error ±",
                state.max_err,
                state.max_err_auto,
                "Results further off than this are not shown".to_string(),
            )
        };
        let shown = move || {
            state.params.with(|p| {
                let v = if trim { p.trim } else { p.max_err };
                format!("{v:.2}")
            })
        };
        view! {
            <div class="limit" title=tip>
                <label>{label}</label>
                <input
                    type="number"
                    class="field narrow"
                    min="0.01"
                    max=if trim { "90" } else { "50" }
                    step="0.05"
                    prop:value=shown
                    on:change=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<f64>()
                            && v > 0.0
                        {
                            signal.set(v);
                            auto.set(false);
                        }
                    }
                />
                <span>"%"</span>
                <Show when=move || !auto.get()>
                    <button
                        class="chip small"
                        title="Use the default for the selected series"
                        on:click=move |_| auto.set(true)
                    >
                        "auto"
                    </button>
                </Show>
            </div>
        }
    }
}

/// The selected result's parts, drawn with their colour bands.
#[component]
fn Parts(state: State) -> impl IntoView {
    move || {
        let row = state.results.with(|r| {
            let rows = r.as_ref().ok()?;
            rows.get(state.selected.get()?).cloned()
        })?;
        let precision = state.mask.get().is_precision();
        let labels = row.kind.labels(row.resistors.len());
        let parts = row
            .resistors
            .iter()
            .enumerate()
            .map(|(i, &r)| {
                let label = (row.resistors.len() > 1).then(|| labels.get(i).copied().unwrap_or(""));
                let value = units::format(r);
                if row.kind.pot_index() == Some(i) {
                    view! {
                        <div class="part">
                            <span class="label">{label}</span>
                            {trimmer_svg()}
                            <span class="value">{value}</span>
                            <span class="weak">"trimmer"</span>
                        </div>
                    }
                    .into_any()
                } else {
                    let bands = colour_code::bands(r, precision);
                    let names = bands.iter().map(|b| b.name).collect::<Vec<_>>().join(" – ");
                    view! {
                        <div class="part" title=names>
                            <span class="label">{label}</span>
                            {resistor_svg(&bands)}
                            <span class="value">{value}</span>
                        </div>
                    }
                    .into_any()
                }
            })
            .collect_view();
        Some(view! { <div class="parts">{parts}</div> })
    }
}

fn rgb(c: [u8; 3]) -> String {
    format!("rgb({},{},{})", c[0], c[1], c[2])
}

/// A resistor body with its colour bands: value bands grouped on the left,
/// the tolerance band set apart on the right.
fn resistor_svg(bands: &[Band]) -> impl IntoView + use<> {
    let body = if bands.len() == 5 {
        "#5a96d2"
    } else {
        "#dec496"
    };
    let n = bands.len();
    let rects = bands
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let x = if i == n - 1 {
                83.0
            } else {
                31.0 + i as f64 * 11.0
            };
            view! { <rect x=x y="2" width="6" height="22" fill=rgb(b.colour) /> }
        })
        .collect_view();
    view! {
        <svg class="resistor" viewBox="0 0 120 26" width="120" height="26">
            <line x1="0" y1="13" x2="120" y2="13" stroke="#969696" stroke-width="2" />
            <clipPath id="body">
                <rect x="19" y="2" width="82" height="22" rx="8" />
            </clipPath>
            <rect x="19" y="2" width="82" height="22" rx="8" fill=body />
            <g clip-path="url(#body)">{rects}</g>
        </svg>
    }
}

fn trimmer_svg() -> impl IntoView {
    view! {
        <svg class="resistor" viewBox="0 0 120 26" width="120" height="26">
            <line x1="0" y1="13" x2="120" y2="13" stroke="#969696" stroke-width="2" />
            <rect x="47" y="1" width="26" height="24" rx="3" fill="#285aaa" />
            <circle cx="60" cy="13" r="8" fill="#e1e1e1" />
            <line x1="55" y1="18" x2="65" y2="8" stroke="#5a5a5a" stroke-width="2" />
        </svg>
    }
}

/// `sk` as an SVG, `scale` pixels to the unit; with `labels`, the parts are
/// named.
fn sketch_svg(sk: &Sketch, scale: f32, labels: bool) -> AnyView {
    // A pixel's margin so strokes at the edges aren't clipped.
    let at = |v: f32| v * scale + 1.0;
    let r = sketch::DOT_RADIUS * scale;
    let shapes = sk
        .shapes
        .iter()
        .map(|shape| match *shape {
            Shape::Line { from, to } => view! {
                <line x1=at(from.0) y1=at(from.1) x2=at(to.0) y2=at(to.1) />
            }
            .into_any(),
            Shape::Resistor {
                x,
                y,
                w,
                h,
                label,
                trimmer,
            } => {
                let arrow = trimmer.then(|| {
                    let (ax, ay, bx, by) =
                        (at(x + 1.0), at(y + h + 1.5), at(x + w - 1.0), at(y - 1.5));
                    let len = ((ax - bx).hypot(ay - by)).max(1e-3);
                    let (ux, uy) = ((ax - bx) / len * 1.8 * scale, (ay - by) / len * 1.8 * scale);
                    let head = format!(
                        "M{},{} L{bx},{by} L{},{}",
                        bx + ux - uy * 0.5,
                        by + uy + ux * 0.5,
                        bx + ux + uy * 0.5,
                        by + uy - ux * 0.5,
                    );
                    view! {
                        <line x1=ax y1=ay x2=bx y2=by />
                        <path d=head />
                    }
                });
                let text = labels.then(|| {
                    let size = (h * scale * 0.6).clamp(9.0, 13.0);
                    view! {
                        <text
                            x=at(x + w / 2.0)
                            y=at(y + h / 2.0)
                            font-size=size
                            text-anchor="middle"
                            dominant-baseline="central"
                        >
                            {label}
                        </text>
                    }
                });
                view! {
                    <rect x=at(x) y=at(y) width=w * scale height=h * scale />
                    {arrow}
                    {text}
                }
                .into_any()
            }
            Shape::Dot { x, y } => {
                view! { <circle class="dot" cx=at(x) cy=at(y) r=r /> }.into_any()
            }
            Shape::Terminal { x, y } => view! { <circle cx=at(x) cy=at(y) r=r /> }.into_any(),
            Shape::Ground { x, y } => [(0.0, 3.0), (1.5, 2.0), (3.0, 1.0)]
                .into_iter()
                .map(|(dx, half)| {
                    view! { <line x1=at(x + dx) y1=at(y - half) x2=at(x + dx) y2=at(y + half) /> }
                })
                .collect_view()
                .into_any(),
        })
        .collect_view();
    let (w, h) = (sk.width * scale + 2.0, sk.height * scale + 2.0);
    view! {
        <svg class="sketch" class:large=labels width=w height=h viewBox=format!("0 0 {w} {h}")>
            {shapes}
        </svg>
    }
    .into_any()
}

/// The divider schematic, with its legs labelled `top` and `bottom`; with
/// `trimmer`, R1, P and R2 in a chain with the output on the wiper.
#[component]
fn DividerDiagram(
    trimmer: bool,
    #[prop(default = "R1")] top: &'static str,
    #[prop(default = "R2")] bottom: &'static str,
) -> impl IntoView {
    if trimmer {
        view! {
            <svg class="diagram" viewBox="0 0 260 56" width="260" height="56">
                <text x="2" y="38">"Vin"</text>
                <line x1="26" y1="34" x2="222" y2="34" />
                <rect x="44" y="28" width="30" height="12" />
                <text class="small" x="59" y="37" text-anchor="middle">"R1"</text>
                <rect x="92" y="27" width="40" height="14" />
                <text class="small" x="112" y="38" text-anchor="middle">"P"</text>
                <rect x="150" y="28" width="30" height="12" />
                <text class="small" x="165" y="37" text-anchor="middle">"R2"</text>
                <line x1="222" y1="34" x2="222" y2="44" />
                <line x1="216" y1="44" x2="228" y2="44" />
                <line x1="219" y1="47" x2="225" y2="47" />
                <line x1="120" y1="13" x2="120" y2="27" />
                <line x1="117" y1="23" x2="120" y2="27" />
                <line x1="123" y1="23" x2="120" y2="27" />
                <line x1="120" y1="13" x2="138" y2="13" />
                <text x="142" y="17">"Vout"</text>
            </svg>
        }
        .into_any()
    } else {
        view! {
            <svg class="diagram" viewBox="0 0 260 56" width="260" height="56">
                <text x="2" y="16">"Vin"</text>
                <line x1="26" y1="12" x2="54" y2="12" />
                <rect x="54" y="6" width="34" height="12" />
                <text class="small" x="71" y="15" text-anchor="middle">{top}</text>
                <line x1="88" y1="12" x2="144" y2="12" />
                <circle cx="114" cy="12" r="2.2" class="dot" />
                <text x="148" y="16">"Vout"</text>
                <line x1="114" y1="12" x2="114" y2="23" />
                <rect x="108" y="23" width="12" height="22" />
                <text class="small" x="124" y="37">{bottom}</text>
                <line x1="114" y1="45" x2="114" y2="49" />
                <line x1="108" y1="49" x2="120" y2="49" />
            </svg>
        }
        .into_any()
    }
}

#[component]
fn Results(state: State) -> impl IntoView {
    move || {
        let rows = match state.results.get() {
            Ok(rows) => rows,
            Err(msg) => return view! { <p class="message warn">{msg}</p> }.into_any(),
        };
        let p = state.params.get();
        let cols = p.mode.columns();
        let header = cols
            .iter()
            .map(|&c| view! { <th class:circuit=c == "Circuit">{c}</th> })
            .collect_view();
        if rows.is_empty() {
            return view! {
                <table>
                    <thead>
                        <tr>{header}</tr>
                    </thead>
                </table>
                <p class="message">{p.mode.empty_message()}</p>
            }
            .into_any();
        }
        let body = rows
            .iter()
            .take(MAX_SHOWN)
            .enumerate()
            .map(|(i, row)| {
                let rating = ["good", "fair", "poor"][table::rating(&p, row) as usize];
                let cells = table::cells(&p, row)
                    .into_iter()
                    .enumerate()
                    .map(|(c, cell)| {
                        if cols[c] == "Circuit"
                            && let Some(sk) = sketch::sketch(row.kind)
                        {
                            // A little schematic, with the text as a tooltip.
                            let icon = sketch_svg(&sk, 20.0 / sk.height, false);
                            return view! { <td class="circuit" title=cell>{icon}</td> }.into_any();
                        }
                        let class = if c == 0 { rating } else { "" };
                        view! { <td class=class>{cell}</td> }.into_any()
                    })
                    .collect_view();
                view! {
                    <tr
                        class:sel=move || state.selected.get() == Some(i)
                        on:click=move |_| state.selected.set(Some(i))
                        on:dblclick=move |_| state.copy_row(i)
                    >
                        {cells}
                    </tr>
                }
            })
            .collect_view();
        let more = (rows.len() > MAX_SHOWN).then(|| {
            view! {
                <p class="message">
                    {format!("Showing the best {MAX_SHOWN}; “Copy all” copies all {}.", rows.len())}
                </p>
            }
        });
        view! {
            <table>
                <thead>
                    <tr>{header}</tr>
                </thead>
                <tbody>{body}</tbody>
            </table>
            {more}
        }
        .into_any()
    }
}

#[component]
fn StatusBar(state: State) -> impl IntoView {
    let count = move || {
        state.results.with(|r| match r {
            Ok(rows) => format!(
                "{} result{}",
                rows.len(),
                if rows.len() == 1 { "" } else { "s" }
            ),
            Err(_) => String::new(),
        })
    };
    view! {
        <footer>
            <span class="weak">{count}</span>
            <span class="flash">{move || state.flash.get()}</span>
            <span class="spacer"></span>
            <button
                class="chip small"
                title="Copy the selected row (Ctrl+C, or double-click a row)"
                on:click=move |_| {
                    if let Some(i) = state.selected.get_untracked() {
                        state.copy_row(i);
                    }
                }
            >
                "Copy row"
            </button>
            <button
                class="chip small"
                title="Copy every row, tab-separated, ready for a spreadsheet"
                on:click=move |_| state.copy_all()
            >
                "Copy all"
            </button>
        </footer>
    }
}

#[component]
fn About() -> impl IntoView {
    let keys = [
        ("Alt+N / S / P / W / D / T / A", "Switch tab"),
        (
            "Alt+6 / 1 / 4 / 8 / 9 / 2",
            "Toggle E6 / E12 / E24 / E48 / E96 / E192",
        ),
        ("↑ ↓ PgUp PgDn", "Move through results"),
        ("Ctrl+C, double-click", "Copy selected result"),
    ];
    view! {
        <div class="about">
            <h1>"ResCAD NG"</h1>
            <p>{format!("version {}", env!("CARGO_PKG_VERSION"))}</p>
            <p>
                "Standard-value resistor calculator: nearest value, pairs, three-resistor networks, dividers and trimmer circuits."
            </p>
            <p class="weak">
                "A tribute to Resistor CAD 2.0 © 1996–2006 Terry Harris, Vader Systems"
            </p>
            <table class="keys">
                {keys
                    .iter()
                    .map(|&(k, v)| {
                        view! {
                            <tr>
                                <td class="mono">{k}</td>
                                <td>{v}</td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </table>
            <p class="weak">
                "Values: 4k7, 4.7k, 2R2, 1M5, 470, 33 kΩ.  Prefixes p n u m R k M G T (m = milli, M = mega)."
            </p>
            <p class="weak">
                "Runs entirely in your browser. Also available as a desktop app: "
                <a href="https://github.com/mayl/rescad-ng">"github.com/mayl/rescad-ng"</a>
            </p>
        </div>
    }
}
