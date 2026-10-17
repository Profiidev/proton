{
  lib,
  stdenv,
  rustPlatform,
  pkg-config,
  makeWrapper,
  fontconfig,
  freetype,
  libxkbcommon,
  libxcb,
  wayland,
  vulkan-loader,
  addDriverRunpath,
  # launched Minecraft (lwjgl natives)
  libGL,
  libx11,
  libxcursor,
  libxext,
  libxrandr,
  libxxf86vm,
  libxrender,
  libxtst,
  libxi,
  flite,
  alsa-lib,
  libjack2,
  libpulseaudio,
  pipewire,
  udev,
  glfw3-minecraft,
  openal,
}:

let
  # gpui dlopens these
  gpuiLibs = [
    wayland
    vulkan-loader
    libxkbcommon
  ];

  mcLibs = [
    addDriverRunpath.driverLink
    # glfw
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
    # native versions
    glfw3-minecraft
    openal
    wayland
    libxkbcommon
  ];
in

rustPlatform.buildRustPackage {
  pname = "proton";
  version = (lib.importTOML ../Cargo.toml).workspace.package.version;

  src = ./..;

  cargoLock.lockFile = ../Cargo.lock;

  cargoBuildFlags = [
    "--package"
    "proton"
  ];

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    fontconfig
    freetype
    libxkbcommon
    libxcb
  ];

  # the gpui tests open windows, CI runs them (tests.yml)
  doCheck = false;

  postInstall = ''
    install -Dm644 packaging/icons/32x32.png $out/share/icons/hicolor/32x32/apps/proton.png
    install -Dm644 packaging/icons/128x128.png $out/share/icons/hicolor/128x128/apps/proton.png
    install -Dm644 packaging/icons/128x128@2x.png $out/share/icons/hicolor/256x256/apps/proton.png
    install -Dm644 packaging/proton.desktop $out/share/applications/proton.desktop
    substituteInPlace $out/share/applications/proton.desktop \
      --replace-fail "{{categories}}" "Game;" \
      --replace-fail "{{exec}}" "$out/bin/proton" \
      --replace-fail "{{icon}}" "proton" \
      --replace-fail "{{name}}" "Proton"
    sed -i '/{{/d' $out/share/applications/proton.desktop
  '';

  postFixup = ''
    patchelf --add-rpath ${lib.makeLibraryPath gpuiLibs} $out/bin/proton
    wrapProgram $out/bin/proton \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath mcLibs}
  '';

  meta = {
    description = "Minecraft launcher";
    homepage = "https://github.com/Profiidev/proton";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "proton";
  };
}
