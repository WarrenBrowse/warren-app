# Warren VPN for NixOS, built from the .deb the Linux release job already
# produces. Nothing is recompiled: the store path holds the very bytes that
# ship to every other Linux distribution, relinked against the Nix store.
#
# Building from source would mean a second Rust + Electron toolchain to keep in
# step with build.sh, and a NixOS user would run a binary nobody else runs.
# Unpacking the release artifact is how nixpkgs packages Mullvad, the app this
# is forked from, for the same reasons.
{
  lib,
  stdenv,
  fetchurl,
  dpkg,
  patchelf,
  makeWrapper,
  coreutils,
  gnugrep,

  alsa-lib,
  at-spi2-atk,
  at-spi2-core,
  atk,
  cairo,
  cups,
  dbus,
  expat,
  fontconfig,
  freetype,
  gdk-pixbuf,
  glib,
  gtk3,
  libGL,
  libappindicator,
  libdrm,
  libgbm,
  libnotify,
  libpulseaudio,
  libsecret,
  libxkbcommon,
  nspr,
  nss,
  pango,
  pipewire,
  systemd,
  vulkan-loader,
  wayland,
  xorg,

  release,
  # Overridable so a local build can use an artifact on disk instead of the
  # published one. Users never set it: the default is the pinned release.
  #
  # Deliberately not called `src`: callPackage resolves every argument against
  # nixpkgs first, and nixpkgs has a package named `src`, so the default here
  # would be silently replaced by it.
  debArchive ? fetchurl { inherit (release) url hash; },
}:

let
  inherit (release)
    version
    productName
    packageName
    suffix
    ;

  # Everything the daemon, the CLI and the Electron runtime need: what they link
  # directly, plus what Chromium dlopen's (no ELF header names those, so they
  # have to be on the RPATH explicitly).
  deps = [
    alsa-lib
    at-spi2-atk
    at-spi2-core
    atk
    cairo
    cups
    dbus
    expat
    fontconfig
    freetype
    gdk-pixbuf
    glib
    gtk3
    libappindicator
    libdrm
    libgbm
    libnotify
    libxkbcommon
    nspr
    nss
    pango
    systemd
    xorg.libX11
    xorg.libXScrnSaver
    xorg.libXcomposite
    xorg.libXcursor
    xorg.libXdamage
    xorg.libXext
    xorg.libXfixes
    xorg.libXi
    xorg.libXrandr
    xorg.libXrender
    xorg.libXtst
    xorg.libxcb
    xorg.libxshmfence

    (lib.getLib systemd)
    libGL
    libpulseaudio
    libsecret
    pipewire
    vulkan-loader
    wayland
    stdenv.cc.cc
  ];

  libraryPath = lib.makeLibraryPath deps;

  # Where a NEEDED entry may legitimately be found: the RPATH, plus the C
  # library the interpreter loads from its own default path.
  lookupPath = lib.makeLibraryPath (deps ++ [ stdenv.cc.libc ]);
in

stdenv.mkDerivation {
  pname = packageName;
  inherit version;
  src = debArchive;

  nativeBuildInputs = [
    dpkg
    makeWrapper
    patchelf
  ];

  dontBuild = true;
  dontConfigure = true;

  unpackPhase = ''
    runHook preUnpack
    dpkg-deb -x "$src" .
    runHook postUnpack
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out/share/warren $out/bin

    mv usr/share/* $out/share
    mv usr/bin/* $out/bin
    mv "opt/${productName}"/* $out/share/warren

    # The GUI launcher resolves its Electron binary relative to its own path, so
    # both have to sit in the same directory.
    ln -s $out/share/warren/${packageName} $out/bin/
    ln -s $out/share/warren/warren-gui${suffix} $out/bin/
    # The .deb ships this one as an absolute symlink into /opt, which resolves
    # to nothing on NixOS.
    ln -sf $out/share/warren/resources/warren-problem-report \
      $out/bin/warren-problem-report${suffix}

    # NetworkManager VPN service plugin, which is what makes the desktop show
    # a VPN is up. NetworkManager reads the .name from lib/NetworkManager/VPN
    # of every package listed in networking.networkmanager.plugins, and spawns
    # the absolute path the file names, so that path is rewritten into the
    # store here.
    mkdir -p $out/lib/warren-vpn $out/lib/NetworkManager/VPN
    mv usr/lib/warren-vpn${suffix}/warren-nm-vpn-service $out/lib/warren-vpn/
    substitute usr/lib/NetworkManager/VPN/warren-vpn${suffix}.name \
      $out/lib/NetworkManager/VPN/warren-vpn${suffix}.name \
      --replace-fail /usr/lib/warren-vpn${suffix}/warren-nm-vpn-service \
        $out/lib/warren-vpn/warren-nm-vpn-service

    runHook postInstall
  '';

  postFixup = ''
    # Relink every prebuilt ELF against the store.
    #
    # autoPatchelfHook is the usual way, and it is deliberately not used here:
    # its worker is a Python program, and the release pipeline builds this
    # package under x86_64 emulation where the interpreter loses argv[0] across
    # exec, so it cannot find its own modules. Setting the interpreter and the
    # RPATH by hand is what nixpkgs does for the Electron binary itself, needs
    # no Python, and behaves the same on every host.
    #
    # $out/share/warren is on the RPATH because the Electron runtime loads its
    # own libffmpeg/libEGL/libGLESv2 from beside itself.
    interpreter="$(cat "$NIX_CC/nix-support/dynamic-linker")"
    rpath="${libraryPath}:$out/share/warren"
    while IFS= read -r elf; do
      # patchelf refuses anything that is not an ELF, which is the cheapest way
      # to tell the binaries apart from the asar bundle and the icons.
      patchelf --print-rpath "$elf" > /dev/null 2>&1 || continue
      if patchelf --print-interpreter "$elf" > /dev/null 2>&1; then
        patchelf --set-interpreter "$interpreter" "$elf"
      fi
      patchelf --set-rpath "$rpath" "$elf"
    done < <(find $out -type f)

    # Without autoPatchelfHook nothing fails the build on a library nobody
    # provides, so the completeness check has to be explicit. A missing
    # dependency would otherwise reach the user as an app that does not start.
    # Resolution is done against an index of the search path rather than with
    # ldd, which is not on PATH in a build and would silently check nothing.
    libIndex="$NIX_BUILD_TOP/lib-index"
    # Split on the colons: a literal string is one word however many separators
    # it contains, so the path has to go through a variable to be split at all.
    lookupDirs="${lookupPath}:$out/share/warren"
    ( IFS=:
      for dir in $lookupDirs; do
        [ -d "$dir" ] && ls -1 "$dir"
      done
    ) > "$libIndex"

    missing=0
    while IFS= read -r elf; do
      patchelf --print-rpath "$elf" > /dev/null 2>&1 || continue
      for needed in $(patchelf --print-needed "$elf"); do
        if ! grep -qxF "$needed" "$libIndex"; then
          echo "$elf needs $needed, which nothing on the RPATH provides" >&2
          missing=1
        fi
      done
    done < <(find $out -type f)
    if [ "$missing" -ne 0 ]; then
      echo "add the missing libraries to deps in package.nix" >&2
      exit 1
    fi

    # The in-app updater would offer a download that cannot replace a read-only
    # store path. On NixOS the flake is the update channel.
    wrapProgram $out/bin/${packageName} \
      --set MULLVAD_DISABLE_UPDATE_NOTIFICATION 1 \
      --prefix PATH : ${
        lib.makeBinPath [
          coreutils
          gnugrep
        ]
      }

    # The daemon reads its bundled resources (the signed bootstrap exit list,
    # the problem-report helper) from this directory. --set-default so the
    # NixOS module or an operator can still point it elsewhere.
    wrapProgram $out/bin/warren-daemon${suffix} \
      --set-default WARREN_RESOURCE_DIR "$out/share/warren/resources"

    # Wayland is opt-in through NIXOS_OZONE_WL, the switch every Electron
    # package in nixpkgs honours.
    wrapProgram $out/bin/warren-gui${suffix} \
      --add-flags "\''${NIXOS_OZONE_WL:+\''${WAYLAND_DISPLAY:+--enable-features=UseOzonePlatform --ozone-platform=wayland --enable-wayland-ime=true}}"

    # electron-builder quotes the path, because the product name has spaces in
    # it. --replace-fail so a change to that line breaks the build instead of
    # shipping a launcher entry pointing into a /opt that NixOS does not have.
    desktopExec='Exec="/opt/${productName}/${packageName}"'
    substituteInPlace $out/share/applications/${packageName}.desktop \
      --replace-fail "$desktopExec" "Exec=$out/bin/${packageName}"
  '';

  # Read by the NixOS module: a package that carries neither is the wrong
  # package, and saying so at evaluation time beats a service that never starts.
  passthru = {
    hasWarrenDaemon = true;
    hasWarrenGui = true;
    inherit release;
  };

  meta = {
    description = "Warren VPN desktop client${
      lib.optionalString (suffix != "") " (${release.channel} channel)"
    }";
    homepage = "https://warren.ro";
    sourceProvenance = with lib.sourceTypes; [ binaryNativeCode ];
    license = lib.licenses.gpl3Only;
    mainProgram = packageName;
    platforms = [ "x86_64-linux" ];
  };
}
