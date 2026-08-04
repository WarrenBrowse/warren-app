# Scenery art: reimporting "L'univers de Bula"

The connect screen of all three clients draws the same illustrated scene: a
landscape for the exit country, the burrow in the foreground, and Bula the
rabbit, who ducks inside once the tunnel is up. New art arrives from the
designer as a numbered version (V14, V15, ...) of full-canvas masters.

## The procedure, when new masters land

1. Put the masters in `new-da/calques/` at the repo root. That path is
   git-ignored, and it is where `process-scenery.sh` looks by default. A folder
   named anything else (`new DA/`, `V15/`) is NOT ignored and will end up in a
   commit.
2. Update the `LAYERS` table in
   `desktop/packages/mullvad-vpn/scripts/process-scenery.sh` if the file names
   carry a new version number, then run it with no arguments.
3. Commit the regenerated assets. `test/unit/scenery-assets.spec.ts` is the gate;
   it runs in `warren-checks`.

**Never convert a layer by hand, and never regenerate one platform alone.** That
is not a style preference: the layers are pre-registered full-frame images that
stack with no per-layer positioning, so they only line up if every client ships
the same canvas at the same size. Singapore stayed photoreal on desktop for a
whole release after the watercolor pass because one platform was exported
separately. The script emits all three from one source of truth in one run.

## What the masters must be

Every layer is authored on the same canvas (1417x2120 for V14) and exported
full-frame: the burrow layer is transparent everywhere except the mound, the
rabbit layer everywhere except the rabbit and its cast shadow. The empty regions
cost almost nothing once encoded, and they are what makes the renderer a trivial
three-image stack instead of a positioning problem.

Backgrounds are opaque and may be JPEG. The two foreground layers need a real
alpha channel, so they are PNG.

## The output formats, and why each one

Output width is 1140, three times the 380px desktop window, so the art stays
crisp on HiDPI.

| client | backgrounds | alpha layers |
|---|---|---|
| desktop | WebP q82 | WebP q88, `alpha-quality=100` |
| Android | the same WebP files, byte for byte | the same |
| iOS | JPEG q92, 4:4:4 chroma | PNG |

**iOS backgrounds are JPEG, not PNG.** Asset catalogs reject WebP (`actool`
answers "does not have a valid extension"), so iOS cannot share the WebP. Storing
the landscapes as PNG conserved nothing, because the masters are themselves JPEG
q99, and it cost 10 MB: `Assets.car` measured 17 MB with PNG landscapes against
7 MB with JPEG ones.

The measurement that settles the quality question: `xcrun assetutil --info`
reports the JPEG entries with `Encoding: JPEG` at exactly their source size, so
`actool` stores the JPEG stream verbatim rather than re-encoding it. There is one
lossy generation, not two. Against the lossless downscale, q92 measures 38.8 to
43.1 dB PSNR depending on the image; on the busiest one the error peaks at 5% of
range and averages 0.7%, spread as grain over the paper texture with no ringing
on the pencil lines.

Two details that are load-bearing rather than incidental:

- **4:4:4 chroma** (`-sampling-factor 1x1`). The art leans on thin coloured
  strokes, flags and window frames, which default chroma subsampling smears.
- **The alpha layers stay PNG.** Asking `actool` for `compression-type: lossy`
  takes the catalog down to 3.7 MB, but it puts the alpha channel through a codec
  we do not control, and a lossy alpha halos the fur outline against the
  landscape. Those two layers are only 1.8 MB; the trade is not worth it.

HEIC was measured and rejected: feeding `actool` a lossy HEIC costs a second
encode of already-degraded data and still lands at 12 MB.

## Adding a country

`resolveScenery` and its country table are duplicated per client, deliberately,
because each renders natively:

- `desktop/packages/mullvad-vpn/src/renderer/components/CountryBackdrop/scenery.ts`
- `android/lib/feature/home/impl/.../connect/ConnectionPhase.kt`
- `ios/WarrenVPN/View controllers/Tunnel/MapViewController.swift`

Add the slug to the script's `LAYERS` table and to all three lookups. An exit
with no bespoke art falls back to the plain, which is also the backdrop of the
states where no tunnel carries the traffic (the cameras trained on it are what
tells the user they are exposed). `test/unit/connection-scenery.spec.ts` and
`SceneryStateTest.kt` pin that mapping.
