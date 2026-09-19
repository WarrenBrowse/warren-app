import React from 'react';

import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

/** Toast when a forwarded public port opens, moves or closes. Absent in a
 * settings file written before the setting existed, and on there: a user who
 * opened a port wants to be told when it moves. */
export function usePortForwardingNotifications() {
  const portForwardingNotifications = useSelector(
    (state) => state.settings.guiSettings.portForwardingNotifications ?? true,
  );

  const { setPortForwardingNotifications: contextSetPortForwardingNotifications } = useAppContext();

  const setPortForwardingNotifications = React.useCallback(
    (value: boolean) => {
      try {
        contextSetPortForwardingNotifications(value);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set port forwarding notifications', message);
      }
    },
    [contextSetPortForwardingNotifications],
  );

  return { portForwardingNotifications, setPortForwardingNotifications };
}
