import { messages } from '../../../../shared/gettext';
import { AutoConnectSetting, AutoStartSetting } from '../../../features/client/components';
import { AllowExternalDnsSetting } from '../../../features/dns/components';
import { AllowLanSetting } from '../../../features/lan-sharing/components';
import {
  EnableIpv6Setting,
  LockdownModeSetting,
  QuantumResistantSetting,
} from '../../../features/tunnel/components';
// `warren-mode` settings (failover, API URL) are deliberately NOT
// surfaced in the UI: they are developer/self-hosting toggles whose
// defaults are the only sensible choice for end users. Power users can
// still override them via `WARREN_FAILOVER`, `WARREN_API_URL` env vars
// or by editing `/etc/warren-vpn/settings.json` directly.
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';
import {
  AntiCensorshipListItem,
  CustomDnsSettings,
  DnsBlockerSettings,
  IpOverrideSettings,
  IpVersionSetting,
  KillSwitchSetting,
  MtuSetting,
  ResetPinnedExitKeys,
} from './components';

export function VpnSettingsView() {
  const { pop } = useHistory();

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={
              // TRANSLATORS: Title label in navigation bar
              messages.pgettext('vpn-settings-view', 'VPN settings')
            }
          />

          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" gap="medium" flexDirection="column">
                <HeaderTitle>{messages.pgettext('vpn-settings-view', 'VPN settings')}</HeaderTitle>

                <FlexColumn gap="medium">
                  <FlexColumn>
                    <AutoStartSetting />
                    <AutoConnectSetting />
                  </FlexColumn>

                  <AllowLanSetting />

                  <FlexColumn gap="small">
                    <DnsBlockerSettings position="solo" />
                    <CustomDnsSettings position="solo" />
                    <AllowExternalDnsSetting position="solo" />
                  </FlexColumn>

                  <EnableIpv6Setting />
                  <FlexColumn>
                    <KillSwitchSetting />
                    <LockdownModeSetting />
                  </FlexColumn>
                  <AntiCensorshipListItem position="solo" />
                  <QuantumResistantSetting position="solo" />
                  <IpVersionSetting />
                  <MtuSetting />
                  <IpOverrideSettings position="solo" />
                  <ResetPinnedExitKeys position="solo" />
                </FlexColumn>
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
