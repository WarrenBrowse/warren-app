import { splitModeAvailability } from '../../../../../../../shared/app-routing';
import { useUserInterfaceIsMacOs13OrNewer } from '../../../../../../redux/hooks';
import { useSelector } from '../../../../../../redux/store';
import { useSplitTunnelingSettingsContext } from '../SplitTunnelingSettingsContext';

export function useSplitModeAvailability() {
  const supported = useSelector((state) => state.settings.splitTunnelingSupported);
  const { isMacOs13OrNewer } = useUserInterfaceIsMacOs13OrNewer();
  const { loadingDiskPermissions, splitTunnelingAvailable } = useSplitTunnelingSettingsContext();
  const platform = window.env.platform;

  const needsFullDiskAccess =
    platform !== 'darwin'
      ? false
      : loadingDiskPermissions || splitTunnelingAvailable === undefined
        ? undefined
        : !splitTunnelingAvailable;

  return splitModeAvailability({ platform, supported, needsFullDiskAccess, isMacOs13OrNewer });
}
