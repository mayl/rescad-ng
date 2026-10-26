# ResCAD NG

A next-generation, cross-platform rewrite of **Resistor CAD 2.0** (ResCad, © 1996–2006 Terry Harris,
Vader Systems), the little Windows freeware tool for finding standard (E-series) resistor values.

It is a single self-contained executable. There's nothing to install: download it and double-click it.
It opens in a fraction of a second and recalculates as you type. The same calculators also run in a web
browser, compiled to WebAssembly: **[try it online](https://mayl.github.io/rescad-ng/)**.

## What it does

| Tab | Finds |
|---|---|
| **Nearest** | The preferred value closest to what you typed, plus the next one either side |
| **Series** | Pairs where R1 + R2 ≈ the desired value |
| **Parallel** | Pairs where R1 ‖ R2 ≈ the desired value |
| **Network** | Three-resistor networks R1 + (R2 ‖ R3) and R1 ‖ (R2 + R3), which get far closer than any pair |
| **Divider** | Voltage dividers where R2/(R1+R2) ≈ a ratio, with the output resistance R1 ‖ R2 kept between a min and max. **3 resistors** mode makes one leg a series or parallel pair (R1+R2 / R3, R1‖R2 / R3, R1 / R2+R3, R1 / R2‖R3) for ratios no pair can hit |
| **Trim** | Adjustable circuits built around a trimmer: a **divider** (R1, trimmer, R2, output on the wiper), or a **resistor** (Rs + P, Rs + (P ‖ Rp) or (Rs + P) ‖ Rp) |

The Nearest, Series, Parallel and Divider searches, default error limits and value notation were
recovered from the original program, so results line up with ResCad's.

### Trimmer circuits

When fixed resistors aren't precise enough, a trimmer padded with fixed resistors gives a circuit
you can adjust onto the target, with fine resolution because the trimmer only covers a narrow range.
The Trim tab finds combinations whose range covers the target ± the **trim range**, even with the
trimmer 20 % below its nominal value. The target must also fall in the middle half of the
trimmer's travel. Trimmers come from the usual 1-2-5 values (10R to 2M).

- The trim range defaults to **auto**, which covers the tolerance of the coarsest selected series,
  e.g. ±1 % for E96. For a divider that's 2 × (1 − ratio) × tolerance, since both resistors can drift.
- Results are sorted by **±adj %**, half the adjustment range relative to the target: smaller means
  finer adjustment. It's green when it's close to the minimum the trim range allows.
- **Wiper** is where the target falls on the trimmer's travel, so 50 % is mid-travel.
- In divider mode, **RO** is the output resistance at the target setting, which must lie between
  the min and max.

**What's new compared with the original:**

- Linux, Windows and macOS, as a single file with nothing to install
- Results update live as you type. Pressing Enter isn't needed any more.
- Three-resistor networks and dividers, and trimmable dividers and resistors
- E48 added to the E6, E12, E24, E96 and E192 series. Series can be combined, e.g. E12 + E96.
- Results are sorted by actual error and colour-coded green, amber or red
- Extra columns show the combined value (R1+R2, R1‖R2, and the divider's actual ratio and RO)
- A colour-band preview of the selected resistors: 4-band, or 5-band for 3-digit values
- An adjustable max-error limit
- Copy one row, or all rows as tab-separated text, ready to paste into a spreadsheet
- Light and dark themes, which follow the system by default

## Entering values

`4k7`, `4.7k`, `2R2`, `1M5`, `470`, `33 kΩ` and `k47` are all accepted. The prefixes are
`p n u m R k M G T`, where `m` is milli and `M` is mega. A prefix letter can stand in for the
decimal point, as on resistor markings.

## Keyboard

| Keys | Action |
|---|---|
| Alt+N / S / P / W / D / T / A | Nearest / Series / Parallel / Network / Divider / Trim / About |
| Ctrl+Tab | Next tab |
| Alt+6 / 1 / 4 / 8 / 9 / 2 | Toggle E6 / E12 / E24 / E48 / E96 / E192 |
| ↑ ↓ PgUp PgDn | Move through the results while typing |
| Ctrl+C, double-click | Copy the selected result |
| Right-click | Copy a row, or all rows |
| Alt+C | Close |

## Platform notes

- **Linux:** one binary that needs only glibc 2.17 or newer, so any distribution from about 2012
  onwards works. It runs on both X11 and Wayland, and needs OpenGL, which every desktop has.
  Run `chmod +x` once if your browser drops the executable bit.
- **Windows:** one `.exe` that runs on Windows 10/11 with nothing to install and no extra DLLs.
  It needs an OpenGL 2 capable graphics driver, which any normal PC has. Some bare virtual
  machines don't.
- **macOS:** `ResCAD NG.app` is a universal binary for Apple Silicon and Intel. It isn't notarised,
  so the first time, right-click it and choose **Open**.

## Web version

`web/` is a [Leptos](https://leptos.dev) front end over the same calculation code, compiled to
WebAssembly. It runs entirely in the browser: about 480 KB of WASM, 170 KB gzipped, with no server
logic. It has the same tabs, keyboard shortcuts and copy-to-spreadsheet behaviour, and the layout
adapts to phones.

It is published at **https://mayl.github.io/rescad-ng/** by `.github/workflows/pages.yml`, which
builds `.#web` with Nix on every push to `main`.

```sh
nix run .#serve-web                   # build the site and serve it on http://127.0.0.1:8080/
nix build .#web                       # just the static site, in ./result: host it anywhere
cd web && nix develop .. -c trunk serve   # development, with live reload
```

## Building

Everything builds with Nix, so you don't need to install anything on the host.

**Nix / NixOS:** the flake provides a package with a desktop entry and icon, for x86_64 and aarch64 Linux:

```sh
nix run github:mayl/rescad-ng        # run it without cloning
nix run .                            # or from a checkout
nix profile install github:mayl/rescad-ng   # put ResCAD NG in your application menu
```

On NixOS, add the flake input `rescad-ng.url = "github:mayl/rescad-ng"` and `inputs.rescad-ng.packages.${pkgs.system}.default` to `environment.systemPackages`.

**Portable executables and development:**

```sh
nix build .#windows                   # Windows .exe, cross-compiled with Nix's MinGW toolchain
nix develop -c cargo test --workspace --exclude rescad-web   # run the unit tests
nix develop -c cargo run              # run a debug build
nix develop -c ./build-dist.sh        # portable Linux + Windows executables into ./dist
```

The portable Linux binary is linked against glibc 2.17 using `cargo-zigbuild`. Nix's own glibc is
too new for a binary that has to run on other distributions, and that is the only thing zig is
used for. macOS can only be built on a Mac, so `.github/workflows/release.yml` builds all three
platforms on GitHub Actions. Pushing a `v*` tag publishes them as a release.

Without Nix, a stable Rust toolchain is enough for a native build: `cargo build --release`.

## Code layout

| File | Contents |
|---|---|
| `core/src/eseries.rs` | E-series tables (IEC 60063) and the nearest-value lookup |
| `core/src/calc.rs` | The Nearest, Series, Parallel, Network, Divider (2 and 3 resistors) and Trim searches |
| `core/src/units.rs` | Parsing and formatting of RKM-notation values |
| `core/src/colour_code.rs` | The colour-band code (IEC 60062) |
| `core/src/table.rs` | Calculator modes, input handling and result formatting, shared by both front ends |
| `src/app.rs` | The desktop user interface (egui) |
| `web/src/main.rs`, `web/style.css` | The browser user interface (Leptos) |
| `src/icon.rs`, `build.rs` | The procedurally drawn icon, and the Windows icon and version resource |
| `flake.nix`, `nix/write-icon.rs` | The Nix packages and dev shell, and the icon PNGs for the desktop entry |
