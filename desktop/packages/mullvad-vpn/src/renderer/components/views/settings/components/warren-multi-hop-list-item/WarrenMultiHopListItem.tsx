import { messages } from '../../../../../../shared/gettext';
import { RoutePath } from '../../../../../../shared/routes';
import { ListItemProps } from '../../../../../lib/components/list-item';
import { SettingsNavigationListItem } from '../../../../settings-navigation-list-item';
import { useIsOn } from './hooks';

export type WarrenMultiHopListItemProps = Omit<ListItemProps, 'children'>;

// Settings entry that navigates to the Warren two-relayed QUIC
// multi-hop view (M4.E.D). Distinct from the upstream `MultihopListItem`
// which targets the WireGuard multi-hop constraint.
export function WarrenMultiHopListItem(props: WarrenMultiHopListItemProps) {
  const isOn = useIsOn();

  return (
    <SettingsNavigationListItem to={RoutePath.warrenMultiHopSettings} {...props}>
      <SettingsNavigationListItem.Item>
        <SettingsNavigationListItem.Item.Label>
          {messages.pgettext('settings-view', 'Warren multi-hop')}
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
