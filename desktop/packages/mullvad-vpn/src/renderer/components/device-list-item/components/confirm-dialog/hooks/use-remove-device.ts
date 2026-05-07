import React from 'react';

import { useAppContext } from '../../../../../context';
import { useSelector } from '../../../../../redux/store';

export const useRemoveDevice = () => {
  const { removeDevice: contextRemoveDevice } = useAppContext();
  const pubkey = useSelector((state) => state.account.pubkey)!;
  const removeDevice = React.useCallback(
    async (deviceId: string) => {
      await contextRemoveDevice({ pubkey, deviceId });
    },
    [contextRemoveDevice, pubkey],
  );

  return removeDevice;
};
