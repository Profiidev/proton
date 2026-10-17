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
  packages = buildDeps ++ gpuiLibs ++ mcLibs;

  languages = {
    rust = {
      enable = true;
      channel = "stable";
    };

    # oxfmt/oxlint (and the old Svelte app until it is deleted)
    javascript = {
      enable = true;
      npm = {
        enable = true;
      };
    };
  };

  env.LD_LIBRARY_PATH = lib.makeLibraryPath (gpuiLibs ++ mcLibs);
}
