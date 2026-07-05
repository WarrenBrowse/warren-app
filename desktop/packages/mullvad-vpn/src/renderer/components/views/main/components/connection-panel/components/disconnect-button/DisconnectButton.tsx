import { useCallback } from 'react';

import { messages } from '../../../../../../../../shared/gettext';
import log from '../../../../../../../../shared/logging';
import { useAppContext } from '../../../../../../../context';
import { Button } from '../../../../../../../lib/components';
import { useSelector } from '../../../../../../../redux/store';

export function DisconnectButton() {
  const { disconnectTunnel } = useAppContext();
  const tunnelState = useSelector((state) => state.connection.status.state);

  const onDisconnect = useCallback(async () => {
    try {
      await disconnectTunnel('gui-disconnect-button');
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to disconnect the tunnel: ${error.message}`);
    }
  }, [disconnectTunnel]);

  // This button also renders in the error/blocked state (the kill switch is on),
  // where the action is still "turn it off" = disconnect, not "connecting".
  // Colour tracks the same phase as the rest of the screen: green when up, orange
  // while coming up, neutral when blocked.
  const connecting = tunnelState === 'connecting';
  const connected = tunnelState === 'connected';
  const variant = connected ? 'success' : connecting ? 'warning' : 'primary';

  return (
    <Button variant={variant} onClick={onDisconnect}>
      <Button.Text>
        {connecting
          ? messages.pgettext('tunnel-control', 'Connecting...')
          : messages.gettext('Disconnect')}
      </Button.Text>
    </Button>
  );
}
