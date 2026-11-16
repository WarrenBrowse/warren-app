import { describe, expect, it } from 'vitest';

import {
  homeVariables,
  surfaceDarkVariables,
  surfaceLightVariables,
} from '../../src/renderer/lib/foundations/variables';

// The tables the browser extension's design sync turns into CSS.
describe('the home tokens as CSS custom properties', () => {
  it('writes each measure with the unit its key names', () => {
    expect(homeVariables['--home-hero-header-inset']).toBe('14px');
    expect(homeVariables['--home-title-text-size']).toBe('26px');
    expect(homeVariables['--home-title-text-weight']).toBe('600');
    expect(homeVariables['--home-title-letter-spacing-em']).toBe('-0.01em');
    expect(homeVariables['--home-hero-scrim-stop']).toBe('0.34');
    expect(homeVariables['--home-hero-frame-ms']).toBe('900ms');
  });

  it('gives both palettes every home surface', () => {
    expect(surfaceDarkVariables['--surface-home-page']).toBe('rgb(20, 19, 17)');
    expect(surfaceLightVariables['--surface-home-page']).toBe('rgb(244, 240, 232)');
    expect(Object.keys(surfaceLightVariables)).toEqual(Object.keys(surfaceDarkVariables));
  });
});
