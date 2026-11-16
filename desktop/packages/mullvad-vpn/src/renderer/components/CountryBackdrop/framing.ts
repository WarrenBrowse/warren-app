import sceneryManifest from '../../../../assets/images/scenery/scenery.json';
import type { ConnectionPhase } from '../../lib/connection-phase';

// The hero framing every client computes the same way, pinned by
// fixtures/client-rules/scenery_framing.json. The three layers are registered on
// one canvas, so they are always drawn with one transform; what changes is which
// rows of that canvas must be fully visible between the floating header and the
// fade into the panel.

export const CANVAS_WIDTH = sceneryManifest.canvas.width;
export const CANVAS_HEIGHT = sceneryManifest.canvas.height;

// The canvas edge facing a headroom strip or a side margin fades over this many
// canvas rows into the landscape's top fill, so the join does not show.
export const FEATHER_ROWS = 48;

// The cameras and the whole of Bula (his feet are the last row of the rabbit
// layer with alpha >= 128); the burrow mouth lies inside.
const GROUND_SPAN = { top: 320, bottom: 1131 } as const;
// Once Bula is in, the skyline down to the burrow mouth he hides in.
const SKY_BOTTOM_ROW = 930;

const THUMB_MARGIN_ROWS = 40;

export interface FramingSpan {
  top: number;
  bottom: number;
}

export interface FramingInput {
  width: number;
  height: number;
  // Below this offset nothing floats over the art (header bottom plus margin).
  clearTop: number;
  // The bottom band where the art fades into the panel.
  fade: number;
  span: FramingSpan;
}

export interface Frame {
  scale: number;
  left: number;
  top: number;
  width: number;
  height: number;
  // The strip above a canvas that had to drop to clear the header.
  headroom: number;
}

export function frameScenery({ width, height, clearTop, fade, span }: FramingInput): Frame {
  const clearBottom = height - fade;
  const scale = Math.min(width / CANVAS_WIDTH, (clearBottom - clearTop) / (span.bottom - span.top));
  const yMin = clearTop - span.top * scale;
  const yMax = clearBottom - span.bottom * scale;
  const top = Math.min(yMax, Math.max(yMin, 0));
  return {
    scale,
    left: (width - CANVAS_WIDTH * scale) / 2,
    top,
    width: CANVAS_WIDTH * scale,
    height: CANVAS_HEIGHT * scale,
    headroom: Math.max(0, top),
  };
}

type LandscapeLayer = (typeof sceneryManifest.layers)[number] & {
  skylineTopRow: number;
  topFill: string;
};

function isLandscape(layer: (typeof sceneryManifest.layers)[number]): layer is LandscapeLayer {
  return 'skylineTopRow' in layer && 'topFill' in layer;
}

const plainLayer = sceneryManifest.layers.filter(isLandscape).find((l) => l.role === 'plain');
if (!plainLayer) throw new Error('scenery.json has no plain landscape');
const PLAIN: LandscapeLayer = plainLayer;

const LANDSCAPES: Readonly<Record<string, LandscapeLayer>> = Object.fromEntries(
  sceneryManifest.layers
    .filter(isLandscape)
    .filter((layer) => layer.role === 'country')
    .flatMap((layer) => [
      [(layer.countryName ?? '').toLowerCase(), layer],
      [(layer.country ?? '').toLowerCase(), layer],
    ]),
);

// The landscape a country is drawn with: its own art, or the plain.
export function landscapeFor(country: string | undefined): LandscapeLayer {
  return LANDSCAPES[(country ?? '').trim().toLowerCase()] ?? PLAIN;
}

export function hasCountryArt(country: string | undefined): boolean {
  return (country ?? '').trim().toLowerCase() in LANDSCAPES;
}

export function framingSpan(phase: ConnectionPhase, country: string | undefined): FramingSpan {
  const row = sceneryManifest.phases[phase];
  if (row.framing === 'ground') {
    return { ...GROUND_SPAN };
  }
  const landscape = row.landscape === 'country' ? landscapeFor(country) : PLAIN;
  return { top: landscape.skylineTopRow, bottom: SKY_BOTTOM_ROW };
}

// The round location thumbnail: a full-width square of the landscape whose top
// sits just above the skyline, so the landmark crowns it.
export function thumbnailTopRow(skylineTopRow: number): number {
  return Math.min(CANVAS_HEIGHT - CANVAS_WIDTH, Math.max(0, skylineTopRow - THUMB_MARGIN_ROWS));
}
