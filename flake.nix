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

        libraries = with pkgs;[
          webkitgtk
          gtk3
          cairo
          gdk-pixbuf
          glib
          dbus
          openssl_3
          librsvg
        ];

        packages = with pkgs; [
          curl
          wget
          pkg-config
          dbus
          openssl_3
          glib
          gtk3
          libsoup
          webkitgtk
          librsvg
          rustc
          cargo
          nodejs-18_x
          yarn
          alsaLib
          webrtc-audio-processing
          libclang
          rustup
          nixd
        ];
      in
      {
        devShell = self.devShells.${system}.default or pkgs.mkShell {
          buildInputs = packages;

          # Use a shellHook to append to PATH
          shellHook = ''
            export RUST_SRC_PATH="${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            export PATH="$HOME/.cargo/bin:$PATH"
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
            export LIBCLANG_PATH=${pkgs.libclang.lib}/lib
            export XDG_DATA_DIRS=${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS
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
          ];

          buildInputs = [
            # Add any additional build-time dependencies here
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
      }
    );
}
