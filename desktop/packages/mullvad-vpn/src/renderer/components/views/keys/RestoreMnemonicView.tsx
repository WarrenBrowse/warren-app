import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
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
import { countMnemonicWords, MnemonicTextarea, normalizeMnemonic } from '../../warren-mnemonic';

const DangerCallout = styled.div`
  padding: ${spacings.small} ${spacings.medium};
  border-radius: ${Radius.radius4};
  background-color: ${colors.redAlpha40};
  border: 1px solid ${colors.red40};
`;

export function RestoreMnemonicView() {
  const history = useHistory();
  const { setWarrenMnemonic } = useAppContext();

  const [input, setInput] = useState('');
  const [confirmed, setConfirmed] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Drop the pasted recovery phrase from React memory when leaving the
  // view (it is a secret; do not let it linger after navigation).
  useEffect(() => {
    return () => setInput('');
  }, []);

  const wordCount = countMnemonicWords(input);
  const wordCountValid = wordCount === 12 || wordCount === 24;
  const canSubmit = wordCountValid && confirmed && !submitting && !success;

  const onSubmit = useCallback(async () => {
    setError(null);
    setSubmitting(true);
    try {
      await setWarrenMnemonic(normalizeMnemonic(input));
      setSuccess(true);
    } catch (e) {
      const err = e as Error;
      log.error(`setWarrenMnemonic failed: ${err.message}`);
      setError(
        messages.pgettext(
          'keys-view',
          'Daemon rejected the mnemonic. Check spelling, word count, and try again.',
        ),
      );
    } finally {
      setSubmitting(false);
    }
  }, [input, setWarrenMnemonic]);

  const onDone = useCallback(() => history.pop(), [history]);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={history.pop}>
        <NavigationContainer>
          <AppNavigationHeader title={messages.pgettext('keys-view', 'Restore identity')} />

          <NavigationScrollbars>
            <View.Content>
              <View.Container flexDirection="column" horizontalMargin="medium" gap="medium">
                <HeaderTitle>{messages.pgettext('keys-view', 'Restore from mnemonic')}</HeaderTitle>

                {success ? (
                  <FlexColumn gap="medium">
                    <Text variant="bodySmall" color="whiteAlpha80">
                      {messages.pgettext(
                        'keys-view',
                        'Identity restored. Your new Warren wallet is now active.',
                      )}
                    </Text>
                    <Button variant="success" onClick={onDone}>
                      <Button.Text>{messages.pgettext('keys-view', 'Done')}</Button.Text>
                    </Button>
                  </FlexColumn>
                ) : (
                  <FlexColumn gap="medium">
                    <DangerCallout>
                      <Text variant="bodySmallSemibold" color="white">
                        {messages.pgettext(
                          'keys-view',
                          'WARNING: This will REPLACE your current identity. Any subscription tied to the current identity will be unrecoverable.',
                        )}
                      </Text>
                    </DangerCallout>

                    <Text variant="bodySmall" color="whiteAlpha80">
                      {messages.pgettext(
                        'keys-view',
                        'Enter your 12-word (or 24-word) BIP39 mnemonic, separated by spaces.',
                      )}
                    </Text>

                    <MnemonicTextarea
                      value={input}
                      onValueChange={setInput}
                      placeholder="abandon abandon abandon ... about"
                    />

                    <Checkbox checked={confirmed} onCheckedChange={setConfirmed}>
                      <Flex gap="small" alignItems="flex-start">
                        <Checkbox.Trigger>
                          <Checkbox.Input />
                        </Checkbox.Trigger>
                        <Checkbox.Label>
                          {messages.pgettext(
                            'keys-view',
                            'I understand this will permanently replace my current identity.',
                          )}
                        </Checkbox.Label>
                      </Flex>
                    </Checkbox>

                    {error && (
                      <Text variant="bodySmall" color="red">
                        {error}
                      </Text>
                    )}

                    <Button variant="destructive" disabled={!canSubmit} onClick={onSubmit}>
                      <Button.Text>
                        {submitting
                          ? messages.pgettext('keys-view', 'Restoring...')
                          : messages.pgettext('keys-view', 'Restore identity')}
                      </Button.Text>
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
