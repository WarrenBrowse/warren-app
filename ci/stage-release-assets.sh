#!/usr/bin/env bash
# Stage built installers into release-assets/ under harmonized, OS- and
# arch-explicit names, so every GitHub Release asset clearly states its target
# platform at a glance:
#
#   WarrenVPN-1.0.10-windows-x64.exe
#   WarrenVPN-1.0.10-windows-arm64.exe
#   WarrenVPN-1.0.10-windows-universal.exe
#   WarrenVPN-1.0.10-macos-universal.pkg
#   WarrenVPN-1.0.10-linux-amd64.deb
#   WarrenVPN-1.0.10-linux-x86_64.rpm
#   WarrenVPN-1.0.10-android.apk
#   WarrenVPN-1.0.10-android.aab
#
# Usage: ci/stage-release-assets.sh <macos|linux|windows|android> <version>
#
# <version> is the release version WITHOUT a leading "v" (e.g. 1.0.10); the
# build jobs pass "${GITHUB_REF_NAME#v}". A single version token is used across
# every platform so the asset set is uniform regardless of each tool's internal
# naming. The upstream build.sh names desktop installers
# WarrenVPN-<ver>[_<arch>].<ext>; the Android Gradle output is
# app-prod-{debug,release}.{apk,aab} with no version in the name at all. This
# script normalises all of them into one place with consistent names.
set -euo pipefail
shopt -s nullglob

platform="${1:?usage: stage-release-assets.sh <macos|linux|windows|android> <version>}"
version="${2:?missing version (e.g. 1.0.10)}"
OUT="release-assets"
mkdir -p "$OUT"

stage() { # stage <srcfile> <dstname>
  cp -f "$1" "$OUT/$2"
  echo "  staged $(basename "$1") -> $2"
}

case "$platform" in
  macos)
    # macOS ships a single universal (x86_64 + arm64) installer.
    for f in dist/WarrenVPN-*.pkg; do stage "$f" "WarrenVPN-${version}-macos-universal.pkg"; done
    for f in dist/WarrenVPN-*.dmg; do stage "$f" "WarrenVPN-${version}-macos-universal.dmg"; done
    ;;
  linux)
    for f in dist/WarrenVPN-*_amd64.deb;  do stage "$f" "WarrenVPN-${version}-linux-amd64.deb";  done
    for f in dist/WarrenVPN-*_x86_64.rpm; do stage "$f" "WarrenVPN-${version}-linux-x86_64.rpm"; done
    # Daemon-only packages (only produced by `build.sh --daemon-only`; absent in
    # the normal full build, harmlessly skipped by nullglob).
    for f in dist/warren-vpn-daemon_*_amd64.deb;   do stage "$f" "warren-vpn-daemon-${version}-linux-amd64.deb";   done
    for f in dist/warren-vpn-daemon_*_arm64.deb;   do stage "$f" "warren-vpn-daemon-${version}-linux-arm64.deb";   done
    for f in dist/warren-vpn-daemon_*_x86_64.rpm;  do stage "$f" "warren-vpn-daemon-${version}-linux-x86_64.rpm";  done
    for f in dist/warren-vpn-daemon_*_aarch64.rpm; do stage "$f" "warren-vpn-daemon-${version}-linux-aarch64.rpm"; done
    ;;
  windows)
    # Single-arch NSIS installers...
    for f in dist/WarrenVPN-*_x64.exe;   do stage "$f" "WarrenVPN-${version}-windows-x64.exe";   done
    for f in dist/WarrenVPN-*_arm64.exe; do stage "$f" "WarrenVPN-${version}-windows-arm64.exe"; done
    # ...and the universal installer-downloader (the bare WarrenVPN-<ver>.exe,
    # i.e. the one WITHOUT an _x64/_arm64 arch suffix).
    for f in dist/WarrenVPN-*.exe; do
      case "$f" in *_x64.exe | *_arm64.exe) continue ;; esac
      stage "$f" "WarrenVPN-${version}-windows-universal.exe"
    done
    for f in dist/WarrenVPN-*.msi; do stage "$f" "WarrenVPN-${version}-windows-x64.msi"; done
    ;;
  android)
    # Gradle output carries no version in the filename and only one universal
    # APK is produced; prefer the signed release artifact, fall back to debug.
    for f in android/app/build/outputs/apk/prod/release/*.apk \
             android/app/build/outputs/apk/prod/debug/*.apk \
             android/app/build/outputs/apk/*/release/*.apk \
             android/app/build/outputs/apk/*/debug/*.apk; do
      stage "$f" "WarrenVPN-${version}-android.apk"
      break
    done
    for f in android/app/build/outputs/bundle/prodRelease/*.aab \
             android/app/build/outputs/bundle/*/*.aab; do
      stage "$f" "WarrenVPN-${version}-android.aab"
      break
    done
    ;;
  *)
    echo "unknown platform: $platform" >&2
    exit 2
    ;;
esac

if [ -z "$(ls -A "$OUT" 2>/dev/null)" ]; then
  # A platform build that produced no installer is a real failure: fail loudly
  # here rather than publishing an incomplete release. This is the backstop that
  # would have caught the Linux build silently shipping nothing after ring failed
  # to compile (masked by the build step's `| tee` losing build.sh's exit code).
  echo "::error::no installers found to stage for platform=$platform; the build produced no artifacts"
  exit 1
else
  echo "Staged release assets:"
  ls -1 "$OUT"
fi
