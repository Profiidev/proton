{
  description = "Proton";

  nixConfig = {
    extra-substituters = [
      "https://profidev.cachix.org"
    ];

    extra-trusted-public-keys = [
      "profidev.cachix.org-1:xdwadal2vlCD50JtDTy8NwjOJvkOtjdjy1y91ElU9GE="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    nix-filter.url = "github:numtide/nix-filter";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      nix-filter,
      ...
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
          version = "0.2.4";

          src = nix-filter {
            root = ./.;
            include = [
              "package.json"
              "package-lock.json"
              "Cargo.toml"
              "Cargo.lock"
              "app/src"
              "app/src-tauri"
              "app/static"
              "app/package.json"
              "app/svelte.config.js"
              "app/tsconfig.json"
              "app/vite.config.js"
              "backend/Cargo.toml"
            ];
          };

          npmDeps = pkgs.importNpmLock {
            npmRoot = src;
          };

          nativeBuildInputs = with pkgs; [
            cargo-tauri.hook
            nodejs
            pkg-config
            importNpmLock.npmConfigHook
            npmHooks.npmInstallHook
            cargo-tauri.hook
            wrapGAppsHook4
          ];

          runtimeDependencies = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux (
            pkgs.lib.makeLibraryPath (
              with pkgs;
              [
                addDriverRunpath.driverLink

                # glfw
                libGL
                xorg.libX11
                xorg.libXcursor
                xorg.libXext
                xorg.libXrandr
                xorg.libXxf86vm
                xorg.libXrender
                xorg.libXtst
                xorg.libXi
                freetype

                # lwjgl
                (lib.getLib stdenv.cc.cc)

                # narrator support
                flite

                # openal
                alsa-lib
                libjack2
                libpulseaudio
                pipewire

                # oshi
                udev

                vulkan-loader # VulkanMod's lwjgl
                ## native versions
                glfw3-minecraft
                openal
                wayland
                libxkbcommon
              ]
            )
          );

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

          postInstall = ''
            gappsWrapperArgs+=(
              ${pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.xorg.xrandr ]}
                --set LD_LIBRARY_PATH ${runtimeDependencies}
              ''}
            )

            wrapGAppsHook
          '';

          fixupPhase = ''
            substituteInPlace "$out/share/applications/proton.desktop" \
              --replace-fail "Exec=proton" "Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 proton"
          '';
        };
      }
    );
}
