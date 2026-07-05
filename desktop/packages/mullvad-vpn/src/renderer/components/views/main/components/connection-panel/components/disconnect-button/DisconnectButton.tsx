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

  const connected = tunnelState === 'connected';

  // Green only once actually protected; orange while the tunnel is still coming
  // up, so the button colour tracks the same phase as the rest of the screen.
  return (
    <Button variant={connected ? 'success' : 'warning'} onClick={onDisconnect}>
      <Button.Text>
        {connected
          ? messages.gettext('Disconnect')
          : messages.pgettext('tunnel-control', 'Connecting...')}
      </Button.Text>
    </Button>
  );
}
