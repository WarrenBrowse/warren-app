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

// The mockup's proportions (Warren VPN Accueil, 360x640): a 300px hero on a
// 360px width, with the mockup's short fade into the page behind the status
// pill. The clear zone ends where that fade reaches half opacity.
export const HERO_HEIGHT_RATIO = 300 / 360;
export const HERO_MASK_STOPS: Array<[number, number]> = [
  [0, 1],
  [0.86, 1],
  [0.94, 0.5],
  [1, 0],
];
export const CLEAR_BOTTOM_FRACTION = 0.94;

// The band under the clear zone, for a hero of this height.
export function heroFade(heroHeight: number): number {
  return heroHeight * (1 - CLEAR_BOTTOM_FRACTION);
}

// The cameras, the whole burrow and Bula down to his shins while he is out,
// which puts the cameras where the mockup draws them.
const GROUND_SPAN = { top: 320, bottom: 995 } as const;
// Once he is in, the skyline down to the top of the burrow.
const SKY_BOTTOM_ROW = 760;

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

// The landscape a phase shows: the exit country's while a tunnel carries the
// traffic there, the plain otherwise.
export function phaseLandscape(
  phase: ConnectionPhase,
  country: string | undefined,
): LandscapeLayer {
  return sceneryManifest.phases[phase].landscape === 'country' ? landscapeFor(country) : PLAIN;
}

export function framingSpan(phase: ConnectionPhase, country: string | undefined): FramingSpan {
  if (sceneryManifest.phases[phase].framing === 'ground') {
    return { ...GROUND_SPAN };
  }
  return { top: phaseLandscape(phase, country).skylineTopRow, bottom: SKY_BOTTOM_ROW };
}
