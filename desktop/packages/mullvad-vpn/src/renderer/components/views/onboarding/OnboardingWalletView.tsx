import React from 'react';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Spinner, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { useHistory } from '../../../lib/history';
import { countMnemonicWords, MnemonicGrid, MnemonicTextarea } from '../../warren-mnemonic';
import { OnboardingLayout } from './components';

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

export function OnboardingWalletView() {
  const { push } = useHistory();
  const { getWarrenMnemonic, setWarrenMnemonic } = useAppContext();

  const [mode, setMode] = React.useState<'pick' | 'generate' | 'import'>('pick');
  const [mnemonic, setMnemonic] = React.useState<string | null>(null);
  const [revealed, setRevealed] = React.useState(false);
  const [importInput, setImportInput] = React.useState('');
  const [submitting, setSubmitting] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const importWordCount = countMnemonicWords(importInput);
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

  const next = React.useCallback(() => push(RoutePath.onboardingSubscription), [push]);

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

  const description =
    mode === 'pick'
      ? messages.pgettext(
          'warren-onboarding',
          'Warren uses a non-custodial wallet (Ed25519 + BIP39). You own the keys; we never see them.',
        )
      : mode === 'generate'
        ? messages.pgettext(
            'warren-onboarding',
            'Write down these 12 words in order. If you lose them, you lose access to your subscription. Copy to clipboard is intentionally disabled to keep your secret away from malware.',
          )
        : messages.pgettext(
            'warren-onboarding',
            'Paste your 12-word BIP39 mnemonic to restore your existing Warren identity.',
          );

  let actions: React.ReactNode;
  if (mode === 'pick') {
    actions = (
      <>
        <Button variant="success" onClick={pickGenerate} data-testid="onboarding-wallet-generate">
          <Button.Text>
            {messages.pgettext('warren-onboarding', 'Back up my new wallet (recommended)')}
          </Button.Text>
        </Button>
        <Button variant="primary" onClick={pickImport} data-testid="onboarding-wallet-import">
          <Button.Text>
            {messages.pgettext('warren-onboarding', 'Import an existing mnemonic')}
          </Button.Text>
        </Button>
      </>
    );
  } else if (mode === 'generate') {
    actions = (
      <Button
        variant="success"
        onClick={next}
        disabled={!revealed || !mnemonic}
        data-testid="onboarding-wallet-confirm">
        <Button.Text>
          {messages.pgettext('warren-onboarding', 'I have written down the words')}
        </Button.Text>
      </Button>
    );
  } else {
    actions = (
      <Button
        variant="success"
        onClick={submitImport}
        disabled={!importWordCountValid || submitting}
        data-testid="onboarding-wallet-import-confirm">
        {submitting ? (
          <Spinner />
        ) : (
          <Button.Text>{messages.pgettext('warren-onboarding', 'Restore wallet')}</Button.Text>
        )}
      </Button>
    );
  }

  return (
    <OnboardingLayout
      title={messages.pgettext('warren-onboarding', 'Your Warren wallet')}
      description={description}
      actions={actions}>
      <FlexColumn gap="medium">
        {error && (
          <Text variant="bodySmall" color="red">
            {error}
          </Text>
        )}
        {mode === 'generate' && mnemonic && (
          <MnemonicGrid
            mnemonic={mnemonic}
            revealed={revealed}
            onClick={reveal}
            data-testid="onboarding-mnemonic-blur"
          />
        )}
        {mode === 'import' && (
          <MnemonicTextarea
            value={importInput}
            onValueChange={setImportInput}
            data-testid="onboarding-mnemonic-input"
          />
        )}
      </FlexColumn>
    </OnboardingLayout>
  );
}
