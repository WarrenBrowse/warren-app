import * as grpc from '@grpc/grpc-js';
import { expect, it } from 'vitest';

import { isDaemonAccessRefusal } from '../../src/main/daemon-access';
import userInterfaceActions from '../../src/renderer/redux/userinterface/actions';
import userInterfaceReducer from '../../src/renderer/redux/userinterface/reducers';

function grpcError(code: grpc.status, details: string): Error {
  return Object.assign(new Error(`${code} ${grpc.status[code]}: ${details}`), { code, details });
}

it('recognises the refusal the daemon sends to an account that does not own Warren', () => {
  const refusal = grpcError(
    grpc.status.PERMISSION_DENIED,
    'Warren is set up by another account on this computer',
  );

  expect(isDaemonAccessRefusal(refusal)).toBe(true);
});

it('does not take an unreachable daemon or any other failure for a refusal', () => {
  expect(isDaemonAccessRefusal(grpcError(grpc.status.UNAVAILABLE, 'connection refused'))).toBe(
    false,
  );
  expect(isDaemonAccessRefusal(grpcError(grpc.status.NOT_FOUND, 'no subscription'))).toBe(false);
  expect(isDaemonAccessRefusal(new Error('No connection established to daemon'))).toBe(false);
  expect(isDaemonAccessRefusal(undefined)).toBe(false);
});

it('keeps the refusal in the renderer state until the daemon accepts the account again', () => {
  const refused = userInterfaceReducer(undefined, userInterfaceActions.setDaemonAccessDenied(true));
  expect(refused.daemonAccessDenied).toBe(true);

  const accepted = userInterfaceReducer(refused, userInterfaceActions.setDaemonAccessDenied(false));
  expect(accepted.daemonAccessDenied).toBe(false);
});
