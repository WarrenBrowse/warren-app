import { useCallback } from 'react';

import { messages } from '../../../../../../../../shared/gettext';
import log from '../../../../../../../../shared/logging';
import { useAppContext } from '../../../../../../../context';
import { Button, ButtonProps } from '../../../../../../../lib/components';

export function ConnectButton(props: ButtonProps) {
  const { connectTunnel } = useAppContext();

  const onConnect = useCallback(async () => {
    try {
      await connectTunnel();
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to connect the tunnel: ${error.message}`);
    }
  }, [connectTunnel]);

  // Green: the button is a call-to-action, not a state indicator. The exposed
  // state is already signalled in red by the status card (red eye + "You are
  // visible"); the button's job is to invite the safe action, so it wears the
  // "go" colour to inspire confidence in connecting.
  return (
    <Button variant="success" onClick={onConnect} {...props}>
      <Button.Text>{messages.pgettext('tunnel-control', 'Connect')}</Button.Text>
    </Button>
  );
}
