import { describe, expect, it } from 'vitest';

import { isDaitaUnavailable } from '../../src/renderer/features/daita/daita-unavailable';
import { FeatureIndicator, TunnelState } from '../../src/shared/daemon-rpc-types';

const connectedWith = (featureIndicators: FeatureIndicator[]) =>
  ({ state: 'connected', details: {}, featureIndicators }) as unknown as TunnelState;

describe('isDaitaUnavailable', () => {
  it('true when connected and the daemon flags DAITA as unavailable', () => {
    expect(isDaitaUnavailable(connectedWith([FeatureIndicator.daitaUnavailable]))).toBe(true);
  });

  it('false when connected with DAITA actually running', () => {
    expect(isDaitaUnavailable(connectedWith([FeatureIndicator.daita]))).toBe(false);
    expect(isDaitaUnavailable(connectedWith([]))).toBe(false);
  });

  it('false before the negotiation settles (connecting) and when disconnected', () => {
    const connecting = {
      state: 'connecting',
      featureIndicators: [FeatureIndicator.daitaUnavailable],
    } as unknown as TunnelState;
    const disconnected = { state: 'disconnected', lockedDown: false } as TunnelState;
    expect(isDaitaUnavailable(connecting)).toBe(false);
    expect(isDaitaUnavailable(disconnected)).toBe(false);
  });
});
