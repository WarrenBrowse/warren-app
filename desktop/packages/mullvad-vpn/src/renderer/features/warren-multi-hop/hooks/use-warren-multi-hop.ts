import React from 'react';

import { WarrenMultiHopSettings } from '../../../../shared/daemon-rpc-types';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

// Hook exposing the persisted Warren two-relayed QUIC multi-hop
// settings (M4.E.D). A daemon restart is required for a change to
// take effect (the supervisor is wired at boot from the
// settings-file path + WARREN_MULTI_HOP env var). Default values
// follow `warren_multihop_doctrine_v1`: OFF, no preferred countries,
// 4h HPKE epoch rotation.
export function useWarrenMultiHop() {
  const warrenMultiHop = useSelector((state) => state.settings.warrenMultiHop);
  const { setWarrenMultiHop: contextSetWarrenMultiHop } = useAppContext();

  const setWarrenMultiHop = React.useCallback(
    async (next: WarrenMultiHopSettings) => {
      try {
        await contextSetWarrenMultiHop(next);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set Warren multi-hop settings', message);
      }
    },
    [contextSetWarrenMultiHop],
  );

  const setEnabled = React.useCallback(
    (enabled: boolean) => setWarrenMultiHop({ ...warrenMultiHop, enabled }),
    [setWarrenMultiHop, warrenMultiHop],
  );

  const setEntryCountry = React.useCallback(
    (entryCountry: string) => setWarrenMultiHop({ ...warrenMultiHop, entryCountry }),
    [setWarrenMultiHop, warrenMultiHop],
  );

  const setExitCountry = React.useCallback(
    (exitCountry: string) => setWarrenMultiHop({ ...warrenMultiHop, exitCountry }),
    [setWarrenMultiHop, warrenMultiHop],
  );

  return {
    warrenMultiHop,
    setWarrenMultiHop,
    setEnabled,
    setEntryCountry,
    setExitCountry,
  };
}
