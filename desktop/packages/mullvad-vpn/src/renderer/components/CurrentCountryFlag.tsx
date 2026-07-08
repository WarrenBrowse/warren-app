import { useCallback, useState } from 'react';
import styled from 'styled-components';

import { countryCodeFromName } from '../lib/country-code';
import { colors } from '../lib/foundations';
import { useSelector } from '../redux/store';

const FLAGS_BASE = 'assets/images/flags';
const UNKNOWN_FLAG = `${FLAGS_BASE}/xx.svg`;

const StyledFlag = styled.img`
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 1px solid ${colors.whiteAlpha20};
  flex-shrink: 0;
  user-select: none;
  -webkit-user-drag: none;
`;

/**
 * Round flag of the country the user currently appears in, whatever the tunnel
 * state: the geoip country (their real one when disconnected, the exit's when
 * connected). Hidden until the first geoip result arrives.
 */
export function CurrentCountryFlag(props: { className?: string }) {
  const country = useSelector((state) => state.connection.country);
  const relayLocations = useSelector((state) => state.settings.relayLocations);
  // Remember which exact file 404'd so a later country change retries normally.
  const [failedSrc, setFailedSrc] = useState<string>();

  const onError = useCallback((e: React.SyntheticEvent<HTMLImageElement>) => {
    setFailedSrc(e.currentTarget.getAttribute('src') ?? undefined);
  }, []);

  if (!country) {
    return null;
  }
  const code = countryCodeFromName(country, relayLocations);
  const wanted = code ? `${FLAGS_BASE}/${code}.svg` : UNKNOWN_FLAG;
  const src = failedSrc === wanted ? UNKNOWN_FLAG : wanted;

  return (
    <StyledFlag
      className={props.className}
      src={src}
      onError={onError}
      alt={country}
      title={country}
    />
  );
}
