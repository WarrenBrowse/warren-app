/* eslint-disable @typescript-eslint/naming-convention */
// The property names below are the fixture's own spelling, shared with the
// Kotlin and Swift readers of the same file, so they stay snake_case here.
import { describe, expect, it } from 'vitest';

import sceneryManifest from '../../assets/images/scenery/scenery.json';
import {
  CLEAR_BOTTOM_FRACTION,
  COMPACT_TRANSITION_MS,
  frameScenery,
  framingSpan,
  HERO_COMPACT_RATIO,
  HERO_HEIGHT_RATIO,
  HERO_MASK_STOPS,
  HERO_MAX_HEIGHT_RATIO,
  heroFade,
  MARGIN_BLUR_PX,
} from '../../src/renderer/components/CountryBackdrop/framing';
import { loadClientRules } from './client-rules';

interface FramingCase {
  name: string;
  hero_px: [number, number];
  clear_top_px: number;
  fade_px: number;
  span_rows: [number, number];
  compact?: boolean;
  expect: {
    scale: number;
    canvas_left_px: number;
    canvas_top_px: number;
    canvas_width_px: number;
    canvas_height_px: number;
    headroom_px: number;
    span_top_px: number;
    span_bottom_px: number;
  };
}

interface FramingFixture {
  canvas_px: [number, number];
  hero_height_ratio: number;
  hero_max_height_ratio: number;
  compact_height_ratio: number;
  compact_transition_ms: number;
  margin_blur_px: number;
  mask_stops: Array<[number, number]>;
  clear_bottom_fraction: number;
  feather_rows: number;
  framings: {
    ground: { top_row: number; bottom_row: number };
    sky: { bottom_row: number };
  };
  cases: FramingCase[];
}

const fixture = loadClientRules<FramingFixture>('scenery_framing.json');

describe('scenery framing (fixtures/client-rules/scenery_framing.json)', () => {
  it('works on the canvas scenery.json declares', () => {
    expect(fixture.canvas_px).toEqual([
      sceneryManifest.canvas.width,
      sceneryManifest.canvas.height,
    ]);
  });

  it('draws the hero at the mockup proportions and fades it along the mockup curve', () => {
    expect(HERO_HEIGHT_RATIO).toBeCloseTo(fixture.hero_height_ratio, 6);
    expect(HERO_MASK_STOPS).toEqual(fixture.mask_stops);
    expect(CLEAR_BOTTOM_FRACTION).toBe(fixture.clear_bottom_fraction);
    expect(HERO_MAX_HEIGHT_RATIO).toBe(fixture.hero_max_height_ratio);
    expect(HERO_COMPACT_RATIO).toBeCloseTo(fixture.compact_height_ratio, 6);
    expect(COMPACT_TRANSITION_MS).toBe(fixture.compact_transition_ms);
    expect(MARGIN_BLUR_PX).toBe(fixture.margin_blur_px);
  });

  it('puts the clear bottom where the mask crosses half opacity', () => {
    const stops = fixture.mask_stops;
    const at = fixture.clear_bottom_fraction;
    const index = stops.findIndex(([fraction]) => fraction >= at);
    const [f0, a0] = stops[index - 1];
    const [f1, a1] = stops[index];
    expect(a0 + ((a1 - a0) * (at - f0)) / (f1 - f0)).toBeCloseTo(0.5, 6);
  });

  it('ends the clear zone where the fade reaches half opacity', () => {
    for (const fixtureCase of fixture.cases) {
      expect(heroFade(fixtureCase.hero_px[1])).toBeCloseTo(fixtureCase.fade_px, 2);
    }
  });

  for (const fixtureCase of fixture.cases) {
    it(`frames ${fixtureCase.name}`, () => {
      const [width, height] = fixtureCase.hero_px;
      const [top, bottom] = fixtureCase.span_rows;
      const frame = frameScenery({
        width,
        height,
        clearTop: fixtureCase.clear_top_px,
        fade: fixtureCase.fade_px,
        span: { top, bottom },
        compact: fixtureCase.compact,
      });
      const e = fixtureCase.expect;
      expect(frame.scale).toBeCloseTo(e.scale, 4);
      expect(frame.left).toBeCloseTo(e.canvas_left_px, 1);
      expect(frame.top).toBeCloseTo(e.canvas_top_px, 1);
      expect(frame.width).toBeCloseTo(e.canvas_width_px, 1);
      expect(frame.height).toBeCloseTo(e.canvas_height_px, 1);
      expect(frame.headroom).toBeCloseTo(e.headroom_px, 1);
      expect(frame.top + top * frame.scale).toBeCloseTo(e.span_top_px, 1);
      expect(frame.top + bottom * frame.scale).toBeCloseTo(e.span_bottom_px, 1);
    });
  }

  it('keeps every full-height span between the header and the fade', () => {
    for (const fixtureCase of fixture.cases.filter((c) => !c.compact)) {
      const [width, height] = fixtureCase.hero_px;
      const [top, bottom] = fixtureCase.span_rows;
      const frame = frameScenery({
        width,
        height,
        clearTop: fixtureCase.clear_top_px,
        fade: fixtureCase.fade_px,
        span: { top, bottom },
      });
      expect(frame.top + top * frame.scale).toBeGreaterThanOrEqual(fixtureCase.clear_top_px - 0.01);
      expect(frame.top + bottom * frame.scale).toBeLessThanOrEqual(
        height - fixtureCase.fade_px + 0.01,
      );
    }
  });

  it('keeps a compact hero at width-fit with the span top under the header', () => {
    for (const fixtureCase of fixture.cases.filter((c) => c.compact)) {
      const [width, height] = fixtureCase.hero_px;
      const [top, bottom] = fixtureCase.span_rows;
      const frame = frameScenery({
        width,
        height,
        clearTop: fixtureCase.clear_top_px,
        fade: fixtureCase.fade_px,
        span: { top, bottom },
        compact: true,
      });
      expect(frame.width).toBeCloseTo(width, 6);
      expect(frame.top + top * frame.scale).toBeCloseTo(fixtureCase.clear_top_px, 6);
    }
  });

  it('frames the ground span whenever Bula is out, the skyline once he is in', () => {
    for (const [phase, row] of Object.entries(sceneryManifest.phases)) {
      expect(row.framing, phase).toBe(row.bula ? 'ground' : row.framing);
    }
    expect(framingSpan('exposed', 'netherlands')).toEqual({
      top: fixture.framings.ground.top_row,
      bottom: fixture.framings.ground.bottom_row,
    });
    expect(framingSpan('protected', 'netherlands')).toEqual({
      top: 100,
      bottom: fixture.framings.sky.bottom_row,
    });
    // An exit without bespoke art is framed on the plain's own skyline.
    expect(framingSpan('protected', 'sweden')).toEqual({
      top: 313,
      bottom: fixture.framings.sky.bottom_row,
    });
  });
});
