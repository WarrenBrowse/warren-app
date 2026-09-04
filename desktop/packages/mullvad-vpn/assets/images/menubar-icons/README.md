This directory contains the images for the menubar/traybar. The content consists of:
  * SVG files for the colored version of each frame
  * png/ico files which are created from the svg files. These should not be edited or replaced
  manually.

## Build script
The png/ico files are generated using the script `../../scripts/generate-menubar-icons.sh` which can be
run from the `desktop/packages/mullvad-vpn`-directory using
```sh
./scripts/generate-menubar-icons.sh
```

The script crates all menubar images for all platforms including the monochrome ones.

It writes each platform's tree twice: the prod icons at the root, and a `beta/`
subdirectory holding the same file names with the lock drawn in another hue
family. `src/main/tray-icon.ts` inserts that segment for every non-prod product
environment (beta and staging), so a machine running two Warren installs never
shows two identical tray icons.

The accent table, and why those hues, is
[`graphics/menubar-beta-palette.txt`](../../../../../../graphics/menubar-beta-palette.txt);
the generator refuses to write an icon in which a production accent survived,
and `test/unit/tray-icon-beta-palette.spec.ts` reads the pixels back. The
monochrome variants (macOS Template, Windows black and white, Linux white) hold
identical bytes in both trees: they are single-tint alpha masks with no colour
to move, so the distinction is lost for anyone who turns the monochromatic tray
icon on.

### Dependencies
Imagemagick is required for the script to run.

