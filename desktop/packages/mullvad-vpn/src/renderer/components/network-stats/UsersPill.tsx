import styled from 'styled-components';

import { colors, Radius } from '../../lib/foundations';
import { PersonGlyph } from './glyphs';

const StyledPill = styled.span({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '4px',
  flexShrink: 0,
  height: '18px',
  padding: '0 6px',
  borderRadius: Radius.radiusFull,
  border: `1px solid ${colors.whiteAlpha20}`,
  color: colors.whiteAlpha80,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '11px',
  fontWeight: 600,
  lineHeight: '16px',
  whiteSpace: 'nowrap',
  fontVariantNumeric: 'tabular-nums',
});

export type UsersPillProps = {
  // `< 5`, `40+`, `< 20` per exit, or an exact fleet count.
  label: string;
} & Pick<React.AriaAttributes, 'aria-label' | 'aria-hidden'>;

export function UsersPill({ label, ...props }: UsersPillProps) {
  return (
    <StyledPill data-testid="users-pill" {...props}>
      <PersonGlyph />
      {label}
    </StyledPill>
  );
}
