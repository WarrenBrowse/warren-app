import { describe, expect, it } from 'vitest';

import { urls } from '../../src/shared/constants';
import {
  AuthFailedError,
  ErrorStateCause,
  TunnelState,
  WarrenAccountBan,
} from '../../src/shared/daemon-rpc-types';
import { ErrorNotificationProvider } from '../../src/shared/notifications';

function bannedState(authFailedError: AuthFailedError): TunnelState {
  return {
    state: 'error',
    details: { cause: ErrorStateCause.authFailed, authFailedError },
  } as TunnelState;
}

function message(authFailedError: AuthFailedError, ban: WarrenAccountBan | null): string {
  return new ErrorNotificationProvider({
    tunnelState: bannedState(authFailedError),
    hasExcludedApps: false,
    splitTunnelingSupported: true,
    ban,
    locale: 'en',
  }).getSystemNotification()!.message;
}

/** 2027-09-24T00:00:00Z. */
const LAPSE = 1_821_744_000;

describe('the suspension message', () => {
  it('names the day a port-forwarding ban lapses', () => {
    expect(
      message(AuthFailedError.bannedPortForwarding, {
        reason: 'port-forwarding-abuse',
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: LAPSE,
      }),
    ).to.equal(
      `Blocking internet: your access has been suspended until September 24, 2027 after repeated abuse reports about a forwarded port. You can contest this at ${urls.reports}`,
    );
  });

  it('names the day any other ban lapses', () => {
    expect(
      message(AuthFailedError.banned, {
        reason: 'other',
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: LAPSE,
      }),
    ).to.contain('suspended until September 24, 2027 for a usage policy violation');
  });

  it('names no day when the lapse is unknown', () => {
    expect(message(AuthFailedError.bannedPortForwarding, null)).to.equal(
      `Blocking internet: your access has been suspended after repeated abuse reports about a forwarded port. You can contest this at ${urls.reports}`,
    );
  });
});
