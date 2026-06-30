import React from 'react';

import { WarrenCustomExitSettings } from '../../../../shared/daemon-rpc-types';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

// Hook for the advanced Warren "custom exit" override. Exposes the
// current value plus per-field setters that patch one field and push the
// whole object to the daemon (which reconnects on change). Empty cover
// domain is normalised to `undefined` so it round-trips as "absent"
// (RPK-via-SNI mode) rather than an empty-string X.509 cover.
export function useWarrenCustomExit() {
  const warrenCustomExit = useSelector((state) => state.settings.warrenCustomExit);
  const { setWarrenCustomExit: contextSetWarrenCustomExit } = useAppContext();

  const update = React.useCallback(
    async (next: WarrenCustomExitSettings) => {
      try {
        await contextSetWarrenCustomExit(next);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set Warren custom exit', message);
      }
    },
    [contextSetWarrenCustomExit],
  );

  const setEnabled = React.useCallback(
    (enabled: boolean) => update({ ...warrenCustomExit, enabled }),
    [update, warrenCustomExit],
  );

  const setEndpoint = React.useCallback(
    (endpoint: string) => update({ ...warrenCustomExit, endpoint }),
    [update, warrenCustomExit],
  );

  const setPubkeyHex = React.useCallback(
    (pubkeyHex: string) => update({ ...warrenCustomExit, pubkeyHex }),
    [update, warrenCustomExit],
  );

  const setCoverDomain = React.useCallback(
    (coverDomain: string) =>
      update({ ...warrenCustomExit, coverDomain: coverDomain === '' ? undefined : coverDomain }),
    [update, warrenCustomExit],
  );

  return { warrenCustomExit, setEnabled, setEndpoint, setPubkeyHex, setCoverDomain };
}
