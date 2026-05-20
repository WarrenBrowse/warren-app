import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';

// M5.B.3 step 1: welcome banner. Sets the tone "Warren VPN, no
// compromise privacy" and routes the user into the wallet step.
// First-launch detection (`onboardingCompletedUnix` undefined)
// dispatches the user here from the boot route; users replaying the
// wizard from Settings land here too.
export function OnboardingWelcomeView() {
  const { push } = useHistory();
  return (
    <View backgroundColor="darkBlue">
      <View.Content>
        <View.Container horizontalMargin="medium" flexDirection="column" gap="large">
          <Text variant="titleBig" color="white">
            {messages.pgettext('warren-onboarding', 'Welcome to Warren VPN')}
          </Text>
          <Text variant="bodySmall" color="whiteAlpha80">
            {messages.pgettext(
              'warren-onboarding',
              'A VPN experience without compromise on privacy. No logs. No accounts. No tracking. Just bandwidth.',
            )}
          </Text>
          <FlexColumn gap="medium">
            <button
              type="button"
              onClick={() => push(RoutePath.onboardingWallet)}
              data-testid="onboarding-welcome-next"
            >
              {messages.pgettext('warren-onboarding', 'Get started')}
            </button>
            <button
              type="button"
              onClick={() => push(RoutePath.main)}
              data-testid="onboarding-welcome-skip"
            >
              {messages.pgettext('warren-onboarding', 'Skip wizard (advanced)')}
            </button>
          </FlexColumn>
        </View.Container>
      </View.Content>
    </View>
  );
}
