#!/usr/bin/env bash
# Turn the raw "L'univers de Bula" master art (calques) into the optimized,
# pre-registered layers the connect screen composites at runtime, for all three
# clients at once: desktop WebP, Android WebP drawables and iOS asset-catalog
# PNGs.
#
# Why pre-registered full-frame layers: every master is authored on the same
# 1417x2120 canvas, so background + terrier + bula stack pixel-perfect with no
# per-layer positioning. Keeping the full frame (transparent elsewhere) makes the
# renderer a trivial 3x full-bleed image stack; the empty regions cost almost
# nothing in WebP.
#
# Why one script for three platforms: the layers only register if every client
# ships the SAME canvas, and a per-platform export drifts. Deriving the three
# outputs from one source of truth in one run is what keeps them in step.
#
# Output width 1140 = 3x the 380px window, so the art stays crisp on HiDPI.
#
# Usage: process-scenery.sh [SRC_DIR]
#   SRC_DIR defaults to <repo>/new-da/calques (the master art, git-ignored).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$PKG_DIR/../../.." && pwd)"
SRC_DIR="${1:-$REPO_DIR/new-da/calques}"

DESKTOP_DIR="$PKG_DIR/assets/images/scenery"
ANDROID_DIR="$REPO_DIR/android/lib/ui/resource/src/main/res/drawable-nodpi"
IOS_DIR="$REPO_DIR/ios/WarrenVPN/Supporting Files/Assets.xcassets"

WIDTH=1140
BG_QUALITY=82   # opaque landscapes
FG_QUALITY=88   # alpha layers (bula, terrier): keep soft edges clean

# iOS asset catalogs reject WebP, so the landscapes ship as JPEG there. Storing
# them as PNG conserved nothing (the masters are themselves JPEG q99) and cost
# 10MB of Assets.car; encoded straight from the master at q92 the error peaks at
# 5/255 on the paper grain, with no ringing on the pencil lines. 4:4:4 chroma
# because the art leans on thin coloured strokes (flags, window frames) that
# subsampling smears. The alpha layers stay PNG: a lossy alpha halos the fur.
IOS_BG_QUALITY=92

# master file | slug | iOS imageset | bg (opaque) or fg (alpha)
LAYERS=(
  "INTERFACE PLAINE V14.jpg|plaine|SceneryPlaine|bg"
  "INTERFACE ALLEMAGNE V14.jpg|germany|SceneryGermany|bg"
  "INTERFACE FINLANDE V14.jpg|finland|SceneryFinland|bg"
  "INTERFACE PAYS BAS V14.jpg|netherlands|SceneryNetherlands|bg"
  "INTERFACE SINGAPOUR V14.jpg|singapore|ScenerySingapore|bg"
  "INTERFACE TERRIER V14.png|terrier|SceneryTerrier|fg"
  "INTERFACE LAPIN V14.png|bula|SceneryBula|fg"
)

mkdir -p "$DESKTOP_DIR" "$ANDROID_DIR"

echo "Encoding scenery from $SRC_DIR"
for layer in "${LAYERS[@]}"; do
  IFS='|' read -r master slug imageset kind <<<"$layer"
  src="$SRC_DIR/$master"
  [ -f "$src" ] || { echo "missing master: $src" >&2; exit 1; }

  if [ "$kind" = "bg" ]; then
    webp_opts=(-strip -quality "$BG_QUALITY")
    ios_name="$slug.jpg"
    ios_opts=(-strip -quality "$IOS_BG_QUALITY" -sampling-factor 1x1)
  else
    # Full-quality alpha so the fur outline and the cast shadow keep their soft
    # edges instead of banding against the landscape underneath.
    webp_opts=(-strip -define webp:alpha-quality=100 -quality "$FG_QUALITY")
    ios_name="$slug.png"
    ios_opts=(-strip)
  fi

  magick "$src" -resize "${WIDTH}x" "${webp_opts[@]}" "$DESKTOP_DIR/$slug.webp"
  cp "$DESKTOP_DIR/$slug.webp" "$ANDROID_DIR/scenery_$slug.webp"

  # Universal and unscaled: the scenery layout fits the canvas to the frame
  # width itself, so there is no @2x/@3x variant. The imageset is rewritten
  # whole so a format change never leaves the previous file behind, which
  # actool would still compile into the catalog.
  set_dir="$IOS_DIR/$imageset.imageset"
  rm -rf "$set_dir"
  mkdir -p "$set_dir"
  magick "$src" -resize "${WIDTH}x" "${ios_opts[@]}" "$set_dir/$ios_name"
  cat >"$set_dir/Contents.json" <<JSON
{
  "images" : [
    {
      "filename" : "$ios_name",
      "idiom" : "universal"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON

  echo "  $slug"
done

echo "Done."
