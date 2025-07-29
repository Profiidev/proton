{
  description = "Proton";

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
        jdks = with pkgs; [
          jdk8
          jdk17
          jdk21
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage rec {
          pname = "proton";
          version = "0.1.0";

          src = ./.;

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

          postBuild = ''
            gappsWrapperArgs+=(
              --prefix PATH : ${pkgs.lib.makeSearchPath "bin/java" jdks}
              ${pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.xorg.xrandr ]}
                --set LD_LIBRARY_PATH $runtimeDependencies
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
