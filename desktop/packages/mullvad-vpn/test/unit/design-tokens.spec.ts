import { readFileSync } from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import {
  buildTokens,
  cssColorToHex,
  JSON_PATH,
  KOTLIN_PATH,
  renderJson,
  renderKotlin,
  sha256,
} from '../../../../../scripts/design-tokens/lib.mjs';

// The desktop token files and the connect-screen component sources are the
// single source of truth for the design system. design-tokens.json at the repo
// root is generated from them, and Android's DesignTokens.kt is generated from
// the JSON. The lockstep used to be a comment and a hand copy, which is how the
// BETA chip, the status tints and the card geometry drifted on Android. This
// gate regenerates both outputs in memory and fails on any staleness, the way
// scenery-assets.spec.ts pins the three art trees. Regenerate with
// `node scripts/design-tokens/gen.mjs` and commit both files.

const REPO = path.resolve(__dirname, '../../../../..');
const readRepo = (relative: string) => readFileSync(path.join(REPO, relative), 'utf8');

describe('design tokens stay generated from the desktop sources', () => {
  const tokens = buildTokens(REPO);
  const json = renderJson(tokens);

  it('design-tokens.json is what the desktop sources produce today', () => {
    expect(readRepo(JSON_PATH)).toBe(json);
  });

  it('DesignTokens.kt is what design-tokens.json produces today', () => {
    expect(readRepo(KOTLIN_PATH)).toBe(renderKotlin(tokens, json));
  });

  it('DesignTokens.kt names the hash of the JSON it was generated from', () => {
    // The JVM gate compares this constant against the JSON on disk, so the two
    // gates meet on the same digest.
    expect(readRepo(KOTLIN_PATH)).toContain(
      `DESIGN_TOKENS_SHA256 = "${sha256(readRepo(JSON_PATH))}"`,
    );
  });

  it('carries the connect-screen primitives the Android parity test enumerates', () => {
    // Values are read from the component sources, so a desktop move shows up
    // here as a stale JSON rather than as a silent Android divergence.
    expect(tokens.components.connectionCard.radius).toEqual({ value: 16, unit: 'dp' });
    expect(tokens.components.connectionStatus.wellFillAlpha).toEqual({
      value: 0.22,
      unit: 'ratio',
    });
    expect(tokens.components.featureChip.paddingHorizontal).toEqual({ value: 8, unit: 'dp' });
    expect(tokens.components.countryFlag.size).toEqual({ value: 22, unit: 'dp' });
    expect(tokens.components.scenery.washBlend).toBe('soft-light');
    expect(tokens.components.navigation.pushOldTo).toEqual({ value: -0.33, unit: 'ratio' });
  });
});

describe('css colours convert to the Android ARGB literal', () => {
  it('keeps an opaque rgb() opaque', () => {
    expect(cssColorToHex('rgb(247, 247, 248)')).toBe('#FFF7F7F8');
  });

  it('rounds the rgba() alpha onto the 8-bit channel', () => {
    expect(cssColorToHex('rgba(247, 247, 248, 0.2)')).toBe('#33F7F7F8');
    expect(cssColorToHex('rgba(0, 0, 0, 0.5)')).toBe('#80000000');
  });

  it('maps transparent to fully transparent black', () => {
    expect(cssColorToHex('transparent')).toBe('#00000000');
  });

  it('refuses a colour it cannot represent instead of guessing', () => {
    expect(() => cssColorToHex('hsl(0, 0%, 0%)')).toThrow();
  });
});
