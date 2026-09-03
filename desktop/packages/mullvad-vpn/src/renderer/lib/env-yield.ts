import { TunnelState, WarrenEnvYield } from '../../shared/daemon-rpc-types';

// Whether the Connect button is pressable, given the tunnel state and the
// stand-down this build holds.
//
// A held stand-down is refused by the daemon with a typed error, so a
// pressable button in that state can only ever answer with a failure the user
// did not cause and cannot read. The banner above carries the explanation and
// the way out; the button just stops lying about what it can do.
export function connectButtonDisabled(
  tunnelState: TunnelState['state'],
  envYield: WarrenEnvYield | null | undefined,
): boolean {
  return tunnelState === 'disconnecting' || (envYield !== null && envYield !== undefined);
}
