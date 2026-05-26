import React from 'react';

import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

// Hook that exposes the persistent toggle for the Iroh tunnel mode.
// A daemon restart is required to apply the change (the mode is read
// at boot by `warren_mode::resolve` on the Rust side).
export function useWarrenMode() {
  const warrenMode = useSelector((state) => state.settings.warrenMode);
  const { setWarrenMode: contextSetWarrenMode } = useAppContext();

  const setWarrenMode = React.useCallback(
    async (value: boolean) => {
      try {
        await contextSetWarrenMode(value);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set Warren tunnel mode', message);
      }
    },
    [contextSetWarrenMode],
  );

  return { warrenMode, setWarrenMode };
}

// Equivalent hook for the local account mode.
export function useWarrenLocalAccount() {
  const warrenLocalAccount = useSelector((state) => state.settings.warrenLocalAccount);
  const { setWarrenLocalAccount: contextSetWarrenLocalAccount } = useAppContext();

  const setWarrenLocalAccount = React.useCallback(
    async (value: boolean) => {
      try {
        await contextSetWarrenLocalAccount(value);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set Warren local account mode', message);
      }
    },
    [contextSetWarrenLocalAccount],
  );

  return { warrenLocalAccount, setWarrenLocalAccount };
}

// Hook for the multi-exit auto-failover toggle (M5.B.2). GUI-only:
// the daemon handles failover unconditionally; this flag controls
// whether the UI shows the toggle as ON and whether the failover
// notification toast is displayed. No daemon restart needed.
export function useWarrenFailover() {
  const warrenFailover = useSelector((state) => state.settings.warrenFailover);
  const { setWarrenFailover: contextSetWarrenFailover } = useAppContext();

  const setWarrenFailover = React.useCallback(
    async (value: boolean) => {
      try {
        await contextSetWarrenFailover({ enabled: value });
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set Warren failover', message);
      }
    },
    [contextSetWarrenFailover],
  );

  return { warrenFailover: warrenFailover.enabled, setWarrenFailover };
}

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
