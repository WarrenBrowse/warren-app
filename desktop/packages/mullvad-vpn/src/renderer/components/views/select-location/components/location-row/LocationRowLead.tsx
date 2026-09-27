import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../../shared/gettext';
import { CountryFlag } from '../../../../../features/app-routing/components';
import { Text } from '../../../../../lib/components';
import { useListItemContext } from '../../../../../lib/components/list-item/ListItemContext';
import { useSelectableLabelColor } from '../../../../../lib/components/selectable-label/hooks';
import { colors } from '../../../../../lib/foundations';
import { useLocationContext } from '../location-list-item/LocationContext';

const FLAG_SIZE = 20;

const StyledLead = styled.span({
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  minWidth: 0,
});

// The flag slot: a country flag, or a list glyph for a custom list. Selection
// rings it, so the mark never pushes the name aside.
const StyledBadge = styled.span<{ $selected: boolean }>(({ $selected }) => ({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  flexShrink: 0,
  width: `${FLAG_SIZE}px`,
  height: `${FLAG_SIZE}px`,
  borderRadius: '50%',
  color: colors.whiteAlpha60,
  boxShadow: $selected ? `0 0 0 2px ${colors.green}` : '0 0 0 0 transparent',
  transition: 'box-shadow 150ms ease-out',
}));

// A long name is cut with an ellipsis rather than wrapped: the load shares the row.
const ellipsis = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
} as const;

const StyledText = styled.span({
  display: 'flex',
  flexDirection: 'column',
  minWidth: 0,
  '& > :last-child:not(:first-child)': ellipsis,
});

const StyledTitleLine = styled.span({
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  minWidth: 0,
  '& > :first-child': ellipsis,
});

function ListGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden fill="currentColor">
      <circle cx="2" cy="3" r="1.25" />
      <circle cx="2" cy="7" r="1.25" />
      <circle cx="2" cy="11" r="1.25" />
      <rect x="5" y="2.2" width="8" height="1.6" rx="0.8" />
      <rect x="5" y="6.2" width="8" height="1.6" rx="0.8" />
      <rect x="5" y="10.2" width="8" height="1.6" rx="0.8" />
    </svg>
  );
}

function CheckGlyph() {
  return (
    <svg
      data-testid="selected-check"
      width="12"
      height="12"
      viewBox="0 0 12 12"
      aria-hidden
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round">
      <path d="M2 6.4 4.8 9 10 3" />
    </svg>
  );
}

export type LocationRowLeadProps = {
  label: string;
  // Muted second line: where a city or a relay is, or how much a list holds.
  subtitle?: string;
  // Lowercase ISO code of the flag to lead with.
  country?: string;
  list?: boolean;
};

/**
 * The start of every row of the location picker: the flag (or a list glyph),
 * the name and an optional second line. The selected row takes the accent
 * colour and a ring around its flag; a row without a flag gets a small check
 * after its name instead.
 */
export function LocationRowLead({ label, subtitle, country, list }: LocationRowLeadProps) {
  const { selected = false } = useLocationContext();
  const { disabled } = useListItemContext();
  const color = useSelectableLabelColor(selected, disabled);
  const hasBadge = list === true || country !== undefined;

  return (
    <StyledLead data-testid="location-row-lead" data-selected={selected}>
      {hasBadge && (
        <StyledBadge $selected={selected}>
          {list ? <ListGlyph /> : <CountryFlag country={country!} size={FLAG_SIZE} />}
        </StyledBadge>
      )}
      <StyledText>
        <StyledTitleLine>
          <Text variant="bodySmallSemibold" color={color}>
            {label}
          </Text>
          {selected && !hasBadge && (
            <Text as="span" color={color}>
              <CheckGlyph />
            </Text>
          )}
        </StyledTitleLine>
        {subtitle && (
          <Text variant="footnoteMini" color="whiteAlpha60">
            {subtitle}
          </Text>
        )}
      </StyledText>
    </StyledLead>
  );
}

/** The second line of a custom list: how many places it holds. */
export function listSizeLabel(count: number): string {
  return sprintf(
    // TRANSLATORS: How many locations a custom list holds, under its name.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(count)d - the number of locations in the list
    messages.npgettext('select-location-view', '%(count)d location', '%(count)d locations', count),
    { count },
  );
}
