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
