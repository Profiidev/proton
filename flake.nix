{
  description = "A Nix flake for building a Tauri app with npm.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage rec {
          pname = "proton";
          version = "0.1.0";

          src = ./.; # Assuming your Tauri app source is in the current directory

          npmDeps = pkgs.fetchNpmDeps {
            #npmRoot = src;
            inherit src;
            #package = pkgs.lib.importJSON ./app/package.json;
            hash = "sha256-IqjSZ23klD9R3y38P/nNQk9Faer+lacO1VbDMYnDZWk=";
          };

          nativeBuildInputs = with pkgs; [
            cargo-tauri.hook
            nodejs
            pkg-config
            npmHooks.npmConfigHook
            npmHooks.npmInstallHook
            cargo-tauri.hook
          ];

          buildInputs = with pkgs; [
            webkitgtk_4_1
            openssl
          ];

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          # Compile fails due to perl missing
          doCheck = false;
          cargoTestFlags = [
            "--package"
            "proton-backend"
          ];

          meta = with pkgs.lib; {
            description = "A Tauri application built with npm";
            homepage = "https://tauri.app/";
            license = licenses.mit; # Adjust based on your app's license
            maintainers = with maintainers; [ ];
            platforms = platforms.linux ++ platforms.darwin ++ platforms.windows;
          };
        };
      }
    );
}
