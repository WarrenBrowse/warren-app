import React from 'react';

import { AppRoutingTab } from '../../../../shared/ipc-types';
import { RoutePath } from '../../../../shared/routes';
import { TransitionType, useHistory } from '../../../lib/history';

export function useOpenAppRouting() {
  const history = useHistory();

  return React.useCallback(
    (tab: AppRoutingTab) => {
      history.push(RoutePath.splitTunneling, {
        transition: TransitionType.show,
        options: [{ type: 'app-routing-tab', tab }],
      });
    },
    [history],
  );
}
