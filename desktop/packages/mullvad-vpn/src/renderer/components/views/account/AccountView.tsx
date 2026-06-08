import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { urls } from '../../../../shared/constants';
import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Checkbox, Flex, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { useExclusiveTask } from '../../../lib/hooks/use-exclusive-task';
import { useEffectEvent } from '../../../lib/utility-hooks';
import { useSelector } from '../../../redux/store';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { ModalAlert, ModalAlertType } from '../../Modal';
import { RedeemVoucherButton } from '../../RedeemVoucher';
import { HeaderTitle } from '../../SettingsHeader';
import { AccountExpiryRow, LabelledRow, WarrenPubKeyRow } from './components';

const StyledViewContainer = styled(View.Container)`
  height: 100%;
  justify-content: space-between;
`;

export function AccountView() {
  const history = useHistory();
  const isOffline = useSelector((state) => state.connection.isBlocked);
  const { updateAccountData, openUrlWithAuth, logout } = useAppContext();

  const [buyMore] = useExclusiveTask(async () => {
    await openUrlWithAuth(urls.purchase);
  });

  // `updateAccountData` rejects when the API returns 404 (= no
  // active subscription yet on a freshly created Warren account)
  // or on transient communication failures. Without a `.catch()`,
  // the Promise rejection bubbles up as an unhandled rejection and
  // can interact badly with React error boundaries on some
  // platforms — silent render failures, blank-screen navigations
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

              <FlexColumn gap="large">
                <LabelledRow label={messages.pgettext('account-view', 'Public key')}>
                  <WarrenPubKeyRow />
                </LabelledRow>

                <LabelledRow gap="tiny" label={messages.pgettext('account-view', 'Paid until')}>
                  <AccountExpiryRow />
                </LabelledRow>
              </FlexColumn>
            </FlexColumn>

            <FlexColumn gap="medium">
              <Button
                variant="success"
                disabled={isOffline}
                onClick={buyMore}
                aria-description={messages.pgettext('accessibility', 'Opens externally')}>
                <Button.Text>{messages.gettext('Buy more credit')}</Button.Text>
                <Button.Icon icon="external" />
              </Button>

              <RedeemVoucherButton />

              <Button onClick={goToKeys}>
                <Button.Text>
                  {
                    // TRANSLATORS: Button label that opens the Keys
                    // TRANSLATORS: backup view (= reveal/copy BIP39 mnemonic).
                    messages.pgettext('account-view', 'Backup keys')
                  }
                </Button.Text>
              </Button>

              <Button variant="destructive" onClick={openLogoutConfirm}>
                <Button.Text>
                  {
                    // TRANSLATORS: Button label for logging out.
                    messages.pgettext('account-view', 'Log out')
                  }
                </Button.Text>
              </Button>
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
            'Logging out erases this account from this device. There is no email or password to log back in — your recovery phrase is the ONLY way to restore it.',
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
