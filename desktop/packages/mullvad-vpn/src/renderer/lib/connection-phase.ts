import sceneryManifest from '../../../assets/images/scenery/scenery.json';
import { TunnelState } from '../../shared/daemon-rpc-types';
import { Colors, colors, surfaces, SurfaceToken } from './foundations';

// The connect screen collapses the daemon tunnel states into four visual phases,
// each with its own accent colour. This is the single source of truth so the
// backdrop wash, the eye icon, the status label and the action button never
// drift apart.
//   exposed     : leaking, traffic in the clear (red)
//   connecting  : tunnel coming up or down (orange)
//   protected   : tunnel up (green)
//   interrupted : tunnel nominally up but the host is offline (orange)
//   blocked     : kill switch active, nothing leaks but nothing flows (neutral)
export type ConnectionPhase = 'exposed' | 'connecting' | 'protected' | 'interrupted' | 'blocked';

export function getConnectionPhase(
  tunnelState: TunnelState,
  hostOffline = false,
  exitEgressDead = false,
): ConnectionPhase {
  switch (tunnelState.state) {
    case 'connected':
      // The daemon holds Connected through its offline migration grace
      // window and the supervisor redials transparently, so "connected
      // while the host is offline" is a real, user-visible window. A
      // green "protected" there is a lie: nothing flows. Same for an
      // exit that stopped forwarding (egress probe verdict): the QUIC
      // session looks alive but no traffic gets through. Only this
      // state degrades; every other state's presentation already tells
      // the truth on its own.
      return hostOffline || exitEgressDead ? 'interrupted' : 'protected';
    case 'connecting':
    case 'disconnecting':
      return 'connecting';
    case 'disconnected':
      // Locked down = kill switch holding traffic, not raw exposure.
      return tunnelState.lockedDown ? 'blocked' : 'exposed';
    case 'error':
      // blockingError = the daemon failed to install the block, so traffic may
      // be leaking (exposed). Without it the kill switch holds: secured but
      // offline (blocked). An error is never "connected".
      return tunnelState.details.blockingError ? 'exposed' : 'blocked';
  }
}

export function getPhaseAccentColor(phase: ConnectionPhase): string {
  return colors[getPhaseAccentColorName(phase)];
}

// Same accent as a colour-token name, for APIs (like <Icon color>) that take a
// token key rather than a resolved value.
export function getPhaseAccentColorName(phase: ConnectionPhase): Colors {
  return sceneryManifest.phases[phase].accent as Colors;
}

// The card is its own surface, light or dark with the theme, so it writes the
// phase in the surface palette rather than in the scenery accents above. The
// title carries the hue and the well behind the eye is a quiet fill of it.
const phaseCardColors: Record<ConnectionPhase, { title: SurfaceToken; well: SurfaceToken }> = {
  exposed: { title: 'exposed', well: 'exposedWell' },
  connecting: { title: 'connecting', well: 'connectingWell' },
  protected: { title: 'protected', well: 'protectedWell' },
  interrupted: { title: 'connecting', well: 'connectingWell' },
  blocked: { title: 'text', well: 'button' },
};

export function getPhaseCardColors(phase: ConnectionPhase): { title: string; well: string } {
  const { title, well } = phaseCardColors[phase];
  return { title: surfaces[title], well: surfaces[well] };
}
