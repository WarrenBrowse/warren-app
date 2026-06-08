import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Checkbox, Text } from '../../../lib/components';
import { Flex } from '../../../lib/components/flex';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { colors, Radius, spacings } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';
import { CopyMnemonicButton, MnemonicGrid } from '../../warren-mnemonic';

const Callout = styled.div<{ $tone: 'warning' | 'danger' }>`
  padding: ${spacings.small} ${spacings.medium};
  border-radius: ${Radius.radius4};
  background-color: ${({ $tone }) =>
    $tone === 'danger' ? colors.redAlpha40 : colors.greenAlpha40};
  border: 1px solid ${({ $tone }) => ($tone === 'danger' ? colors.red40 : colors.green40)};
`;

export function KeysView() {
  const history = useHistory();
  const { getWarrenMnemonic } = useAppContext();

  const [mnemonic, setMnemonic] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onReveal = useCallback(async () => {
    try {
      const m = await getWarrenMnemonic();
      if (!m) {
        setError(
          messages.pgettext(
            'keys-view',
            'No mnemonic available. Log in or restore an identity first.',
          ),
        );
        return;
      }
      setMnemonic(m);
      setRevealed(true);
    } catch (e) {
      const err = e as Error;
      log.error(`getWarrenMnemonic failed: ${err.message}`);
      setError(messages.pgettext('keys-view', 'Failed to retrieve mnemonic from daemon.'));
    }
  }, [getWarrenMnemonic]);

  // Cleanup secret from React memory if the user navigates away (= back).
  useEffect(() => {
    return () => {
      setMnemonic(null);
      setRevealed(false);
      setConfirmed(false);
    };
  }, []);

  const onDone = useCallback(() => history.pop(), [history]);
  const onRestore = useCallback(() => history.push(RoutePath.restoreKeys), [history]);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={history.pop}>
        <NavigationContainer>
          <AppNavigationHeader title={messages.pgettext('keys-view', 'Keys')} />

          <NavigationScrollbars>
            <View.Content>
              <View.Container flexDirection="column" horizontalMargin="medium" gap="medium">
                <HeaderTitle>{messages.pgettext('keys-view', 'Backup keys')}</HeaderTitle>

                <FlexColumn gap="medium">
                  <Text variant="bodySmall" color="whiteAlpha80">
                    {messages.pgettext(
                      'keys-view',
                      'Your 12-word mnemonic is the ONLY way to restore your Warren identity on another device. If lost, your subscription is unrecoverable.',
                    )}
                  </Text>

                  <Callout $tone="warning">
                    <Text variant="bodySmallSemibold" color="white">
                      {messages.pgettext(
                        'keys-view',
                        'Write it down on paper. Never store it in a cloud, screenshot, or email.',
                      )}
                    </Text>
                  </Callout>

                  {error && (
                    <Text variant="bodySmall" color="red">
                      {error}
                    </Text>
                  )}

                  {!revealed && !error && (
                    <Button variant="destructive" onClick={onReveal}>
                      <Button.Text>{messages.pgettext('keys-view', 'Reveal mnemonic')}</Button.Text>
                    </Button>
                  )}

                  {revealed && mnemonic && (
                    <FlexColumn gap="medium">
                      <MnemonicGrid mnemonic={mnemonic} revealed />

                      <CopyMnemonicButton mnemonic={mnemonic} data-testid="keys-mnemonic-copy" />

                      <Checkbox checked={confirmed} onCheckedChange={setConfirmed}>
                        <Flex gap="small" alignItems="center">
                          <Checkbox.Trigger>
                            <Checkbox.Input />
                          </Checkbox.Trigger>
                          <Checkbox.Label>
                            {messages.pgettext(
                              'keys-view',
                              'I have written it down in a safe place.',
                            )}
                          </Checkbox.Label>
                        </Flex>
                      </Checkbox>

                      <Button variant="success" disabled={!confirmed} onClick={onDone}>
                        <Button.Text>{messages.pgettext('keys-view', 'Done')}</Button.Text>
                      </Button>
                    </FlexColumn>
                  )}

                  <Button onClick={onRestore}>
                    <Button.Text>
                      {messages.pgettext('keys-view', 'Restore from mnemonic')}
                    </Button.Text>
                  </Button>
                </FlexColumn>
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
