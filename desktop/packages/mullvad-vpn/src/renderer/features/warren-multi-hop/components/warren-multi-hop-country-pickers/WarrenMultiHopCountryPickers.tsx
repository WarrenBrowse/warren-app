import { useMemo } from 'react';

import { messages, relayLocations } from '../../../../../shared/gettext';
import { SettingsListbox } from '../../../../components/settings-listbox';
import { useSelector } from '../../../../redux/store';
import { useWarrenMultiHop } from '../../hooks';

// Relay-list driven entry / exit country pickers for Warren two-relay
// multi-hop. Replaces the old free-text ISO-code inputs: the options
// come from the live relay list (`getRelayLocations`), so a user can
// only pin a country that actually has a Warren relay. Empty string =
// auto-pick. A daemon restart is required to apply (see the view note).
//
// Rendered only while multi-hop is enabled (picking a hop country is
// meaningless otherwise). The entry and exit hops draw from the same
// Warren relay list.

// Listbox value '' means "Automatic". The daemon stores the ISO 3166
// alpha-2 code lowercased; the relay-list `code` is already lowercase.
const AUTOMATIC = '';

function useCountryOptions() {
  const countries = useSelector((state) => state.settings.relayLocations);
  return useMemo(
    () =>
      countries
        .map((country) => ({ code: country.code, name: relayLocations.gettext(country.name) }))
        .sort((a, b) => a.name.localeCompare(b.name)),
    [countries],
  );
}

function CountryListbox({
  label,
  value,
  onValueChange,
  anchorId,
}: {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  anchorId: 'warren-multi-hop-entry-country' | 'warren-multi-hop-exit-country';
}) {
  const options = useCountryOptions();

  return (
    <SettingsListbox anchorId={anchorId} value={value} onValueChange={onValueChange}>
      <SettingsListbox.Header>
        <SettingsListbox.Header.Item>
          <SettingsListbox.Header.Item.Label>{label}</SettingsListbox.Header.Item.Label>
        </SettingsListbox.Header.Item>
      </SettingsListbox.Header>
      <SettingsListbox.Options>
        <SettingsListbox.Options.BaseOption value={AUTOMATIC}>
          {messages.gettext('Automatic')}
        </SettingsListbox.Options.BaseOption>
        {options.map((option) => (
          <SettingsListbox.Options.BaseOption key={option.code} value={option.code}>
            {option.name}
          </SettingsListbox.Options.BaseOption>
        ))}
      </SettingsListbox.Options>
    </SettingsListbox>
  );
}

export function WarrenMultiHopCountryPickers() {
  const { warrenMultiHop, setEntryCountry, setExitCountry } = useWarrenMultiHop();

  if (!warrenMultiHop.enabled) {
    return null;
  }

  return (
    <>
      <CountryListbox
        anchorId="warren-multi-hop-entry-country"
        label={messages.pgettext('warren-multi-hop-view', 'Entry country')}
        value={warrenMultiHop.entryCountry}
        onValueChange={setEntryCountry}
      />
      <CountryListbox
        anchorId="warren-multi-hop-exit-country"
        label={messages.pgettext('warren-multi-hop-view', 'Exit country')}
        value={warrenMultiHop.exitCountry}
        onValueChange={setExitCountry}
      />
    </>
  );
}
