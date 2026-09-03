# Graphic assets

This folder contains graphic assets that are used to generate assets for either the app or platforms
where the app is distributed.

## Android

The `Android-feature-graphics.psd` file should be used to generate a PNG image to be used as the
feature graphics in the app's Google Play Store listing. The PNG image should be placed in the
`android/app/src/main/play/listings/en-US/graphics/feature-graphics/` directory.

## Logo (canonical V2 sources)

`logo-mark.svg` is the rabbit-ears "W" mark alone and `logo-wordmark.svg` the
full "Warren" lockup, both tight-cropped and painted with `currentColor`. They
are THE reference vectors: every icon below and the in-app logos (desktop
`assets/images/warren-mark.svg` / `warren-wordmark.svg`, Android vector
drawables, iOS icon set) derive from this artwork. Do not reintroduce older
logo drafts; edit these two files and regenerate.

## Icons (the logo in different versions)

### Beta and prod wear the palette swapped, plus a BETA badge

There is no separate beta artwork. A non-prod build (beta, staging) ships the
same icon with the two palette colours exchanged, brown background `#332818`
and sand mark `#F5ECDA`, carrying the amber badge from `beta-badge.svg` in its
lower-right. That is what tells a tester which of the two installs they are
looking at, on the desktop, in the dock, on the home screen and in the Windows
installer.

Two things the badge layout has to respect, both already handled by
`make-beta-icon.py`:

* It sits **inside** the disc rather than overhanging it. Android masks the
  adaptive icon down to a circle and crops anything outside the safe zone.
* Under 128px the lettering is a smudge, so small renders take the amber disc
  alone. It still reads as "not the prod build" at 16px. The mark is shrunk to
  86% to make room, otherwise the badge eats the lower half of the right ear.

The badge lettering is baked as outlines (Open Sans Bold, the app's own font,
already in `desktop/packages/mullvad-vpn/assets/fonts`), so rendering never
depends on which fonts the machine happens to have installed.

The swapped assets are derived from the prod sources by the generation scripts,
never kept as their own files, so the two can never drift apart:

* Desktop: `build-logo-icons.sh` writes `icon-beta.icns`, `icon-macos-beta.icns`,
  `icon-beta.ico` and `windows/installersidebar-beta.bmp` next to their prod
  counterparts, and `tasks/distribution.cjs` picks the set from
  `WARREN_PRODUCT_ENV`.
* Android: `generate-pngs.sh` writes the badged foreground and its monochrome
  counterpart (themed icons draw that layer alone, so the badge is a knockout
  silhouette there) into the beta flavor overlay
  (`android/app/src/beta/res/`), whose `colors.xml` carries the brown
  `icon_background`.
* iOS: `ios/scripts/generate-beta-app-icon.sh` writes the `AppIconBeta`
  appiconset next to `AppIcon`, and `ASSETCATALOG_COMPILER_APPICON_NAME` picks
  the set from `WARREN_APPICON_NAME`. The light appearance is the whole
  artwork so it is rendered from `icon-square.svg`; the dark and tinted ones
  are the mark alone on a transparent canvas, so they take the prod layer with
  the badge composited on, and the tinted badge is the same knockout
  silhouette Android's themed icon uses (iOS recolours that appearance from
  the luminance it is given).

If you change the palette, change it in the SVGs and in the `SAND` / `BROWN`
constants of `make-beta-icon.py`, then regenerate both. The generator refuses
to run if the SVGs stopped using the colours it knows about, so a palette
change can never silently ship a beta icon identical to prod.

### `icon.svg`

The official app icon: the V2 mark on a sand circle (art-direction palette). Used to generate icons on a bunch of platforms.

If `icon.svg` is changed. You need to run the following to generate new assets:
* Desktop: `desktop/packages/mullvad-vpn/scripts/build-logo-icons.sh`
* Android: `android/scripts/generate-pngs.sh`

### `icon-square.svg`

Same mark but on a full sand square instead of the circle.
The mark is drawn slightly larger than in `icon.svg` since the rounded-off corners eat less of it.

#### Desktop

The square icon is used on desktop as the base for the macOS icons. To update them:

1. Create the macOS icons by inserting the updated `/graphics/icon-square.svg` into Apple's macOS
icon template available at https://developer.apple.com/design/resources/.
1. Save the icons to `/graphics/macOS/`
1. Run `scripts/build-logo-icons.sh`

#### Android

The `icon-square.svg` is used to generate Android's square icon used in the app's Google Play Store
listing. The resulting 512x512 PNG image should be placed in the
`android/app/src/main/play/listings/en-US/graphics/icon/` directory. The file can be generate with the
following command:

```
rsvg-convert ./icon-square.svg -w 512 -h 512 -o ../android/app/src/main/play/listings/en-US/graphics/icon/icon.png
```

### `icon-android.svg` & `icon-android-mono.svg`

The icon `icon-android.svg` is used for Android adaptive icon. The icon converted to
Android Vector Drawable format and used as foreground layer for adaptive icon. For background layer is used
solid color layer. Full documentation about adaptive icon available on link below:
https://developer.android.com/guide/practices/ui_guidelines/icon_design_adaptive

`icon-android-mono.svg` is the monochromatic version. It's used as "themed icons" on Android.

### `icon-shaved.svg`

This is a simplified version of the logo with the whiskers and fur removed. This version should be used
when rendering the Warren mark in tiny versions where the little details in the logo would not be visible
anyway, and would just make the small assets look less clean.

Nothing generates from it today: the Android status-bar and quick-settings mark is the
hand-maintained vector `android/lib/ui/resource/src/main/res/drawable/small_logo_*.xml`.
