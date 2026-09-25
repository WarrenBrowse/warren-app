import React from 'react';
import styled from 'styled-components';

import { colors } from '../../../lib/foundations';

const FLAGS_BASE = 'assets/images/flags';
const UNKNOWN_FLAG = `${FLAGS_BASE}/xx.svg`;

const StyledFlag = styled.img<{ $size: number }>((props) => ({
  width: `${props.$size}px`,
  height: `${props.$size}px`,
  borderRadius: '50%',
  border: `1px solid ${colors.whiteAlpha20}`,
  flexShrink: 0,
  userSelect: 'none',
}));

export type CountryFlagProps = {
  // Lowercase ISO 3166-1 alpha-2 code, as the relay list and ExitChoice carry.
  country: string;
  size?: number;
};

// Decorative: every flag in app routing sits next to the country name, which
// is what a screen reader announces.
export function CountryFlag({ country, size = 18 }: CountryFlagProps) {
  const wanted = /^[a-z]{2}$/.test(country) ? `${FLAGS_BASE}/${country}.svg` : UNKNOWN_FLAG;
  const [failedSrc, setFailedSrc] = React.useState<string>();
  const onError = React.useCallback(() => setFailedSrc(wanted), [wanted]);
  const src = failedSrc === wanted ? UNKNOWN_FLAG : wanted;

  return (
    <StyledFlag src={src} alt="" aria-hidden $size={size} draggable={false} onError={onError} />
  );
}
