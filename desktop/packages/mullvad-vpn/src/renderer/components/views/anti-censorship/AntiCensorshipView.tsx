import { messages } from '../../../../shared/gettext';
import { Text } from '../../../lib/components';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';
import { MethodSetting } from './components';

export function AntiCensorshipView() {
  const { pop } = useHistory();
  // When warren_mode is on, the Mullvad-specific obfuscation methods
  // (Shadowsocks, UDP-over-TCP, QUIC, LWO) do not apply: Warren uses
  // its own M4.0 HTTP/3 mimicry obfuscation (ALPN h3 + SNI
  // warrenbrowse.com + Initial split + UDP 443) which is always-on
  // per /v1 doctrine `warren_obfuscation_doctrine_v1`. We surface
  // this as an info-only indicator instead of a togglable picker so
  // the user can verify the obfuscation is active without being able
  // to disable it accidentally.
  const warrenMode = useSelector((state) => state.settings.warrenMode);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={messages.pgettext('anti-censorship-view', 'Anti-censorship')}
          />

          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                <HeaderTitle>
                  {
                    // TRANSLATORS: Page title for anti censorship settings view
                    messages.pgettext('anti-censorship-view', 'Anti-censorship')
                  }
                </HeaderTitle>
                {warrenMode ? (
                  <WarrenObfuscationIndicator />
                ) : (
                  <>
                    <Text variant="labelTiny" color="whiteAlpha60">
                      {
                        // TRANSLATORS: First paragraph of description text in anti-censorship view
                        messages.pgettext(
                          'anti-censorship-view',
                          'These methods may be useful in situations where you are blocked from reaching Mullvad. When "Automatic" is selected, the app will attempt all methods until one works.',
                        )
                      }
                    </Text>
                    <Text variant="labelTinySemiBold" color="whiteAlpha60">
                      {
                        // TRANSLATORS: Second paragraph of description text in anti-censorship view
                        messages.pgettext(
                          'anti-censorship-view',
                          'Please note that these methods do not improve performance, and may increase system utilization and battery consumption.',
                        )
                      }
                    </Text>

                    <MethodSetting />
                  </>
                )}
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}

// Warren M4.0 obfuscation: always-on /v1 info banner. The state is
// pulled from `warrenStatus.obfuscationActive` (pushed by the daemon
// WarrenStatusUpdates stream); a missing snapshot defaults to true so
// the UI never flashes an alarming OFF state during boot. This view
// intentionally has no toggle, per doctrine
// `warren_obfuscation_doctrine_v1`.
function WarrenObfuscationIndicator() {
  const warrenStatus = useSelector((state) => state.settings.warrenStatus);
  const active = warrenStatus?.obfuscationActive ?? true;
  return (
    <>
      <Text variant="labelTinySemiBold" color={active ? 'white' : 'whiteAlpha60'}>
        {active
          ? messages.pgettext(
              'warren-status-view',
              'HTTP/3 mimicry obfuscation is always-on for Warren tunnels (/v1).',
            )
          : messages.pgettext(
              'warren-status-view',
              'HTTP/3 mimicry obfuscation is currently inactive.',
            )}
      </Text>
      <Text variant="labelTiny" color="whiteAlpha60">
        {messages.pgettext(
          'warren-status-view',
          'Warren tunnels masquerade as standard browser HTTP/3 traffic: ALPN h3, SNI warrenbrowse.com, Initial-packet split, UDP/443. This is not togglable in /v1 because disabling it would make Warren clients immediately recognisable on the network.',
        )}
      </Text>
      <Text variant="labelTiny" color="whiteAlpha60">
        {messages.pgettext(
          'warren-status-view',
          'Legacy Mullvad obfuscation methods (Shadowsocks, UDP-over-TCP, QUIC, LWO) do not apply when warren_mode is on.',
        )}
      </Text>
    </>
  );
}
