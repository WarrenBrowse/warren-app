import { useSplitTunnelingSupported } from '../../../../features/split-tunneling/hooks';
import { useUserInterfaceIsMacOs13OrNewer } from '../../../../redux/hooks';

export const useShowSplitTunneling = () => {
  const { isMacOs13OrNewer } = useUserInterfaceIsMacOs13OrNewer();
  const { splitTunnelingSupported } = useSplitTunnelingSupported();
  // Hide the entry when the daemon/platform cannot do split tunneling
  // at all. On macOS this covers unsigned builds (Endpoint Security is
  // refused without a Developer ID + entitlement), so we never present
  // a panel the daemon would reject. macOS also still requires v13+.
  const platformShows = window.env.platform !== 'darwin' || isMacOs13OrNewer;
  return platformShows && splitTunnelingSupported;
};
