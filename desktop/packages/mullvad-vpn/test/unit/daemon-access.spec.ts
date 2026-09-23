import * as grpc from '@grpc/grpc-js';
import { expect, it } from 'vitest';

import { daemonAccessRefusal } from '../../src/main/daemon-access';
import userInterfaceActions from '../../src/renderer/redux/userinterface/actions';
import userInterfaceReducer from '../../src/renderer/redux/userinterface/reducers';

function grpcError(code: grpc.status, details: string, reason?: string): Error {
  const metadata = new grpc.Metadata();
  if (reason !== undefined) {
    metadata.set('grpc-status-details-bin', Buffer.from(reason));
  }
  return Object.assign(new Error(`${code} ${grpc.status[code]}: ${details}`), {
    code,
    details,
    metadata,
  });
}

it('names the refusal of an account that is not the owner', () => {
  const refusal = grpcError(
    grpc.status.PERMISSION_DENIED,
    'Warren is set up by another account on this computer',
    'owned_by_another_account',
  );

  expect(daemonAccessRefusal(refusal)).toBe('ownedByAnotherAccount');
});

it('names the refusal of an account that is not at the screen of an unowned setup', () => {
  const refusal = grpcError(
    grpc.status.PERMISSION_DENIED,
    'Warren is set up on this computer and has no owner yet',
    'claim_needs_console_user',
  );

  expect(daemonAccessRefusal(refusal)).toBe('claimNeedsConsoleUser');
});

it('does not take an unreachable daemon or any other failure for a refusal', () => {
  expect(daemonAccessRefusal(grpcError(grpc.status.UNAVAILABLE, 'connection refused'))).toBeNull();
  expect(daemonAccessRefusal(grpcError(grpc.status.NOT_FOUND, 'no subscription'))).toBeNull();
  expect(daemonAccessRefusal(new Error('No connection established to daemon'))).toBeNull();
  expect(daemonAccessRefusal(undefined)).toBeNull();
});

it('keeps the refusal in the renderer state until the daemon accepts the account again', () => {
  const refused = userInterfaceReducer(
    undefined,
    userInterfaceActions.setDaemonAccessRefusal('claimNeedsConsoleUser'),
  );
  expect(refused.daemonAccessRefusal).toBe('claimNeedsConsoleUser');

  const accepted = userInterfaceReducer(refused, userInterfaceActions.setDaemonAccessRefusal(null));
  expect(accepted.daemonAccessRefusal).toBeNull();
});
