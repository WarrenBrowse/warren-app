import React from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';

// M5.B.3 step 5: done. Persists the `onboardingCompletedUnix`
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
    <View backgroundColor="darkBlue">
      <View.Content>
        <View.Container horizontalMargin="medium" flexDirection="column" gap="large">
          <Text variant="titleBig" color="white">
            {messages.pgettext('warren-onboarding', 'All set')}
          </Text>
          <Text variant="bodySmall" color="whiteAlpha80">
            {messages.pgettext(
              'warren-onboarding',
              'Configuration complete. Pick a country and connect to Warren.',
            )}
          </Text>
          <FlexColumn gap="medium">
            <button type="button" onClick={handleFinish} data-testid="onboarding-done-finish">
              {messages.pgettext('warren-onboarding', 'Pick a country and connect')}
            </button>
          </FlexColumn>
        </View.Container>
      </View.Content>
    </View>
  );
}
