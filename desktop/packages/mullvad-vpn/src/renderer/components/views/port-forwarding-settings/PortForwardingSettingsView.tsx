import { messages } from '../../../../shared/gettext';
import {
  PortForwardingAdvanced,
  PortForwardingSetting,
  PortForwardingStatus,
} from '../../../features/port-forwarding/components';
import { usePortForwarding } from '../../../features/port-forwarding/hooks';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';

/**
 * Port-forwarding settings view (Warren differentiator: Mullvad +
 * IVPN dropped port-forwarding in 2023). Created from scratch since
 * upstream Mullvad removed its own equivalent at the same time.
 *
 * Layout mirrors the existing `MultihopSettingsView` pattern:
 * - Header + back action.
 * - Short explanatory paragraph.
 * - `PortForwardingSetting` row (toggle).
 * - `PortForwardingStatus` block (live public port + countdown when
 *   the refresh loop has produced its first mapping).
 */
export function PortForwardingSettingsView() {
  const { pop } = useHistory();
  // Advanced controls (protocol, preferred port) only matter when the
  // feature is enabled - keeps the screen visually quiet when the
  // user just landed and has the toggle off. Mirrors the
  // WarrenMultiHopSettingsView pattern (country pickers only appear
  // when multi-hop is enabled).
  const { settings } = usePortForwarding();

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={messages.pgettext('port-forwarding-view', 'Port forwarding')}
          />

          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                <HeaderTitle>
                  {messages.pgettext('port-forwarding-view', 'Port forwarding')}
                </HeaderTitle>
                <FlexColumn gap="large">
                  <FlexColumn gap="small">
                    <Text variant="labelTiny" color="whiteAlpha60">
                      {messages.pgettext(
                        'port-forwarding-view',
                        'Opens a public port on the connected exit and forwards it to your device. Useful for peer-to-peer applications, file sharing, or self-hosted services.',
                      )}
                    </Text>
                  </FlexColumn>
                  <PortForwardingSetting />
                  {settings.enabled ? <PortForwardingAdvanced /> : null}
                  <PortForwardingStatus />
                </FlexColumn>
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
