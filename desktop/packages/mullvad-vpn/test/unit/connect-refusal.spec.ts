import { status as grpcStatus } from '@grpc/grpc-js';
import { describe, expect, it, vi } from 'vitest';

import { connectHandlingRefusal, connectRefusalOf } from '../../src/main/connect-refusal';
import TunnelStateHandler from '../../src/main/tunnel-state';
import { TunnelState } from '../../src/shared/daemon-rpc-types';

function rpcError(code: grpcStatus) {
  return Object.assign(new Error('refused'), { code });
}

function deps(connect: () => Promise<void>) {
  return {
    connect: vi.fn(connect),
    discardExpectedState: vi.fn(),
    resyncDeviceState: vi.fn(() => Promise.resolve()),
  };
}

describe('connectRefusalOf', () => {
  it('reads UNAUTHENTICATED as no account logged in', () => {
    expect(connectRefusalOf(rpcError(grpcStatus.UNAUTHENTICATED))).toBe('not-logged-in');
  });

  it('reads FAILED_PRECONDITION as the stand-down', () => {
    expect(connectRefusalOf(rpcError(grpcStatus.FAILED_PRECONDITION))).toBe('stood-down');
  });

  it('reads anything else as no refusal', () => {
    expect(connectRefusalOf(rpcError(grpcStatus.UNAVAILABLE))).toBeUndefined();
    expect(connectRefusalOf(new Error('no code'))).toBeUndefined();
  });
});

describe('connectHandlingRefusal', () => {
  it('drops the predicted connecting state and resyncs the account when logged out', async () => {
    const d = deps(() => Promise.reject(rpcError(grpcStatus.UNAUTHENTICATED)));

    await expect(connectHandlingRefusal(d)).resolves.toBeUndefined();

    expect(d.discardExpectedState).toHaveBeenCalledOnce();
    expect(d.resyncDeviceState).toHaveBeenCalledOnce();
  });

  it('drops the predicted state and rethrows any other failure', async () => {
    const error = rpcError(grpcStatus.UNAVAILABLE);
    const d = deps(() => Promise.reject(error));

    await expect(connectHandlingRefusal(d)).rejects.toBe(error);

    expect(d.discardExpectedState).toHaveBeenCalledOnce();
    expect(d.resyncDeviceState).not.toHaveBeenCalled();
  });

  it('leaves the predicted state to the daemon when the connect is accepted', async () => {
    const d = deps(() => Promise.resolve());

    await connectHandlingRefusal(d);

    expect(d.discardExpectedState).not.toHaveBeenCalled();
    expect(d.resyncDeviceState).not.toHaveBeenCalled();
  });
});

describe('TunnelStateHandler.discardExpectedState', () => {
  it('puts the state it predicted back to the one the daemon last reported', () => {
    const published: TunnelState[] = [];
    const handler = new TunnelStateHandler({
      handleTunnelStateUpdate: (state) => published.push(state),
    });

    handler.expectNextTunnelState('connecting');
    expect(handler.tunnelState.state).toBe('connecting');

    handler.discardExpectedState();

    expect(handler.tunnelState.state).toBe('disconnected');
    expect(published.map((state) => state.state)).toEqual(['connecting', 'disconnected']);
  });

  it('does nothing when no state was predicted', () => {
    const published: TunnelState[] = [];
    const handler = new TunnelStateHandler({
      handleTunnelStateUpdate: (state) => published.push(state),
    });

    handler.discardExpectedState();

    expect(published).toEqual([]);
  });
});
