{ pkgs, lib, ... }:

let
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
    xorg.libX11
    xorg.libXcursor
    xorg.libXext
    xorg.libXrandr
    xorg.libXxf86vm
    xorg.libXrender
    xorg.libXtst
    xorg.libXi
    freetype

    udev # oshi

    vulkan-loader # VulkanMod's lwjgl

    ## narrator
    flite
  ];

  otherLibs = with pkgs; [
    openssl
    at-spi2-atk
    atkmm
    cairo
    gdk-pixbuf
    glib
    gtk3
    harfbuzz
    librsvg
    libsoup_3
    pango
    webkitgtk_4_1
    wayland
    libxkbcommon
  ];
in
{
  packages =
    with pkgs;
    [
      pkg-config
    ]
    ++ mcLibs
    ++ otherLibs;

  languages = {
    rust = {
      enable = true;
      channel = "stable";
    };

    javascript = {
      enable = true;
      npm = {
        enable = true;
      };
    };
  };

  env = {
    LD_LIBRARY_PATH = "${lib.makeLibraryPath mcLibs}:$LD_LIBRARY_PATH";
  };
}
