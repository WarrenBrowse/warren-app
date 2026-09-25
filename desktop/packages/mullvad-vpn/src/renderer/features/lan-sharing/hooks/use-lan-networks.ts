import React from 'react';

import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

export function useLanNetworks() {
  const lanNetworks = useSelector((state) => state.settings.lanNetworks);
  const { setLanNetworks: contextSetLanNetworks } = useAppContext();

  // `undefined` resets the list to the built-in private ranges. Rejects when the daemon refuses
  // the list, so the caller can tell the user.
  const setLanNetworks = React.useCallback(
    (networks?: string[]) => contextSetLanNetworks(networks),
    [contextSetLanNetworks],
  );

  return { lanNetworks, setLanNetworks };
}
