{
  description = "Development environment for the language learning app";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        openglLibraries = with pkgs; [ mesa libglvnd ];

        # Added essential libraries for dynamic linking
        libraries = with pkgs;
          [
            webkitgtk_4_1
            gtk3
            cairo
            gdk-pixbuf
            glib
            dbus
            openssl
            librsvg
            zlib
            stdenv.cc.cc.lib
          ] ++ openglLibraries;

        packages = with pkgs; [
          curl
          wget
          pkg-config
          dbus
          openssl
          glib
          gtk3
          libsoup
          webkitgtk
          librsvg
          mesa
          libglvnd
          rustc
          cargo
          rustfmt
          rust-analyzer
          rustup
          nodejs-18_x
          yarn
          alsaLib
          webrtc-audio-processing
          libclang
          nixd
          cmake
          nixfmt
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
        ];
      in {
        devShell = pkgs.mkShell {
          buildInputs = packages;

          # Modified shellHook with nix-ld configuration
          shellHook = ''
            export RUST_SRC_PATH="${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            export PATH="$HOME/.cargo/bin:$PATH"

            # Combined library paths
            export LD_LIBRARY_PATH=${
              pkgs.lib.makeLibraryPath libraries
            }:$LD_LIBRARY_PATH:$NIX_LD_LIBRARY_PATH

            export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
            export XDG_DATA_DIRS=${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS

            # WebKit environment variables
            export WEBKIT_DISABLE_DMABUF_RENDERER=1
            export WEBKIT_DISABLE_COMPOSITING_MODE=1
          '';
        };

        packages.${system}.default = pkgs.stdenv.mkDerivation {
          name = "language-learning-app";
          src = ./.;

          nativeBuildInputs = [
            pkgs.nodejs-18_x
            pkgs.rustc
            pkgs.cargo
            pkgs.rustup
            pkgs.cmake
            pkgs.nixfmt
          ];

          buildInputs = [ # Keep existing build inputs
          ];

          buildPhase = ''
            npm install
            npm run tauri build
          '';

          installPhase = ''
            mkdir -p $out/bin
            cp -r src-tauri/target/release/* $out/bin/
          '';
        };
      });
}
