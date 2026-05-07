import React from 'react';

import { useAppContext } from '../../../../context';
import { useSelector } from '../../../../redux/store';

export const useFetchDevices = () => {
  const { fetchDevices: contextFetchDevices } = useAppContext();
  const pubkey = useSelector((state) => state.account.pubkey)!;

  return React.useCallback(() => {
    return contextFetchDevices(pubkey);
  }, [pubkey, contextFetchDevices]);
};
