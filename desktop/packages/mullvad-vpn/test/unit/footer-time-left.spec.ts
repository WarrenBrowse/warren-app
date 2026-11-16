import { describe, expect, it } from 'vitest';

import { footerTimeLeft } from '../../src/renderer/components/app-main-header/components/footer-time-left';

const inDays = (days: number) => new Date(Date.now() + days * 86_400_000 + 3_600_000).toISOString();

// The footer's end (mockups 8c and 8d): "14 days left" alone, or "14 days"
// after the beta cap, where "left" would make the line run long.
describe('footerTimeLeft', () => {
  it('says how long is left on its own', () => {
    expect(footerTimeLeft(inDays(14), false)).toBe('14 days left');
  });

  it('gives the bare duration after the beta cap', () => {
    expect(footerTimeLeft(inDays(14), true)).toBe('14 days');
  });

  it('says nothing without an expiry', () => {
    expect(footerTimeLeft(undefined, true)).toBe('');
  });
});
