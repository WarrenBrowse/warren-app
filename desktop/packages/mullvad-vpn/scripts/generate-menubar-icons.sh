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

if ! command -v python3 > /dev/null; then
    echo >&2 "python3 is required to derive the beta pip"
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

# A non-prod build wears the same lock with an amber pip in the corner, so a
# machine running prod and beta side by side never shows two identical tray
# icons. The badged tree keeps the prod file names and lives one directory
# deeper (src/main/tray-icon.ts appends the segment), which leaves the icon
# matrix in tray-icon-controller.ts untouched.
BADGED_DIR_NAME="beta"

# The pip is 3/8 of the canvas: large enough for the knocked-out B to read at
# 44px, small enough to leave the lock recognisable at 16px. It sits flush in
# the BOTTOM-left corner: the shackle is the lock's identity and it sits top
# centre, close enough to the left edge at 16px that a top-left pip would cut it
# in half, while the bottom left is the plain body and survives a clipped
# corner. The forum-activity dot is a circle at the bottom RIGHT, so the two
# never overlap and a square left of a circle stays unambiguous.
PIP_NUMERATOR=3
PIP_DENOMINATOR=8
# Width of the transparent gap knocked out of the lock around the pip, as a
# fraction of the canvas. Without it the pip and the lock merge into one blob
# wherever they touch, which is every monochrome variant: a template image is
# an alpha mask, so both shapes carry the same single tint and only a gap
# separates them.
PIP_HALO_DENOMINATOR=22
# Below this canvas size the letter is a smudge and the pip stays a plain
# square, the same concession build-logo-icons.sh makes for the app icon. Only
# the 16px sub-image of the Windows .ico falls under it.
PIP_LABEL_MIN_PX=20

# Set per pass by generate_all: 1 makes generate_lock_png stamp the pip.
BADGE_PIP=0
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
    # Staging is badged too: it is a non-prod install that has to be tellable
    # from prod on the same machine, and there is no third artwork. The app
    # icon already shares the beta assets with staging for that reason.
    generate_all "$MACOS_DIR/$BADGED_DIR_NAME" "$WINDOWS_DIR/$BADGED_DIR_NAME" \
        "$LINUX_DIR/$BADGED_DIR_NAME" 1

    rmdir "$TMP_DIR"
}

# Generates the whole icon set into one target tree, badged or not.
function generate_all() {
    MACOS_TARGET_DIR="$1"
    WINDOWS_TARGET_DIR="$2"
    LINUX_TARGET_DIR="$3"
    BADGE_PIP="$4"

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

# Generates the placeholder icon is an empty icon which is used as the initial icon on Linux
# until the tunnel state can be determined and the icon can be replaced with another one.
function generate_placeholder() {
    local icon_name="$1"
    local svg_source_path="$SVG_DIR/$icon_name.svg"
    local linux_target_base_path="$LINUX_TARGET_DIR/$icon_name"
    local png_target_path="$linux_target_base_path.png"
    local target_size=48
    local target_padding=4

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

    # Stamped here rather than on each variant, so every downstream step (the
    # notification overlay, the macOS append, the .ico colour reductions)
    # inherits it from the one place the lock is drawn.
    if [ "$BADGE_PIP" = "1" ]; then
        overlay_beta_pip "$png_target_path" "$target_size"
    fi
}

# Stamps the beta pip onto the top-left of an already generated icon. The
# corner is forced: the notification dot is an amber circle at the bottom right
# of the same canvas, and the two must not be confusable.
function overlay_beta_pip() {
    local png_target_path="$1"
    local canvas_size="$2"
    local pip_size=$((canvas_size * PIP_NUMERATOR / PIP_DENOMINATOR))
    local pip_svg_path="$TMP_DIR/beta-pip.svg"
    local pip_png_path="$TMP_DIR/beta-pip.png"
    local cutter_svg_path="$TMP_DIR/beta-pip-cutter.svg"
    local cutter_png_path="$TMP_DIR/beta-pip-cutter.png"
    local badged_png_path="$TMP_DIR/beta-badged.png"
    # An array that is never empty: bash 3.2, which is what /usr/bin/env bash
    # still resolves to on a stock macOS, treats an empty one as unset under
    # `set -u`.
    local pip_args=(--badge "$GRAPHICS_DIR/beta-badge.svg" --square-pip
        --size "$pip_size" --output "$pip_svg_path")

    if [ "$canvas_size" -lt "$PIP_LABEL_MIN_PX" ]; then
        pip_args+=(--no-label)
    fi

    python3 "$GRAPHICS_DIR/make-beta-icon.py" "${pip_args[@]}"
    rsvg-convert -o "$pip_png_path" -w "$pip_size" -h "$pip_size" "$pip_svg_path"

    # The gap is the same rounded square one halo wider, rendered from the same
    # builder so the two shapes can never drift apart, and subtracted from the
    # lock before the pip lands on top. Both sit flush at the corner, so the gap
    # only ever appears on the two edges that face the lock.
    local halo=$((canvas_size / PIP_HALO_DENOMINATOR))
    if [ "$halo" -lt 1 ]; then
        halo=1
    fi
    local cutter_size=$((pip_size + halo))
    python3 "$GRAPHICS_DIR/make-beta-icon.py" --badge "$GRAPHICS_DIR/beta-badge.svg" \
        --square-pip --no-label --size "$cutter_size" --output "$cutter_svg_path"
    rsvg-convert -o "$cutter_png_path" -w "$cutter_size" -h "$cutter_size" "$cutter_svg_path"
    convert -strip -background transparent -colorspace sRGB -gravity SouthWest \
        "$png_target_path" "$cutter_png_path" -compose DstOut -composite \
        "${COMPRESSION_OPTIONS[@]}" "$badged_png_path"
    mv "$badged_png_path" "$png_target_path"

    convert -strip -background transparent -composite -colorspace sRGB -gravity SouthWest \
        "$png_target_path" "$pip_png_path" "${COMPRESSION_OPTIONS[@]}" "$badged_png_path"
    mv "$badged_png_path" "$png_target_path"

    rm "$pip_svg_path" "$pip_png_path" "$cutter_svg_path" "$cutter_png_path"
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

    # MacOS colored
    generate_rectangle "$svg_source_path" "$macos_target_base_path.png" \
        "${macos_target_base_path}_notification.png" 22 3 4
    generate_rectangle "$svg_source_path" "$macos_target_base_path@2x.png" \
        "${macos_target_base_path}_notification@2x.png" 44 6 8

    # MacOS monochrome
    generate_rectangle "$black_svg_source_path" "${macos_target_base_path}Template.png" \
        "${macos_target_base_path}_notificationTemplate.png" 22 3 4
    generate_rectangle "$black_svg_source_path" "${macos_target_base_path}Template@2x.png" \
        "${macos_target_base_path}_notificationTemplate@2x.png" 44 6 8

    # Linux colored
    generate_square "$svg_source_path" "$linux_target_base_path.png" \
        "${linux_target_base_path}_notification.png" 48 4 24

    # Linux white
    generate_square "$white_svg_source_path" "${linux_target_base_path}_white.png" \
        "${linux_target_base_path}_white_notification.png" 48 4 24

    # Windows colored
    generate_ico "$svg_source_path" "$windows_target_base_path"

    # Windows monochrome
    generate_ico "$white_svg_source_path" "${windows_target_base_path}_white"
    generate_ico "$black_svg_source_path" "${windows_target_base_path}_black"

    rm "$black_svg_source_path" "$white_svg_source_path"
}

main

