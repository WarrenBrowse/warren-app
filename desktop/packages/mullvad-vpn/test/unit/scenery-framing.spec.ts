/* eslint-disable @typescript-eslint/naming-convention */
// The property names below are the fixture's own spelling, shared with the
// Kotlin and Swift readers of the same file, so they stay snake_case here.
import { describe, expect, it } from 'vitest';

import sceneryManifest from '../../assets/images/scenery/scenery.json';
import {
  CLEAR_BOTTOM_FRACTION,
  frameScenery,
  framingSpan,
  HERO_HEIGHT_RATIO,
  HERO_MASK_STOPS,
  heroFade,
  thumbnailTopRow,
} from '../../src/renderer/components/CountryBackdrop/framing';
import { loadClientRules } from './client-rules';

interface FramingCase {
  name: string;
  hero_px: [number, number];
  clear_top_px: number;
  fade_px: number;
  span_rows: [number, number];
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
  mask_stops: Array<[number, number]>;
  clear_bottom_fraction: number;
  feather_rows: number;
  framings: {
    ground: { top_row: number; bottom_row: number };
    sky: { bottom_row: number };
  };
  thumbnail: {
    thumb_margin_rows: number;
    cases: Array<{ skyline_top_row: number; top_row: number }>;
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

  it('keeps every span between the header and the fade', () => {
    for (const fixtureCase of fixture.cases) {
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

  it('crowns the thumbnail with the skyline', () => {
    for (const thumbCase of fixture.thumbnail.cases) {
      expect(thumbnailTopRow(thumbCase.skyline_top_row)).toBe(thumbCase.top_row);
    }
  });
});
