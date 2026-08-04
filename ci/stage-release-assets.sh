#!/usr/bin/env bash
# SRC deliberately expands INSIDE the globs below: it carries a [0-9] glob
# class (the prod anchor), which quoting would turn into a literal.
# shellcheck disable=SC2231
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
# Usage: ci/stage-release-assets.sh <macos|linux|windows|android> <version> [prod|beta]
#
# <version> is the release version WITHOUT the tag's channel prefix (e.g.
# 1.0.10, from `v1.0.10` or `beta-v1.0.10`); the build jobs pass the version
# resolved by the release workflow. A single version token is used across
# every platform so the asset set is uniform regardless of each tool's internal
# naming. The upstream build.sh names desktop installers
# WarrenVPN-<ver>[_<arch>].<ext>; the Android Gradle output is
# app-prod-{debug,release}.{apk,aab} with no version in the name at all. This
# script normalises all of them into one place with consistent names.
#
# The optional third argument selects the compiled product environment of the
# artifacts to stage (default prod). "beta" stages the separately-installable
# beta build (a WARREN_PRODUCT_ENV=beta desktop build names its artifacts
# WarrenVPN-Beta-<ver>...; Android's beta flavor outputs live under the beta
# variant dirs), producing WarrenVPN-Beta-<ver>-<os>-<arch>.<ext>. The prod
# globs anchor on the version digit so they can never swallow a beta artifact
# sitting in the same dist/ directory.
set -euo pipefail
shopt -s nullglob

platform="${1:?usage: stage-release-assets.sh <macos|linux|windows|android> <version> [prod|beta]}"
version="${2:?missing version (e.g. 1.0.10)}"
env_mode="${3:-prod}"
OUT="release-assets"
mkdir -p "$OUT"

case "$env_mode" in
  prod)
    # Anchored on the version digit: never matches WarrenVPN-Beta-*.
    SRC="WarrenVPN-[0-9]"
    DST="WarrenVPN"
    ANDROID_FLAVOR="prod"
    ;;
  beta)
    SRC="WarrenVPN-Beta-"
    DST="WarrenVPN-Beta"
    ANDROID_FLAVOR="beta"
    ;;
  *)
    echo "unknown env mode: $env_mode (prod|beta)" >&2
    exit 2
    ;;
esac

staged_count=0
stage() { # stage <srcfile> <dstname>
  cp -f "$1" "$OUT/$2"
  echo "  staged $(basename "$1") -> $2"
  staged_count=$((staged_count + 1))
}

case "$platform" in
  macos)
    # macOS ships a single universal (x86_64 + arm64) installer.
    for f in dist/${SRC}*.pkg; do stage "$f" "${DST}-${version}-macos-universal.pkg"; done
    for f in dist/${SRC}*.dmg; do stage "$f" "${DST}-${version}-macos-universal.dmg"; done
    ;;
  linux)
    for f in dist/${SRC}*_amd64.deb;  do stage "$f" "${DST}-${version}-linux-amd64.deb";  done
    for f in dist/${SRC}*_x86_64.rpm; do stage "$f" "${DST}-${version}-linux-x86_64.rpm"; done
    # Arch Linux package (electron-builder pacman target). Installed with
    # `sudo pacman -U <file>`; the .pacman extension is electron-builder's, the
    # archive itself is a standard xz-compressed Arch package.
    for f in dist/${SRC}*.pacman; do stage "$f" "${DST}-${version}-linux-x86_64.pacman"; done
    # Daemon-only packages (only produced by `build.sh --daemon-only`; absent in
    # the normal full build, harmlessly skipped by nullglob). Prod-only: there
    # is no daemon-only beta channel.
    if [ "$env_mode" = "prod" ]; then
      for f in dist/warren-vpn-daemon_*_amd64.deb;   do stage "$f" "warren-vpn-daemon-${version}-linux-amd64.deb";   done
      for f in dist/warren-vpn-daemon_*_arm64.deb;   do stage "$f" "warren-vpn-daemon-${version}-linux-arm64.deb";   done
      for f in dist/warren-vpn-daemon_*_x86_64.rpm;  do stage "$f" "warren-vpn-daemon-${version}-linux-x86_64.rpm";  done
      for f in dist/warren-vpn-daemon_*_aarch64.rpm; do stage "$f" "warren-vpn-daemon-${version}-linux-aarch64.rpm"; done
    fi
    ;;
  windows)
    # Single-arch NSIS installers...
    for f in dist/${SRC}*_x64.exe;   do stage "$f" "${DST}-${version}-windows-x64.exe";   done
    for f in dist/${SRC}*_arm64.exe; do stage "$f" "${DST}-${version}-windows-arm64.exe"; done
    # ...and the universal installer-downloader (the bare WarrenVPN-<ver>.exe,
    # i.e. the one WITHOUT an _x64/_arm64 arch suffix).
    for f in dist/${SRC}*.exe; do
      case "$f" in *_x64.exe | *_arm64.exe) continue ;; esac
      stage "$f" "${DST}-${version}-windows-universal.exe"
    done
    for f in dist/${SRC}*.msi; do stage "$f" "${DST}-${version}-windows-x64.msi"; done
    ;;
  android)
    # Gradle output carries no version in the filename and only one universal
    # APK is produced per flavor; prefer the signed release artifact, fall
    # back to debug.
    for f in "android/app/build/outputs/apk/${ANDROID_FLAVOR}/release"/*.apk \
             "android/app/build/outputs/apk/${ANDROID_FLAVOR}/debug"/*.apk; do
      stage "$f" "${DST}-${version}-android.apk"
      break
    done
    for f in "android/app/build/outputs/bundle/${ANDROID_FLAVOR}Release"/*.aab; do
      stage "$f" "${DST}-${version}-android.aab"
      break
    done
    ;;
  *)
    echo "unknown platform: $platform" >&2
    exit 2
    ;;
esac

# A platform build that produced no installer is a real failure: fail loudly
# here rather than publishing an incomplete release. Counted per INVOCATION
# (not by the dir being non-empty): the beta pass appends to a dir the prod
# pass already filled, so an empty-dir check would let a broken beta build
# ship a prod-only release silently.
if [ "$staged_count" -eq 0 ]; then
  echo "::error::no installers found to stage for platform=$platform env=$env_mode; the build produced no artifacts"
  exit 1
else
  echo "Staged release assets:"
  ls -1 "$OUT"
fi
