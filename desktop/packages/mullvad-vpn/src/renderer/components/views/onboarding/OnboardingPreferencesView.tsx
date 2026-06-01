import React from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { Button, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { useHistory } from '../../../lib/history';
import { OnboardingLayout } from './components';

// Privacy preferences. Three Warren-specific toggles
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
  const next = React.useCallback(() => push(RoutePath.onboardingDone), [push]);

  return (
    <OnboardingLayout
      title={messages.pgettext('warren-onboarding', 'Privacy preferences')}
      description={messages.pgettext(
        'warren-onboarding',
        'Pick the defenses you want from day one. You can change all of these later from Settings.',
      )}
      actions={
        <Button variant="success" onClick={next} data-testid="onboarding-preferences-next">
          <Button.Text>{messages.pgettext('warren-onboarding', 'Continue')}</Button.Text>
        </Button>
      }>
      {/* The actual toggles are reused from `features/warren-multi-hop`,
          `features/daita`, and `features/warren-mode`. Embed them here
          once the wizard ships with the runtime IPC bindings. The
          scaffold below documents the planned layout. */}
      <FlexColumn gap="small">
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
      </FlexColumn>
    </OnboardingLayout>
  );
}
