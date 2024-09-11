{
  description = "Development environment for the recordr app;

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        # Provide fallbacks for older Nix versions
        devShell = self.devShells.${system}.default or pkgs.mkShell {
          nativeBuildInputs = [
            pkgs.nodejs-18_x 
            pkgs.rustc 
            pkgs.cargo 
          ];

          buildInputs = [
            pkgs.pkg-config 
          ];
        };

        packages.${system}.default = pkgs.stdenv.mkDerivation {
          name = "language-learning-app";
          src = ./.;

          nativeBuildInputs = [
            pkgs.nodejs-18_x
            pkgs.rustc
            pkgs.cargo
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