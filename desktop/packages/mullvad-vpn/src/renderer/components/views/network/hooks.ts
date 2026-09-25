import { relayLocations } from '../../../../shared/gettext';
import { ExitStats } from '../../../../shared/network-stats';
import { relayPlacesByHostname } from '../../../lib/network-stats';
import { useSelector } from '../../../redux/store';

export interface ExitPlace {
  city: string;
  country: string;
}

/**
 * Where each exit is, keyed by `exit_id`, from the signed relay list and in the
 * user's language. The snapshot is unsigned, so its own names are only a
 * fallback for an exit the relay list does not carry. Recomputed on every
 * render: a handful of exits, and the catalog changes with the locale.
 */
export function useExitPlaces(
  exitsByHostname: ReadonlyMap<string, ExitStats>,
): Map<string, ExitPlace> {
  const relayList = useSelector((state) => state.settings.relayLocations);
  const placesByHostname = relayPlacesByHostname(relayList);
  const places = new Map<string, ExitPlace>();
  for (const [hostname, exit] of exitsByHostname) {
    const place = placesByHostname.get(hostname);
    if (place) {
      places.set(exit.exitId, {
        city: relayLocations.gettext(place.city),
        country: relayLocations.gettext(place.country),
      });
    }
  }
  return places;
}
