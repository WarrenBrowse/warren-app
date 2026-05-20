import React from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';

// M5.B.3 step 3: subscription pointer. Warren is paid (~7-10
// EUR/mo). We do **not** embed an iframe to warrenbrowse.com/pricing;
// the link opens in the user's default browser so the SPA UI is not
// coupled to the marketing page lifecycle (the page changes on every
// pricing tier review).
//
// Future: if the user is already enrolled (a daemon-side
// `is_enrolled()` check via the existing enrollment-tokens API
// returns true), this view auto-advances. For now the user can
// click "I already have a subscription" to skip ahead.
export function OnboardingSubscriptionView() {
  const { push } = useHistory();
  const alreadyHave = React.useCallback(() => push(RoutePath.onboardingPreferences), [push]);
  const skip = React.useCallback(() => push(RoutePath.main), [push]);
  return (
    <View backgroundColor="darkBlue">
      <View.Content>
        <View.Container horizontalMargin="medium" flexDirection="column" gap="large">
          <Text variant="titleBig" color="white">
            {messages.pgettext('warren-onboarding', 'Your subscription')}
          </Text>
          <Text variant="bodySmall" color="whiteAlpha80">
            {messages.pgettext(
              'warren-onboarding',
              "You don't have an active subscription yet. Plans start at a few euros per month - no recurring billing, no account creation, pay as you go.",
            )}
          </Text>
          <FlexColumn gap="medium">
            <a
              href="https://warrenbrowse.com/pricing"
              target="_blank"
              rel="noopener noreferrer"
              data-testid="onboarding-subscription-link">
              {messages.pgettext('warren-onboarding', 'View plans (opens in your browser)')}
            </a>
            <button
              type="button"
              onClick={alreadyHave}
              data-testid="onboarding-subscription-already-have">
              {messages.pgettext('warren-onboarding', 'I already have a subscription')}
            </button>
            <button type="button" onClick={skip} data-testid="onboarding-subscription-skip">
              {messages.pgettext('warren-onboarding', 'Skip wizard (advanced)')}
            </button>
          </FlexColumn>
        </View.Container>
      </View.Content>
    </View>
  );
}
