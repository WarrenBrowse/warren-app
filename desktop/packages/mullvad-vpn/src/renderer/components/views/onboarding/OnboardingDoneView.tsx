import React from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button } from '../../../lib/components';
import { useHistory } from '../../../lib/history';
import { OnboardingLayout } from './components';

// Done. Persists the `onboardingCompletedUnix`
// timestamp in the GUI settings via `setOnboardingCompletedUnix` so
// the wizard is not re-shown on the next launch (unless the user
// explicitly chooses "Replay onboarding" from Settings, which clears
// the timestamp). Routes the user to `main` so they can pick a
// country and connect.
export function OnboardingDoneView() {
  const { push } = useHistory();
  const { setOnboardingCompletedUnix } = useAppContext();

  const handleFinish = React.useCallback(() => {
    setOnboardingCompletedUnix(Math.floor(Date.now() / 1000));
    push(RoutePath.main);
  }, [setOnboardingCompletedUnix, push]);

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
