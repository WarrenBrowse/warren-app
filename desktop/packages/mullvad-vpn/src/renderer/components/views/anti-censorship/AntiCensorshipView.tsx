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

export function AntiCensorshipView() {
  const { pop } = useHistory();
  // Warren's HTTP/3 mimicry obfuscation (ALPN h3, per-exit SNI under
  // .exits.warrenbrowse.com, Initial split, UDP 443) is always-on per
  // `warren_obfuscation_doctrine_v1`, so this view is an info-only
  // indicator, never a toggle: the user can verify it is active but
  // cannot disable it accidentally.

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
                <WarrenObfuscationIndicator />
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}

// The state is pulled from `warrenStatus.obfuscationActive` (pushed by
// the daemon WarrenStatusUpdates stream); a missing snapshot defaults to
// true so the UI never flashes an alarming OFF state during boot.
function WarrenObfuscationIndicator() {
  const warrenStatus = useSelector((state) => state.settings.warrenStatus);
  const active = warrenStatus?.obfuscationActive ?? true;
  return (
    <>
      <Text variant="labelTinySemiBold" color={active ? 'white' : 'whiteAlpha60'}>
        {active
          ? // TRANSLATORS: Status line confirming traffic obfuscation is on.
            messages.pgettext('warren-status-view', 'Obfuscation is active.')
          : // TRANSLATORS: Status line shown if traffic obfuscation is off.
            messages.pgettext('warren-status-view', 'Obfuscation is currently inactive.')}
      </Text>
      <Text variant="labelTiny" color="whiteAlpha60">
        {
          // TRANSLATORS: Explains what obfuscation does and why it cannot be
          // TRANSLATORS: turned off.
          messages.pgettext(
            'warren-status-view',
            'To evade censorship, Warren disguises the tunnel as ordinary web browsing: on the network it looks like standard browser traffic to a regular website (HTTP/3 on port 443). Obfuscation cannot be turned off, as traffic that stands out is traffic that can be blocked.',
          )
        }
      </Text>
    </>
  );
}
