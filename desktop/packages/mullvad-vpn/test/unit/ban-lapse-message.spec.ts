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

describe('the revocation message', () => {
  it('names the day a port-forwarding ban lapses', () => {
    expect(
      message(AuthFailedError.bannedPortForwarding, {
        reason: 'port-forwarding-abuse',
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: LAPSE,
      }),
    ).to.equal(
      `Blocking internet: your access has been revoked until September 24, 2027 after repeated abuse reports about a forwarded port. You can contest this at ${urls.reports}`,
    );
  });

  it('names the day any other ban lapses', () => {
    expect(
      message(AuthFailedError.banned, {
        reason: 'other',
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: LAPSE,
      }),
    ).to.contain('revoked until September 24, 2027 for a usage policy violation');
  });

  // "Contact support" named no channel, and the forum is closed to a wallet
  // that never paid: the help page serves everyone.
  it('says where to write about any other ban, with or without a lapse day', () => {
    expect(
      message(AuthFailedError.banned, {
        reason: 'other',
        bannedAtUnixSecs: null,
        lapsesAtUnixSecs: LAPSE,
      }),
    ).to.equal(
      `Blocking internet: your access has been revoked until September 24, 2027 for a usage policy violation. If you believe this is a mistake, write to us from the help page at ${urls.help}`,
    );
    expect(message(AuthFailedError.banned, null)).to.equal(
      `Blocking internet: your access has been revoked for a usage policy violation. If you believe this is a mistake, write to us from the help page at ${urls.help}`,
    );
  });

  it('names no day when the lapse is unknown', () => {
    expect(message(AuthFailedError.bannedPortForwarding, null)).to.equal(
      `Blocking internet: your access has been revoked after repeated abuse reports about a forwarded port. You can contest this at ${urls.reports}`,
    );
  });
});
