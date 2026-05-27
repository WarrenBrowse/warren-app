import React from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { Button } from '../../../lib/components';
import { useHistory } from '../../../lib/history';
import { OnboardingLayout } from './components';

// M5.B.3 step 1: welcome banner. Sets the tone "Warren VPN, no
// compromise privacy" and routes the user into the wallet step.
// First-launch detection (`onboardingCompletedUnix` undefined)
// dispatches the user here from the boot route; users replaying the
// wizard from Settings land here too.
export function OnboardingWelcomeView() {
  const { push } = useHistory();
  const next = React.useCallback(() => push(RoutePath.onboardingWallet), [push]);

  return (
    <OnboardingLayout
      title={messages.pgettext('warren-onboarding', 'Welcome to Warren VPN')}
      description={messages.pgettext(
        'warren-onboarding',
        'A VPN experience without compromise on privacy. No logs. No accounts. No tracking. Just bandwidth.',
      )}
      showBackAction={false}
      actions={
        <Button variant="success" onClick={next} data-testid="onboarding-welcome-next">
          <Button.Text>{messages.pgettext('warren-onboarding', 'Get started')}</Button.Text>
        </Button>
      }
    />
  );
}
