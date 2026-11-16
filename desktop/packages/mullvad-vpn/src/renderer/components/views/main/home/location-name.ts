import { sprintf } from 'sprintf-js';

import { ICustomList } from '../../../../../shared/daemon-rpc-types';
import { messages, relayLocations } from '../../../../../shared/gettext';
import {
  IRelayLocationCountryRedux,
  RelaySettingsRedux,
} from '../../../../redux/settings/reducers';

// The name of the location the user picked, as the location row shows it while
// no tunnel is up.
export function getRelayName(
  relaySettings: RelaySettingsRedux,
  customLists: Array<ICustomList>,
  locations: IRelayLocationCountryRedux[],
): string {
  if ('normal' in relaySettings) {
    const location = relaySettings.normal.location;

    if (location === 'any') {
      return messages.gettext('Automatic');
    } else if ('customList' in location) {
      return (
        customLists.find((list) => list.id === location.customList)?.name ??
        messages.gettext('Unknown')
      );
    } else if ('hostname' in location) {
      const country = locations.find(({ code }) => code === location.country);
      const city = country?.cities.find(({ code }) => code === location.city);
      if (city) {
        return sprintf(
          // TRANSLATORS: The selected location label displayed on the main view, when a user selected a specific host to connect to.
          // TRANSLATORS: Example: Malmö (se-mma-001)
          // TRANSLATORS: Available placeholders:
          // TRANSLATORS: %(city)s - a city name
          // TRANSLATORS: %(hostname)s - a hostname
          messages.pgettext('connect-container', '%(city)s (%(hostname)s)'),
          { city: relayLocations.gettext(city.name), hostname: location.hostname },
        );
      }
    } else if ('city' in location) {
      const country = locations.find(({ code }) => code === location.country);
      const city = country?.cities.find(({ code }) => code === location.city);
      if (city) {
        return relayLocations.gettext(city.name);
      }
    } else if ('country' in location) {
      const country = locations.find(({ code }) => code === location.country);
      if (country) {
        return relayLocations.gettext(country.name);
      }
    }

    return messages.gettext('Unknown');
  } else if (relaySettings.customTunnelEndpoint) {
    return messages.gettext('Custom');
  } else {
    throw new Error('Unsupported relay settings.');
  }
}

// The ISO code of the country the user picked, when the pick names one.
export function selectedCountryCode(relaySettings: RelaySettingsRedux): string | undefined {
  if ('normal' in relaySettings) {
    const location = relaySettings.normal.location;
    if (location !== 'any' && 'country' in location) {
      return location.country;
    }
  }
  return undefined;
}

// The exit, country first, as the location row shows it once a tunnel is up.
export function exitPlaceName(country?: string, city?: string): string {
  if (!country) {
    return '';
  }
  const localCountry = relayLocations.gettext(country);
  return city && city !== country
    ? sprintf(
        // TRANSLATORS: The exit location under the connection status. Available placeholders:
        // TRANSLATORS: %(country)s - the country of the exit, e.g. Sweden
        // TRANSLATORS: %(city)s - its city, e.g. Gothenburg
        messages.pgettext('connect-view', '%(country)s, %(city)s'),
        { country: localCountry, city: relayLocations.gettext(city) },
      )
    : localCountry;
}
