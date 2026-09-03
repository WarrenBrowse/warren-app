import { readdirSync, readFileSync } from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

import {
  buildFlagOutputs,
  circlePath,
  DRAWABLE_DIR,
  expandHex,
  flagCodes,
  rectPath,
  svgToPaths,
  svgToVectorDrawable,
} from '../../../../../scripts/flags/lib.mjs';

// The connection card shows the same round flag on desktop and Android. The
// Android drawables are generated from the desktop SVG set, so the two trees
// are pinned against each other here rather than by discipline, like the
// scenery layers. Regenerate with `node scripts/flags/gen.mjs`.

const REPO = path.resolve(__dirname, '../../../../..');

describe('the Android flag set stays generated from the desktop set', () => {
  const outputs = buildFlagOutputs(REPO);

  it('every desktop flag has its Android drawable and lookup entry, byte for byte', () => {
    for (const [relative, expected] of Object.entries(outputs)) {
      expect(readFileSync(path.join(REPO, relative), 'utf8'), relative).toBe(expected);
    }
  });

  it('no Android flag drawable outlives its desktop source', () => {
    const drawables = readdirSync(path.join(REPO, DRAWABLE_DIR))
      .filter((name) => /^flag_[a-z]{2}\.xml$/.test(name))
      .map((name) => name.slice('flag_'.length, -'.xml'.length))
      .sort();
    expect(drawables).toEqual(flagCodes(REPO));
  });
});

describe('the SVG to vector-drawable conversion', () => {
  const head =
    '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">' +
    '<mask id="a"><circle cx="256" cy="256" r="256" fill="#fff"/></mask><g mask="url(#a)">';
  const tail = '</g></svg>';

  it('resolves a fill inherited from an enclosing group', () => {
    const svg = `${head}<g fill="#eee"><path d="M0 0h1z"/></g>${tail}`;
    expect(svgToPaths(svg)).toEqual([{ fill: '#EEEEEE', d: 'M0 0h1z' }]);
  });

  it('turns circles, ellipses and rounded rects into path data', () => {
    const svg = `${head}<circle cx="256" cy="256" r="10" fill="#d80027"/><rect width="32" height="16" x="120" y="208" fill="#ff9811" ry="8"/>${tail}`;
    const paths = svgToPaths(svg);
    expect(paths[0]).toEqual({ fill: '#D80027', d: circlePath(256, 256, 10) });
    // A rect with only ry takes rx from it, so the corners stay round.
    expect(paths[1]).toEqual({ fill: '#FF9811', d: rectPath(120, 208, 32, 16, undefined, 8) });
    expect(paths[1].d).toContain('a8 8 0 0 1');
  });

  it('drops a fill="none" shape, which paints nothing in the source either', () => {
    const svg = `${head}<path fill="none" d="M0 0h1z"/><path fill="#333" d="M1 1h1z"/>${tail}`;
    expect(svgToPaths(svg)).toEqual([{ fill: '#333333', d: 'M1 1h1z' }]);
  });

  it('refuses an SVG that is not cut with the circle mask', () => {
    expect(() => svgToPaths('<svg viewBox="0 0 10 10"><path d="M0 0h1z"/></svg>')).toThrow();
  });

  it('cuts the drawable with the circle the SVG mask described', () => {
    const drawable = svgToVectorDrawable(
      `${head}<path fill="#eee" d="M0 0h512v512H0z"/>${tail}`,
      'zz',
    );
    expect(drawable).toContain(
      '<clip-path android:pathData="M256 0A256 256 0 1 1 256 512A256 256 0 1 1 256 0Z" />',
    );
    expect(drawable).toContain('android:viewportWidth="512"');
  });

  it('expands the short hex fills the set uses', () => {
    expect(expandHex('#eee')).toBe('#EEEEEE');
    expect(expandHex('#026')).toBe('#002266');
    expect(expandHex('#0052b4')).toBe('#0052B4');
  });
});
