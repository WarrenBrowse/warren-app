import { useCallback } from 'react';

import { messages } from '../../../../../../../../shared/gettext';
import log from '../../../../../../../../shared/logging';
import { useAppContext } from '../../../../../../../context';
import { useSelector } from '../../../../../../../redux/store';
import { CardButton } from '../card-button';

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

  // Red "stop": disconnecting drops the user back to the exposed state, so the
  // button signals its action, not the state. Orange (Cancel) while coming up,
  // neutral in the blocked/error state where the action is "turn off the switch".
  const connecting = tunnelState === 'connecting';
  const connected = tunnelState === 'connected';
  const tone = connected ? 'disconnect' : connecting ? 'cancel' : 'neutral';

  // While connecting the click aborts the attempt, so the button reads "Cancel".
  return (
    <CardButton $tone={tone} onClick={onDisconnect}>
      <CardButton.Text>
        {connecting ? messages.gettext('Cancel') : messages.gettext('Disconnect')}
      </CardButton.Text>
    </CardButton>
  );
}
