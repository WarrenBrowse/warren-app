import { useEffect } from 'react';

import { useUserInterfaceIsMacOs13OrNewer } from '../../../../../../redux/hooks';
import { useSelector } from '../../../../../../redux/store';
import { useFetchNeedFullDiskPermissions } from './use-fetch-need-full-disk-permissions';

// Asks the daemon whether Full Disk Access is missing, only where the answer
// matters: on a macOS build and version that can run the classifier at all.
export function useFullDiskAccessCheck() {
  const supported = useSelector((state) => state.settings.splitTunnelingSupported);
  const { isMacOs13OrNewer } = useUserInterfaceIsMacOs13OrNewer();
  const fetchNeedFullDiskPermissions = useFetchNeedFullDiskPermissions();

  useEffect(() => {
    if (window.env.platform === 'darwin' && supported && isMacOs13OrNewer) {
      void fetchNeedFullDiskPermissions();
    }
  }, [fetchNeedFullDiskPermissions, isMacOs13OrNewer, supported]);
}
