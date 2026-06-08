import React from 'react';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Checkbox, Spinner, Text } from '../../../lib/components';
import { Flex } from '../../../lib/components/flex';
import { FlexColumn } from '../../../lib/components/flex-column';
import { useHistory } from '../../../lib/history';
import {
  CopyMnemonicButton,
  countMnemonicWords,
  MnemonicGrid,
  MnemonicTextarea,
  normalizeMnemonic,
} from '../../warren-mnemonic';
import { OnboardingLayout } from './components';

// Wallet bootstrap. Two paths:
// - Generate: read the daemon's auto-bootstrapped mnemonic via
//   `getWarrenMnemonic` and display the 12 words directly, with a
//   "Copy to clipboard" button. The daemon generates the identity on
//   first run; this step lets the user capture the backup before they
//   ever connect. A confirmation checkbox gates the "Continue" button so
//   the user explicitly acknowledges they have saved the words. This
//   mirrors the settings backup view (`KeysView`) so both mnemonic
//   screens share the same layout and affordances.
// - Import: 12-word textarea, validated daemon-side via
//   `setWarrenMnemonic` (BIP39 wordlist + checksum lives in Rust to
//   keep the renderer surface small).

export function OnboardingWalletView() {
  const { push } = useHistory();
  const { getWarrenMnemonic, setWarrenMnemonic } = useAppContext();

  const [mode, setMode] = React.useState<'pick' | 'generate' | 'import'>('pick');
  const [mnemonic, setMnemonic] = React.useState<string | null>(null);
  const [confirmed, setConfirmed] = React.useState(false);
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
      setConfirmed(false);
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

  const next = React.useCallback(() => push(RoutePath.onboardingSubscription), [push]);

  const submitImport = React.useCallback(async () => {
    setError(null);
    setSubmitting(true);
    try {
      await setWarrenMnemonic(normalizeMnemonic(importInput));
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
            'Write down these 12 words in order and keep them somewhere safe. They are the only way to restore your subscription if you lose access to this device.',
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
            {messages.pgettext('warren-onboarding', 'Create my new wallet (recommended)')}
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
        disabled={!confirmed || !mnemonic}
        data-testid="onboarding-wallet-confirm">
        <Button.Text>{messages.pgettext('warren-onboarding', 'Continue')}</Button.Text>
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
          <FlexColumn gap="medium">
            <MnemonicGrid mnemonic={mnemonic} revealed data-testid="onboarding-mnemonic-grid" />

            <CopyMnemonicButton mnemonic={mnemonic} data-testid="onboarding-mnemonic-copy" />

            <Checkbox checked={confirmed} onCheckedChange={setConfirmed}>
              <Flex gap="small" alignItems="center">
                <Checkbox.Trigger>
                  <Checkbox.Input />
                </Checkbox.Trigger>
                <Checkbox.Label>
                  {messages.pgettext(
                    'warren-onboarding',
                    'I have written it down in a safe place.',
                  )}
                </Checkbox.Label>
              </Flex>
            </Checkbox>
          </FlexColumn>
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
