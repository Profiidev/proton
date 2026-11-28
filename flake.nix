{
  description = "Proton";

  nixConfig = {
    extra-substituters = [
      "https://projects.cache.profidev.io"
    ];

    extra-trusted-public-keys = [
      "profidev.cachix.org:tg4xEn64UMdvA5jJYT8omo/CQHk8+spLyeGT2YAku70="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    (flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages = rec {
          default = proton;
          proton = pkgs.callPackage ./packaging/package.nix { };
          # PR build check: no fat LTO and no optimization, it only proves the package builds
          ci = proton.overrideAttrs (old: {
            env = (old.env or { }) // {
              CARGO_PROFILE_RELEASE_LTO = "off";
              CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16";
              CARGO_PROFILE_RELEASE_OPT_LEVEL = "0";
            };
          });
        };
      }
    ))
    // {
      overlays.default = final: _: {
        proton-launcher = final.callPackage ./packaging/package.nix { };
      };
    };
}
