import { sprintf } from 'sprintf-js';

import { AppRouteLine, MAX_APP_EXITS, SplitModeAvailability } from '../../../shared/app-routing';
import { messages } from '../../../shared/gettext';

export function appRouteLineText(line: AppRouteLine): string {
  switch (line.kind) {
    case 'paused':
      // TRANSLATORS: Status under an app whose country is saved while the
      // TRANSLATORS: "Country per app" switch is off.
      return messages.pgettext('split-tunneling-view', 'Off, uses the main connection');
    case 'bypassed':
      // TRANSLATORS: Status under an app that has a country but is also in
      // TRANSLATORS: the "Bypass VPN" list, which wins.
      return messages.pgettext('split-tunneling-view', 'Bypasses the VPN');
    case 'waiting':
      return messages.pgettext('split-tunneling-view', 'Waiting for the VPN');
    case 'connecting':
      return messages.pgettext('split-tunneling-view', 'Connecting...');
    case 'connected':
      return line.publicIp === undefined
        ? messages.pgettext('split-tunneling-view', 'Connected')
        : sprintf(
            // TRANSLATORS: Status under an app with its own country once its
            // TRANSLATORS: connection is up. Available placeholders:
            // TRANSLATORS: %(ip)s - the public IP address the app appears from
            messages.pgettext('split-tunneling-view', 'Connected, IP %(ip)s'),
            { ip: line.publicIp },
          );
    case 'unavailable':
      switch (line.reason) {
        case 'tunnel-down':
          return messages.pgettext('split-tunneling-view', 'Waiting for the VPN');
        case 'no-token':
          // TRANSLATORS: The per-app connection needs one of the anonymous
          // TRANSLATORS: session tokens and none is left for now.
          return messages.pgettext('split-tunneling-view', 'No session token left');
        case 'limit-reached':
          return messages.pgettext('split-tunneling-view', 'Country limit reached');
        case 'no-relay':
          return messages.pgettext('split-tunneling-view', 'No server there');
        case undefined:
          return messages.pgettext('split-tunneling-view', 'Unavailable');
      }
  }
}

export function appExitLimitText(): string {
  return sprintf(
    // TRANSLATORS: Shown in the country picker once the limit is reached.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(limit)d - the number of countries apps can use at once
    messages.pgettext(
      'split-tunneling-view',
      'Apps can use %(limit)d countries at a time. Pick one already in use, or remove one first.',
    ),
    { limit: MAX_APP_EXITS },
  );
}

// One line for a split mode the device cannot run, or undefined when the view
// shows something richer (the Full Disk Access steps) or nothing at all.
export function splitModeUnavailableText(availability: SplitModeAvailability): string | undefined {
  switch (availability) {
    case 'needs-signed-build':
      return messages.pgettext(
        'split-tunneling-view',
        'This build of Warren VPN cannot do this. It needs a signed build.',
      );
    case 'needs-newer-macos':
      return messages.pgettext('split-tunneling-view', 'This needs macOS 13 or newer.');
    case 'unsupported':
      return messages.pgettext('split-tunneling-view', 'Your system does not support this.');
    case 'available':
    case 'checking':
    case 'needs-full-disk-access':
      return undefined;
  }
}
