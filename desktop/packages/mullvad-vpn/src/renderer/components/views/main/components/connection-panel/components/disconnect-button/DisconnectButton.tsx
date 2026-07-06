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
  // Colour follows the ACTION, not the state (the protected state is already
  // signalled in green by the status card): disconnecting drops the user back to
  // the exposed state, so the button wears the "stop" red to give pause before
  // exposing themselves. Orange while coming up (Cancel), neutral when blocked.
  const connecting = tunnelState === 'connecting';
  const connected = tunnelState === 'connected';
  const variant = connected ? 'destructive' : connecting ? 'warning' : 'primary';

  // While connecting the click aborts the attempt, so the button reads "Cancel".
  // The connecting progress itself is conveyed by the status card (orange eye +
  // "Encrypting your connection"), not by the button label.
  return (
    <Button variant={variant} onClick={onDisconnect}>
      <Button.Text>
        {connecting ? messages.gettext('Cancel') : messages.gettext('Disconnect')}
      </Button.Text>
    </Button>
  );
}
