import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';

// M5.B.3 step 4: privacy preferences. Three Warren-specific toggles
// surfaced as part of the first-run flow so the user understands the
// stack from the start:
//
// - **Multi-hop** (M4.E.D): OFF default - costs ~half the
//   single-hop bandwidth in exchange for entry-exit unlinkability.
// - **DAITA v2** (M5.B.1): OFF default - ~5-15% bandwidth overhead
//   in exchange for traffic-analysis fingerprinting resistance.
// - **Always-on obfuscation** (M4.0): ON default - HTTP/3 mimicry,
//   no bandwidth cost, no reason to disable in nominal use.
//
// The toggles can also be changed later from Settings; this view is
// just a guided introduction. Wired to existing Mullvad
// upstream/Warren hooks rather than introducing duplicate state.
export function OnboardingPreferencesView() {
  const { push } = useHistory();
  return (
    <View backgroundColor="darkBlue">
      <View.Content>
        <View.Container horizontalMargin="medium" flexDirection="column" gap="large">
          <Text variant="titleBig" color="white">
            {messages.pgettext('warren-onboarding', 'Privacy preferences')}
          </Text>
          <Text variant="bodySmall" color="whiteAlpha80">
            {messages.pgettext(
              'warren-onboarding',
              'Pick the defenses you want from day one. You can change all of these later from Settings.',
            )}
          </Text>
          <FlexColumn gap="medium">
            {/* The actual toggles are reused from `features/warren-multi-hop`, `features/daita`, and `features/warren-mode`. Embed them here once the wizard ships with the runtime IPC bindings. The scaffold below documents the planned layout. */}
            <Text variant="labelTiny" color="whiteAlpha60">
              {messages.pgettext(
                'warren-onboarding',
                '• Multi-hop (OFF by default): route through two relays for entry/exit unlinkability.',
              )}
            </Text>
            <Text variant="labelTiny" color="whiteAlpha60">
              {messages.pgettext(
                'warren-onboarding',
                '• DAITA v2 (OFF by default): inject padding to defeat ML traffic analysis. ~5-15% bandwidth.',
              )}
            </Text>
            <Text variant="labelTiny" color="whiteAlpha60">
              {messages.pgettext(
                'warren-onboarding',
                '• Always-on obfuscation (ON by default): HTTP/3 mimicry so the wire blends in.',
              )}
            </Text>
            <button
              type="button"
              onClick={() => push(RoutePath.onboardingDone)}
              data-testid="onboarding-preferences-next"
            >
              {messages.pgettext('warren-onboarding', 'Continue')}
            </button>
            <button
              type="button"
              onClick={() => push(RoutePath.main)}
              data-testid="onboarding-preferences-skip"
            >
              {messages.pgettext('warren-onboarding', 'Skip wizard (advanced)')}
            </button>
          </FlexColumn>
        </View.Container>
      </View.Content>
    </View>
  );
}
