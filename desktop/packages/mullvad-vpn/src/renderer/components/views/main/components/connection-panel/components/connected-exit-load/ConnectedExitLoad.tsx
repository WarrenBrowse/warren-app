import { useWarrenNetworkStats } from '../../../../../../../lib/network-stats';
import { useSelector } from '../../../../../../../redux/store';
import { ExitLoadBadge } from '../../../../../../network-stats';

/**
 * The load of the exit carrying the tunnel, at the trailing edge of the
 * hostname line. It shares that line instead of adding one: the card is
 * anchored to the bottom, so a new row would lift its top edge over the
 * scenery. Mounted only while connected, so the stats are asked for only while
 * the user can see them.
 */
export function ConnectedExitLoad() {
  const hostname = useSelector((state) => state.connection.hostname);
  const locale = useSelector((state) => state.userInterface.locale);
  const { stats, exitsByHostname, stale } = useWarrenNetworkStats();
  const exit = hostname === undefined ? undefined : exitsByHostname.get(hostname);
  if (stats === undefined || exit === undefined) {
    return null;
  }
  return (
    <span data-testid="connected-exit-load">
      <ExitLoadBadge exit={exit} stats={stats} locale={locale} stale={stale} />
    </span>
  );
}
