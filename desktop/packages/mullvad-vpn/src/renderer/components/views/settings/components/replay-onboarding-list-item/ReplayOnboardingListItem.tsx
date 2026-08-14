import React from 'react';

import { messages } from '../../../../../../shared/gettext';
import { RoutePath } from '../../../../../../shared/routes';
import { useAppContext } from '../../../../../context';
import { useHistory } from '../../../../../lib/history';
import { SettingsListItem, SettingsListItemProps } from '../../../../settings-list-item';

export type ReplayOnboardingListItemProps = Omit<SettingsListItemProps, 'children'>;

// Settings entry that raises the onboarding gate and navigates back to the
// wizard's first step. Reachable from the Settings home view so a user can
// re-run the wizard on demand. The flag is raised *before* the navigation so
// the redirect logic in `getNavigationBase` would still send the user to the
// wizard even if the navigation push race-loses with a redux store update.
export function ReplayOnboardingListItem(props: ReplayOnboardingListItemProps) {
  const { setOnboardingPending } = useAppContext();
  const history = useHistory();

  const handleReplay = React.useCallback(() => {
    setOnboardingPending(true);
    history.push(RoutePath.onboardingWelcome);
  }, [setOnboardingPending, history]);

  return (
    <SettingsListItem {...props}>
      <SettingsListItem.Trigger onClick={handleReplay}>
        <SettingsListItem.Item>
          <SettingsListItem.Item.Label>
            {messages.pgettext('settings-view', 'Replay onboarding')}
          </SettingsListItem.Item.Label>
          <SettingsListItem.Item.ActionGroup>
            <SettingsListItem.Item.Icon icon="chevron-right" />
          </SettingsListItem.Item.ActionGroup>
        </SettingsListItem.Item>
      </SettingsListItem.Trigger>
    </SettingsListItem>
  );
}
