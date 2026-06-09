import { messages } from '../../../../../../shared/gettext';
import { RoutePath } from '../../../../../../shared/routes';
import { ListItemProps } from '../../../../../lib/components/list-item';
import { SettingsNavigationListItem } from '../../../../settings-navigation-list-item';
import { useIsOn } from './hooks';

export type PortForwardingListItemProps = Omit<ListItemProps, 'children'>;

// Settings entry that navigates to the Warren port-forwarding view
// (NAT-PMP via the connected exit). Warren-specific differentiator:
// upstream Mullvad + IVPN dropped port-forwarding in 2023. The view
// itself was implemented but had no entry point in the Settings list
// - this component closes that gap. State pill ("On" / "Off") reads
// `Settings::warren_nat_pmp.enabled` from redux so the user can see
// the toggle state without entering the sub-view.
export function PortForwardingListItem(props: PortForwardingListItemProps) {
  const isOn = useIsOn();

  return (
    <SettingsNavigationListItem to={RoutePath.portForwardingSettings} {...props}>
      <SettingsNavigationListItem.Item>
        <SettingsNavigationListItem.Item.Label>
          {messages.pgettext('settings-view', 'Port forwarding')}
        </SettingsNavigationListItem.Item.Label>
        <SettingsNavigationListItem.Item.ActionGroup>
          <SettingsNavigationListItem.Item.Text>
            {isOn ? messages.gettext('On') : messages.gettext('Off')}
          </SettingsNavigationListItem.Item.Text>
          <SettingsNavigationListItem.Item.Icon icon="chevron-right" />
        </SettingsNavigationListItem.Item.ActionGroup>
      </SettingsNavigationListItem.Item>
    </SettingsNavigationListItem>
  );
}
