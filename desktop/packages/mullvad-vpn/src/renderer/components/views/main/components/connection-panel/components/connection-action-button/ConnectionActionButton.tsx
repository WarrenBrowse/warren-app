import { connectButtonDisabled } from '../../../../../../../lib/env-yield';
import { useSelector } from '../../../../../../../redux/store';
import { ConnectButton, DisconnectButton } from '../';

export function ConnectionActionButton() {
  const tunnelState = useSelector((state) => state.connection.status.state);
  const envYield = useSelector((state) => state.settings.warrenStatus?.envYield);

  if (tunnelState === 'disconnected' || tunnelState === 'disconnecting') {
    return <ConnectButton disabled={connectButtonDisabled(tunnelState, envYield)} />;
  } else {
    return <DisconnectButton />;
  }
}
