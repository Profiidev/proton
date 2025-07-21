{ pkgs, ... }:

{
  packages = with pkgs; [
    pkg-config
    openssl
    systemd
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
    nodejs_22
    wrapGAppsHook4
  ];
}
