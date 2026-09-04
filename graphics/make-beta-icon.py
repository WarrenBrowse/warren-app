#!/usr/bin/env python3
"""Derive the beta app icon from a prod icon SVG.

A non-prod build wears the same artwork with the two palette colours
exchanged plus an amber BETA badge, so a tester can tell which of the two
installs they are looking at. Everything is derived from the prod sources at
generation time, never kept as its own artwork, so the two cannot drift apart.

The badge sits inside the disc rather than overhanging it: Android masks the
adaptive icon down to a circle, and anything outside the safe zone is cropped.
The mark is shrunk slightly to make room, otherwise the badge eats the lower
half of the right ear.

Called by desktop/packages/mullvad-vpn/scripts/build-logo-icons.sh,
ios/scripts/generate-beta-app-icon.sh and android/scripts/generate-pngs.sh.
android/scripts/generate-beta-small-logo.py imports the pip geometry and the
letter from here, so the status-bar badge and this one cannot drift apart.
"""

import argparse
import pathlib
import re
import sys

SAND = '#F5ECDA'
BROWN = '#332818'

# Layout, as fractions of the disc radius, measured on the 252-unit prod
# canvas: badge centred at (182,182) with r=44 in a disc of r=126 at (126,126).
BADGE_OFFSET = 56 / 126
BADGE_RADIUS = 44 / 126
MARK_SCALE = 0.86
MARK_SHIFT_X = -8 / 126
MARK_SHIFT_Y = -12 / 126

MARK_ANCHOR = '<g transform="translate('

# The pip Android stamps on its status-bar logo is a rounded SQUARE with the B
# knocked OUT of it rather than drawn on it. A status-bar icon is a silhouette
# the system tints itself, so colour does not survive there and only a shape
# carries the meaning, the same reason lock-10_mono.svg turns its state dot into
# a hole. android/scripts/generate-beta-small-logo.py lays the pip out from
# these constants and the letter below.
PIP_CANVAS = 100.0
PIP_CORNER_RADIUS = 22.0
# Cap height of the knocked-out letter, on the same 100-unit canvas.
PIP_LABEL_HEIGHT = 60.0
# Bounding box of the B in the BETA wordmark's own glyph units, used to lift it
# out of the shared badge asset and to centre it in the pip.
B_GLYPH_BOX = (184.0, -1462.0, 1268.0, 0.0)
B_SUBPATH_COUNT = 3


def swap_palette(svg: str) -> str:
    if SAND not in svg or BROWN not in svg:
        sys.exit(f'source no longer uses {SAND} and {BROWN}, update the palette swap')
    return svg.replace(SAND, '\0').replace(BROWN, SAND).replace('\0', BROWN)


def canvas_size(svg: str) -> float:
    match = re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', svg)
    if not match or match.group(1) != match.group(2):
        sys.exit('expected a square viewBox starting at the origin')
    return float(match.group(1))


def badge_markup(badge_svg: str, centre: float, radius: float, label: bool) -> str:
    """Place the badge asset, scaled to `radius`, at 45 degrees inside the disc."""
    if label:
        inner = re.sub(r'^.*?<svg[^>]*>|</svg>\s*$', '', badge_svg, flags=re.S)
        inner = re.sub(r'<title>.*?</title>', '', inner, flags=re.S)
        # The asset is a 100-unit badge drawn around (50,50).
        scale = 2 * radius / 100
        return (
            f'<g transform="translate({centre - radius:.4f},{centre - radius:.4f}) '
            f'scale({scale:.6f})">{inner}</g>'
        )
    # Below ~128px the lettering is a smudge, so small renders take the disc
    # alone. It still reads as "not the prod icon" at 16px.
    return f'<circle cx="{centre:.4f}" cy="{centre:.4f}" r="{radius:.4f}" fill="#CA963C"/>'


def beta_letter_path(badge_svg: str) -> str:
    """The B of the shared BETA wordmark, so the pip and the app icon agree.

    The wordmark is one path whose subpaths run left to right, so the B is the
    run that starts inside its own bounding box: the outline plus its two
    counters. A wordmark redraw changes that count and stops the script rather
    than emitting a smudge.
    """
    match = re.search(r'<path[^>]*\sd="([^"]+)"', badge_svg)
    if not match:
        sys.exit('the badge asset has no path, cannot lift the B out of it')
    subpaths = ['M' + part for part in match.group(1).split('M') if part]
    x_min, _, x_max, _ = B_GLYPH_BOX
    kept = []
    for subpath in subpaths:
        start = re.match(r'M(-?[\d.]+)', subpath)
        if start and x_min <= float(start.group(1)) <= x_max:
            kept.append(subpath)
    if len(kept) != B_SUBPATH_COUNT:
        sys.exit('the BETA wordmark changed shape, update B_GLYPH_BOX and the subpath count')
    return ''.join(kept)


def build_icon(source: str, badge_svg: str, label: bool) -> str:
    svg = swap_palette(source)
    size = canvas_size(svg)
    disc_radius = size / 2

    anchor = svg.find(MARK_ANCHOR)
    if anchor == -1:
        sys.exit('could not find the mark group to shrink')
    shift_x = MARK_SHIFT_X * disc_radius
    shift_y = MARK_SHIFT_Y * disc_radius
    wrapper = (
        f'<g transform="translate({disc_radius},{disc_radius}) scale({MARK_SCALE}) '
        f'translate({-disc_radius},{-disc_radius}) translate({shift_x:.4f},{shift_y:.4f})">'
    )
    svg = svg[:anchor] + wrapper + svg[anchor:]

    badge = badge_markup(
        badge_svg,
        disc_radius + BADGE_OFFSET * disc_radius,
        BADGE_RADIUS * disc_radius,
        label,
    )
    return svg.replace('</svg>', f'</g>{badge}</svg>')


def build_badge_only(badge_svg: str, size: float, circle_radius: float, label: bool) -> str:
    """Badge alone on a transparent canvas, for the Android foreground composite.

    Android's visible circle is smaller than the canvas (the adaptive-icon safe
    zone), so the caller passes its radius and the badge lands at the same spot
    relative to the disc as it does on the desktop.
    """
    badge = badge_markup(
        badge_svg,
        size / 2 + BADGE_OFFSET * circle_radius,
        BADGE_RADIUS * circle_radius,
        label,
    )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{size:g}" height="{size:g}" '
        f'viewBox="0 0 {size:g} {size:g}">{badge}</svg>\n'
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--badge', required=True, help='path to graphics/beta-badge.svg')
    parser.add_argument('--output', required=True)
    parser.add_argument('--source', help='prod icon SVG to derive from')
    parser.add_argument('--badge-only', action='store_true')
    parser.add_argument('--size', type=float, help='canvas size for --badge-only')
    parser.add_argument('--circle-radius', type=float, help='visible disc radius for --badge-only')
    parser.add_argument('--no-label', action='store_true', help='draw the disc without BETA')
    args = parser.parse_args()

    badge_svg = pathlib.Path(args.badge).read_text()
    if args.badge_only:
        if args.size is None or args.circle_radius is None:
            parser.error('--badge-only needs --size and --circle-radius')
        out = build_badge_only(badge_svg, args.size, args.circle_radius, not args.no_label)
    else:
        if not args.source:
            parser.error('--source is required')
        out = build_icon(pathlib.Path(args.source).read_text(), badge_svg, not args.no_label)
    pathlib.Path(args.output).write_text(out)


if __name__ == '__main__':
    main()
