import React from 'react';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Checkbox, Spinner, Text } from '../../../lib/components';
import { Flex } from '../../../lib/components/flex';
import { FlexColumn } from '../../../lib/components/flex-column';
import { useHistory } from '../../../lib/history';
import { CopyMnemonicButton, MnemonicGrid } from '../../warren-mnemonic';
import { OnboardingLayout } from './components';

// Recovery-phrase backup reminder.
//
// By the time the wizard runs the user is ALREADY logged in with a
// Warren identity: it was either minted (with a mandatory backup step)
// or restored from a phrase, both in `LoginView`. So this step does NOT
// create or import anything anymore, since that would re-display the
// current identity behind a "create" label and read as "it recreated the
// same account". It is purely a second chance to capture the 12 words before
// connecting: read the active mnemonic via `getWarrenMnemonic`, show it,
// and gate "Continue" behind an explicit "I saved it" acknowledgement.

export function OnboardingWalletView() {
  const { push } = useHistory();
  const { getWarrenMnemonic } = useAppContext();

  const [mnemonic, setMnemonic] = React.useState<string | null>(null);
  const [confirmed, setConfirmed] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const m = await getWarrenMnemonic();
        if (cancelled) {
          return;
        }
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
        if (cancelled) {
          return;
        }
        const err = e as Error;
        log.error(`getWarrenMnemonic failed: ${err.message}`);
        setError(
          messages.pgettext('warren-onboarding', 'Failed to retrieve mnemonic from daemon.'),
        );
      }
    })();

    // Drop the secret from React memory when leaving the view (matches
    // the `KeysView` posture - the mnemonic should not linger).
    return () => {
      cancelled = true;
      setMnemonic(null);
      setConfirmed(false);
    };
  }, [getWarrenMnemonic]);

  const next = React.useCallback(() => push(RoutePath.onboardingSubscription), [push]);

  const actions = (
    <Button
      variant="success"
      onClick={next}
      disabled={!confirmed || !mnemonic}
      data-testid="onboarding-wallet-confirm">
      <Button.Text>{messages.pgettext('warren-onboarding', 'Continue')}</Button.Text>
    </Button>
  );

  return (
    <OnboardingLayout
      title={messages.pgettext('warren-onboarding', 'Your Warren wallet')}
      description={messages.pgettext(
        'warren-onboarding',
        'Write down these 12 words in order and keep them somewhere safe. They are the only way to restore your subscription if you lose access to this device.',
      )}
      actions={actions}>
      <FlexColumn gap="medium">
        {error && (
          <Text variant="bodySmall" color="red">
            {error}
          </Text>
        )}
        {!error && !mnemonic && <Spinner />}
        {mnemonic && (
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
      </FlexColumn>
    </OnboardingLayout>
  );
}
