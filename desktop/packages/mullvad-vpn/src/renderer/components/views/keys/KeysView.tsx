import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import ClipboardLabel from '../../ClipboardLabel';
import { BackAction } from '../../keyboard-navigation';
import { HeaderTitle } from '../../SettingsHeader';

/**
 * Displays the BIP39 mnemonic (12 words) in a 3x4 grid layout.
 * Internal component — not reused elsewhere.
 */
const StyledMnemonicGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  padding: 12px;
  border-radius: 8px;
  background-color: rgba(0, 0, 0, 0.18);
  font-family: 'Source Code Pro', Menlo, Consolas, monospace;
  font-size: 14px;
`;

const StyledWord = styled.div`
  display: flex;
  gap: 6px;
  padding: 4px 6px;
  align-items: baseline;

  & > .index {
    color: rgba(255, 255, 255, 0.55);
    min-width: 18px;
    text-align: right;
    font-size: 11px;
  }
`;

interface MnemonicGridProps {
  mnemonic: string;
}

function MnemonicGrid({ mnemonic }: MnemonicGridProps) {
  const words = mnemonic.split(/\s+/).filter((w) => w.length > 0);
  return (
    <StyledMnemonicGrid>
      {words.map((word, idx) => (
        <StyledWord key={idx}>
          <span className="index">{idx + 1}.</span>
          <span>{word}</span>
        </StyledWord>
      ))}
    </StyledMnemonicGrid>
  );
}

const StyledWarning = styled.div`
  padding: 12px;
  border-radius: 6px;
  background-color: rgba(255, 200, 50, 0.15);
  border: 1px solid rgba(255, 200, 50, 0.4);
`;

const StyledCheckbox = styled.label`
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  padding: 8px 0;
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
          // TRANSLATORS: Shown when the daemon has no Warren BIP39 mnemonic
          // TRANSLATORS: stored (= identity never bootstrapped). User must
          // TRANSLATORS: log in or restore from mnemonic first.
          messages.pgettext(
            'keys-view',
            'No mnemonic available — log in or restore an identity first.',
          ),
        );
        return;
      }
      setMnemonic(m);
      setRevealed(true);
    } catch (e) {
      const err = e as Error;
      log.error(`getWarrenMnemonic failed: ${err.message}`);
      setError(
        // TRANSLATORS: Generic error when the daemon RPC fails. Shown
        // TRANSLATORS: only on transient connection issues.
        messages.pgettext('keys-view', 'Failed to retrieve mnemonic from daemon.'),
      );
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

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={history.pop}>
        <AppNavigationHeader
          title={
            // TRANSLATORS: Title label in navigation bar
            messages.pgettext('keys-view', 'Keys')
          }
        />

        <View.Content>
          <View.Container flexDirection="column" horizontalMargin="medium" gap="medium">
            <Text variant="titleBig">
              <HeaderTitle>{messages.pgettext('keys-view', 'Backup keys')}</HeaderTitle>
            </Text>

            <FlexColumn gap="medium">
              <Text variant="bodySmall">
                {messages.pgettext(
                  'keys-view',
                  'Your 12-word mnemonic is the ONLY way to restore your Warren identity on another device. If lost, your subscription is unrecoverable.',
                )}
              </Text>

              <StyledWarning>
                <Text variant="bodySmallSemibold">
                  {messages.pgettext(
                    'keys-view',
                    'Write it down on paper. Never store it in a cloud, screenshot, or email.',
                  )}
                </Text>
              </StyledWarning>

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
                  <MnemonicGrid mnemonic={mnemonic} />

                  <ClipboardLabel
                    value={mnemonic}
                    obscureValue={false}
                    displayValue={
                      messages.pgettext('keys-view', 'Copy mnemonic to clipboard') as string
                    }
                    message={messages.pgettext('keys-view', 'Mnemonic copied') as string}
                  />

                  <StyledCheckbox>
                    <input
                      type="checkbox"
                      checked={confirmed}
                      onChange={(e) => setConfirmed(e.target.checked)}
                    />
                    <Text variant="bodySmall">
                      {messages.pgettext('keys-view', 'I have written it down in a safe place.')}
                    </Text>
                  </StyledCheckbox>

                  <Button variant="success" disabled={!confirmed} onClick={() => history.pop()}>
                    <Button.Text>{messages.pgettext('keys-view', 'Done')}</Button.Text>
                  </Button>
                </FlexColumn>
              )}

              <Button onClick={() => history.push(RoutePath.restoreKeys)}>
                <Button.Text>{messages.pgettext('keys-view', 'Restore from mnemonic')}</Button.Text>
              </Button>
            </FlexColumn>
          </View.Container>
        </View.Content>
      </BackAction>
    </View>
  );
}
