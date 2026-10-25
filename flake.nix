{
  description = "ResCAD NG - next-generation resistor calculator (Rust rewrite of Vader Systems' ResCad)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system:
        f (import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        }));

      # The wasm-bindgen CLI must match the wasm-bindgen crate in Cargo.lock
      # exactly, so build that version rather than whatever nixpkgs has.
      wasmBindgenCli = pkgs:
        let
          src = pkgs.fetchCrate {
            pname = "wasm-bindgen-cli";
            version = "0.2.129";
            hash = "sha256-pcecKQd7E8Opw6bkFoE569epUi7gh5qpQF1e5PJY6V8=";
          };
        in
        pkgs.buildWasmBindgenCli {
          inherit src;
          cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
            inherit src;
            inherit (src) pname version;
            hash = "sha256-vmUrWVU7kPJJxO5qIVeAkwQyWDELO1Z4Z5gitz2kco8=";
          };
        };

      # Libraries the GUI loads at runtime via dlopen.  The package bakes them
      # into the binary's RPATH; the dev shell puts them on LD_LIBRARY_PATH.
      runtimeLibs = pkgs: with pkgs; [
        libGL
        libxkbcommon
        wayland
        libx11
        libxcursor
        libxi
        libxrandr
      ];
    in
    {
      packages = forAllSystems (pkgs:
        let
          toolchain = pkgs.rust-bin.stable.latest.minimal;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          # The whole workspace: Cargo needs every member's manifest and sources.
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./build.rs
              ./src
              ./core
              ./web
              ./nix
            ];
          };
          desktopItem = pkgs.makeDesktopItem {
            name = "rescad-ng";
            desktopName = "ResCAD NG";
            genericName = "Resistor Calculator";
            comment = cargoToml.package.description;
            exec = "rescad-ng";
            icon = "rescad-ng";
            categories = [ "Development" "Electronics" "Engineering" ];
            keywords = [ "resistor" "E12" "E24" "E96" "divider" ];
            startupWMClass = "rescad-ng";
          };
          rescad-ng = rustPlatform.buildRustPackage {
            pname = "rescad-ng";
            version = cargoToml.package.version;

            inherit src;
            cargoLock.lockFile = ./Cargo.lock;
            cargoTestFlags = [ "--workspace" "--exclude" "rescad-web" ];

            nativeBuildInputs = [ pkgs.imagemagick pkgs.copyDesktopItems ];
            desktopItems = [ desktopItem ];

            postInstall = ''
              rustc -O -o write-icon nix/write-icon.rs
              for size in 16 24 32 48 64 128 256; do
                dir=$out/share/icons/hicolor/''${size}x''${size}/apps
                mkdir -p $dir
                ./write-icon $size | magick -size ''${size}x''${size} -depth 8 rgba:- $dir/rescad-ng.png
              done
            '';

            # winit/glutin dlopen these, so a plain RPATH entry is enough.
            postFixup = ''
              patchelf --add-rpath ${pkgs.lib.makeLibraryPath (runtimeLibs pkgs)} $out/bin/rescad-ng
            '';

            meta = {
              description = cargoToml.package.description;
              mainProgram = "rescad-ng";
              platforms = systems;
            };
          };

          # The browser version: a static site (index.html, JS, WASM, CSS) to
          # serve from anywhere, e.g. `nix run .#serve-web` or GitHub Pages.
          web =
            let
              wasmToolchain = pkgs.rust-bin.stable.latest.minimal.override {
                targets = [ "wasm32-unknown-unknown" ];
              };
              wasmPlatform = pkgs.makeRustPlatform {
                cargo = wasmToolchain;
                rustc = wasmToolchain;
              };
            in
            wasmPlatform.buildRustPackage {
              pname = "rescad-web";
              version = cargoToml.package.version;
              inherit src;
              cargoLock.lockFile = ./Cargo.lock;
              nativeBuildInputs = [ pkgs.trunk pkgs.binaryen (wasmBindgenCli pkgs) ];
              buildPhase = ''
                runHook preBuild
                export HOME=$TMPDIR
                cd web
                trunk build --release --offline --dist $TMPDIR/site
                cd ..
                runHook postBuild
              '';
              installPhase = ''
                runHook preInstall
                cp -r $TMPDIR/site $out
                runHook postInstall
              '';
              doCheck = false;
              meta.description = "ResCAD NG for the browser (static site)";
            };

          # Serve the web version locally and print its address.
          serve-web = pkgs.writeShellApplication {
            name = "rescad-serve-web";
            runtimeInputs = [ pkgs.python3 ];
            text = ''
              port=''${1:-8080}
              echo "ResCAD NG: http://127.0.0.1:$port/"
              exec python3 -m http.server "$port" --bind 127.0.0.1 --directory ${web}
            '';
          };
        in
        {
          inherit rescad-ng web serve-web;
          default = rescad-ng;
        }
        // pkgs.lib.optionalAttrs (pkgs.stdenv.hostPlatform.system == "x86_64-linux") {
          # Portable Windows .exe, cross-compiled with nixpkgs' MinGW toolchain.
          windows = pkgs.pkgsCross.mingwW64.rustPlatform.buildRustPackage {
            pname = "rescad-ng";
            version = cargoToml.package.version;
            src = rescad-ng.src;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = false;
          };
        });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/rescad-ng";
        };
      });

      devShells.x86_64-linux.default =
        let
          pkgs = import nixpkgs {
            system = "x86_64-linux";
            overlays = [ rust-overlay.overlays.default ];
          };
          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "clippy" "rustfmt" ];
            targets = [ "x86_64-unknown-linux-gnu" "wasm32-unknown-unknown" ];
          };
        in
        pkgs.mkShell {
          packages = [
            rust
            # Only for the portable Linux binary in build-dist.sh, which must
            # link against an old glibc (2.17); Windows builds via .#windows.
            pkgs.zig
            pkgs.cargo-zigbuild
            # The web version: `cd web && trunk serve`.
            pkgs.trunk
            pkgs.binaryen
            (wasmBindgenCli pkgs)
          ];
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (runtimeLibs pkgs);
        };
    };
}
