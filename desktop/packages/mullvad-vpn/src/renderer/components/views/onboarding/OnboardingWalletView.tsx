import React from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';

// M5.B.3 step 2: wallet bootstrap. Two paths:
// - Generate: ask the daemon for a fresh BIP39 mnemonic, display 12
//   words with a blur+click-to-reveal overlay (anti-shoulder-surf),
//   require explicit confirmation before advancing.
// - Import: 12-word textarea + BIP39 validation hook (offloaded to
//   the daemon to keep the wordlist out of the renderer).
//
// **No "Copy to clipboard" CTA**: the user must write the mnemonic
// down by hand, mitigating the malware-clipboard exfiltration risk.
// This is non-negotiable per the doctrine; do not regress on a UX
// review.
export function OnboardingWalletView() {
  const { push } = useHistory();
  const [mode, setMode] = React.useState<'pick' | 'generate' | 'import'>('pick');
  const [revealed, setRevealed] = React.useState(false);
  const pickGenerate = React.useCallback(() => setMode('generate'), []);
  const pickImport = React.useCallback(() => setMode('import'), []);
  const reveal = React.useCallback(() => setRevealed(true), []);
  const next = React.useCallback(() => push(RoutePath.onboardingSubscription), [push]);
  const skip = React.useCallback(() => push(RoutePath.main), [push]);

  // Wire-up to the daemon `generate_warren_mnemonic` /
  // `import_warren_mnemonic` IPC routes lands as a follow-up. The
  // current scaffold ships the UX shell so the routing + i18n strings
  // are reviewable end-to-end.
  const placeholderMnemonic =
    'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';

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
                {messages.pgettext('warren-onboarding', 'Generate a new wallet (recommended)')}
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
              <div
                role="textbox"
                aria-readonly="true"
                style={{
                  filter: revealed ? 'none' : 'blur(8px)',
                  cursor: 'pointer',
                  padding: 12,
                  border: '1px solid #888',
                }}
                onClick={reveal}
                data-testid="onboarding-mnemonic-blur">
                {placeholderMnemonic}
              </div>
              <button
                type="button"
                onClick={next}
                disabled={!revealed}
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
                data-testid="onboarding-mnemonic-input"
              />
              <button type="button" onClick={next} data-testid="onboarding-wallet-import-confirm">
                {messages.pgettext('warren-onboarding', 'Restore wallet')}
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
