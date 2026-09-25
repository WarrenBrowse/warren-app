import styled from 'styled-components';

import { colors, Radius, spacings } from '../../../../lib/foundations';

export const StyledCard = styled.section<{ $muted?: boolean }>(({ $muted }) => ({
  display: 'flex',
  flexDirection: 'column',
  gap: '12px',
  padding: spacings.medium,
  borderRadius: Radius.radius16,
  backgroundColor: colors.blue10,
  opacity: $muted ? 0.55 : 1,
  transition: 'opacity 300ms ease-out',
}));

export const StyledCardHeader = styled.div({
  display: 'flex',
  alignItems: 'baseline',
  justifyContent: 'space-between',
  gap: spacings.small,
  minWidth: 0,
});

export const StyledLabel = styled.span({
  color: colors.whiteAlpha60,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '12px',
  lineHeight: '18px',
});

export const StyledValue = styled.span({
  color: colors.white,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '13px',
  fontWeight: 600,
  lineHeight: '18px',
  fontVariantNumeric: 'tabular-nums',
  textAlign: 'right',
});

export const StyledRingValue = styled.span({
  color: colors.white,
  fontFamily: 'var(--font-family-source-sans-pro)',
  fontSize: '24px',
  fontWeight: 700,
  lineHeight: '26px',
  fontVariantNumeric: 'tabular-nums',
});

export const StyledRingCaption = styled.span({
  color: colors.whiteAlpha60,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '11px',
  lineHeight: '14px',
});

export const StyledRingBand = styled.span({
  color: colors.white,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '12px',
  fontWeight: 600,
  lineHeight: '15px',
});

// Label and value on one line, the value flush right.
export const StyledFigures = styled.dl({
  display: 'grid',
  gridTemplateColumns: '1fr auto',
  alignItems: 'center',
  columnGap: spacings.small,
  rowGap: '6px',
  margin: 0,
  '& > dd': {
    margin: 0,
    justifySelf: 'end',
  },
});
