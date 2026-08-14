import React from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button } from '../../../lib/components';
import { useHistory } from '../../../lib/history';
import { OnboardingLayout } from './components';

// Done. Clears the onboarding gate in the GUI settings so the wizard is not
// re-shown on the next launch (unless the user explicitly chooses "Replay
// onboarding" from Settings). Routes the user to `main` so they can pick a
// country and connect, with `reset` rather than `push` so the wizard does not
// stay under the main view as the navigation-stack root (see OnboardingLayout).
export function OnboardingDoneView() {
  const { reset } = useHistory();
  const { setOnboardingPending } = useAppContext();

  const handleFinish = React.useCallback(() => {
    setOnboardingPending(false);
    reset(RoutePath.main);
  }, [setOnboardingPending, reset]);

  return (
    <OnboardingLayout
      title={messages.pgettext('warren-onboarding', 'All set')}
      description={messages.pgettext(
        'warren-onboarding',
        'Configuration complete. Pick a country and connect to Warren.',
      )}
      allowSkip={false}
      actions={
        <Button variant="success" onClick={handleFinish} data-testid="onboarding-done-finish">
          <Button.Text>
            {messages.pgettext('warren-onboarding', 'Pick a country and connect')}
          </Button.Text>
        </Button>
      }
    />
  );
}
