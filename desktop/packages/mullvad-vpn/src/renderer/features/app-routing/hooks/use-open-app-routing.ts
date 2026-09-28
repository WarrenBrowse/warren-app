import React from 'react';

import { RoutePath } from '../../../../shared/routes';
import { TransitionType, useHistory } from '../../../lib/history';

export function useOpenAppRouting() {
  const history = useHistory();

  return React.useCallback(() => {
    history.push(RoutePath.splitTunneling, { transition: TransitionType.show });
  }, [history]);
}
