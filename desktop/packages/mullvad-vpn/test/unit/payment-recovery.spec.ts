import { describe, expect, it } from 'vitest';

import { PaymentRecoveryAction, paymentRecoveryAction } from '../../src/renderer/lib/payment';

// The external checkout opens in the system browser, which cannot
// reach the network while the app firewall blocks: the flow must
// route the user through the unblocking step instead of a dead click.
describe('paymentRecoveryAction', () => {
  it('opens the browser directly when nothing blocks', () => {
    expect(paymentRecoveryAction(false, false)).toBe(PaymentRecoveryAction.openBrowser);
  });

  it('asks to disconnect first when blocked without lockdown (out of time, connecting loop)', () => {
    expect(paymentRecoveryAction(false, true)).toBe(PaymentRecoveryAction.disconnect);
  });

  it('requires disabling lockdown mode when blocked with lockdown on', () => {
    expect(paymentRecoveryAction(true, true)).toBe(PaymentRecoveryAction.disableLockdownMode);
  });

  it('opens the browser directly when lockdown is on but nothing is blocked (tunnel up)', () => {
    expect(paymentRecoveryAction(true, false)).toBe(PaymentRecoveryAction.openBrowser);
  });
});
