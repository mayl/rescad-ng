//! Writes the application icon as raw RGBA to stdout, for the Nix package's
//! hicolor PNGs.  Usage: `write-icon <size>`.

#[path = "../src/icon.rs"]
mod icon;

use std::io::Write;

fn main() {
    let size = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("usage: write-icon <size>");
    std::io::stdout().write_all(&icon::render(size)).unwrap();
}
