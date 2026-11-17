import * as grpcTypes from 'management-interface/management-interface/grpc-types';
import { describe, expect, it } from 'vitest';

import { convertFromTunnelState } from '../../src/main/grpc-type-convertions';
import { AuthFailedError, ErrorStateCause } from '../../src/shared/daemon-rpc-types';

function blockedFor(error: grpcTypes.ErrorState.AuthFailedError): grpcTypes.TunnelState {
  const errorState = new grpcTypes.ErrorState();
  errorState.setCause(grpcTypes.ErrorState.Cause.AUTH_FAILED);
  errorState.setAuthFailedError(error);
  const blocked = new grpcTypes.TunnelState.Error();
  blocked.setErrorState(errorState);
  const tunnelState = new grpcTypes.TunnelState();
  tunnelState.setError(blocked);
  return tunnelState;
}

// The daemon blocks with this cause once an active subscription found no
// session token the exit admits: the screens offer the account's devices on it.
describe('a block for a session with no token', () => {
  it('reaches the renderer as its own cause', () => {
    const state = convertFromTunnelState(
      blockedFor(grpcTypes.ErrorState.AuthFailedError.NO_SESSION_TOKEN),
    );

    expect(AuthFailedError.noSessionToken).to.be.a('number');
    expect(AuthFailedError.noSessionToken).to.not.equal(AuthFailedError.unknown);
    expect(state).to.deep.equal({
      state: 'error',
      details: {
        blockingError: undefined,
        cause: ErrorStateCause.authFailed,
        authFailedError: AuthFailedError.noSessionToken,
      },
    });
  });
});

// The daemon blocks with this cause once two exits refused this device's
// time: the banner asks for automatic date and time.
describe('a block for a refused device clock', () => {
  it('reaches the renderer as its own cause', () => {
    const state = convertFromTunnelState(
      blockedFor(grpcTypes.ErrorState.AuthFailedError.CLOCK_SKEW),
    );

    expect(AuthFailedError.clockSkew).to.be.a('number');
    expect(AuthFailedError.clockSkew).to.not.equal(AuthFailedError.unknown);
    expect(state).to.deep.equal({
      state: 'error',
      details: {
        blockingError: undefined,
        cause: ErrorStateCause.authFailed,
        authFailedError: AuthFailedError.clockSkew,
      },
    });
  });
});
