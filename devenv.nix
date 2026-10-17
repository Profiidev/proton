{ pkgs, lib, ... }:

let
  buildDeps = with pkgs; [
    pkg-config
    fontconfig
    freetype
    libxkbcommon
    libxcb
  ];

  # gpui dlopens these
  gpuiLibs = with pkgs; [
    wayland
    vulkan-loader
    libxkbcommon
  ];

  # launched Minecraft (lwjgl natives)
  mcLibs = with pkgs; [
    ## native versions
    glfw3-minecraft
    openal

    ## openal
    alsa-lib
    libjack2
    libpulseaudio
    pipewire

    ## glfw
    libGL
    libx11
    libxcursor
    libxext
    libxrandr
    libxxf86vm
    libxrender
    libxtst
    libxi
    freetype

    udev # oshi

    wayland
    vulkan-loader # VulkanMod's lwjgl

    ## narrator
    flite
  ];
in
{
  # oxfmt: json/md/yaml formatting, no node needed
  packages = buildDeps ++ gpuiLibs ++ mcLibs ++ [
    pkgs.oxfmt
    pkgs.cargo-llvm-cov
  ];

  languages = {
    rust = {
      enable = true;
      channel = "stable";
      # cargo-llvm-cov
      components = [
        "rustc"
        "cargo"
        "clippy"
        "rustfmt"
        "rust-analyzer"
        "llvm-tools-preview"
      ];
    };
  };

  env.LD_LIBRARY_PATH = lib.makeLibraryPath (gpuiLibs ++ mcLibs);
}
