import styled from 'styled-components';

import { type AppRoute } from '../../../../../../shared/app-routing';
import { messages } from '../../../../../../shared/gettext';
import { CountryFlag, LockGlyph } from '../../../../../features/app-routing/components';
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

const StyledLock = styled.span({
  display: 'flex',
  marginInlineStart: '-2px',
});

export type RouteChipProps = {
  route: AppRoute;
  // The exit's place as shown: the city when one is chosen, else the country.
  exitLabel?: string;
  // Locked to the VPN: a padlock closes the chip.
  locked?: boolean;
};

// The route of a rule, as the pill at the end of its row. Part of the row's
// button, which carries the accessible name.
export function RouteChip({ route, exitLabel, locked }: RouteChipProps) {
  const lock = locked ? (
    <StyledLock data-testid="route-chip-lock">
      <LockGlyph size={15} />
    </StyledLock>
  ) : null;
  switch (route.kind) {
    case 'country':
      return (
        <StyledChip aria-hidden>
          <CountryFlag country={route.exit.country} size={22} />
          <StyledLabel>{exitLabel}</StyledLabel>
          {lock}
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
          {lock}
        </StyledChip>
      );
  }
}
