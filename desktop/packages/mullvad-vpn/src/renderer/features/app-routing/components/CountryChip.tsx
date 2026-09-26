import styled from 'styled-components';

import { tinyText } from '../../../components/common-styles';
import { Icon } from '../../../lib/components';
import { colors } from '../../../lib/foundations';
import { CountryFlag } from './CountryFlag';

const StyledChip = styled.button<{ $active: boolean }>((props) => ({
  ...tinyText,
  display: 'inline-flex',
  alignItems: 'center',
  gap: '6px',
  flexShrink: 0,
  maxWidth: '150px',
  paddingBlock: '3px',
  paddingInlineStart: '4px',
  paddingInlineEnd: '10px',
  borderRadius: '14px',
  border: `1px solid ${props.$active ? colors.whiteAlpha40 : colors.whiteAlpha20}`,
  backgroundColor: props.$active ? colors.blue40 : 'transparent',
  color: props.$active ? colors.white : colors.whiteAlpha80,
  cursor: 'default',
  '&&:not(:disabled):hover': {
    borderColor: colors.whiteAlpha80,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '1px',
  },
  '&&:disabled': {
    opacity: 0.5,
  },
}));

const StyledName = styled.span({
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
});

const StyledAddIcon = styled.span({
  display: 'grid',
  placeItems: 'center',
  width: '18px',
  height: '18px',
});

export type CountryChipProps = {
  // The exit's country code, or undefined for an app without a country.
  country?: string;
  label: string;
  accessibleLabel: string;
  disabled?: boolean;
  onClick: () => void;
};

export function CountryChip({
  country,
  label,
  accessibleLabel,
  disabled,
  onClick,
}: CountryChipProps) {
  return (
    <StyledChip
      type="button"
      $active={country !== undefined}
      aria-label={accessibleLabel}
      aria-haspopup="dialog"
      disabled={disabled}
      onClick={onClick}>
      {country !== undefined ? (
        <CountryFlag country={country} />
      ) : (
        <StyledAddIcon>
          <Icon icon="add" size="tiny" color="whiteAlpha80" />
        </StyledAddIcon>
      )}
      <StyledName>{label}</StyledName>
    </StyledChip>
  );
}
