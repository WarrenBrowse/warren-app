#!/usr/bin/env bash

# This script generates the PNG/ICO menubar icons from the SVG files in `/menubar-icons/svg/`.
# Please see /menubar-icons/README.md for more information.

set -eu

if ! command -v convert > /dev/null; then
    echo >&2 "convert (imagemagick) is required to run this script"
    exit 1
fi

if ! command -v rsvg-convert > /dev/null; then
    echo >&2 "rsvg-convert (librsvg) is required to run this script"
    exit 1
fi

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

MENUBAR_ICONS_DIR="${SCRIPT_DIR}/../assets/images/menubar-icons"
GRAPHICS_DIR="$( cd "$SCRIPT_DIR/../../../.." && pwd )/graphics"

SVG_DIR="$MENUBAR_ICONS_DIR/svg"
MACOS_DIR="$MENUBAR_ICONS_DIR/darwin"
WINDOWS_DIR="$MENUBAR_ICONS_DIR/win32"
LINUX_DIR="$MENUBAR_ICONS_DIR/linux"
TMP_DIR=$(mktemp -d)

# A non-prod build wears the same lock drawn in another hue family, so a machine
# running prod and beta side by side never shows two identical tray icons. That
# tree keeps the prod file names and lives one directory deeper
# (src/main/tray-icon.ts appends the segment), which leaves the icon matrix in
# tray-icon-controller.ts untouched.
NON_PROD_DIR_NAME="beta"

# The accent table, and the reasoning behind the hues it moves to.
BETA_PALETTE_FILE="$GRAPHICS_DIR/menubar-beta-palette.txt"

# Set per pass by generate_all: 1 draws the coloured variants in the non-prod
# palette. The monochrome ones are single-tint alpha masks with no colour to
# move, so both trees hold the same bytes for those.
RECOLOR=0
MACOS_TARGET_DIR="$MACOS_DIR"
WINDOWS_TARGET_DIR="$WINDOWS_DIR"
LINUX_TARGET_DIR="$LINUX_DIR"

COMPRESSION_OPTIONS=(
    -define png:compression-filter=5
    -define png:compression-level=9
    -define png:compression-strategy=1
    -define png:exclude-chunk=all
    -strip
)

function main() {
    generate_all "$MACOS_DIR" "$WINDOWS_DIR" "$LINUX_DIR" 0
    # Staging takes the same tree: it is a non-prod install that has to be
    # tellable from prod on the same machine, and there is no third palette.
    # The app icon already shares the beta assets with staging for that reason.
    generate_all "$MACOS_DIR/$NON_PROD_DIR_NAME" "$WINDOWS_DIR/$NON_PROD_DIR_NAME" \
        "$LINUX_DIR/$NON_PROD_DIR_NAME" 1

    rmdir "$TMP_DIR"
}

# Generates the whole icon set into one target tree, in the prod palette or the
# non-prod one.
function generate_all() {
    MACOS_TARGET_DIR="$1"
    WINDOWS_TARGET_DIR="$2"
    LINUX_TARGET_DIR="$3"
    RECOLOR="$4"

    mkdir -p "$MACOS_TARGET_DIR" "$WINDOWS_TARGET_DIR" "$LINUX_TARGET_DIR"

    # The placeholder is used as the initial tray icon on Linux
    generate_placeholder "lock-placeholder"

    for frame in {1..9}; do
        generate "lock-$frame" "lock-$frame"
    done
    # The monochrome source svg differs from the colored one. The red circle is a hole in the monochrome
    # one. "lock-10_mono.svg" is the same icon but with a hole instead of a circle.
    generate lock-10 lock-10_mono
}

# The `prod beta` accent pairs, one per line, from the palette table. Every
# comment in that file opens with a hash and a space, so a data row can never be
# mistaken for one.
function palette_rows() {
    awk '/^#[0-9A-Fa-f]{6}[[:space:]]+#[0-9A-Fa-f]{6}[[:space:]]*$/ { print $1, $2 }' \
        "$BETA_PALETTE_FILE"
}

# Rewrites the accents of a coloured lock source into the non-prod palette.
# Refuses to emit a source still painting anything the table does not name, so a
# frame restyled or added in prod cannot quietly ship a beta build wearing the
# production colours, which is the one thing this tree exists to prevent. The
# check is "nothing outside the beta column survives" rather than "no prod
# accent survives", because an accent DROPPED from the table is exactly the case
# a per-row check cannot see.
function recolor_to_beta() {
    local source_path="$1"
    local target_path="$2"
    local sed_program=""
    local prod beta

    while read -r prod beta; do
        sed_program="${sed_program}s/${prod}/${beta}/g;"
    done < <(palette_rows)

    if [ -z "$sed_program" ]; then
        echo >&2 "no accent pairs in $BETA_PALETTE_FILE"
        exit 1
    fi

    sed "$sed_program" "$source_path" > "$target_path"

    local leftover
    leftover=$(grep -oE '#[0-9a-fA-F]{6}' "$target_path" | sort -u \
        | grep -vxiF "$(palette_rows | awk '{ print $2 }')" || true)
    if [ -n "$leftover" ]; then
        echo >&2 "$(basename "$source_path") still paints $leftover after the palette."
        echo >&2 "Update the table in $BETA_PALETTE_FILE to cover the artwork."
        exit 1
    fi
}

# Generates the placeholder icon is an empty icon which is used as the initial icon on Linux
# until the tunnel state can be determined and the icon can be replaced with another one.
function generate_placeholder() {
    local icon_name="$1"
    local svg_source_path="$SVG_DIR/$icon_name.svg"
    local linux_target_base_path="$LINUX_TARGET_DIR/$icon_name"
    local png_target_path="$linux_target_base_path.png"
    local target_size=48
    local target_padding=4

    # Drawn in neutral greys and showing no tunnel state, so it has no accent to
    # move and both trees hold the same bytes for it.
    generate_lock_png "$svg_source_path" "$png_target_path" "$target_size" "$target_padding"
}

# Generates the ico icons for the Windows tray icon. The ico consists of 3 different resolutions with
# 3 different bit depths each. Each icon is also available with and without notification dot.
function generate_ico() {
    local svg_source_path="$1"
    local ico_target_path="$2"

    local tmp_file_paths=()
    local notification_icon_tmp_file_paths=()
    for size in 16 32 48; do
        local padding=$((size / 16))
        local notification_icon_size=$((size / 2))
        local png_tmp_path="$TMP_DIR/$size"

        generate_square "$svg_source_path" "$png_tmp_path.png" \
            "${png_tmp_path}_notification.png" "$size" "$padding" "$notification_icon_size"

        # 4- and 8-bit versions for RDP
        convert -colors 256 +dither "$png_tmp_path.png" png8:"$png_tmp_path-8.png"
        convert -colors 16  +dither "$png_tmp_path-8.png" "$png_tmp_path-4.png"

        convert -colors 256 +dither "${png_tmp_path}_notification.png" \
            png8:"${png_tmp_path}_notification-8.png"
        convert -colors 16  +dither "${png_tmp_path}_notification-8.png" \
            "${png_tmp_path}_notification-4.png"

        tmp_file_paths+=("$png_tmp_path.png" "$png_tmp_path-8.png" "$png_tmp_path-4.png")
        notification_icon_tmp_file_paths+=(
            "${png_tmp_path}_notification.png"
            "${png_tmp_path}_notification-8.png"
            "${png_tmp_path}_notification-4.png"
        )
    done

    convert "${tmp_file_paths[@]}" "${COMPRESSION_OPTIONS[@]}" "$ico_target_path.ico"
    convert "${notification_icon_tmp_file_paths[@]}" "${COMPRESSION_OPTIONS[@]}" \
        "${ico_target_path}_notification.ico"

    rm "${tmp_file_paths[@]}"
    rm "${notification_icon_tmp_file_paths[@]}"
}

# Generates pngs both for regular icon and icon with notification symbol next to the icon, ending
# up with a rectangular icon.
function generate_rectangle() {
    local svg_source_path="$1"
    local png_target_path="$2"
    local png_notification_target_path="$3"
    local target_size=$4
    local target_padding=$5
    local notification_width=$6
    local target_size_no_padding=$((target_size - target_padding * 2))
    local png_tmp_path="$TMP_DIR/tmp.png"

    generate_lock_png "$svg_source_path" "$png_target_path" "$target_size" "$target_padding"
    append_notification_icon "$png_target_path" "$png_notification_target_path" \
        "$notification_width"

    rm "$png_tmp_path"
}

# Generates pngs both for regular icon and icon with notification symbol, ending up with a square
# icon since the notification dot is overlapping the lock.
function generate_square() {
    local svg_source_path="$1"
    local png_target_path="$2"
    local png_notification_target_path="$3"
    local target_size=$4
    local target_padding=$5
    local notification_width=$6
    local target_size_no_padding=$((target_size - target_padding * 2))
    local png_tmp_path="$TMP_DIR/tmp.png"

    generate_lock_png "$svg_source_path" "$png_target_path" "$target_size" "$target_padding"
    overlay_notification_icon "$png_target_path" "$png_notification_target_path" \
        "$notification_width"

    rm "$png_tmp_path"
}

# Generates the lock png
function generate_lock_png() {
    local svg_source_path="$1"
    local png_target_path="$2"
    local target_size=$3
    local target_padding=$4
    local target_size_no_padding=$((target_size - target_padding * 2))
    local png_tmp_path="$TMP_DIR/tmp.png"

    rsvg-convert -o "$png_tmp_path" -w $target_size_no_padding -h $target_size_no_padding \
        "$svg_source_path"
    convert -background transparent "$png_tmp_path" -gravity center \
        -extent "${target_size}x$target_size" "${COMPRESSION_OPTIONS[@]}" "$png_target_path"
}

# Creates a copy of the icon at $source_path and appends the notification symbol to it
function append_notification_icon() {
    local source_path="$1"
    local target_path="$2"
    local width="$3"
    local padding="${4:-0}"
    local size=$((width + 2))
    local notification_icon_tmp_path="$TMP_DIR/notification.png"

    rsvg-convert -o "$notification_icon_tmp_path" -w $size -h $size \
        --left "$padding" --page-width $((size + padding)) --page-height $size \
        "$SVG_DIR/notification.svg"
    convert -strip -background transparent -colorspace sRGB -gravity center \
        +append "$source_path" "$notification_icon_tmp_path" "$target_path"

    rm "$notification_icon_tmp_path"
}

# Creates a copy of the icon at $source_path and puts the notification symbol on top of it in the
# bottom right corner.
function overlay_notification_icon() {
    local source_path="$1"
    local target_path="$2"
    local size="$3"
    local notification_icon_tmp_path="$TMP_DIR/notification.png"

    rsvg-convert -o "$notification_icon_tmp_path" -w "$size" -h "$size" "$SVG_DIR/notification.svg"
    convert -strip -background transparent -composite -colorspace sRGB -gravity SouthEast \
        "$source_path" "$notification_icon_tmp_path" "$target_path"

    rm "$notification_icon_tmp_path"
}

# Generates all icon versions for a specific frame.
function generate() {
    local icon_name="$1"
    local svg_source_path="$SVG_DIR/$icon_name.svg"
    local monochrome_svg_source_path="$SVG_DIR/$2.svg"

    local black_svg_source_path="$TMP_DIR/black.svg"
    local white_svg_source_path="$TMP_DIR/white.svg"

    local macos_target_base_path="$MACOS_TARGET_DIR/$icon_name"
    local linux_target_base_path="$LINUX_TARGET_DIR/$icon_name"
    local windows_target_base_path="$WINDOWS_TARGET_DIR/$icon_name"

    sed -E 's/#[0-9a-fA-F]{6}/#000000/g' "$monochrome_svg_source_path" > "$black_svg_source_path"
    sed -E 's/#[0-9a-fA-F]{6}/#FFFFFF/g' "$monochrome_svg_source_path" > "$white_svg_source_path"

    # Only the coloured variants below take the palette. The monochrome ones are
    # rendered from the two sources just flattened to a single tint.
    local colored_svg_source_path="$svg_source_path"
    if [ "$RECOLOR" = "1" ]; then
        colored_svg_source_path="$TMP_DIR/colored.svg"
        recolor_to_beta "$svg_source_path" "$colored_svg_source_path"
    fi

    # MacOS colored
    generate_rectangle "$colored_svg_source_path" "$macos_target_base_path.png" \
        "${macos_target_base_path}_notification.png" 22 3 4
    generate_rectangle "$colored_svg_source_path" "$macos_target_base_path@2x.png" \
        "${macos_target_base_path}_notification@2x.png" 44 6 8

    # MacOS monochrome
    generate_rectangle "$black_svg_source_path" "${macos_target_base_path}Template.png" \
        "${macos_target_base_path}_notificationTemplate.png" 22 3 4
    generate_rectangle "$black_svg_source_path" "${macos_target_base_path}Template@2x.png" \
        "${macos_target_base_path}_notificationTemplate@2x.png" 44 6 8

    # Linux colored
    generate_square "$colored_svg_source_path" "$linux_target_base_path.png" \
        "${linux_target_base_path}_notification.png" 48 4 24

    # Linux white
    generate_square "$white_svg_source_path" "${linux_target_base_path}_white.png" \
        "${linux_target_base_path}_white_notification.png" 48 4 24

    # Windows colored
    generate_ico "$colored_svg_source_path" "$windows_target_base_path"

    # Windows monochrome
    generate_ico "$white_svg_source_path" "${windows_target_base_path}_white"
    generate_ico "$black_svg_source_path" "${windows_target_base_path}_black"

    rm "$black_svg_source_path" "$white_svg_source_path"
    if [ "$RECOLOR" = "1" ]; then
        rm "$colored_svg_source_path"
    fi
}

main
