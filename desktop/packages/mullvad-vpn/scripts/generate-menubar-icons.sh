#!/usr/bin/env bash

# This script generates the PNG/ICO menubar icons from the SVG files in `/menubar-icons/svg/`.
# Please see /menubar-icons/README.md for more information.

set -eu

if ! command -v convert > /dev/null; then
    echo >&2 "convert (imagemagick) is required to run this script"
    exit 1
fi

if ! command -v python3 > /dev/null; then
    echo >&2 "python3 is required to run this script"
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
    # The frame sources are drawn from graphics/logo-mark.svg, never by hand.
    python3 "$SCRIPT_DIR/menubar-icon-frames.py" "$SVG_DIR"

    rm -f "$MACOS_DIR"/lock-* "$WINDOWS_DIR"/lock-* "$LINUX_DIR"/lock-* \
        "$MACOS_DIR/$NON_PROD_DIR_NAME"/lock-* "$WINDOWS_DIR/$NON_PROD_DIR_NAME"/lock-* \
        "$LINUX_DIR/$NON_PROD_DIR_NAME"/lock-*

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
    generate_placeholder "tray-placeholder"

    local frame_count
    frame_count=$(find "$SVG_DIR" -name 'tray-*.svg' | grep -cE '/tray-[0-9]+\.svg$')
    for frame in $(seq 1 "$frame_count"); do
        generate "tray-$frame"
    done
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

    # The notification dot keeps its colour in both builds: it means
    # "attention", not a tunnel state.
    local allowed
    allowed="$(palette_rows | awk '{ print $2 }')
$(grep -oE '#[0-9a-fA-F]{6}' "$SVG_DIR/notification.svg")"

    local leftover
    leftover=$(grep -oE '#[0-9a-fA-F]{6}' "$target_path" | sort -u \
        | grep -vxiF "$allowed" || true)
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

    # Drawn in a neutral grey and showing no tunnel state, so it has no accent
    # to move and both trees hold the same bytes for it.
    render_png "$SVG_DIR/$icon_name.svg" "$LINUX_TARGET_DIR/$icon_name.png" 48 4
}

# Renders one source onto a square canvas of `size`, the mark itself spanning
# `size - 2 * padding`. The sources carry a margin of 3/16 of the mark on every
# side (MARGIN_RATIO in menubar-icon-frames.py) for the glow of the secured
# frame, so the render is 11/8 of the mark, then cut or padded to the canvas.
function render_png() {
    local svg_source_path="$1"
    local png_target_path="$2"
    local size=$3
    local padding=$4
    local mark=$((size - padding * 2))
    local render=$(((mark * 11 + 4) / 8))
    local png_tmp_path="$TMP_DIR/render.png"

    rsvg-convert -o "$png_tmp_path" -w $render -h $render "$svg_source_path"
    convert -background transparent "$png_tmp_path" -gravity center \
        -extent "${size}x$size" "${COMPRESSION_OPTIONS[@]}" "$png_target_path"
    rm "$png_tmp_path"
}

# Generates the ico icons for the Windows tray icon. The ico consists of 3 different resolutions with
# 3 different bit depths each.
function generate_ico() {
    local svg_source_path="$1"
    local ico_target_path="$2"

    local tmp_file_paths=()
    for size in 16 32 48; do
        local png_tmp_path="$TMP_DIR/$size"

        render_png "$svg_source_path" "$png_tmp_path.png" "$size" $((size / 16))

        # 4- and 8-bit versions for RDP
        convert -colors 256 +dither "$png_tmp_path.png" png8:"$png_tmp_path-8.png"
        convert -colors 16  +dither "$png_tmp_path-8.png" "$png_tmp_path-4.png"

        tmp_file_paths+=("$png_tmp_path.png" "$png_tmp_path-8.png" "$png_tmp_path-4.png")
    done

    convert "${tmp_file_paths[@]}" "${COMPRESSION_OPTIONS[@]}" "$ico_target_path.ico"
    rm "${tmp_file_paths[@]}"
}

# Generates all icon versions for one frame, with and without the notification
# dot. The dot is drawn into its own sources, between the ears, so a notification
# never changes the size of the icon.
function generate() {
    local icon_name="$1"

    for notification in "" "_notification"; do
        local colored_svg_source_path="$SVG_DIR/$icon_name$notification.svg"
        local mono_svg_source_path="$SVG_DIR/${icon_name}_mono$notification.svg"
        local black_svg_source_path="$TMP_DIR/black.svg"
        local white_svg_source_path="$TMP_DIR/white.svg"

        local macos_target_base_path="$MACOS_TARGET_DIR/$icon_name$notification"
        local linux_target_base_path="$LINUX_TARGET_DIR/$icon_name"
        local windows_target_base_path="$WINDOWS_TARGET_DIR/$icon_name"

        sed -E 's/#[0-9a-fA-F]{6}/#000000/g' "$mono_svg_source_path" > "$black_svg_source_path"
        sed -E 's/#[0-9a-fA-F]{6}/#FFFFFF/g' "$mono_svg_source_path" > "$white_svg_source_path"

        # Only the coloured variants below take the palette. The monochrome ones
        # are rendered from the two sources just flattened to a single tint.
        if [ "$RECOLOR" = "1" ]; then
            recolor_to_beta "$colored_svg_source_path" "$TMP_DIR/colored.svg"
            colored_svg_source_path="$TMP_DIR/colored.svg"
        fi

        # MacOS colored
        render_png "$colored_svg_source_path" "$macos_target_base_path.png" 22 3
        render_png "$colored_svg_source_path" "$macos_target_base_path@2x.png" 44 6

        # MacOS monochrome
        render_png "$black_svg_source_path" "${macos_target_base_path}Template.png" 22 3
        render_png "$black_svg_source_path" "${macos_target_base_path}Template@2x.png" 44 6

        # Linux colored and white
        render_png "$colored_svg_source_path" "$linux_target_base_path$notification.png" 48 4
        render_png "$white_svg_source_path" "${linux_target_base_path}_white$notification.png" 48 4

        # Windows colored and monochrome
        generate_ico "$colored_svg_source_path" "$windows_target_base_path$notification"
        generate_ico "$white_svg_source_path" "${windows_target_base_path}_white$notification"
        generate_ico "$black_svg_source_path" "${windows_target_base_path}_black$notification"

        rm "$black_svg_source_path" "$white_svg_source_path"
        if [ "$RECOLOR" = "1" ]; then
            rm "$TMP_DIR/colored.svg"
        fi
    done
}

main
