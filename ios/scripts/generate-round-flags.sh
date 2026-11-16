#!/usr/bin/env bash
# Rasterises the desktop's round flags into the iOS asset catalog, one imageset
# per ISO code under `Flags/`, for the home's location row (28 points, drawn
# from a single @3x bitmap). The SVGs in desktop/.../assets/images/flags are the
# source; run this again whenever they change rather than editing a PNG.
#
# Needs rsvg-convert (librsvg); oxipng, when present, shrinks the PNGs
# losslessly.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/../.." && pwd)"
source_dir="$repo_dir/desktop/packages/mullvad-vpn/assets/images/flags"
catalog_dir="$repo_dir/ios/WarrenVPN/Supporting Files/Assets.xcassets/Flags"
side_px=84

command -v rsvg-convert >/dev/null || { echo "rsvg-convert is required" >&2; exit 1; }

rm -rf "$catalog_dir"
mkdir -p "$catalog_dir"
cat >"$catalog_dir/Contents.json" <<'JSON'
{
  "info" : {
    "author" : "xcode",
    "version" : 1
  },
  "properties" : {
    "provides-namespace" : true
  }
}
JSON

count=0
for svg in "$source_dir"/*.svg; do
  code="$(basename "$svg" .svg)"
  set_dir="$catalog_dir/$code.imageset"
  mkdir -p "$set_dir"
  rsvg-convert -w "$side_px" -h "$side_px" "$svg" -o "$set_dir/$code.png"
  cat >"$set_dir/Contents.json" <<JSON
{
  "images" : [
    {
      "filename" : "$code.png",
      "idiom" : "universal",
      "scale" : "3x"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON
  count=$((count + 1))
done

if command -v oxipng >/dev/null; then
  oxipng --quiet --opt 4 --strip safe "$catalog_dir"/*.imageset/*.png
fi

echo "wrote $count flags to $catalog_dir"
