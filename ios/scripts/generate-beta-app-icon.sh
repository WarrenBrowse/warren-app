#!/usr/bin/env bash
# The non-prod iOS app icon: the prod artwork with the palette swapped and the
# amber BETA badge, so a tester can tell the two installs apart on the home
# screen and in the App Switcher. Selected by ASSETCATALOG_COMPILER_APPICON_NAME
# through WARREN_APPICON_NAME (ios/Configurations/ProductEnv.xcconfig).
#
# Everything is derived from the prod set at generation time by
# graphics/make-beta-icon.py, the same producer the desktop icns/ico and the
# Android launcher icon already use, so the platforms cannot drift apart. Never
# hand-convert a layer, and never edit the generated PNGs.
#
# Usage: ios/scripts/generate-beta-app-icon.sh

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/../.." && pwd)"
graphics_dir="$repo_dir/graphics"
catalog_dir="$repo_dir/ios/WarrenVPN/Supporting Files/Assets.xcassets"
prod_set="$catalog_dir/AppIcon.appiconset"
beta_set="$catalog_dir/AppIconBeta.appiconset"

for tool in rsvg-convert magick python3; do
    if ! command -v "$tool" > /dev/null; then
        echo >&2 "$tool is required to generate the beta app icon"
        exit 1
    fi
done

# App Store artwork is a single 1024px layer per appearance; iOS derives every
# smaller size itself.
size=1024
# The badge sits at the same spot relative to the artwork as it does on the
# desktop and on Android. make-beta-icon.py measures its layout against the
# VISIBLE disc radius, which here is half the full-bleed square, and shrinks
# the mark to 86% so the badge does not eat the lower half of the right ear.
radius=$((size / 2))
mark_scale=86
# -8/126 and -12/126 of the disc radius, the generator's MARK_SHIFT_X/Y.
shift_x=$((-8 * radius / 126))
shift_y=$((-12 * radius / 126))

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

make_beta_svg() {
    python3 "$graphics_dir/make-beta-icon.py" --badge "$graphics_dir/beta-badge.svg" "$@"
}

mkdir -p "$beta_set"

# The light appearance is the full artwork, so it is rendered from the square
# source rather than composited: palette swapped (brown ground, sand mark) with
# the badge drawn into the same canvas.
make_beta_svg --source "$graphics_dir/icon-square.svg" --output "$tmp_dir/icon-square-beta.svg"
rsvg-convert -w "$size" -h "$size" -o "$beta_set/Icon-Light-1024x1024.png" \
    "$tmp_dir/icon-square-beta.svg"

# The dark and tinted appearances are the mark alone on a transparent canvas
# (iOS lays them over its own ground), so they take the prod layer with the
# badge composited on top rather than a second copy of the artwork.
make_beta_svg --badge-only --size "$size" --circle-radius "$radius" \
    --output "$tmp_dir/badge.svg"
rsvg-convert -w "$size" -h "$size" -o "$tmp_dir/badge.png" "$tmp_dir/badge.svg"

echo "$prod_set/Icon-Dark-1024x1024.png -> (badge) $beta_set/Icon-Dark-1024x1024.png"
magick -size "${size}x${size}" xc:none \
    \( "$prod_set/Icon-Dark-1024x1024.png" -resize "${mark_scale}%" \) \
    -gravity center -geometry "${shift_x}${shift_y}" -composite \
    "$tmp_dir/badge.png" -gravity northwest -geometry +0+0 -composite \
    -strip "PNG32:$beta_set/Icon-Dark-1024x1024.png"

# The tinted appearance is recoloured by iOS from the luminance it is given, so
# the badge has to be a silhouette there: a filled disc with the lettering
# knocked out of it, exactly as the Android themed icon and the macOS menu-bar
# template do.
magick "$tmp_dir/badge.png" -fuzz 25% -transparent "#332818" -fill white -colorize 100 \
    "PNG32:$tmp_dir/badge-mono.png"

echo "$prod_set/Icon-Tinted-1024x1024.png -> (badge knockout) $beta_set/Icon-Tinted-1024x1024.png"
magick -size "${size}x${size}" xc:none \
    \( "$prod_set/Icon-Tinted-1024x1024.png" -resize "${mark_scale}%" \) \
    -gravity center -geometry "${shift_x}${shift_y}" -composite \
    "$tmp_dir/badge-mono.png" -gravity northwest -geometry +0+0 -composite \
    -strip "PNG32:$beta_set/Icon-Tinted-1024x1024.png"

cp "$prod_set/Contents.json" "$beta_set/Contents.json"
