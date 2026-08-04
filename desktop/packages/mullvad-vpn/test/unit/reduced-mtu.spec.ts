import { describe, expect, it } from 'vitest';

import { reducedMtuIndicatorLabel } from '../../src/renderer/features/reduced-mtu/reduced-mtu';

describe('reducedMtuIndicatorLabel', () => {
  it('includes the measured size when one was sampled', () => {
    expect(reducedMtuIndicatorLabel(1184)).toContain('1184');
  });

  it('falls back to a plain warning without a measured size', () => {
    const label = reducedMtuIndicatorLabel(undefined);
    expect(label).not.toMatch(/\d/);
    expect(label.length).toBeGreaterThan(0);
  });

  it('treats a zero measurement as no value rather than "(0)"', () => {
    expect(reducedMtuIndicatorLabel(0)).not.toContain('0');
  });
});
