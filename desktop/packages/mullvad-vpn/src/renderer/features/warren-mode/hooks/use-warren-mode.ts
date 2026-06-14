import React from 'react';

import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

// Hook for the persistent warren-api URL. Empty string or undefined →
// unset on the daemon side (= fallback to Mullvad upstream).
export function useWarrenApiUrl() {
  const warrenApiUrl = useSelector((state) => state.settings.warrenApiUrl);
  const { setWarrenApiUrl: contextSetWarrenApiUrl } = useAppContext();

  const setWarrenApiUrl = React.useCallback(
    async (value: string) => {
      try {
        await contextSetWarrenApiUrl(value);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set Warren api URL', message);
      }
    },
    [contextSetWarrenApiUrl],
  );

  return { warrenApiUrl, setWarrenApiUrl };
}
