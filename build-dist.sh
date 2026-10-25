#!/usr/bin/env bash
# Build the portable release executables into ./dist.
# Run inside the Nix dev shell:  nix develop -c ./build-dist.sh
set -euo pipefail
cd "$(dirname "$0")"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
mkdir -p dist

# Linux: linked against glibc 2.17 so it runs on any distro from ~2012 onwards.
# This is the only thing zig is used for: Nix's own glibc is too new to target.
cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17
cp target/x86_64-unknown-linux-gnu/release/rescad-ng "dist/rescad-ng-$version-linux-x86_64"

# Windows: cross-compiled by Nix's MinGW toolchain; needs only DLLs that ship with Windows.
out=$(nix build .#windows --no-link --print-out-paths)
install -m 755 "$out/bin/rescad-ng.exe" "dist/rescad-ng-$version-windows-x86_64.exe"

ls -lh dist
