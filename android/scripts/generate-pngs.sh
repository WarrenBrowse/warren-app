#!/usr/bin/env bash

set -eu

if ! command -v rsvg-convert > /dev/null; then
    echo >&2 "rsvg-convert (librsvg) is required to run this script"
    exit 1
fi

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

ICON_SVG_PATH="../../graphics/icon.svg"

# The following helper function converts an SVG image into a PNG image for a specific DPI
#
# Parameters:
#   1. Path to source SVG image
#   2. DPI config, a string with two parameters separated by a '-'
#      a. the DPI specification, as the suffix of the output directory (e.g., mdpi, xxhdpi)
#      b. the size of the generated PNG image
#   3. (optional) The destination image name, if not specified it is assumed to be the same as the
#      input image name, with any '-'s replaced with '_'s
#   4. (optional) Destination directory type, either 'drawable' (the default) or 'mipmap'
#
# Examples:
#
# The following will generate a 50 by 50 image in android/lib/ui/resource/src/main/res/drawable-hdpi/my_image.png
#
#     convert_image /tmp/my-image.svg hdpi-50
#
# The following will generate a 50 by 50 image in android/lib/ui/resource/src/main/res/drawable-mdpi/other_image.png
#
#     convert_image /tmp/my-other-image.svg mdpi-50 other_image
#
# The following will generate a 50 by 50 image in android/lib/ui/resource/src/main/res/mipmap-xxhdpi/my_icon.png
#
#     convert_image /tmp/my-final-image.svg xxhdpi-50 my_icon mipmap
function convert_image() {
    if (( $# < 2 )); then
        echo "Too few arguments passed to 'convert_image'" >&2
        exit 1
    fi

    local source_image="$1"
    local dpi_config="$2"
    local destination_image

    if (( $# >= 3 )); then
        destination_image="$3"
    else
        destination_image="$(basename "$source_image" .svg | sed -e 's/-/_/g')"
    fi

    if (( $# >= 4 )); then
        local destination_dir="$4"
    else
        local destination_dir="drawable"
    fi

    local dpi
    dpi="$(echo "$dpi_config" | cut -f1 -d'-')"
    local size
    size="$(echo "$dpi_config" | cut -f2 -d'-')"

    local dpi_dir="../lib/ui/resource/src/main/res/${destination_dir}-${dpi}"

    echo "$source_image -> ($size x $size) ${dpi_dir}/${destination_image}.png"
    mkdir -p "$dpi_dir"
    rsvg-convert "$source_image" -w "$size" -h "$size" -o "${dpi_dir}/${destination_image}.png"
}

# Launcher icon
for dpi_size in "mdpi-48" "hdpi-72" "xhdpi-96" "xxhdpi-144" "xxxhdpi-192"; do
    convert_image "$ICON_SVG_PATH" "$dpi_size" "ic_launcher" "mipmap"
done

# Logo used in some GUI areas
for dpi_size in "mdpi-50" "hdpi-75" "xhdpi-100" "xxhdpi-150" "xxxhdpi-200"; do
    convert_image "$ICON_SVG_PATH" "$dpi_size" "logo_icon"
done

# Large logo used in the launch screen
for dpi_size in "mdpi-120" "hdpi-180" "xhdpi-240" "xxhdpi-360" "xxxhdpi-480"; do
    convert_image "$ICON_SVG_PATH" "$dpi_size" "launch_logo"
done

# The status-bar and quick-settings mark is NOT generated here. It is the
# hand-maintained vector lib/ui/resource/src/main/res/drawable/small_logo_*.xml,
# carrying the current logo path; this script used to emit dpi-qualified PNGs of
# the older shaved mark under the same names, and a dpi-qualified drawable beats
# an unqualified one on every device that is not exactly mdpi. That would have
# shadowed both the vector and the beta overlay
# (android/scripts/generate-beta-small-logo.py).

# Beta flavor adaptive-icon foreground: the mark recoloured to the sand tone
# and shrunk to clear the amber BETA badge, sitting on the brown background set
# by the beta colours overlay (android/app/src/beta/res/values/colors.xml).
# Derived from the prod foreground and the same badge asset the desktop icons
# use, so the two platforms cannot drift apart.
#
# The badge lands inside the adaptive-icon safe zone (the inner 72dp of 108dp,
# radius 144 of the 432px foreground): a launcher masks the icon to a circle
# and anything outside that is cropped. Its offset matches the desktop layout,
# where the mark is scaled to 86% and nudged up and left by 8/126 and 12/126 of
# the disc radius.
if ! command -v magick > /dev/null; then
    echo >&2 "magick (imagemagick) is required to generate the beta launcher icon"
    exit 1
fi

if ! command -v rsvg-convert > /dev/null; then
    echo >&2 "rsvg-convert (librsvg) is required to generate the beta launcher icon"
    exit 1
fi

if ! command -v python3 > /dev/null; then
    echo >&2 "python3 is required to generate the beta launcher icon"
    exit 1
fi

prod_foreground="../lib/ui/resource/src/main/res/drawable-nodpi/ic_warren_foreground.png"
beta_foreground="../app/src/beta/res/drawable-nodpi/ic_warren_foreground.png"
badge_svg="$(mktemp -t beta-badge).svg"
badge_png="$(mktemp -t beta-badge).png"

python3 ../../graphics/make-beta-icon.py \
    --badge ../../graphics/beta-badge.svg \
    --badge-only --size 432 --circle-radius 144 \
    --output "$badge_svg"
rsvg-convert -w 432 -h 432 -o "$badge_png" "$badge_svg"

echo "$prod_foreground -> (sand + badge) $beta_foreground"
mkdir -p "$(dirname "$beta_foreground")"
magick -size 432x432 xc:none \
    \( "$prod_foreground" -fill "#F5ECDA" -colorize 100 -resize 86% \) \
    -gravity center -geometry -9-14 -composite \
    "$badge_png" -gravity northwest -geometry +0+0 -composite \
    "PNG32:$beta_foreground"

# Themed icons (Android 13+) draw the monochrome layer alone, tinted by the
# launcher, so the beta badge has to be a silhouette there: a filled disc with
# the lettering knocked out of it.
prod_mono="../lib/ui/resource/src/main/res/drawable-nodpi/ic_warren_mono.png"
beta_mono="../app/src/beta/res/drawable-nodpi/ic_warren_mono.png"
badge_mono_png="$(mktemp -t beta-badge-mono).png"

magick "$badge_png" -fuzz 25% -transparent "#332818" -fill white -colorize 100 \
    "PNG32:$badge_mono_png"

echo "$prod_mono -> (badge knockout) $beta_mono"
magick -size 432x432 xc:none \
    \( "$prod_mono" -resize 86% \) -gravity center -geometry -9-14 -composite \
    "$badge_mono_png" -gravity northwest -geometry +0+0 -composite \
    "PNG32:$beta_mono"

rm -f "$badge_svg" "$badge_png" "$badge_mono_png"
