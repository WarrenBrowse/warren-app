import styled from 'styled-components';

import { type GeographicalLocation } from '../../../../../../../features/locations/types';
import { spacings } from '../../../../../../../lib/foundations';
import { singleExitHostname, useWarrenNetworkStats } from '../../../../../../../lib/network-stats';
import { useSelector } from '../../../../../../../redux/store';
import { ExitLoadBadge } from '../../../../../../network-stats';

const StyledTrailing = styled.span({
  display: 'flex',
  marginInlineStart: 'auto',
  paddingInlineStart: spacings.small,
  marginInlineEnd: spacings.small,
});

export type LocationExitLoadProps = {
  location: GeographicalLocation;
};

/**
 * The load of the exit a row stands for. A country or a city shows it only
 * when it holds exactly one exit, so an expanded branch does not repeat the
 * same figures on every level.
 */
export function LocationExitLoad({ location }: LocationExitLoadProps) {
  const { stats, exitsByHostname, stale } = useWarrenNetworkStats();
  const locale = useSelector((state) => state.userInterface.locale);

  const hostname = singleExitHostname(location);
  const exit = hostname === undefined ? undefined : exitsByHostname.get(hostname);
  if (stats === undefined || exit === undefined) {
    return null;
  }

  return (
    <StyledTrailing>
      <ExitLoadBadge exit={exit} stats={stats} locale={locale} stale={stale} throughput />
    </StyledTrailing>
  );
}
