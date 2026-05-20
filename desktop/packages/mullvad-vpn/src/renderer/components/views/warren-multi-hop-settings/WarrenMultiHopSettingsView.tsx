import { messages } from '../../../../shared/gettext';
import {
  WarrenMultiHopCountryPickers,
  WarrenMultiHopSetting,
} from '../../../features/warren-multi-hop';
import { useWarrenMultiHop } from '../../../features/warren-multi-hop/hooks';
import { Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';

// Dedicated Warren two-relayed QUIC multi-hop settings view (M4.E.D).
// Doctrine `warren_multihop_doctrine_v1`: opt-in privacy with the
// Apple Private Relay two-hop HPKE pattern. Toggle defaults to OFF
// because single-hop is materially faster and the entry-side
// unlinkability that multi-hop buys is only worth it for users who
// actively want it.
export function WarrenMultiHopSettingsView() {
  const { pop } = useHistory();
  const { warrenMultiHop } = useWarrenMultiHop();

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={messages.pgettext('warren-multi-hop-view', 'Warren multi-hop')}
          />

          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                <HeaderTitle>
                  {messages.pgettext('warren-multi-hop-view', 'Warren multi-hop')}
                </HeaderTitle>
                <FlexColumn gap="large">
                  <FlexColumn gap="small">
                    <Text variant="labelTiny" color="whiteAlpha60">
                      {messages.pgettext(
                        'warren-multi-hop-view',
                        'Routes your traffic through two Warren relays using HPKE end-to-end encryption. The entry relay only sees ciphertext; the exit only sees the decrypted payload, never your IP. Pattern inspired by Apple Private Relay.',
                      )}
                    </Text>
                    <Text variant="labelTiny" color="whiteAlpha60">
                      {messages.pgettext(
                        'warren-multi-hop-view',
                        'Trade-off: roughly half the single-hop bandwidth. Off by default; opt-in for privacy. Daemon restart required after changing this setting.',
                      )}
                    </Text>
                  </FlexColumn>
                  <WarrenMultiHopSetting />
                  {warrenMultiHop.enabled ? (
                    <FlexColumn gap="small">
                      <Text variant="labelTiny" color="whiteAlpha60">
                        {messages.pgettext(
                          'warren-multi-hop-view',
                          'Optional: pin the entry and exit relay countries (ISO 3166 alpha-2 codes such as fr, de, se). Leave empty for auto-pick.',
                        )}
                      </Text>
                      <WarrenMultiHopCountryPickers />
                    </FlexColumn>
                  ) : (
                    <Text variant="labelTiny" color="whiteAlpha60">
                      {messages.pgettext(
                        'warren-multi-hop-view',
                        'Single-hop Warren active. Full bandwidth, identical privacy guarantees as the single-hop default mode.',
                      )}
                    </Text>
                  )}
                </FlexColumn>
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
