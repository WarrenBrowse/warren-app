import { sprintf } from 'sprintf-js';

import { type AppRoute, type SplitModeAvailability } from '../../../../../../shared/app-routing';
import { strings } from '../../../../../../shared/constants';
import { messages } from '../../../../../../shared/gettext';

// The two routes an app can take besides a country, named the same way on the
// "Other apps" switch, the option cards and the chips.
export function throughVpnLabel(): string {
  // TRANSLATORS: A route in App routing: the app uses the VPN.
  return messages.pgettext('split-tunneling-view', 'Through the VPN');
}

export function outsideVpnLabel(): string {
  // TRANSLATORS: A route in App routing: the app connects directly, without
  // TRANSLATORS: the VPN.
  return messages.pgettext('split-tunneling-view', 'Outside the VPN');
}

// Why "Outside the VPN" cannot be chosen, as the line under the "Other apps"
// switch. Undefined when it can, or when the view shows the Full Disk Access
// steps instead.
export function outsideUnavailableNote(
  availability: SplitModeAvailability,
  platform: NodeJS.Platform,
): string | undefined {
  switch (availability) {
    case 'needs-signed-build':
      return messages.pgettext(
        'split-tunneling-view',
        'Outside the VPN needs a signed build of Warren VPN.',
      );
    case 'needs-newer-macos':
      return messages.pgettext('split-tunneling-view', 'Outside the VPN needs macOS 13 or newer.');
    case 'unsupported':
      return platform === 'linux'
        ? sprintf(
            // TRANSLATORS: Information about split tunneling being unavailable due to
            // TRANSLATORS: missing support in the user's operating system.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(splitTunneling)s - will be replaced with Split tunneling
            messages.pgettext(
              'split-tunneling-view',
              'To use %(splitTunneling)s, please update to a Linux kernel version that supports cgroup v2.',
            ),
            { splitTunneling: strings.splitTunneling },
          )
        : messages.pgettext(
            'split-tunneling-view',
            'Your system cannot send apps outside the VPN.',
          );
    case 'available':
    case 'checking':
    case 'needs-full-disk-access':
      return undefined;
  }
}

// The same reason, short enough for the subtitle of an option card.
export function outsideUnavailableReason(availability: SplitModeAvailability): string | undefined {
  switch (availability) {
    case 'needs-signed-build':
      return messages.pgettext('split-tunneling-view', 'Needs a signed build of Warren VPN');
    case 'needs-full-disk-access':
      return messages.pgettext('split-tunneling-view', 'Needs Full Disk Access');
    case 'needs-newer-macos':
      return messages.pgettext('split-tunneling-view', 'Needs macOS 13 or newer');
    case 'unsupported':
      return messages.pgettext('split-tunneling-view', 'Not available on this system');
    case 'available':
    case 'checking':
      return undefined;
  }
}

// A route in words, for the accessible name of a rule.
export function routeDescription(route: AppRoute, exitLabel: string | undefined): string {
  switch (route.kind) {
    case 'vpn':
      return throughVpnLabel();
    case 'direct':
      return outsideVpnLabel();
    case 'country':
      return exitLabel ?? route.exit.country.toUpperCase();
  }
}

export function neverWithoutVpnLabel(): string {
  // TRANSLATORS: Switch on the route of an app: the app is blocked whenever
  // TRANSLATORS: the VPN does not carry it, so it never connects without it.
  return messages.pgettext('split-tunneling-view', 'Never without the VPN');
}

// A rule in words, for its accessible name: its route, and its lock.
export function ruleDescription(
  route: AppRoute,
  exitLabel: string | undefined,
  locked: boolean,
): string {
  const description = routeDescription(route, exitLabel);
  return locked
    ? sprintf(
        // TRANSLATORS: Accessibility description of a rule locked to the VPN.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(route)s - its route: "Through the VPN" or the country
        // TRANSLATORS: it leaves from
        messages.pgettext('split-tunneling-view', '%(route)s, never without the VPN'),
        { route: description },
      )
    : description;
}
