import { describe, expect, it } from 'vitest';

import { surfaceTokens } from '../../src/renderer/lib/foundations/tokens/surface-tokens';
import {
  surfaces,
  surfaceVariables,
} from '../../src/renderer/lib/foundations/variables/surface-variables';

function hex(css: string): string {
  const m = /^rgba?\((\d+), (\d+), (\d+)/.exec(css);
  if (!m) throw new Error(`not an rgb colour: ${css}`);
  return `#${[m[1], m[2], m[3]].map((v) => Number(v).toString(16).padStart(2, '0')).join('')}`;
}

describe('the connect screen surfaces', () => {
  // The values the popup mockup states, in hex as it states them. A surface
  // drifting from the mockup fails here rather than in a screenshot review.
  it('paint the dark theme as the mockup does', () => {
    const dark = surfaceTokens.dark;
    expect(hex(dark.card)).toBe('#282623');
    expect(hex(dark.line)).toBe('#45423c');
    expect(hex(dark.pill)).toBe('#d9a441');
    expect(hex(dark.pillText)).toBe('#231a06');
    expect(hex(dark.text)).toBe('#f2efe6');
    expect(hex(dark.textSecondary)).toBe('#d6d2c8');
    expect(hex(dark.textMuted)).toBe('#b5b0a4');
    expect(hex(dark.button)).toBe('#3a3834');
    expect(hex(dark.buttonLine)).toBe('#55524b');
    expect(hex(dark.exposed)).toBe('#f08a6e');
    expect(hex(dark.exposedWell)).toBe('#4a2a22');
    expect(hex(dark.protected)).toBe('#9fd07e');
    expect(hex(dark.protectedWell)).toBe('#243a1f');
  });

  it('paint the light theme in cream as the mockup does', () => {
    const light = surfaceTokens.light;
    expect(hex(light.card)).toBe('#f7f1e3');
    expect(hex(light.line)).toBe('#d9cdb2');
    expect(hex(light.pill)).toBe('#7a5412');
    expect(hex(light.pillText)).toBe('#ffffff');
    expect(hex(light.text)).toBe('#2a2822');
    expect(hex(light.textSecondary)).toBe('#4a463d');
    expect(hex(light.textMuted)).toBe('#5c574c');
    expect(hex(light.button)).toBe('#ebe3d0');
    expect(hex(light.buttonLine)).toBe('#cfc2a5');
    expect(hex(light.exposed)).toBe('#a3321c');
    expect(hex(light.exposedWell)).toBe('#f3d9cf');
    expect(hex(light.protected)).toBe('#2f6a2a');
    expect(hex(light.protectedWell)).toBe('#d8e8cc');
  });

  it('keep the connect and disconnect fills, white on dark green and brick red, in both themes', () => {
    for (const theme of [surfaceTokens.dark, surfaceTokens.light]) {
      expect(hex(theme.connect)).toBe('#3f6b2e');
      expect(hex(theme.disconnect)).toBe('#a8381f');
      expect(hex(theme.actionText)).toBe('#ffffff');
    }
  });

  it('write each theme under kebab-case variable names', () => {
    expect(surfaceVariables('light')['--surface-text-secondary']).toBe(
      surfaceTokens.light.textSecondary,
    );
    expect(surfaceVariables('dark')['--surface-card']).toBe(surfaceTokens.dark.card);
  });

  it('let components reference the variable, never the value', () => {
    expect(surfaces.textSecondary).toBe('var(--surface-text-secondary)');
    expect(surfaces.card).toBe('var(--surface-card)');
  });
});
