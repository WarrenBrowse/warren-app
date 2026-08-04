import { useCallback, useEffect, useRef, useState } from 'react';

import { messages } from '../../../shared/gettext';
import log from '../../../shared/logging';
import { useAppContext } from '../../context';
import { Button } from '../../lib/components';
import { useExclusiveTask } from '../../lib/hooks/use-exclusive-task';
import { PaymentRecoveryAction, paymentRecoveryAction } from '../../lib/payment';
import { useSelector } from '../../redux/store';
import { ModalAlert, ModalAlertType, ModalMessage } from '../Modal';
import { LockdownModeAlert } from './LockdownModeAlert';

interface ExternalPaymentButtonProps {
  buttonText: string;
  'data-testid'?: string;
}

// The one way to open the external checkout (doc 35). Always
// clickable: when the firewall blocks the browser (out-of-time
// connecting loop, blocking error state, lockdown) the click routes
// the user through the unblocking step instead of presenting a dead
// greyed-out button.
export function ExternalPaymentButton(props: ExternalPaymentButtonProps) {
  const { buyCredit, disconnectTunnel } = useAppContext();
  const isBlocked = useSelector((state) => state.connection.isBlocked);
  const lockdownMode = useSelector((state) => state.settings.lockdownMode);
  const recoveryAction = paymentRecoveryAction(lockdownMode, isBlocked);

  const [showLockdownAlert, setShowLockdownAlert] = useState(false);
  const [showDisconnectAlert, setShowDisconnectAlert] = useState(false);

  // Live view of isBlocked for the post-disconnect wait below:
  // disconnectTunnel resolves when the daemon ACCEPTS the command, not
  // when the firewall is actually lifted, and the selector value
  // captured by the task closure would be stale.
  const isBlockedRef = useRef(isBlocked);
  useEffect(() => {
    isBlockedRef.current = isBlocked;
  }, [isBlocked]);

  const [onClick, working] = useExclusiveTask(async () => {
    switch (recoveryAction) {
      case PaymentRecoveryAction.disableLockdownMode:
        setShowLockdownAlert(true);
        break;
      case PaymentRecoveryAction.disconnect:
        setShowDisconnectAlert(true);
        break;
      case PaymentRecoveryAction.openBrowser:
        try {
          await buyCredit();
        } catch (e) {
          const error = e as Error;
          log.error(`Failed to open checkout: ${error.message}`);
        }
        break;
    }
  });

  const [disconnectAndBuy, disconnecting] = useExclusiveTask(async () => {
    setShowDisconnectAlert(false);
    try {
      await disconnectTunnel('gui-buy-credit');
      // Wait for the firewall to actually lift before opening the
      // browser, otherwise the checkout tab loads into a blocked
      // network and shows a connection error. Bounded: open anyway
      // after 5s, the user can reload the tab.
      for (let i = 0; i < 50 && isBlockedRef.current; i++) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      await buyCredit();
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to disconnect and open checkout: ${error.message}`);
    }
  });

  const closeLockdownAlert = useCallback(() => setShowLockdownAlert(false), []);
  const closeDisconnectAlert = useCallback(() => setShowDisconnectAlert(false), []);
  const onDisconnectAndBuy = useCallback(() => void disconnectAndBuy(), [disconnectAndBuy]);

  return (
    <>
      <Button
        variant="success"
        disabled={working || disconnecting}
        onClick={onClick}
        data-testid={props['data-testid']}
        aria-description={
          // TRANSLATORS: Accessibility label for the button that opens the browser to buy credit.
          messages.pgettext('accessibility', 'Opens externally')
        }>
        <Button.Text>{props.buttonText}</Button.Text>
        <Button.Icon icon="external" />
      </Button>

      <ModalAlert
        isOpen={showDisconnectAlert}
        type={ModalAlertType.caution}
        buttons={[
          <Button key="disconnect" variant="destructive" onClick={onDisconnectAndBuy}>
            <Button.Text>
              {
                // TRANSLATORS: Button label for disconnecting from the VPN.
                messages.pgettext('connect-view', 'Disconnect')
              }
            </Button.Text>
          </Button>,
          <Button key="cancel" onClick={closeDisconnectAlert}>
            <Button.Text>{messages.gettext('Cancel')}</Button.Text>
          </Button>,
        ]}
        close={closeDisconnectAlert}>
        <ModalMessage>
          {messages.pgettext(
            'connect-view',
            'To add more, you will need to disconnect and access the Internet with an unsecure connection.',
          )}
        </ModalMessage>
      </ModalAlert>

      <LockdownModeAlert isOpen={showLockdownAlert} onClose={closeLockdownAlert} />
    </>
  );
}
