import React from 'react';

import { messages } from '../../../../../../../../shared/gettext';
import { useAppContext } from '../../../../../../../context';
import { Button } from '../../../../../../../lib/components';

// Shown on the launch view, i.e. only while the daemon is unreachable. The
// daemon exits fail-closed on a crash, so a user whose daemon cannot come
// back up may be sitting behind the failsafe block: this is their no-CLI
// way out, behind the OS elevation prompt.
export function UnblockNetworkButton() {
  const { unblockNetwork } = useAppContext();
  const [pending, setPending] = React.useState(false);

  const handleClick = React.useCallback(() => {
    setPending(true);
    void unblockNetwork().finally(() => setPending(false));
  }, [unblockNetwork]);

  return (
    <Button onClick={handleClick} disabled={pending}>
      <Button.Text>
        {
          // TRANSLATORS: Button label on the launch view that lifts the failsafe
          // TRANSLATORS: firewall block when the system service cannot be reached,
          // TRANSLATORS: restoring internet access without VPN protection.
          messages.pgettext('launch-view', 'Restore internet without VPN')
        }
      </Button.Text>
    </Button>
  );
}
