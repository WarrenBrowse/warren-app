import { useCallback, useState } from 'react';
import { sprintf } from 'sprintf-js';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Flex, Spinner } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { useExclusiveTask } from '../../../lib/hooks/use-exclusive-task';
import { IconBadge } from '../../../lib/icon-badge';
import { PaymentRecoveryAction, paymentRecoveryAction } from '../../../lib/payment';
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
import { ExternalPaymentButton } from '../../payment';

export function ExpiredAccountErrorView() {
  const { push } = useHistory();
  const isNewAccount = useIsNewAccount();

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
              <DisconnectButton />

              <ExternalPaymentButton
                buttonText={
                  isNewAccount
                    ? messages.gettext('Buy credit')
                    : messages.gettext('Buy more credit')
                }
              />

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
          </View.Container>
        </View.Content>
      </StyledCustomScrollbars>
    </View>
  );
}

function WelcomeView() {
  const account = useSelector((state) => state.account);
  const recoveryMessage = useRecoveryMessage();

  return (
    <>
      <StyledTitle data-testid="title">
        {messages.pgettext('connect-view', 'Congrats!')}
      </StyledTitle>
      <StyledWarrenPubKeyMessage>
        {messages.pgettext('connect-view', 'Here’s your public key. Save it!')}
        <StyledWarrenPubKeyContainer>
          <StyledWarrenPubKeyLabel pubkey={account.pubkey || ''} />
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
  const recoveryMessage = useRecoveryMessage();

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

// Plain "just give me my internet back" affordance while the firewall
// blocks: disconnecting must not force the user through a checkout
// tab (the buy button's dialog also disconnects, but always opens the
// browser too).
function DisconnectButton() {
  const isBlocked = useSelector((state) => state.connection.isBlocked);
  const lockdownMode = useSelector((state) => state.settings.lockdownMode);
  const { disconnectTunnel } = useAppContext();

  const [disconnect, disconnecting] = useExclusiveTask(async () => {
    try {
      await disconnectTunnel('gui-expired-account');
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to disconnect the tunnel: ${error.message}`);
    }
  });

  if (paymentRecoveryAction(lockdownMode, isBlocked) !== PaymentRecoveryAction.disconnect) {
    return null;
  }

  return (
    <Button variant="destructive" disabled={disconnecting} onClick={disconnect}>
      <Button.Text>
        {
          // TRANSLATORS: Button label for disconnecting from the VPN.
          messages.pgettext('connect-view', 'Disconnect')
        }
      </Button.Text>
    </Button>
  );
}

// Manual "Check subscription" refresh for users who completed the
// external payment. The main-process purchase poll normally credits
// on its own; while it runs the label reflects it.
function CheckSubscriptionButton() {
  const purchaseInFlight = useSelector((state) => state.account.purchaseInFlight);
  const isBlocked = useSelector((state) => state.connection.isBlocked);
  const lockdownMode = useSelector((state) => state.settings.lockdownMode);
  const { checkPendingPurchases, updateAccountData } = useAppContext();
  const [checking, setChecking] = useState(false);

  const handleCheck = useCallback(async () => {
    setChecking(true);
    try {
      // First give any pending app-initiated purchase an immediate
      // redeem attempt (that is what actually credits the account),
      // then refresh the expiry the UI shows.
      await checkPendingPurchases();
      await updateAccountData();
    } catch (e) {
      const err = e as Error;
      log.error(`Manual subscription check failed: ${err.message}`);
    } finally {
      setChecking(false);
    }
  }, [checkPendingPurchases, updateAccountData]);

  const onClickCheck = useCallback(() => {
    void handleCheck();
  }, [handleCheck]);

  // While the firewall blocks, the check cannot reach the API and
  // would fail silently: the recovery guidance handles this state.
  if (paymentRecoveryAction(lockdownMode, isBlocked) === PaymentRecoveryAction.disconnect) {
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
          {purchaseInFlight
            ? messages.pgettext('connect-view', 'Checking... (click to refresh now)')
            : messages.pgettext('connect-view', "I've completed payment")}
        </Button.Text>
      )}
    </Button>
  );
}

function useRecoveryMessage(): string {
  const isBlocked = useSelector((state) => state.connection.isBlocked);
  const lockdownMode = useSelector((state) => state.settings.lockdownMode);

  switch (paymentRecoveryAction(lockdownMode, isBlocked)) {
    case PaymentRecoveryAction.openBrowser:
    case PaymentRecoveryAction.disableLockdownMode:
      return messages.pgettext(
        'connect-view',
        'Either buy credit on our website or redeem a voucher.',
      );
    case PaymentRecoveryAction.disconnect:
      return messages.pgettext(
        'connect-view',
        'To add more, you will need to disconnect and access the Internet with an unsecure connection.',
      );
  }
}

const useIsNewAccount = () => {
  const account = useSelector((state) => state.account);
  return account.status.type === 'ok' && account.status.method === 'new_account';
};
