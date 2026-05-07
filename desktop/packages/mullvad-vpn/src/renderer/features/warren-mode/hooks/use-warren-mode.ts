import React from 'react';

import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

// Warren fork — Phase H : hook qui expose le toggle persistant du mode
// tunnel Iroh. Le restart du daemon est requis pour appliquer le
// changement (le mode est lu au boot par `warren_mode::resolve` côté Rust).
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

// Warren fork — Phase H : hook équivalent pour le mode account local.
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
