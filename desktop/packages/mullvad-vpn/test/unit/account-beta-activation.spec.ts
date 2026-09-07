import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import Account from '../../src/main/account';
import type { DaemonRpc } from '../../src/main/daemon-rpc';
import {
  AccountDataResponse,
  DeviceEvent,
  TunnelState,
  VoucherResponse,
} from '../../src/shared/daemon-rpc-types';

const pubkey = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
const activatedExpiry = '2099-10-01T00:00:00.000Z';
const disconnected: TunnelState = { state: 'disconnected', lockedDown: false };

function makeDaemon(overrides: Partial<Record<string, unknown>> = {}) {
  const noSubscription: AccountDataResponse = { type: 'error', error: 'no-subscription' };
  const activated: VoucherResponse = {
    type: 'success',
    newExpiry: activatedExpiry,
    secondsAdded: 3600,
  };
  return {
    isConnected: true,
    createNewAccount: vi.fn().mockResolvedValue(pubkey),
    submitVoucher: vi.fn().mockResolvedValue(activated),
    getAccountData: vi.fn().mockResolvedValue(noSubscription),
    getAccountHistory: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function makeDelegate(tunnelState: TunnelState = disconnected) {
  return {
    onDeviceEvent: vi.fn(),
    onAccountData: vi.fn(),
    getTunnelState: vi.fn().mockReturnValue(tunnelState),
    getLocale: vi.fn().mockReturnValue('en'),
    notify: vi.fn(),
    closeNotificationsInCategory: vi.fn(),
  };
}

const loggedIn: DeviceEvent = {
  deviceState: { type: 'logged in', warrenIdentity: { pubkey } },
} as unknown as DeviceEvent;

// The beta access used to be requested only at step 3 of the onboarding
// wizard. A first run whose window hid before that step left the wallet
// registered nowhere and every screen saying "out of time" (topic 195).
// Registering at creation is idempotent server side, so the wizard step
// becomes a confirmation rather than the only chance.
describe('Account.createNewAccount on a beta build', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('asks the daemon for the beta auto-voucher right after the wallet is minted', async () => {
    const daemon = makeDaemon();
    const account = new Account(makeDelegate(), daemon as unknown as DaemonRpc, {
      betaBuild: true,
    });
    account.handleDeviceEvent(loggedIn);

    await expect(account.createNewAccount()).resolves.toBe(pubkey);
    await vi.waitFor(() => expect(daemon.submitVoucher).toHaveBeenCalledWith(''));
    await vi.waitFor(() => expect(account.accountData?.expiry).toBe(activatedExpiry));
  });

  it('leaves a prod build alone: no voucher call, the checkout owns activation', async () => {
    const daemon = makeDaemon();
    const account = new Account(makeDelegate(), daemon as unknown as DaemonRpc, {
      betaBuild: false,
    });
    account.handleDeviceEvent(loggedIn);

    await account.createNewAccount();
    await vi.advanceTimersByTimeAsync(50);
    expect(daemon.submitVoucher).not.toHaveBeenCalled();
  });

  it('still returns the wallet when the activation fails, the wizard retries it', async () => {
    const daemon = makeDaemon({
      submitVoucher: vi.fn().mockRejectedValue(new Error('offline')),
    });
    const account = new Account(makeDelegate(), daemon as unknown as DaemonRpc, {
      betaBuild: true,
    });
    account.handleDeviceEvent(loggedIn);

    await expect(account.createNewAccount()).resolves.toBe(pubkey);
    await vi.advanceTimersByTimeAsync(50);
    expect(daemon.submitVoucher).toHaveBeenCalledTimes(1);
  });
});

// An expired expiry with a tunnel that is not "disconnected" (blocked, or
// connecting) took the scheduling branch meant for a live subscription:
// `expiry - now - 3 days` is negative, `setTimeout` clamps it to 1 ms, and
// the main process re-ran the same evaluation about a thousand times a
// second until the state changed. Suppressing the toast on a
// never-activated beta wallet would have widened that spin to every fresh
// install, so the expired case now settles instead of scheduling.
describe('Account expiry scheduling on an expired account', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('does not spin when the expiry is past and the toast is not shown', async () => {
    const blocked: TunnelState = {
      state: 'error',
      details: { cause: 'is_offline' },
    } as unknown as TunnelState;
    const delegate = makeDelegate(blocked);
    const daemon = makeDaemon();
    const account = new Account(delegate, daemon as unknown as DaemonRpc, { betaBuild: true });
    account.handleDeviceEvent(loggedIn);

    await vi.advanceTimersByTimeAsync(5_000);

    expect(delegate.notify).not.toHaveBeenCalled();
    expect(delegate.closeNotificationsInCategory.mock.calls.length).toBeLessThan(10);
  });
});
