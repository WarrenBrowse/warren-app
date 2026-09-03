#!/usr/bin/env python3
"""Render the three "Auto-connect & Lockdown mode" carousel slides.

The slides depict the Android system VPN settings, which name the app and show
its launcher icon, so they have to be regenerated whenever the product name or
the icon changes. The plates in `plates/` carry everything that does not depend
on the product (the system chrome, the switches, the arrows); this script draws
the app name and the launcher icon onto them and emits the five density buckets.

    ./make.py --product "Warren VPN Beta" \\
              --icon ../../app/src/beta/res/drawable-nodpi/ic_warren_foreground.png \\
              --icon-background '#332818' \\
              --out ../../app/src/beta/res

The font has to be Roboto, the family the depicted screens are drawn in; it is
not in the repo, so pull it from any Android device or emulator:

    adb pull /system/fonts/Roboto-Regular.ttf

Requires Pillow (`pip install pillow`).
"""

import argparse
import pathlib

from PIL import Image, ImageDraw, ImageFont

# The plates are xxxhdpi (4x); the other buckets are downscales of them.
DENSITIES = {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}

# Measured off the plates, in plate pixels: text left edge and baseline, size,
# weight on Roboto's variable axis, and colour. The app bar title is drawn in
# Medium like the system draws it, the list row title in Regular.
APP_BAR = dict(x=216, baseline=103, size=60, weight=500, fill=(255, 255, 255, 255))
LIST_ROW = dict(x=226, baseline=262, size=63, weight=400, fill=(224, 224, 224, 255))

# The launcher icon in the VPN list, masked to a circle by the system.
ICON_BOX = (49, 210, 124)  # left, top, diameter

# An adaptive icon's foreground is a 108 dp canvas whose visible part is the
# middle 72 dp; everything outside is what the system's mask crops away.
ADAPTIVE_VISIBLE = 72 / 108

SLIDES = {
    "carousel_slide_1_cogwheel": ("slide_1_cogwheel", LIST_ROW, True),
    "carousel_slide_2_always_on": ("slide_2_always_on", APP_BAR, False),
    "carousel_slide_3_block_connections": ("slide_3_block_connections", APP_BAR, False),
}


def roboto(font_path: pathlib.Path, size: int, weight: int) -> ImageFont.FreeTypeFont:
    font = ImageFont.truetype(str(font_path), size)
    font.set_variation_by_axes([float(weight), 100.0, 0.0])
    return font


def launcher_icon(foreground: pathlib.Path, background: str, diameter: int) -> Image.Image:
    layer = Image.new("RGBA", Image.open(foreground).size, background)
    layer.alpha_composite(Image.open(foreground).convert("RGBA"))
    keep = round(layer.width * ADAPTIVE_VISIBLE)
    offset = (layer.width - keep) // 2
    layer = layer.crop((offset, offset, offset + keep, offset + keep)).resize(
        (diameter, diameter), Image.LANCZOS
    )
    mask = Image.new("L", (diameter, diameter), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, diameter - 1, diameter - 1), fill=255)
    icon = Image.new("RGBA", (diameter, diameter), (0, 0, 0, 0))
    icon.paste(layer, (0, 0), mask)
    return icon


def render(plate: pathlib.Path, product: str, text: dict, icon: Image.Image | None,
           font_path: pathlib.Path) -> Image.Image:
    image = Image.open(plate).convert("RGBA")
    if icon is not None:
        image.alpha_composite(icon, (ICON_BOX[0], ICON_BOX[1]))
    ImageDraw.Draw(image).text(
        (text["x"], text["baseline"]),
        product,
        font=roboto(font_path, text["size"], text["weight"]),
        fill=text["fill"],
        anchor="ls",
    )
    return image


def main() -> None:
    here = pathlib.Path(__file__).parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", default="Warren VPN", help="app name as Android lists it")
    parser.add_argument(
        "--icon",
        type=pathlib.Path,
        default=here / "../../lib/ui/resource/src/main/res/drawable-nodpi/ic_warren_foreground.png",
        help="adaptive icon foreground layer",
    )
    parser.add_argument("--icon-background", default="#F5ECDA", help="adaptive icon background")
    parser.add_argument("--font", type=pathlib.Path, default=here / "Roboto-Regular.ttf")
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=here / "../../lib/ui/resource/src/main/res",
        help="res directory receiving the drawable-<density> buckets",
    )
    args = parser.parse_args()

    icon = launcher_icon(args.icon, args.icon_background, ICON_BOX[2])
    for name, (plate, text, with_icon) in SLIDES.items():
        full = render(
            here / "plates" / f"{plate}.png",
            args.product,
            text,
            icon if with_icon else None,
            args.font,
        )
        for density, scale in DENSITIES.items():
            bucket = args.out / f"drawable-{density}"
            bucket.mkdir(parents=True, exist_ok=True)
            size = (round(full.width * scale / 4), round(full.height * scale / 4))
            full.resize(size, Image.LANCZOS).save(bucket / f"{name}.png", optimize=True)
        print(f"{name}: {full.width}x{full.height} -> {len(DENSITIES)} buckets")


if __name__ == "__main__":
    main()
