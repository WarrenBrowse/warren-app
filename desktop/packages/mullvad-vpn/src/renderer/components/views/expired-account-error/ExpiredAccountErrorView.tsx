import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { LockdownModeSwitch } from '../../../features/tunnel/components';
import { Button, Flex, Spinner } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { spacings } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { useExclusiveTask } from '../../../lib/hooks/use-exclusive-task';
import { IconBadge } from '../../../lib/icon-badge';
import { useSelector } from '../../../redux/store';
import { AppMainHeader } from '../../app-main-header';
import {
  StyledCustomScrollbars,
  StyledMessage,
  StyledTitle,
  StyledWarrenPubKeyContainer,
  StyledWarrenPubKeyLabel,
  StyledWarrenPubKeyMessage,
} from '../../ExpiredAccountErrorViewStyles';
import { ModalAlert, ModalAlertType, ModalMessage } from '../../Modal';
import { SettingsListItem } from '../../settings-list-item';

enum RecoveryAction {
  openBrowser,
  disconnect,
  disableLockdownMode,
}

const StyledSettingsToggleListItem = styled(SettingsListItem)`
  margin-top: ${spacings.medium};
`;

export function ExpiredAccountErrorView() {
  return (
    <ExpiredAccountContextProvider>
      <ExpiredAccountErrorViewComponent />
    </ExpiredAccountContextProvider>
  );
}

function ExpiredAccountErrorViewComponent() {
  const { push } = useHistory();
  const { disconnectTunnel } = useAppContext();

  const { recoveryAction } = useRecoveryAction();
  const isNewAccount = useIsNewAccount();

  const [disconnect, disconnecting] = useExclusiveTask(async () => {
    try {
      await disconnectTunnel('gui-expired-account');
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to disconnect the tunnel: ${error.message}`);
    }
  });

  const navigateToRedeemVoucher = useCallback(() => {
    push(RoutePath.redeemVoucher);
  }, [push]);

  return (
    <View backgroundColor="darkBlue">
      <AppMainHeader
        variant={isNewAccount ? 'default' : 'basedOnConnectionStatus'}
        size="basedOnLoginStatus">
        <AppMainHeader.AccountButton />
        <AppMainHeader.SettingsButton />
      </AppMainHeader>
      <StyledCustomScrollbars fillContainer>
        <View.Content>
          <View.Container
            flexDirection="column"
            horizontalMargin="large"
            margin={{ top: 'large' }}
            flexGrow={1}
            justifyContent="space-between">
            <FlexColumn>{isNewAccount ? <WelcomeView /> : <Content />}</FlexColumn>

            <FlexColumn gap="medium">
              {recoveryAction === RecoveryAction.disconnect && (
                <Button variant="destructive" disabled={disconnecting} onClick={disconnect}>
                  <Button.Text>
                    {
                      // TRANSLATORS: Button label for disconnecting from the VPN.
                      messages.pgettext('connect-view', 'Disconnect')
                    }
                  </Button.Text>
                </Button>
              )}

              <ExternalPaymentButton />

              <CheckSubscriptionButton />

              <Button variant="success" onClick={navigateToRedeemVoucher}>
                <Button.Text>
                  {
                    // TRANSLATORS: Button label for navigating to the voucher redemption view.
                    messages.pgettext('connect-view', 'Redeem voucher')
                  }
                </Button.Text>
              </Button>
            </FlexColumn>

            <LockdownModeAlert />
          </View.Container>
        </View.Content>
      </StyledCustomScrollbars>
    </View>
  );
}

function WelcomeView() {
  const account = useSelector((state) => state.account);
  const { recoveryMessage } = useRecoveryAction();

  return (
    <>
      <StyledTitle data-testid="title">
        {messages.pgettext('connect-view', 'Congrats!')}
      </StyledTitle>
      <StyledWarrenPubKeyMessage>
        {messages.pgettext('connect-view', 'Here’s your public key. Save it!')}
        <StyledWarrenPubKeyContainer>
          <StyledWarrenPubKeyLabel pubkey={account.pubkey || ''} obscureValue={false} />
        </StyledWarrenPubKeyContainer>
      </StyledWarrenPubKeyMessage>

      <StyledMessage>
        {sprintf('%(introduction)s %(recoveryMessage)s', {
          introduction: messages.pgettext(
            'connect-view',
            'To start using the app, you first need to add time to your account.',
          ),
          recoveryMessage,
        })}
      </StyledMessage>
    </>
  );
}

function Content() {
  const { recoveryMessage } = useRecoveryAction();

  return (
    <>
      <Flex justifyContent="center" margin={{ bottom: 'medium' }}>
        <IconBadge state="negative" />
      </Flex>
      <StyledTitle data-testid="title">
        {messages.pgettext('connect-view', 'Out of time')}
      </StyledTitle>
      <StyledMessage>
        {sprintf('%(introduction)s %(recoveryMessage)s', {
          introduction: messages.pgettext(
            'connect-view',
            'You have no more VPN time left on this account.',
          ),
          recoveryMessage,
        })}
      </StyledMessage>
    </>
  );
}

function ExternalPaymentButton() {
  const { setShowLockdownModeAlert, startPolling } = useExpiredAccountContext();
  const { recoveryAction } = useRecoveryAction();
  const { buyCredit } = useAppContext();
  const isNewAccount = useIsNewAccount();

  const buttonText = isNewAccount
    ? messages.gettext('Buy credit')
    : messages.gettext('Buy more credit');

  const [openExternalPayment, openingExternalPayment] = useExclusiveTask(async () => {
    if (recoveryAction === RecoveryAction.disableLockdownMode) {
      setShowLockdownModeAlert(true);
    } else {
      // Opens the checkout bound to a fresh purchase id and starts the
      // auto-redeem poll (doc 35): the credit lands without the user
      // copying any voucher.
      await buyCredit();
      // G-5: Also auto-poll the account data as a safety net (covers a
      // credit applied through any other path).
      startPolling();
    }
  });

  return (
    <Button
      variant="success"
      disabled={openingExternalPayment || recoveryAction === RecoveryAction.disconnect}
      onClick={openExternalPayment}
      aria-description={
        // TRANSLATORS: Accessibility label for the button that opens the browser to buy credit.
        messages.pgettext('accessibility', 'Opens externally')
      }>
      <Button.Text>{buttonText}</Button.Text>
      <Button.Icon icon="external" />
    </Button>
  );
}

// G-5: Manual "Check subscription" button for users who completed
// external payment and want to refresh immediately.
function CheckSubscriptionButton() {
  const { polling } = useExpiredAccountContext();
  const { recoveryAction } = useRecoveryAction();
  const { updateAccountData } = useAppContext();
  const [checking, setChecking] = useState(false);

  const handleCheck = useCallback(async () => {
    setChecking(true);
    try {
      await updateAccountData();
    } catch (e) {
      const err = e as Error;
      log.error(`Manual subscription check failed: ${err.message}`);
    } finally {
      setChecking(false);
    }
  }, [updateAccountData]);

  const onClickCheck = useCallback(() => {
    void handleCheck();
  }, [handleCheck]);

  // Only show after the user has opened the external payment page
  // (polling started) or as a persistent manual refresh option.
  if (recoveryAction === RecoveryAction.disconnect) {
    return null;
  }

  return (
    <Button
      disabled={checking}
      onClick={onClickCheck}
      data-testid="expired-account-check-subscription">
      {checking ? (
        <Spinner size="small" />
      ) : (
        <Button.Text>
          {polling
            ? messages.pgettext('connect-view', 'Checking... (click to refresh now)')
            : messages.pgettext('connect-view', "I've completed payment")}
        </Button.Text>
      )}
    </Button>
  );
}

function LockdownModeAlert() {
  const { showLockdownModeAlert, setShowLockdownModeAlert } = useExpiredAccountContext();

  const onCloseLockdownModeInstructions = useCallback(() => {
    setShowLockdownModeAlert(false);
  }, [setShowLockdownModeAlert]);

  return (
    <ModalAlert
      isOpen={showLockdownModeAlert}
      type={ModalAlertType.caution}
      buttons={[
        <Button key="cancel" onClick={onCloseLockdownModeInstructions}>
          <Button.Text>{messages.gettext('Close')}</Button.Text>
        </Button>,
      ]}
      close={onCloseLockdownModeInstructions}>
      <ModalMessage>
        {messages.pgettext(
          'connect-view',
          'You need to disable "Lockdown mode" in order to access the Internet to add time.',
        )}
      </ModalMessage>
      <ModalMessage>
        {messages.pgettext(
          'connect-view',
          'Remember, turning it off will allow network traffic while the VPN is disconnected until you turn it back on under Advanced settings.',
        )}
      </ModalMessage>
      <StyledSettingsToggleListItem>
        <SettingsListItem.Item>
          <LockdownModeSwitch>
            <LockdownModeSwitch.Label variant="titleMedium">
              {messages.pgettext('vpn-settings-view', 'Lockdown mode')}
            </LockdownModeSwitch.Label>
            <SettingsListItem.Item.ActionGroup>
              <LockdownModeSwitch.Input />
            </SettingsListItem.Item.ActionGroup>
          </LockdownModeSwitch>
        </SettingsListItem.Item>
      </StyledSettingsToggleListItem>
    </ModalAlert>
  );
}

type ExpiredAccountContextType = {
  setShowLockdownModeAlert: (val: boolean) => void;
  showLockdownModeAlert: boolean;
  polling: boolean;
  startPolling: () => void;
};

const ExpiredAccountContext = createContext<ExpiredAccountContextType | undefined>(undefined);

const ExpiredAccountContextProvider = ({ children }: { children: ReactNode }) => {
  const [showLockdownModeAlert, setShowLockdownModeAlert] = useState(false);
  const [polling, setPolling] = useState(false);
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pollDeadlineRef = useRef<number>(0);
  const { updateAccountData } = useAppContext();

  // Cleanup auto-poll on unmount.
  useEffect(() => {
    return () => {
      if (pollTimerRef.current) {
        clearInterval(pollTimerRef.current);
      }
    };
  }, []);

  const startPolling = useCallback(() => {
    // Auto-poll every 10s for 2 minutes after the user opens the
    // external payment page. If the daemon reports a valid expiry,
    // the existing StateTriggeredNavigation will route to main.
    if (pollTimerRef.current) {
      clearInterval(pollTimerRef.current);
    }
    pollDeadlineRef.current = Date.now() + 2 * 60 * 1000;
    setPolling(true);
    pollTimerRef.current = setInterval(() => {
      if (Date.now() > pollDeadlineRef.current) {
        if (pollTimerRef.current) {
          clearInterval(pollTimerRef.current);
          pollTimerRef.current = null;
        }
        setPolling(false);
        return;
      }
      void updateAccountData().catch((e: unknown) => {
        const err = e as Error;
        log.error(`Auto-poll subscription check failed: ${err.message}`);
      });
    }, 10_000);
  }, [updateAccountData]);

  const value: ExpiredAccountContextType = useMemo(
    () => ({
      setShowLockdownModeAlert,
      showLockdownModeAlert,
      polling,
      startPolling,
    }),
    [setShowLockdownModeAlert, showLockdownModeAlert, polling, startPolling],
  );
  return <ExpiredAccountContext.Provider value={value}>{children}</ExpiredAccountContext.Provider>;
};

const useExpiredAccountContext = () => {
  const context = useContext(ExpiredAccountContext);
  if (!context) {
    throw new Error(
      'useExpiredAccountContext must be used within an ExpiredAccountContextProvider',
    );
  }

  return context;
};

const useRecoveryAction = () => {
  const isBlocked = useSelector((state) => state.connection.isBlocked);
  const lockdownMode = useSelector((state) => state.settings.lockdownMode);

  let recoveryAction: RecoveryAction;

  if (lockdownMode && isBlocked) {
    recoveryAction = RecoveryAction.disableLockdownMode;
  } else if (!lockdownMode && isBlocked) {
    recoveryAction = RecoveryAction.disconnect;
  } else {
    recoveryAction = RecoveryAction.openBrowser;
  }

  let recoveryMessage: string;

  switch (recoveryAction) {
    case RecoveryAction.openBrowser:
    case RecoveryAction.disableLockdownMode:
      recoveryMessage = messages.pgettext(
        'connect-view',
        'Either buy credit on our website or redeem a voucher.',
      );
      break;
    case RecoveryAction.disconnect:
      recoveryMessage = messages.pgettext(
        'connect-view',
        'To add more, you will need to disconnect and access the Internet with an unsecure connection.',
      );
      break;
  }

  return { recoveryAction, recoveryMessage };
};

const useIsNewAccount = () => {
  const account = useSelector((state) => state.account);
  return account.status.type === 'ok' && account.status.method === 'new_account';
};
