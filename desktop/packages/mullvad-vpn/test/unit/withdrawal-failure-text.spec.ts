import { describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env.platform` at top level.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import { withdrawalFailureText } from '../../src/renderer/components/views/account/WithdrawalCard';
import { urls } from '../../src/shared/constants';

// Only a person can settle two withdrawals with different refund addresses,
// so the refusal has to say where that person is reached.
describe('a withdrawal already recorded with another address', () => {
  it('names the help page to write to', () => {
    expect(withdrawalFailureText('already-filed')).toBe(
      `A withdrawal with another refund address is already recorded: write to us from the help page at ${urls.help}`,
    );
  });
});
