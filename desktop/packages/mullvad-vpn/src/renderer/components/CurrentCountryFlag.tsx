import { useCallback, useState } from 'react';
import styled from 'styled-components';

import { countryCodeFromName, systemRegionCode } from '../lib/country-code';
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
 * Round flag of the country the user currently appears in. With the tunnel up
 * (or coming up/down) that is the exit country reported by the daemon. Without
 * a tunnel there is no geoip source at all (Warren deliberately skips the
 * Mullvad conncheck, and redux then holds the SELECTED country, not the real
 * one), so the OS locale region stands in for the user's own country: offline,
 * no network call, no leak.
 */
export function CurrentCountryFlag(props: { className?: string }) {
  const tunnelState = useSelector((state) => state.connection.status.state);
  const tunnelCountry = useSelector((state) => state.connection.country);
  const relayLocations = useSelector((state) => state.settings.relayLocations);
  const locale = useSelector((state) => state.userInterface.locale);
  // Remember which exact file 404'd so a later country change retries normally.
  const [failedSrc, setFailedSrc] = useState<string>();

  const onError = useCallback((e: React.SyntheticEvent<HTMLImageElement>) => {
    setFailedSrc(e.currentTarget.getAttribute('src') ?? undefined);
  }, []);

  const tunneled =
    tunnelState === 'connected' || tunnelState === 'connecting' || tunnelState === 'disconnecting';

  let code: string | undefined;
  let name: string | undefined;
  if (tunneled) {
    code = countryCodeFromName(tunnelCountry, relayLocations);
    name = tunnelCountry;
  } else {
    code = systemRegionCode();
    if (code) {
      try {
        name = new Intl.DisplayNames(locale, { type: 'region' }).of(code.toUpperCase());
      } catch {
        name = undefined;
      }
    }
  }

  if (!code && !name) {
    return null;
  }
  const wanted = code ? `${FLAGS_BASE}/${code}.svg` : UNKNOWN_FLAG;
  const src = failedSrc === wanted ? UNKNOWN_FLAG : wanted;

  return (
    <StyledFlag
      className={props.className}
      src={src}
      onError={onError}
      alt={name ?? ''}
      title={name}
    />
  );
}
