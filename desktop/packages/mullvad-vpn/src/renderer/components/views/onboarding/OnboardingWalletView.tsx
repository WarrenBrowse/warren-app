import React from 'react';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';

// M5.B.3 step 2: wallet bootstrap. Two paths:
// - Generate: read the daemon's auto-bootstrapped mnemonic via
//   `getWarrenMnemonic` and display the 12 words behind a
//   click-to-reveal blur overlay (anti-shoulder-surf). The daemon
//   generates the identity on first run; this step lets the user
//   capture the backup before they ever connect.
// - Import: 12-word textarea, validated daemon-side via
//   `setWarrenMnemonic` (BIP39 wordlist + checksum lives in Rust to
//   keep the renderer surface small).
//
// **No "Copy to clipboard" CTA**: the user must write the mnemonic
// down by hand, mitigating the malware-clipboard exfiltration risk.
// This is non-negotiable per the doctrine; do not regress on a UX
// review.

function countWords(input: string): number {
  return input.split(/\s+/).filter((w) => w.length > 0).length;
}

export function OnboardingWalletView() {
  const { push } = useHistory();
  const { getWarrenMnemonic, setWarrenMnemonic } = useAppContext();

  const [mode, setMode] = React.useState<'pick' | 'generate' | 'import'>('pick');
  const [mnemonic, setMnemonic] = React.useState<string | null>(null);
  const [revealed, setRevealed] = React.useState(false);
  const [importInput, setImportInput] = React.useState('');
  const [submitting, setSubmitting] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const importWordCount = countWords(importInput);
  const importWordCountValid = importWordCount === 12 || importWordCount === 24;

  // Drop the secret from React memory when the user leaves the view
  // (matches the `KeysView` posture - mnemonic should not linger).
  React.useEffect(() => {
    return () => {
      setMnemonic(null);
      setRevealed(false);
      setImportInput('');
    };
  }, []);

  const pickGenerate = React.useCallback(async () => {
    setError(null);
    setMode('generate');
    try {
      const m = await getWarrenMnemonic();
      if (!m) {
        setError(
          messages.pgettext(
            'warren-onboarding',
            'Daemon has no mnemonic yet - log in or restart the app.',
          ),
        );
        return;
      }
      setMnemonic(m);
    } catch (e) {
      const err = e as Error;
      log.error(`getWarrenMnemonic failed: ${err.message}`);
      setError(messages.pgettext('warren-onboarding', 'Failed to retrieve mnemonic from daemon.'));
    }
  }, [getWarrenMnemonic]);

  const pickImport = React.useCallback(() => {
    setError(null);
    setMode('import');
  }, []);

  const reveal = React.useCallback(() => setRevealed(true), []);

  const onImportChange = React.useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => setImportInput(e.target.value),
    [],
  );

  const next = React.useCallback(() => push(RoutePath.onboardingSubscription), [push]);
  const skip = React.useCallback(() => push(RoutePath.main), [push]);

  const submitImport = React.useCallback(async () => {
    setError(null);
    setSubmitting(true);
    try {
      const normalized = importInput.trim().toLowerCase().split(/\s+/).join(' ');
      await setWarrenMnemonic(normalized);
      push(RoutePath.onboardingSubscription);
    } catch (e) {
      const err = e as Error;
      log.error(`setWarrenMnemonic failed: ${err.message}`);
      setError(
        messages.pgettext(
          'warren-onboarding',
          'Daemon rejected the mnemonic. Check spelling and word count, then try again.',
        ),
      );
    } finally {
      setSubmitting(false);
    }
  }, [importInput, setWarrenMnemonic, push]);

  return (
    <View backgroundColor="darkBlue">
      <View.Content>
        <View.Container horizontalMargin="medium" flexDirection="column" gap="large">
          <Text variant="titleBig" color="white">
            {messages.pgettext('warren-onboarding', 'Your Warren wallet')}
          </Text>
          {mode === 'pick' && (
            <FlexColumn gap="medium">
              <Text variant="bodySmall" color="whiteAlpha80">
                {messages.pgettext(
                  'warren-onboarding',
                  'Warren uses a non-custodial wallet (Ed25519 + BIP39). You own the keys; we never see them.',
                )}
              </Text>
              <button type="button" onClick={pickGenerate} data-testid="onboarding-wallet-generate">
                {messages.pgettext('warren-onboarding', 'Back up my new wallet (recommended)')}
              </button>
              <button type="button" onClick={pickImport} data-testid="onboarding-wallet-import">
                {messages.pgettext('warren-onboarding', 'Import an existing mnemonic')}
              </button>
            </FlexColumn>
          )}
          {mode === 'generate' && (
            <FlexColumn gap="medium">
              <Text variant="bodySmall" color="whiteAlpha80">
                {messages.pgettext(
                  'warren-onboarding',
                  'Write down these 12 words in order. If you lose them, you lose access to your subscription. Copy to clipboard is intentionally disabled to keep your secret away from malware.',
                )}
              </Text>
              {error && (
                <Text variant="bodySmall" color="red">
                  {error}
                </Text>
              )}
              {mnemonic && (
                <div
                  role="textbox"
                  aria-readonly="true"
                  style={{
                    filter: revealed ? 'none' : 'blur(8px)',
                    cursor: 'pointer',
                    padding: 12,
                    border: '1px solid #888',
                    fontFamily: "'Source Code Pro', Menlo, Consolas, monospace",
                  }}
                  onClick={reveal}
                  data-testid="onboarding-mnemonic-blur">
                  {mnemonic}
                </div>
              )}
              <button
                type="button"
                onClick={next}
                disabled={!revealed || !mnemonic}
                data-testid="onboarding-wallet-confirm">
                {messages.pgettext('warren-onboarding', 'I have written down the words')}
              </button>
            </FlexColumn>
          )}
          {mode === 'import' && (
            <FlexColumn gap="medium">
              <Text variant="bodySmall" color="whiteAlpha80">
                {messages.pgettext(
                  'warren-onboarding',
                  'Paste your 12-word BIP39 mnemonic to restore your existing Warren identity.',
                )}
              </Text>
              <textarea
                rows={3}
                placeholder="word1 word2 word3 ..."
                value={importInput}
                onChange={onImportChange}
                spellCheck={false}
                autoCapitalize="off"
                autoCorrect="off"
                data-testid="onboarding-mnemonic-input"
              />
              <Text variant="bodySmall" color="whiteAlpha60">
                {importWordCount}
                {' / 12 '}
                {messages.pgettext('warren-onboarding', 'words')}
              </Text>
              {error && (
                <Text variant="bodySmall" color="red">
                  {error}
                </Text>
              )}
              <button
                type="button"
                onClick={submitImport}
                disabled={!importWordCountValid || submitting}
                data-testid="onboarding-wallet-import-confirm">
                {submitting
                  ? messages.pgettext('warren-onboarding', 'Restoring...')
                  : messages.pgettext('warren-onboarding', 'Restore wallet')}
              </button>
            </FlexColumn>
          )}
          <button type="button" onClick={skip}>
            {messages.pgettext('warren-onboarding', 'Skip wizard (advanced)')}
          </button>
        </View.Container>
      </View.Content>
    </View>
  );
}
