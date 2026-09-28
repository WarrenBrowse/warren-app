#!/usr/bin/env python3
"""Writes the SVG sources of the tray icon frames from the logo mark.

The tray icon is the W of graphics/logo-mark.svg. Its tunnel state is told by
how much of the inside of the W is filled, so it survives the monochrome
variants, where colour is gone:

  unsecured  outline only
  securing   filled to just under half (traffic blocked, no tunnel yet)
  secured    filled up to the tips of the ears, with a soft glow

The frames in between are the fill rising, which is what the tray animates
through when the state changes. generate-menubar-icons.sh renders them.

Usage: menubar-icon-frames.py [output-dir]
Standard library only, so it runs wherever the generator does.
"""

import math
import re
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
LOGO = SCRIPT_DIR.parents[3] / 'graphics' / 'logo-mark.svg'
DEFAULT_OUT = SCRIPT_DIR.parent / 'assets' / 'images' / 'menubar-icons' / 'svg'

# Every colour below has a beta counterpart in graphics/menubar-beta-palette.txt;
# the generator refuses a source painting anything that table does not name.
UNSECURED = '#C44E3A'
SECURING = '#C98A2E'
SECURING_FILL = '#E6BE78'
SECURED = '#7E964E'
SECURED_FILL = '#B2C77E'
PLACEHOLDER = '#8C8C8C'
# The notification dot keeps its colour in both builds: it means "attention",
# not a tunnel state. notification.svg is where that colour is set.
NOTIFICATION_SOURCE = 'notification.svg'

# Frames of the fill rising into the head, then into the ears. The tray steps
# one frame per tick, so these counts and the controller's pace set how long a
# state change takes.
HEAD_STEPS = 8
EAR_STEPS = 5

# How much of the inside of the W the securing state fills, by area.
SECURING_SHARE = 0.45

# The glow lives in the margin the generator leaves around the mark (3 px of 22
# on macOS), so the viewBox carries that margin too.
MARGIN_RATIO = 3 / 16


def parse_logo():
    """The logo's path data and its flattened contour, in y-down user units."""
    text = LOGO.read_text()
    d = re.search(r' d="([^"]+)"', text).group(1)
    view_box = [float(v) for v in re.search(r'viewBox="([^"]+)"', text).group(1).split()]
    flip = float(re.search(r'translate\(0,([-\d.]+)\)', text).group(1))
    tokens = re.findall(r'[MmCcLlZz]|-?\d+\.?\d*', d)
    points = []
    i = 0
    cmd = None
    x = y = 0.0
    while i < len(tokens):
        if tokens[i].isalpha():
            cmd = tokens[i]
            i += 1
        if cmd in 'Zz':
            continue
        count = 6 if cmd in 'Cc' else 2
        n = [float(v) for v in tokens[i:i + count]]
        i += count
        if cmd in 'Mm':
            x, y = n
            points.append((x, y))
            cmd = 'l' if cmd == 'm' else 'L'
        elif cmd == 'l':
            x, y = x + n[0], y + n[1]
            points.append((x, y))
        elif cmd == 'L':
            x, y = n
            points.append((x, y))
        else:
            rel = cmd == 'c'
            p0 = (x, y)
            p1 = (x + n[0], y + n[1]) if rel else (n[0], n[1])
            p2 = (x + n[2], y + n[3]) if rel else (n[2], n[3])
            p3 = (x + n[4], y + n[5]) if rel else (n[4], n[5])
            for k in range(1, 17):
                t = k / 16
                u = 1 - t
                points.append((
                    u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                    u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1],
                ))
            x, y = p3
    return d, flip, view_box, [(px, flip - py) for px, py in points]


def area(polygon):
    return abs(sum(
        a[0] * b[1] - b[0] * a[1] for a, b in zip(polygon, polygon[1:] + polygon[:1])
    )) / 2


def inner_ear_tips(contour, view_box):
    """The two free ends of the inner ear strokes: the lowest points of the
    contour in the middle band, where the band of the W opens onto the notch."""
    x0, y0, w, h = view_box
    tips = [
        i for i, p in enumerate(contour)
        if x0 + w * 0.2 < p[0] < x0 + w * 0.8 and y0 + h * 0.4 < p[1] < y0 + h * 0.75
        and p[1] >= contour[i - 1][1] and p[1] >= contour[(i + 1) % len(contour)][1]
    ]
    if len(tips) != 2:
        sys.exit(f'expected two inner ear tips in {LOGO}, found {len(tips)}')
    return sorted(tips)


def interior(contour, view_box):
    """The inside of the W: the side of the contour between the two ear tips
    that faces inwards, closed by a gentle curve across the notch."""
    a, b = inner_ear_tips(contour, view_box)
    arcs = [contour[a:b + 1], contour[b:] + contour[:a + 1]]
    arc = min(arcs, key=area)
    start, end = arc[-1], arc[0]
    sag = abs(end[0] - start[0]) * 0.08
    control = ((start[0] + end[0]) / 2, (start[1] + end[1]) / 2 + sag)
    bridge = [
        (
            (1 - t) ** 2 * start[0] + 2 * (1 - t) * t * control[0] + t * t * end[0],
            (1 - t) ** 2 * start[1] + 2 * (1 - t) * t * control[1] + t * t * end[1],
        )
        for t in (k / 12 for k in range(1, 12))
    ]
    return arc + bridge


def ease(t):
    # Half linear, half sine: soft at both ends without a first or last step so
    # small that it repeats the frame before it.
    return 0.5 * t + 0.5 * (0.5 - math.cos(math.pi * t) / 2)


def area_below(polygon, level):
    """Area of the part of `polygon` below the line y = level (y grows down)."""
    clipped = []
    for a, b in zip(polygon, polygon[1:] + polygon[:1]):
        a_in, b_in = a[1] >= level, b[1] >= level
        if a_in:
            clipped.append(a)
        if a_in != b_in:
            t = (level - a[1]) / (b[1] - a[1])
            clipped.append((a[0] + t * (b[0] - a[0]), level))
    return area(clipped) if len(clipped) > 2 else 0.0


def path_of(polygon):
    return 'M' + ' L'.join(f'{x:.0f} {y:.0f}' for x, y in polygon) + ' Z'


class Mark:
    def __init__(self):
        self.d, self.flip, logo_box, contour = parse_logo()
        polygon = interior(contour, logo_box)
        self.interior = path_of(polygon)
        self.bottom = max(y for _, y in polygon)
        self.top = min(y for _, y in polygon)
        self.polygon = polygon
        self.total_area = area(polygon)
        # A square viewBox centred on the mark, so the renderer never stretches
        # it, grown by the glow margin.
        x0, y0, w, h = logo_box
        side = max(w, h)
        margin = side * MARGIN_RATIO
        cx, cy = x0 + w / 2, y0 + h / 2
        self.view_box = (cx - side / 2 - margin, cy - side / 2 - margin,
                         side + 2 * margin, side + 2 * margin)
        self.unit = side / 16
        # The notification dot sits in the notch between the ear tops, the one
        # place of the mark that no state ever paints.
        ear_tops = [p for p in contour if p[1] < y0 + h * 0.3]
        left = max(p[0] for p in ear_tops if p[0] < cx)
        right = min(p[0] for p in ear_tops if p[0] > cx)
        self.dot = ((left + right) / 2, y0 + h * 0.14, self.unit * 1.8)

    def level_at(self, share):
        """The fill line under which `share` of the inside of the W lies. The
        frames step evenly in area rather than height, or the narrow base and
        the thin ears leave runs of frames that look the same."""
        low, high = self.top, self.bottom
        for _ in range(60):
            mid = (low + high) / 2
            if area_below(self.polygon, mid) > share * self.total_area:
                low = mid
            else:
                high = mid
        return (low + high) / 2

    def svg(self, outline, fill, level, glow, notification, mono, notification_colour=None):
        vb = ' '.join(f'{v:.1f}' for v in self.view_box)
        x, y, w, h = self.view_box
        if mono:
            outline = fill = '#000000'
        dot_x, dot_y, dot_r = self.dot
        gap = self.unit * 0.7
        defs = [
            f'<clipPath id="level"><rect x="{x:.0f}" y="{level:.0f}" '
            f'width="{w:.0f}" height="{y + h - level:.0f}"/></clipPath>',
        ]
        body = []
        if glow > 0 and not mono:
            defs.append(
                f'<filter id="glow" x="{x:.0f}" y="{y:.0f}" width="{w:.0f}" height="{h:.0f}" '
                f'filterUnits="userSpaceOnUse"><feGaussianBlur stdDeviation="{self.unit * 0.9:.0f}"/>'
                '</filter>'
            )
            body.append(
                f'<g filter="url(#glow)" opacity="{glow:.2f}" fill="{outline}">'
                f'<path d="{self.interior}"/>'
                f'<g transform="translate(0,{self.flip:.0f}) scale(1,-1)"><path d="{self.d}"/></g></g>'
            )
        if fill is not None and level < self.bottom:
            # The stroke tucks the edge of the fill under the outline, or the two
            # anti-aliased edges leave a light seam between them.
            body.append(
                f'<path fill="{fill}" stroke="{fill}" stroke-width="{self.unit * 0.6:.0f}" '
                f'stroke-linejoin="round" clip-path="url(#level)" d="{self.interior}"/>'
            )
        # A touch of weight on the outline: the logo's stroke is under a pixel
        # wide at 16 px and would never paint one fully opaque pixel there.
        body.append(
            f'<g transform="translate(0,{self.flip:.0f}) scale(1,-1)">'
            f'<path fill="{outline}" stroke="{outline}" stroke-width="{self.unit * 0.3:.0f}" '
            f'stroke-linejoin="round" d="{self.d}"/></g>'
        )
        content = ''.join(body)
        if notification:
            defs.append(
                f'<mask id="dot-gap" maskUnits="userSpaceOnUse" x="{x:.0f}" y="{y:.0f}" '
                f'width="{w:.0f}" height="{h:.0f}"><rect x="{x:.0f}" y="{y:.0f}" width="{w:.0f}" '
                f'height="{h:.0f}" fill="white"/><circle cx="{dot_x:.0f}" cy="{dot_y:.0f}" '
                f'r="{dot_r + gap:.0f}" fill="black"/></mask>'
            )
            dot_colour = '#000000' if mono else notification_colour
            content = (
                f'<g mask="url(#dot-gap)">{content}</g>'
                f'<circle fill="{dot_colour}" cx="{dot_x:.0f}" cy="{dot_y:.0f}" r="{dot_r:.0f}"/>'
            )
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}">'
            f'<defs>{"".join(defs)}</defs>{content}</svg>\n'
        )


def frames(mark):
    """(outline, fill, level, glow) for every frame, unsecured first."""
    out = [(UNSECURED, None, mark.bottom, 0.0)]
    for step in range(1, HEAD_STEPS + 1):
        share = SECURING_SHARE * ease(step / HEAD_STEPS)
        out.append((SECURING, SECURING_FILL, mark.level_at(share), 0.0))
    for step in range(1, EAR_STEPS + 1):
        t = ease(step / EAR_STEPS)
        share = SECURING_SHARE + (1 - SECURING_SHARE) * t
        level = mark.top if step == EAR_STEPS else mark.level_at(share)
        out.append((SECURED, SECURED_FILL, level, 0.55 * t))
    return out


def main():
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_OUT
    out_dir.mkdir(parents=True, exist_ok=True)
    notification_colour = re.search(
        r'#[0-9A-Fa-f]{6}', (DEFAULT_OUT / NOTIFICATION_SOURCE).read_text()).group(0)
    for stale in out_dir.glob('tray-*.svg'):
        stale.unlink()
    mark = Mark()
    for index, (outline, fill, level, glow) in enumerate(frames(mark), start=1):
        for notification in (False, True):
            for mono in (False, True):
                name = f'tray-{index}' + ('_mono' if mono else '') + (
                    '_notification' if notification else '')
                (out_dir / f'{name}.svg').write_text(
                    mark.svg(outline, fill, level, glow, notification, mono, notification_colour))
    (out_dir / 'tray-placeholder.svg').write_text(
        mark.svg(PLACEHOLDER, None, mark.bottom, 0.0, False, False))
    print(f'{len(frames(mark))} frames written to {out_dir}')


if __name__ == '__main__':
    main()
