import React from 'react';

import { messages } from '../../../../../shared/gettext';
import { RoutePath } from '../../../../../shared/routes';
import { useAppContext } from '../../../../context';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { Link } from '../../../../lib/components/link';
import { View } from '../../../../lib/components/view';
import { useHistory } from '../../../../lib/history';
import { AppNavigationHeader } from '../../..';
import { BackAction } from '../../../keyboard-navigation';
import { NavigationContainer } from '../../../NavigationContainer';
import { NavigationScrollbars } from '../../../NavigationScrollbars';

export type OnboardingLayoutProps = React.PropsWithChildren<{
  // Title shown both in the navigation bar (large header) and as the H1
  // inside the page body once the user scrolls past the hero area.
  title: string;
  // Short tagline rendered under the page title. Optional - omit for
  // screens that put their introduction inside `children`.
  description?: React.ReactNode;
  // Action buttons shown in a sticky footer at the bottom of the view.
  // Pass `Button` components from `lib/components` here.
  actions?: React.ReactNode;
  // When true (default) the user can skip the rest of the wizard via a
  // discreet link below the actions. Disable for the final "Done" step
  // where skipping has no meaning.
  allowSkip?: boolean;
  // When true (default) wraps the view in `BackAction` so the nav-bar
  // back button is shown and the keyboard Escape shortcut pops the
  // route. Disable for the welcome step where there is nothing to go
  // back to.
  showBackAction?: boolean;
}>;

// Shared chrome for the Warren onboarding wizard. Mirrors the
// `MultihopSettingsView` layout (View > BackAction > NavigationContainer
// > AppNavigationHeader + NavigationScrollbars > View.Content) so the
// onboarding screens are visually indistinguishable from the rest of
// Settings - same dark-blue background, same scrollbar, same nav-bar
// chevron, same focus ring.
export function OnboardingLayout({
  title,
  description,
  actions,
  children,
  allowSkip = true,
  showBackAction = true,
}: OnboardingLayoutProps) {
  const { push, pop } = useHistory();
  const { setOnboardingCompletedUnix } = useAppContext();
  // Skipping the wizard marks the onboarding as completed in
  // `IGuiSettingsState` so the next boot does NOT route the user
  // back to the welcome step. Without this side effect, every
  // restart re-opens the wizard because `getNavigationBase` only
  // unblocks `RoutePath.main` when `onboardingCompletedUnix` is set.
  // The user can replay the wizard manually via
  // Settings → "Replay onboarding" (which clears the timestamp).
  const skip = React.useCallback(() => {
    setOnboardingCompletedUnix(Math.floor(Date.now() / 1000));
    push(RoutePath.main);
  }, [setOnboardingCompletedUnix, push]);

  const body = (
    <NavigationContainer>
      <AppNavigationHeader title={title} />
      <NavigationScrollbars>
        <View.Content>
          <View.Container horizontalMargin="medium" flexDirection="column" gap="large">
            <FlexColumn gap="small">
              <Text as="h1" variant="titleBig" color="white">
                {title}
              </Text>
              {description && (
                <Text variant="bodySmall" color="whiteAlpha80">
                  {description}
                </Text>
              )}
            </FlexColumn>
            {children}
          </View.Container>
        </View.Content>
        {(actions || allowSkip) && (
          <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
            {actions && <FlexColumn gap="medium">{actions}</FlexColumn>}
            {allowSkip && (
              <FlexColumn alignItems="center" gap="small">
                <Link as="button" onClick={skip} data-testid="onboarding-skip">
                  <Link.Text variant="labelTinySemiBold">
                    {messages.pgettext('warren-onboarding', 'Skip wizard (advanced)')}
                  </Link.Text>
                </Link>
              </FlexColumn>
            )}
          </View.Container>
        )}
      </NavigationScrollbars>
    </NavigationContainer>
  );

  return (
    <View backgroundColor="darkBlue">
      {showBackAction ? <BackAction action={pop}>{body}</BackAction> : body}
    </View>
  );
}
