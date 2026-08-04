import { FeatureIndicator, TunnelState } from '../../../shared/daemon-rpc-types';

// The daemon computes this indicator from the NEGOTIATED endpoint (the server
// did not grant the defense for this session), so it is only meaningful once
// connected: while connecting the endpoint still echoes the requested setting.
export function isDaitaUnavailable(tunnelState: TunnelState): boolean {
  return (
    tunnelState.state === 'connected' &&
    (tunnelState.featureIndicators?.includes(FeatureIndicator.daitaUnavailable) ?? false)
  );
}
