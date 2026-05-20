import React from 'react';

import { NatPmpProto, NatPmpSettings } from '../../../../shared/daemon-rpc-types';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

/**
 * Hook exposing the NAT-PMP port-forwarding settings + live status to
 * the port-forwarding view + its sub-components.
 *
 * Reads:
 * - `settings`: the persisted user setting (toggle + lifetime + ...).
 * - `status`: the live refresh-loop status pushed by the daemon
 *   (`{ state: 'disabled' | 'requesting' | 'mapped' | 'failed' }`).
 *
 * Writes:
 * - `setEnabled(boolean)`: flip the toggle and push the whole struct
 *   back to the daemon. The daemon picks up the change on the next
 *   tunnel reconnect; the live `status` cache snaps to `disabled` on
 *   a turn-off so the UI hides the public-port row immediately.
 * - `setLifetimeSecs(number)`: change the requested lifetime. Same
 *   pickup semantics as `setEnabled`.
 */
export function usePortForwarding() {
  const settings = useSelector((state) => state.settings.warrenNatPmp);
  const status = useSelector((state) => state.settings.natPmpStatus) ?? {
    state: 'disabled' as const,
  };
  const { setNatPmpSettings } = useAppContext();

  const pushUpdate = React.useCallback(
    async (patch: Partial<NatPmpSettings>) => {
      try {
        await setNatPmpSettings({ ...settings, ...patch });
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set NAT-PMP settings', message);
      }
    },
    [settings, setNatPmpSettings],
  );

  const setEnabled = React.useCallback(
    async (enabled: boolean) => {
      await pushUpdate({ enabled });
    },
    [pushUpdate],
  );

  const setLifetimeSecs = React.useCallback(
    async (lifetimeSecs: number) => {
      await pushUpdate({ lifetimeSecs });
    },
    [pushUpdate],
  );

  const setProtocol = React.useCallback(
    async (protocol: NatPmpProto) => {
      await pushUpdate({ protocol });
    },
    [pushUpdate],
  );

  return {
    settings,
    status,
    setEnabled,
    setLifetimeSecs,
    setProtocol,
  };
}
