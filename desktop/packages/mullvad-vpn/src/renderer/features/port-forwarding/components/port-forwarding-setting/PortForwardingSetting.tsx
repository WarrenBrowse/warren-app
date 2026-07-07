import { messages } from '../../../../../shared/gettext';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { PortForwardingSwitch } from '../port-forwarding-switch/PortForwardingSwitch';

/**
 * Row that wraps the port-forwarding toggle inside a `SettingsListItem`
 * (the same shell every other settings row uses). Lives at the top of
 * the port-forwarding view so the toggle is the first thing the user
 * sees.
 */
export function PortForwardingSetting() {
  return (
    <SettingsListItem anchorId="port-forwarding-setting">
      <SettingsListItem.Item>
        {/* inputId wires the label to the checkbox: clicking the label toggles
            it natively. Stretching the label to fill the row makes the whole
            empty area clickable, with no JS and no double-toggle. */}
        <PortForwardingSwitch inputId="port-forwarding-enable">
          <PortForwardingSwitch.Label
            style={{ flex: 1, alignSelf: 'stretch', display: 'flex', alignItems: 'center' }}>
            {messages.gettext('Enable')}
          </PortForwardingSwitch.Label>
          <SettingsListItem.Item.ActionGroup>
            <PortForwardingSwitch.Input />
          </SettingsListItem.Item.ActionGroup>
        </PortForwardingSwitch>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
