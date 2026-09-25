import { useCallback } from 'react';

import { type ILinuxSplitTunnelingApplication } from '../../../../../../../shared/application-types';
import { useAppContext } from '../../../../../../context';
import { useLinuxSettingsContext } from '../LinuxSettingsContext';

export function useLaunchApplication() {
  const { launchExcludedApplication, launchIncludedApplication } = useAppContext();
  const { launchMode, setBrowseError } = useLinuxSettingsContext();

  const launchApplication = useCallback(
    async (application: ILinuxSplitTunnelingApplication | string) => {
      const launch =
        launchMode === 'include' ? launchIncludedApplication : launchExcludedApplication;
      const result = await launch(application);
      if ('error' in result) {
        setBrowseError(result.error);
      }
    },
    [launchExcludedApplication, launchIncludedApplication, launchMode, setBrowseError],
  );

  return launchApplication;
}
