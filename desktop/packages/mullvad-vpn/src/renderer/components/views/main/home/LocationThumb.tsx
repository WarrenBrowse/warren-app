import styled from 'styled-components';

import { homeTokens } from '../../../../lib/foundations';
import {
  CANVAS_HEIGHT,
  CANVAS_WIDTH,
  hasCountryArt,
  landscapeFor,
  thumbnailTopRow,
} from '../../../CountryBackdrop/framing';

const { controls } = homeTokens;
const size = controls.thumb;

const StyledThumb = styled.span`
  flex-shrink: 0;
  width: ${size}px;
  height: ${size}px;
  border-radius: 50%;
  background-repeat: no-repeat;
  background-color: rgba(0, 0, 0, 0.1);
`;

const StyledFlag = styled.img`
  flex-shrink: 0;
  width: ${size}px;
  height: ${size}px;
  border-radius: 50%;
  object-fit: cover;
  user-select: none;
  -webkit-user-drag: none;
`;

interface LocationThumbProps {
  // The English relay-list name or the ISO code of the country.
  country?: string;
  // Its ISO code, for the flag of a country without bespoke art.
  code?: string;
}

// The round picture of a location: a square of its landscape crowned by the
// skyline, the flag of a country without art, the watched plain otherwise.
export function LocationThumb({ country, code }: LocationThumbProps) {
  if (!hasCountryArt(country) && !hasCountryArt(code) && code) {
    return <StyledFlag src={`assets/images/flags/${code.toLowerCase()}.svg`} alt="" aria-hidden />;
  }
  const landscape = landscapeFor(hasCountryArt(country) ? country : code);
  const scale = size / CANVAS_WIDTH;
  return (
    <StyledThumb
      aria-hidden
      data-testid="location-thumb"
      style={{
        backgroundImage: `url(assets/images/scenery/${landscape.slug}.webp)`,
        backgroundSize: `${size}px ${CANVAS_HEIGHT * scale}px`,
        backgroundPosition: `0 ${-thumbnailTopRow(landscape.skylineTopRow) * scale}px`,
      }}
    />
  );
}
