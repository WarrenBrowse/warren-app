import { describe, expect, it } from 'vitest';

import { degradedBondIndicatorLabel } from '../../src/renderer/features/degraded-bond/degraded-bond';

describe('degradedBondIndicatorLabel', () => {
  it('names how many legs stalled out of how many are bonded', () => {
    const label = degradedBondIndicatorLabel(1, 8);
    expect(label).toContain('1');
    expect(label).toContain('8');
  });

  it('falls back to a plain warning when the daemon sent no bundle width', () => {
    const label = degradedBondIndicatorLabel(1, undefined);
    expect(label).not.toMatch(/\d/);
    expect(label.length).toBeGreaterThan(0);
  });

  it('falls back to a plain warning when no leg is reported stalled', () => {
    expect(degradedBondIndicatorLabel(0, 8)).not.toMatch(/\d/);
  });
});
