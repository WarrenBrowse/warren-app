import { messages } from '../../../../shared/gettext';
import { usePop } from '../../../history/hooks';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { AppNavigationHeader } from '../../';
import { BackAction } from '../../keyboard-navigation';
import { SettingsNavigationScrollbars } from '../../Layout';
import { NavigationContainer } from '../../NavigationContainer';
import {
  ApiAccessMethodsListItem,
  AppInfoListItem,
  DaitaListItem,
  DebugListItem,
  PortForwardingListItem,
  QuitButton,
  ReplayOnboardingListItem,
  SplitTunnelingListItem,
  SupportListItem,
  UserInterfaceSettingsListItem,
  VpnSettingsListItem,
  WarrenMultiHopListItem,
} from './components';
// Upstream `MultihopListItem` is intentionally **not** rendered:
// it surfaces the WireGuard multi-hop constraint, which is dead
// weight for Warren (Warren tunnels run over QUIC, not
// WireGuard). `WarrenMultiHopListItem` is the only multi-hop entry
// users see - its label is rendered as plain "Multihop" since the
// Warren context is implicit (the host app *is* Warren VPN). The
// upstream component is kept around in the `components` barrel so a
// future rebase against Mullvad doesn't dirty-diff the import.
import { useShowDebug, useShowSplitTunneling, useShowSubSettings } from './hooks';

export function SettingsView() {
  const pop = usePop();

  const showSubSettings = useShowSubSettings();
  const showSplitTunneling = useShowSplitTunneling();
  const showDebug = useShowDebug();

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={
              // TRANSLATORS: Title label in navigation bar
              messages.pgettext('settings-view', 'Settings')
            }
            titleVisible
          />

          <SettingsNavigationScrollbars fillContainer>
            <View.Content>
              <View.Container horizontalMargin="medium" gap="large" flexDirection="column">
                <FlexColumn gap="medium">
                  {showSubSettings ? (
                    <>
                      <FlexColumn>
                        <DaitaListItem />
                        <WarrenMultiHopListItem />
                        <PortForwardingListItem />
                        <VpnSettingsListItem />
                        <UserInterfaceSettingsListItem />
                      </FlexColumn>
                      {showSplitTunneling && <SplitTunnelingListItem position="solo" />}
                    </>
                  ) : (
                    <UserInterfaceSettingsListItem position="solo" />
                  )}

                  <ApiAccessMethodsListItem position="solo" />

                  <FlexColumn>
                    <SupportListItem />
                    <AppInfoListItem />
                  </FlexColumn>

                  <ReplayOnboardingListItem position="solo" />

                  {showDebug && <DebugListItem />}
                </FlexColumn>
                <QuitButton />
              </View.Container>
            </View.Content>
          </SettingsNavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
