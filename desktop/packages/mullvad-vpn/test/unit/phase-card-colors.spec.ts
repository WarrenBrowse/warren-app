import { describe, expect, it } from 'vitest';

import { getPhaseCardColors } from '../../src/renderer/lib/connection-phase';
import { surfaces } from '../../src/renderer/lib/foundations/variables/surface-variables';

describe('the connection card writes each phase in its own colours', () => {
  it('writes the exposed state in the salmon of the mockup', () => {
    expect(getPhaseCardColors('exposed')).toEqual({
      title: surfaces.exposed,
      well: surfaces.exposedWell,
    });
  });

  it('writes the protected state in the green of the mockup', () => {
    expect(getPhaseCardColors('protected')).toEqual({
      title: surfaces.protected,
      well: surfaces.protectedWell,
    });
  });

  it('writes an interrupted tunnel like one coming up, since nothing flows in either', () => {
    expect(getPhaseCardColors('interrupted')).toEqual(getPhaseCardColors('connecting'));
    expect(getPhaseCardColors('connecting').title).toBe(surfaces.connecting);
  });

  it('keeps the kill switch neutral: no hue is its signal', () => {
    expect(getPhaseCardColors('blocked')).toEqual({ title: surfaces.text, well: surfaces.button });
  });
});
