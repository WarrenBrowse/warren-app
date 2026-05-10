import { useCallback, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { Button, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { HeaderTitle } from '../../SettingsHeader';

const StyledTextarea = styled.textarea`
  width: 100%;
  min-height: 110px;
  padding: 10px;
  font-family: 'Source Code Pro', Menlo, Consolas, monospace;
  font-size: 14px;
  line-height: 1.5;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.25);
  background-color: rgba(0, 0, 0, 0.18);
  color: white;
  resize: vertical;

  &:focus {
    outline: 2px solid rgba(95, 173, 230, 0.7);
    border-color: rgba(95, 173, 230, 0.7);
  }
`;

const StyledDanger = styled.div`
  padding: 12px;
  border-radius: 6px;
  background-color: rgba(255, 80, 80, 0.18);
  border: 1px solid rgba(255, 80, 80, 0.5);
`;

const StyledCheckbox = styled.label`
  display: flex;
  align-items: flex-start;
  gap: 8px;
  cursor: pointer;
  padding: 8px 0;

  & > input {
    margin-top: 4px;
  }
`;

/**
 * Compte les mots non vides de l'input. Sert au feedback live "X / 12 words"
 * et au gating du bouton submit (= 12 ou 24 mots requis pour BIP39).
 */
function countWords(input: string): number {
  return input.split(/\s+/).filter((w) => w.length > 0).length;
}

export function RestoreMnemonicView() {
  const history = useHistory();
  const { setWarrenMnemonic } = useAppContext();

  const [input, setInput] = useState('');
  const [confirmed, setConfirmed] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const wordCount = countWords(input);
  const wordCountValid = wordCount === 12 || wordCount === 24;
  const canSubmit = wordCountValid && confirmed && !submitting && !success;

  const onSubmit = useCallback(async () => {
    setError(null);
    setSubmitting(true);
    try {
      const normalized = input.trim().toLowerCase().split(/\s+/).join(' ');
      await setWarrenMnemonic(normalized);
      setSuccess(true);
    } catch (e) {
      const err = e as Error;
      log.error(`setWarrenMnemonic failed: ${err.message}`);
      setError(
        // TRANSLATORS: Shown when the daemon rejects the mnemonic
        // TRANSLATORS: (typically: invalid BIP39 word, bad checksum,
        // TRANSLATORS: wrong word count). User must check + retry.
        messages.pgettext(
          'keys-view',
          'Daemon rejected the mnemonic. Check spelling, word count, and try again.',
        ),
      );
    } finally {
      setSubmitting(false);
    }
  }, [input, setWarrenMnemonic]);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={history.pop}>
        <AppNavigationHeader
          title={
            // TRANSLATORS: Title label in navigation bar
            messages.pgettext('keys-view', 'Restore identity')
          }
        />

        <View.Content>
          <View.Container flexDirection="column" horizontalMargin="medium" gap="medium">
            <Text variant="titleBig">
              <HeaderTitle>{messages.pgettext('keys-view', 'Restore from mnemonic')}</HeaderTitle>
            </Text>

            {success ? (
              <FlexColumn gap="medium">
                <StyledDanger>
                  <Text variant="bodySmallSemibold">
                    {messages.pgettext(
                      'keys-view',
                      'Identity restored. Restart the Warren VPN daemon to activate the new identity.',
                    )}
                  </Text>
                </StyledDanger>
                <Button variant="success" onClick={() => history.pop()}>
                  <Button.Text>{messages.pgettext('keys-view', 'Done')}</Button.Text>
                </Button>
              </FlexColumn>
            ) : (
              <FlexColumn gap="medium">
                <StyledDanger>
                  <Text variant="bodySmallSemibold">
                    {messages.pgettext(
                      'keys-view',
                      'WARNING — This will REPLACE your current identity. Any subscription tied to the current identity will be unrecoverable.',
                    )}
                  </Text>
                </StyledDanger>

                <Text variant="bodySmall">
                  {messages.pgettext(
                    'keys-view',
                    'Enter your 12-word (or 24-word) BIP39 mnemonic, separated by spaces.',
                  )}
                </Text>

                <StyledTextarea
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  spellCheck={false}
                  autoCapitalize="off"
                  autoCorrect="off"
                  placeholder="abandon abandon abandon ... about"
                />

                <Text variant="bodySmall">
                  {wordCount}
                  {' / 12 '}
                  {messages.pgettext('keys-view', 'words')}
                </Text>

                <StyledCheckbox>
                  <input
                    type="checkbox"
                    checked={confirmed}
                    onChange={(e) => setConfirmed(e.target.checked)}
                  />
                  <Text variant="bodySmall">
                    {messages.pgettext(
                      'keys-view',
                      'I understand this will permanently replace my current identity.',
                    )}
                  </Text>
                </StyledCheckbox>

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
      </BackAction>
    </View>
  );
}
