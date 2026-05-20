import { messages } from '../../../../shared/gettext';
import {
  PortForwardingSetting,
  PortForwardingStatus,
} from '../../../features/port-forwarding/components';
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
                        'Warren restores port-forwarding: a unique differentiator since Mullvad and IVPN removed this feature in 2023. Toggle below to ask the exit for a public port mapped to your device.',
                      )}
                    </Text>
                  </FlexColumn>
                  <PortForwardingSetting />
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
