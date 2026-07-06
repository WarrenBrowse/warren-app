#!/usr/bin/env bash
# Turn the raw "L'univers de Bula" master art (calques) into the optimized,
# pre-registered WebP layers the connect screen composites at runtime.
#
# Why pre-registered full-frame layers: every master is authored on the same
# 1417x2120 canvas, so background + terrier + bula stack pixel-perfect with no
# per-layer positioning. Keeping the full frame (transparent elsewhere) makes the
# renderer a trivial 3x full-bleed <img> stack; the empty regions cost almost
# nothing in WebP.
#
# Output width 1140 = 3x the 380px window, so the art stays crisp on HiDPI.
#
# Usage: process-scenery.sh [SRC_DIR]
#   SRC_DIR defaults to ../../../new-da/calques (the master art, git-ignored).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SRC_DIR="${1:-$PKG_DIR/../../../new-da/calques}"
OUT_DIR="$PKG_DIR/assets/images/scenery"

WIDTH=1140
BG_QUALITY=82   # opaque landscapes
FG_QUALITY=88   # alpha layers (bula, terrier): keep soft edges clean

mkdir -p "$OUT_DIR"

# background name  ->  source file (opaque landscapes)
bg() { magick "$SRC_DIR/$1" -resize "${WIDTH}x" -strip -quality "$BG_QUALITY" "$OUT_DIR/$2"; echo "  $2"; }
# alpha layer name ->  source file (keep transparency, full frame)
fg() { magick "$SRC_DIR/$1" -resize "${WIDTH}x" -strip -define webp:alpha-quality=100 -quality "$FG_QUALITY" "$OUT_DIR/$2"; echo "  $2"; }

echo "Encoding scenery WebP into $OUT_DIR"
bg "Plaine V1.jpg"    "plaine.webp"
bg "allemagne V1.jpg" "germany.webp"
bg "Pays bas.jpg"     "netherlands.webp"
bg "Singapour V1.jpg" "singapore.webp"
fg "Terrier V1.png"   "terrier.webp"
fg "Bula V1.png"      "bula.webp"
echo "Done."
