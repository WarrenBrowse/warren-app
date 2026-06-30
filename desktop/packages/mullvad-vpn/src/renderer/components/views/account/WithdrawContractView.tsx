import { useCallback, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { Button, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { colors, Radius, spacings } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';

const Callout = styled.div`
  padding: ${spacings.small} ${spacings.medium};
  border-radius: ${Radius.radius4};
  background-color: ${colors.redAlpha40};
  border: 1px solid ${colors.red40};
`;

// EU CRD art. 11a consumer withdrawal: the dedicated confirmation screen
// reached from the "Withdraw from your contract" button on the account
// view. This is the second of the two steps (trigger -> confirm -> done);
// the confirm button below ends the current subscription term.
export function WithdrawContractView() {
  const history = useHistory();
  const { withdrawSubscription } = useAppContext();

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  const onConfirm = useCallback(async () => {
    setError(null);
    setSubmitting(true);
    try {
      const response = await withdrawSubscription();
      if (response.type === 'success') {
        setDone(true);
      } else {
        setError(
          messages.pgettext(
            'account-view',
            'Could not withdraw right now. Check your connection and try again.',
          ),
        );
      }
    } catch (e) {
      const err = e as Error;
      log.error(`withdrawSubscription failed: ${err.message}`);
      setError(
        messages.pgettext(
          'account-view',
          'Could not withdraw right now. Check your connection and try again.',
        ),
      );
    } finally {
      setSubmitting(false);
    }
  }, [withdrawSubscription]);

  const onDone = useCallback(() => history.pop(), [history]);
  const onCancel = useCallback(() => history.pop(), [history]);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={history.pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={
              // TRANSLATORS: Title label in navigation bar for the
              // TRANSLATORS: contract withdrawal confirmation screen.
              messages.pgettext('account-view', 'Withdraw from contract')
            }
          />

          <NavigationScrollbars>
            <View.Content>
              <View.Container flexDirection="column" horizontalMargin="medium" gap="medium">
                <HeaderTitle>
                  {messages.pgettext('account-view', 'Withdraw from your contract')}
                </HeaderTitle>

                {done ? (
                  <FlexColumn gap="medium">
                    <Text variant="bodySmall" color="whiteAlpha80">
                      {messages.pgettext(
                        'account-view',
                        'You have withdrawn from your contract. Your subscription has ended. Access stops shortly and you will not be charged again.',
                      )}
                    </Text>
                    <Button variant="success" onClick={onDone}>
                      <Button.Text>{messages.gettext('Done')}</Button.Text>
                    </Button>
                  </FlexColumn>
                ) : (
                  <FlexColumn gap="medium">
                    <Callout>
                      <Text variant="bodySmallSemibold" color="white">
                        {messages.pgettext(
                          'account-view',
                          'Withdrawing ends your current subscription immediately. Your access will stop shortly after.',
                        )}
                      </Text>
                    </Callout>

                    <Text variant="bodySmall" color="whiteAlpha80">
                      {messages.pgettext(
                        'account-view',
                        'This exercises your right of withdrawal from the contract. Your identity and recovery phrase stay on this device, so you can subscribe again later.',
                      )}
                    </Text>

                    {error && (
                      <Text variant="bodySmall" color="red">
                        {error}
                      </Text>
                    )}

                    <Button variant="destructive" disabled={submitting} onClick={onConfirm}>
                      <Button.Text>
                        {submitting
                          ? // TRANSLATORS: Button label shown while the withdrawal request is in flight.
                            messages.pgettext('account-view', 'Withdrawing...')
                          : // TRANSLATORS: Button that confirms withdrawal from the contract.
                            messages.pgettext('account-view', 'Confirm withdrawal')}
                      </Button.Text>
                    </Button>

                    <Button variant="primary" disabled={submitting} onClick={onCancel}>
                      <Button.Text>{messages.gettext('Cancel')}</Button.Text>
                    </Button>
                  </FlexColumn>
                )}
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
