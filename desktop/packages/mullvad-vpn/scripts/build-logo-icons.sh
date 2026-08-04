#!/usr/bin/env bash

# This script creates the macOS .icns from the icons in /graphics/macOS/ which need to be updated
# first if the source SVGs have been updated. More info about how to update them can be found in
# the readme.
#
# Icon guidelines for macOS:
# https://developer.apple.com/design/human-interface-guidelines/macos/icons-and-images/app-icon/
#
# Icon templates for macOS:
# https://developer.apple.com/design/resources/
#
# Icon guidelines for Windows:
# https://docs.microsoft.com/en-us/windows/uwp/design/style/app-icons-and-logos#target-size-app-icon-assets
# https://docs.microsoft.com/en-us/windows/win32/uxguide/vis-icons

echo "Press enter to continue if you've followed the instructions in graphics/README.md"
read -r

set -eu

if ! command -v convert > /dev/null; then
    echo >&2 "convert (imagemagick) is required to run this script"
    exit 1
fi

if ! command -v rsvg-convert > /dev/null; then
    echo >&2 "rsvg-convert (librsvg) is required to run this script"
    exit 1
fi

if ! command -v iconutil > /dev/null; then
    echo >&2 "iconutil is required to run this script"
    exit 1
fi

if ! command -v python3 > /dev/null; then
    echo >&2 "python3 is required to derive the beta icons"
    exit 1
fi



SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Anchored on the repo root: the relative paths this script used to carry
# broke when the GUI moved under desktop/packages/mullvad-vpn.
REPO_ROOT="$( cd "$SCRIPT_DIR/../../../.." && pwd )"
GRAPHICS_DIR="$REPO_ROOT/graphics"
DIST_ASSETS_DIR="$REPO_ROOT/dist-assets"
SVG_SOURCE_PATH="$GRAPHICS_DIR/icon.svg"
SQUARE_SVG_SOURCE_PATH="$GRAPHICS_DIR/icon-square.svg"
TMP_DIR=$(mktemp -d)
TMP_ICO_DIR="$TMP_DIR/ico"
TMP_ICONSET_DIR="$TMP_DIR/icon.iconset"

mkdir "$TMP_ICONSET_DIR"
mkdir "$TMP_ICO_DIR"

# An array, not a string: quoted as one word ImageMagick reads the whole thing
# as a single unrecognized option and the .ico step dies.
COMPRESSION_OPTIONS=(-define png:compression-filter=5 -define png:compression-level=9
    -define png:compression-strategy=1 -define png:exclude-chunk=all -strip)

# Below this pixel size the BETA lettering is a smudge, so the small renders
# take the badge disc alone. Apple asks for simplified small icons anyway, and
# an amber disc still reads as "not the prod build" at 16px.
BADGE_LABEL_MIN_PX=128

# Renders every size from `large_svg`, falling back to `small_svg` under the
# label threshold. Prod passes the same path twice.
svg_for_size() {
    local size="$1"
    local small_svg="$2"
    local large_svg="$3"

    if [ "$size" -ge "$BADGE_LABEL_MIN_PX" ]; then
        echo "$large_svg"
    else
        echo "$small_svg"
    fi
}

build_icns_from_svg() {
    local small_svg="$1"
    local large_svg="$2"
    local target_path="$3"
    local iconset_dir="$TMP_DIR/$(basename "$target_path" .icns).iconset"

    mkdir -p "$iconset_dir"
    for size in 16 32 128 256 512; do
        double_size="$((size * 2))"
        rsvg-convert -o "$iconset_dir"/icon-$size.png -w $size -h $size \
            "$(svg_for_size "$size" "$small_svg" "$large_svg")"
        rsvg-convert -o "$iconset_dir"/icon-$size@2x.png -w "$double_size" -h "$double_size" \
            "$(svg_for_size "$double_size" "$small_svg" "$large_svg")"
    done
    iconutil --convert icns --output "$target_path" "$iconset_dir"
    rm -rf "$iconset_dir"
}

build_ico_from_svg() {
    local small_svg="$1"
    local large_svg="$2"
    local target_path="$3"
    local ico_dir="$TMP_DIR/$(basename "$target_path" .ico).ico.d"

    mkdir -p "$ico_dir"
    for size in 16 20 24 30 32 36 40 48 60 64 72 80 96 256 512; do
        rsvg-convert -o "$ico_dir"/$size.png -w $size -h $size \
            "$(svg_for_size "$size" "$small_svg" "$large_svg")"
    done
    convert "$ico_dir"/* "${COMPRESSION_OPTIONS[@]}" "$target_path"
    rm -rf "$ico_dir"
}

# The beta artwork (palette swapped, BETA badge) is derived from the prod SVGs
# here rather than committed as its own files, so the two cannot drift apart.
make_beta_svg() {
    python3 "$GRAPHICS_DIR/make-beta-icon.py" --badge "$GRAPHICS_DIR/beta-badge.svg" "$@"
}

BETA_SVG_PATH="$TMP_DIR/icon-beta.svg"
BETA_SVG_SMALL_PATH="$TMP_DIR/icon-beta-small.svg"
BETA_SQUARE_SVG_PATH="$TMP_DIR/icon-square-beta.svg"
BETA_SQUARE_SVG_SMALL_PATH="$TMP_DIR/icon-square-beta-small.svg"
make_beta_svg --source "$SVG_SOURCE_PATH" --output "$BETA_SVG_PATH"
make_beta_svg --source "$SVG_SOURCE_PATH" --output "$BETA_SVG_SMALL_PATH" --no-label
make_beta_svg --source "$SQUARE_SVG_SOURCE_PATH" --output "$BETA_SQUARE_SVG_PATH"
make_beta_svg --source "$SQUARE_SVG_SOURCE_PATH" --output "$BETA_SQUARE_SVG_SMALL_PATH" --no-label

# macOS .icns icon
for icon in "$GRAPHICS_DIR/macOS"/*; do
    cp "$icon" "$TMP_ICONSET_DIR"/
done

iconutil --convert icns --output "$DIST_ASSETS_DIR/icon-macos.icns" "$TMP_ICONSET_DIR"
rm "$TMP_ICONSET_DIR"/*
rm -rf "$TMP_ICONSET_DIR"

# The prod macOS iconset is hand-exported from Apple's template into
# /graphics/macOS, so the beta one is rendered from the square SVG the
# template was built on. Keep that SVG in step with the exported PNGs.
build_icns_from_svg "$BETA_SQUARE_SVG_SMALL_PATH" "$BETA_SQUARE_SVG_PATH" \
    "$DIST_ASSETS_DIR/icon-macos-beta.icns"

# Linux .icns icon
build_icns_from_svg "$SVG_SOURCE_PATH" "$SVG_SOURCE_PATH" "$DIST_ASSETS_DIR/icon.icns"
build_icns_from_svg "$BETA_SVG_SMALL_PATH" "$BETA_SVG_PATH" "$DIST_ASSETS_DIR/icon-beta.icns"

# Windows .ico icon
build_ico_from_svg "$SVG_SOURCE_PATH" "$SVG_SOURCE_PATH" "$DIST_ASSETS_DIR/icon.ico"
build_ico_from_svg "$BETA_SVG_SMALL_PATH" "$BETA_SVG_PATH" "$DIST_ASSETS_DIR/icon-beta.ico"
rm -rf "$TMP_ICO_DIR"

# Windows installer sidebar
# "bmp3" specifies the Windows 3.x format which is required for the image to be displayed
build_installer_sidebar() {
    local svg_path="$1"
    local background="$2"
    local target_path="$3"
    local sidebar_path="$TMP_DIR/sidebar.png"
    local sidebar_logo_size=234

    rsvg-convert -o "$sidebar_path" -w $sidebar_logo_size -h $sidebar_logo_size "$svg_path"
    convert -background "$background" "$sidebar_path" \
        -gravity center -extent ${sidebar_logo_size}x314 \
        -gravity west -crop 164x314+10+0 "bmp3:$target_path"
    rm "$sidebar_path"
}

build_installer_sidebar "$SVG_SOURCE_PATH" "#1F1F20" \
    "$DIST_ASSETS_DIR/windows/installersidebar.bmp"
# Sand behind the beta artwork, the mirror of the near-black prod panel.
build_installer_sidebar "$BETA_SVG_PATH" "#F5ECDA" \
    "$DIST_ASSETS_DIR/windows/installersidebar-beta.bmp"

# GUI notification icon
rsvg-convert -o ../assets/images/icon-notification.png -w 128 -h 128 $SVG_SOURCE_PATH

rm -rf "$TMP_DIR"

