import React from 'react';

import { exitChoiceNames } from '../../../../shared/app-routing';
import { ExitChoice } from '../../../../shared/daemon-rpc-types';
import { relayLocations as relayLocationsCatalog } from '../../../../shared/gettext';
import { useSelector } from '../../../redux/store';

export function useExitChoiceNames() {
  const locations = useSelector((state) => state.settings.relayLocations);
  // The catalog changes with the app language, which re-renders from the top.
  const translate = React.useCallback((name: string) => relayLocationsCatalog.gettext(name), []);

  return React.useCallback(
    (exit: ExitChoice) => exitChoiceNames(exit, locations, translate),
    [locations, translate],
  );
}
