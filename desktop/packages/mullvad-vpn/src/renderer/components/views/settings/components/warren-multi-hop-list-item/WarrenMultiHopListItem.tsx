import { messages } from '../../../../../../shared/gettext';
import { RoutePath } from '../../../../../../shared/routes';
import { ListItemProps } from '../../../../../lib/components/list-item';
import { SettingsNavigationListItem } from '../../../../settings-navigation-list-item';
import { useIsOn } from './hooks';

export type WarrenMultiHopListItemProps = Omit<ListItemProps, 'children'>;

// Settings entry that navigates to the Warren two-relayed QUIC
// multi-hop view. The legacy upstream `MultihopListItem`
// (WireGuard multi-hop constraint) is hidden from the Warren build -
// see `SettingsView.tsx` - so this is the only multi-hop entry the
// user sees. The label drops the "Warren" qualifier because the host
// app *is* Warren VPN; users see plain "Multihop" matching the
// upstream Mullvad label they may already be familiar with.
export function WarrenMultiHopListItem(props: WarrenMultiHopListItemProps) {
  const isOn = useIsOn();

  return (
    <SettingsNavigationListItem to={RoutePath.warrenMultiHopSettings} {...props}>
      <SettingsNavigationListItem.Item>
        <SettingsNavigationListItem.Item.Label>
          {messages.pgettext('settings-view', 'Multihop')}
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
