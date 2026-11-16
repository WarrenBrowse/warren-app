import { sprintf } from 'sprintf-js';

import { TunnelState } from '../../../../../shared/daemon-rpc-types';
import { messages, relayLocations } from '../../../../../shared/gettext';
import type { ConnectionPhase } from '../../../../lib/connection-phase';

export interface HomeLocation {
  country?: string;
  city?: string;
  hostname?: string;
}

export interface HomeStatus {
  pill: string;
  title: string;
  subtitle: string;
  // The line under a protected title is the exit host, set in the mono face.
  subtitleIsHostname: boolean;
}

function pillLabel(phase: ConnectionPhase): string {
  switch (phase) {
    case 'exposed':
      // TRANSLATORS: Short state badge on the home screen while the traffic
      // TRANSLATORS: leaves the device unencrypted.
      return messages.pgettext('tunnel-control', 'Unprotected');
    case 'connecting':
      return messages.gettext('Connecting');
    case 'protected':
      // TRANSLATORS: Short state badge on the home screen once the tunnel is up.
      return messages.pgettext('tunnel-control', 'Protected');
    case 'interrupted':
      // TRANSLATORS: Short state badge on the home screen while the tunnel is
      // TRANSLATORS: up but the device has no internet connection.
      return messages.pgettext('tunnel-control', 'Interrupted');
    case 'blocked':
      // TRANSLATORS: Short state badge on the home screen while the kill switch
      // TRANSLATORS: holds all traffic.
      return messages.pgettext('tunnel-control', 'Blocked');
  }
}

// City first, the way a place is named on a ticket: "Helsinki, Finland".
function placeName({ country, city }: HomeLocation): string {
  if (!country) {
    return '';
  }
  const localCountry = relayLocations.gettext(country);
  return city && city !== country
    ? sprintf(messages.pgettext('tray-icon-tooltip', '%(city)s, %(country)s'), {
        city: relayLocations.gettext(city),
        country: localCountry,
      })
    : localCountry;
}

const visible = () => messages.pgettext('tunnel-control', 'You are visible');
const notEncrypted = () => messages.pgettext('tunnel-control', 'Your connection is not encrypted');
const inProgress = () => messages.pgettext('tunnel-control', 'Connection in progress');

function status(
  pill: string,
  title: string,
  subtitle: string,
  subtitleIsHostname = false,
): HomeStatus {
  return { pill, title, subtitle, subtitleIsHostname };
}

export function getHomeStatus(
  tunnelState: TunnelState,
  phase: ConnectionPhase,
  location: HomeLocation,
  includeOnly: boolean,
): HomeStatus {
  const pill = pillLabel(phase);

  switch (phase) {
    case 'exposed':
      return status(pill, visible(), notEncrypted());
    case 'blocked':
      // TRANSLATORS: Title on the home screen while the kill switch holds all
      // TRANSLATORS: traffic, so nothing leaks but nothing flows.
      return status(pill, messages.pgettext('tunnel-control', 'Connection blocked'), '');
    case 'interrupted':
      return status(
        pill,
        messages.pgettext('tunnel-control', 'Connection interrupted'),
        messages.pgettext('tunnel-control', 'You are protected'),
      );
    case 'connecting': {
      if (tunnelState.state === 'disconnecting') {
        return status(pill, visible(), messages.pgettext('tunnel-control', 'Disconnecting...'));
      }
      const place = placeName(location);
      return place ? status(pill, place, inProgress()) : status(pill, inProgress(), '');
    }
    case 'protected': {
      const place = placeName(location) || messages.pgettext('tunnel-control', 'You are protected');
      if (includeOnly) {
        // TRANSLATORS: Secondary line shown below the status title while only
        // TRANSLATORS: the apps chosen in "VPN only for" use the VPN.
        return status(
          pill,
          place,
          messages.pgettext('tunnel-control', 'Only selected apps are protected'),
        );
      }
      return location.hostname
        ? status(pill, place, location.hostname, true)
        : status(pill, place, messages.pgettext('tunnel-control', 'You are protected'));
    }
  }
}
