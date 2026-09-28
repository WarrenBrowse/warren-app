import styled from 'styled-components';

import { type AppRoute } from '../../../../../../shared/app-routing';
import { messages } from '../../../../../../shared/gettext';
import { CountryFlag } from '../../../../../features/app-routing/components';
import { OutsideGlyph, ShieldGlyph } from './glyphs';
import { outsideVpnLabel } from './strings';
import { pill } from './styles';

const StyledChip = styled.span({
  ...pill,
  flexShrink: 0,
  maxWidth: '100%',
  padding: '0 12px 0 6px',
});

const StyledLabel = styled.span({
  overflow: 'hidden',
  textOverflow: 'ellipsis',
});

const StyledGlyph = styled.span({
  display: 'flex',
  marginInlineStart: '4px',
});

export type RouteChipProps = {
  route: AppRoute;
  // The exit's place as shown: the city when one is chosen, else the country.
  exitLabel?: string;
};

// The route of a rule, as the pill at the end of its row. Part of the row's
// button, which carries the accessible name.
export function RouteChip({ route, exitLabel }: RouteChipProps) {
  switch (route.kind) {
    case 'country':
      return (
        <StyledChip aria-hidden>
          <CountryFlag country={route.exit.country} size={22} />
          <StyledLabel>{exitLabel}</StyledLabel>
        </StyledChip>
      );
    case 'direct':
      return (
        <StyledChip aria-hidden>
          <StyledGlyph>
            <OutsideGlyph />
          </StyledGlyph>
          <StyledLabel>{outsideVpnLabel()}</StyledLabel>
        </StyledChip>
      );
    case 'vpn':
      return (
        <StyledChip aria-hidden>
          <StyledGlyph>
            <ShieldGlyph />
          </StyledGlyph>
          <StyledLabel>
            {
              // TRANSLATORS: Chip of an app that uses the VPN while the other
              // TRANSLATORS: apps connect directly. Keep it as short as "VPN".
              messages.pgettext('split-tunneling-view', 'VPN')
            }
          </StyledLabel>
        </StyledChip>
      );
  }
}
