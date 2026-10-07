import { sprintf } from 'sprintf-js';

import { AppRouteLine } from '../../../shared/app-routing';
import { ISplitTunnelingApplication } from '../../../shared/application-types';
import { messages } from '../../../shared/gettext';

// Why an app of the Linux list cannot be given a country.
export function routingLimitationText(
  limitation: NonNullable<ISplitTunnelingApplication['routingLimitation']>,
): string {
  switch (limitation) {
    case 'flatpak':
      // TRANSLATORS: Line under a Flatpak app in the "Country per app" list.
      return messages.pgettext('split-tunneling-view', 'Flatpak apps cannot use a country yet');
    case 'snap':
      // TRANSLATORS: Line under a Snap app in the "Country per app" list.
      return messages.pgettext('split-tunneling-view', 'Snap apps cannot use a country yet');
    case 'script':
      // TRANSLATORS: Line under an app started by a launcher script, which
      // TRANSLATORS: Warren cannot follow. "Find another app" is the button
      // TRANSLATORS: at the bottom of the list.
      return messages.pgettext(
        'split-tunneling-view',
        'Opens through a script: pick its program with Find another app',
      );
  }
}

export function appRouteLineText(line: AppRouteLine): string {
  switch (line.kind) {
    case 'blocked':
      // TRANSLATORS: Status under an app locked to the VPN while the VPN is
      // TRANSLATORS: not connected: the app has no Internet meanwhile.
      return messages.pgettext('split-tunneling-view', 'Blocked until the VPN connects');
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
          // TRANSLATORS: Every anonymous session token of the account is in
          // TRANSLATORS: use by its other connections, so this per-app
          // TRANSLATORS: connection cannot get one.
          return messages.pgettext('split-tunneling-view', 'Session limit reached');
        case 'waiting-for-route':
          // TRANSLATORS: The server runs a limited number of per-app
          // TRANSLATORS: connections at once and all of them are in use. This
          // TRANSLATORS: one starts by itself as soon as one is free.
          return messages.pgettext('split-tunneling-view', 'Waiting for a free route');
        case 'no-relay':
          return messages.pgettext('split-tunneling-view', 'No server there');
        case 'no-dialable-network':
          // TRANSLATORS: None of the entry servers this per-app connection
          // TRANSLATORS: may use answers on the address type (IPv4 or IPv6)
          // TRANSLATORS: of the network the device is on.
          return messages.pgettext('split-tunneling-view', 'This network reaches no entry server');
        case undefined:
          return messages.pgettext('split-tunneling-view', 'Unavailable');
      }
  }
}
