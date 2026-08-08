import { useCallback, useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { formatRemainingTime, hasExpired } from '../../../../shared/account-expiry';
import { isBetaBuild } from '../../../../shared/constants/product-env';
import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Checkbox, Flex, Icon, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { colors } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { useEffectEvent } from '../../../lib/utility-hooks';
import { useSelector } from '../../../redux/store';
import { AppNavigationHeader } from '../..';
import { BetaBadge } from '../../beta-badge';
import { BackAction } from '../../keyboard-navigation';
import { ModalAlert, ModalAlertType } from '../../Modal';
import { ExternalPaymentButton } from '../../payment';
import { RedeemVoucherButton } from '../../RedeemVoucher';
import { HeaderTitle } from '../../SettingsHeader';
import { AccountExpiryRow, ForumHandleRow, LabelledRow, WarrenPubKeyRow } from './components';

const StyledViewContainer = styled(View.Container)`
  height: 100%;
  justify-content: space-between;
`;

// Surface card that lifts the account facts off the flat background and fills
// the vertical space that used to sit empty above the buttons.
const Card = styled.div`
  background-color: ${colors.blue10};
  border-radius: 16px;
  padding: 16px;
`;

// The backup action is navigation, not a coloured command: an outlined row with
// a chevron sets it apart from the solid buy/voucher buttons so no two actions
// share the same weight.
const BackupRow = styled.button`
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 14px 16px;
  border-radius: 12px;
  background: ${colors.transparent};
  border: 1px solid ${colors.whiteAlpha20};
  transition: background-color 150ms ease;
  &:not(:disabled):hover {
    background-color: ${colors.whiteAlpha20};
  }
`;

// Log out is rare and destructive: a ghost red link at the very bottom, kept
// clearly separate from the everyday actions above.
const LogoutButton = styled.button`
  align-self: center;
  padding: 10px 12px;
  background: ${colors.transparent};
  color: ${colors.red};
  &:not(:disabled):hover {
    text-decoration: underline;
  }
`;

function SubscriptionCard() {
  const accountExpiry = useSelector((state) => state.account.expiry);
  const active = accountExpiry ? !hasExpired(accountExpiry) : false;
  const remaining = active && accountExpiry ? formatRemainingTime(accountExpiry) : undefined;

  return (
    <Card>
      <FlexColumn gap="small">
        {remaining ? (
          <Text variant="titleLarge" color="green">
            {remaining}
          </Text>
        ) : null}
        <FlexColumn gap="tiny">
          <Text variant="labelTiny" color="whiteAlpha60">
            {messages.pgettext('account-view', 'Paid until')}
          </Text>
          <AccountExpiryRow />
        </FlexColumn>
      </FlexColumn>
    </Card>
  );
}

// Device-held auto-renewal mandate (warren-core doc 65): display data pushed by the
// main process, turn-off is one click with no extra confirmation
// (anti-subscription-trap posture: cancelling must be effortless).
function RenewalCard() {
  const renewalState = useSelector((state) => state.account.renewalState);
  const { disableRenewal } = useAppContext();
  const [disabling, setDisabling] = useState(false);
  const onDisable = useCallback(async () => {
    setDisabling(true);
    try {
      await disableRenewal();
    } finally {
      setDisabling(false);
    }
  }, [disableRenewal]);

  if (renewalState === undefined) {
    return null;
  }
  const card =
    renewalState.cardBrand && renewalState.cardLast4
      ? `${renewalState.cardBrand} \u2022\u2022\u2022\u2022 ${renewalState.cardLast4}`
      : undefined;
  // Permanent in-app half of the pre-renewal notice: the next renewal
  // date stays visible whenever the account view is open.
  const renewsOn =
    renewalState.renewsAtMs !== undefined
      ? sprintf(
          // TRANSLATORS: Permanent line showing the next automatic renewal date.
          // TRANSLATORS: Available placeholder:
          // TRANSLATORS: %(date)s - the renewal date
          messages.pgettext('account-view', 'Renews monthly, next around %(date)s'),
          { date: new Date(renewalState.renewsAtMs).toLocaleDateString() },
        )
      : undefined;
  const lastCharge =
    renewalState.lastChargeMs !== undefined
      ? sprintf(
          // TRANSLATORS: Line showing the date of the last automatic renewal charge.
          // TRANSLATORS: Available placeholder:
          // TRANSLATORS: %(date)s - the charge date
          messages.pgettext('account-view', 'Last renewal on %(date)s'),
          { date: new Date(renewalState.lastChargeMs).toLocaleDateString() },
        )
      : undefined;
  return (
    <Card>
      <FlexColumn gap="tiny">
        <Text variant="labelTiny" color="whiteAlpha60">
          {
            // TRANSLATORS: Label of the auto-renewal block on the account view.
            messages.pgettext('account-view', 'Automatic renewal')
          }
        </Text>
        <Flex justifyContent="space-between" alignItems="center">
          <Text variant="bodySmall">
            {card ??
              // TRANSLATORS: Shown while the saved card details are unknown.
              messages.pgettext('account-view', 'Active')}
          </Text>
          <Button variant="destructive" disabled={disabling} onClick={onDisable} width="fit">
            <Button.Text>
              {
                // TRANSLATORS: Button that turns automatic renewal off.
                messages.pgettext('account-view', 'Turn off')
              }
            </Button.Text>
          </Button>
        </Flex>
        {renewsOn !== undefined && (
          <Text variant="labelTiny" color="whiteAlpha60">
            {renewsOn}
          </Text>
        )}
        {lastCharge !== undefined && (
          <Text variant="labelTiny" color="whiteAlpha60">
            {lastCharge}
          </Text>
        )}
      </FlexColumn>
    </Card>
  );
}

export function AccountView() {
  const history = useHistory();
  const forumHandle = useSelector((state) => state.account.forumIdentity?.handle);
  const { updateAccountData, logout } = useAppContext();

  // `updateAccountData` rejects when the API returns 404 (= no
  // active subscription yet on a freshly created Warren account)
  // or on transient communication failures. Without a `.catch()`,
  // the Promise rejection bubbles up as an unhandled rejection and
  // can interact badly with React error boundaries on some
  // platforms - silent render failures, blank-screen navigations
  // and similar UX bugs were traced back to this missing handler.
  // The retry strategy is owned upstream by `account-data-cache`,
  // so swallowing the failure here is safe.
  const onMount = useEffectEvent(() => {
    updateAccountData().catch(() => {
      // Intentionally silent: error already surfaced via the
      // redux account.expiry state and the cache's own retry log.
    });
  });
  // These lint rules are disabled for now because the react plugin for eslint does
  // not understand that useEffectEvent should not be added to the dependency array.
  // Enable these rules again when eslint can lint useEffectEvent properly.
  // eslint-disable-next-line react-compiler/react-compiler
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => onMount(), []);

  // Logout is a TRUE sign-out: the daemon erases the recovery phrase
  // from this device. Gate it behind an explicit confirmation that the
  // user has backed up their phrase, otherwise the account (and its
  // subscription) is unrecoverable.
  const [logoutConfirmOpen, setLogoutConfirmOpen] = useState(false);
  const [backedUp, setBackedUp] = useState(false);

  const openLogoutConfirm = useCallback(() => {
    setBackedUp(false);
    setLogoutConfirmOpen(true);
  }, []);
  const closeLogoutConfirm = useCallback(() => setLogoutConfirmOpen(false), []);

  // Hack needed because if we just call `logout` directly in `onClick`
  // then it is run with the wrong `this`.
  const doLogout = useCallback(async () => {
    setLogoutConfirmOpen(false);
    await logout('gui-logout-button');
  }, [logout]);

  const goToKeys = useCallback(() => {
    history.push(RoutePath.keys);
  }, [history]);

  const goBackupPhrase = useCallback(() => {
    setLogoutConfirmOpen(false);
    history.push(RoutePath.keys);
  }, [history]);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={history.pop}>
        <AppNavigationHeader
          title={
            // TRANSLATORS: Title label in navigation bar
            messages.pgettext('account-view', 'Account')
          }
        />

        <View.Content>
          <StyledViewContainer flexDirection="column" horizontalMargin="medium">
            <FlexColumn gap="medium">
              <Text variant="titleBig">
                <HeaderTitle>{messages.pgettext('account-view', 'Account')}</HeaderTitle>
              </Text>

              <SubscriptionCard />

              <BetaBadge variant="row" />

              {isBetaBuild ? null : <RenewalCard />}

              <Card>
                <FlexColumn gap="medium">
                  <LabelledRow
                    label={messages.pgettext('account-view', 'Public key (wallet address)')}>
                    <WarrenPubKeyRow />
                  </LabelledRow>
                  {forumHandle !== undefined && (
                    <LabelledRow
                      label={
                        // TRANSLATORS: Label of the community-forum name derived
                        // TRANSLATORS: from this wallet. Shown only after the
                        // TRANSLATORS: user has signed in to the forum once.
                        messages.pgettext('account-view', 'Forum name')
                      }>
                      <ForumHandleRow />
                    </LabelledRow>
                  )}
                </FlexColumn>
              </Card>
            </FlexColumn>

            <FlexColumn gap="small">
              {isBetaBuild ? null : (
                <>
                  <ExternalPaymentButton buttonText={messages.gettext('Buy more credit')} />

                  <RedeemVoucherButton variant="primary" />
                </>
              )}

              <BackupRow onClick={goToKeys}>
                <Text variant="bodySmallSemibold">
                  {
                    // TRANSLATORS: Button label that opens the Keys
                    // TRANSLATORS: backup view (= reveal/copy BIP39 mnemonic).
                    messages.pgettext('account-view', 'Backup keys')
                  }
                </Text>
                <Icon icon="chevron-right" color="whiteAlpha60" />
              </BackupRow>

              <LogoutButton onClick={openLogoutConfirm}>
                <Text variant="bodySmallSemibold" color="red">
                  {
                    // TRANSLATORS: Button label for logging out.
                    messages.pgettext('account-view', 'Log out')
                  }
                </Text>
              </LogoutButton>
            </FlexColumn>
          </StyledViewContainer>
        </View.Content>
      </BackAction>

      <ModalAlert
        isOpen={logoutConfirmOpen}
        type={ModalAlertType.caution}
        title={messages.pgettext('account-view', 'Log out of this account?')}
        message={[
          messages.pgettext(
            'account-view',
            'Logging out erases this account from this device. There is no email or password to log back in, your recovery phrase is the ONLY way to restore it.',
          ),
          messages.pgettext(
            'account-view',
            'If you have not backed up your recovery phrase, your subscription will be lost permanently.',
          ),
        ]}
        gridButtons={[
          <Button key="backup" onClick={goBackupPhrase}>
            <Button.Text>
              {
                // TRANSLATORS: Button that opens the recovery-phrase backup view before logging out.
                messages.pgettext('account-view', 'Back up my phrase')
              }
            </Button.Text>
          </Button>,
          <Button key="logout" variant="destructive" disabled={!backedUp} onClick={doLogout}>
            <Button.Text>{messages.pgettext('account-view', 'Log out')}</Button.Text>
          </Button>,
        ]}
        close={closeLogoutConfirm}>
        <Checkbox checked={backedUp} onCheckedChange={setBackedUp}>
          <Flex gap="small" alignItems="flex-start">
            <Checkbox.Trigger>
              <Checkbox.Input />
            </Checkbox.Trigger>
            <Checkbox.Label>
              {messages.pgettext(
                'account-view',
                'I have backed up my recovery phrase and understand this account will be removed from this device.',
              )}
            </Checkbox.Label>
          </Flex>
        </Checkbox>
      </ModalAlert>
    </View>
  );
}
