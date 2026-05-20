import React from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { useWarrenMultiHop } from '../../hooks';

// Two text inputs for the ISO 3166 alpha-2 entry / exit country codes.
// Empty string = auto-pick from the relay list. The richer country
// dropdown (driven by /v1/relays) is M4.H.C.X follow-up; this minimal
// surface is sufficient for power users to pin the route countries
// without blocking on the relay-list endpoint.
const StyledInput = styled.input({
  background: 'transparent',
  border: 'none',
  borderBottom: '1px solid rgba(255,255,255,0.4)',
  color: 'white',
  fontFamily: 'inherit',
  fontSize: '14px',
  padding: '4px 0',
  width: '4ch',
  textTransform: 'uppercase',
  '&:focus': {
    outline: 'none',
    borderBottomColor: 'white',
  },
});

const StyledRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  width: '100%',
});

function CountryRow({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  const handleChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => onChange(e.target.value.toLowerCase()),
    [onChange],
  );
  return (
    <StyledRow>
      <Text variant="bodySmall">{label}</Text>
      <StyledInput
        value={value}
        maxLength={2}
        placeholder={placeholder}
        aria-label={label}
        onChange={handleChange}
      />
    </StyledRow>
  );
}

export function WarrenMultiHopCountryPickers() {
  const { warrenMultiHop, setEntryCountry, setExitCountry } = useWarrenMultiHop();

  return (
    <SettingsListItem anchorId="warren-multi-hop-country-pickers">
      <SettingsListItem.Item>
        <FlexColumn gap="small" style={{ width: '100%' }}>
          <CountryRow
            label={messages.pgettext('warren-multi-hop-view', 'Entry country')}
            value={warrenMultiHop.entryCountry}
            onChange={setEntryCountry}
            placeholder={messages.pgettext('warren-multi-hop-view', 'auto')}
          />
          <CountryRow
            label={messages.pgettext('warren-multi-hop-view', 'Exit country')}
            value={warrenMultiHop.exitCountry}
            onChange={setExitCountry}
            placeholder={messages.pgettext('warren-multi-hop-view', 'auto')}
          />
        </FlexColumn>
      </SettingsListItem.Item>
    </SettingsListItem>
  );
}
