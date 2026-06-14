import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import {
  WarrenMultiHopCountryPickers,
  WarrenMultiHopSetting,
} from '../../../features/warren-multi-hop';
import { Image, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';

// Reused from the upstream Mullvad `MultihopSettingsView`: the two-
// server hop diagram is concept-accurate for the Warren two-relay
// HPKE pattern (entry → exit, traffic encrypted end-to-end). Keeping
// the same asset means one less SVG to maintain and a familiar
// visual for users coming from upstream Mullvad.
const StyledIllustration = styled(Image)({
  width: '100%',
});

// Dedicated Warren two-relayed QUIC multi-hop settings view (M4.E.D).
// Doctrine `warren_multihop_doctrine_v1`: opt-in privacy with the
// Apple Private Relay two-hop HPKE pattern. Toggle defaults to OFF
// because single-hop is materially faster and the entry-side
// unlinkability that multi-hop buys is only worth it for users who
// actively want it.
export function WarrenMultiHopSettingsView() {
  const { pop } = useHistory();

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader title={messages.pgettext('warren-multi-hop-view', 'Multihop')} />

          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                <HeaderTitle>{messages.pgettext('warren-multi-hop-view', 'Multihop')}</HeaderTitle>
                <FlexColumn gap="large">
                  <FlexColumn gap="small">
                    <StyledIllustration source="multihop-illustration" />
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
                  <WarrenMultiHopCountryPickers />
                </FlexColumn>
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
