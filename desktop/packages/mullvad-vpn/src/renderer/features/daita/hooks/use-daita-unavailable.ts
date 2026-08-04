import { useSelector } from '../../../redux/store';
import { isDaitaUnavailable } from '../daita-unavailable';

// True when the current connection runs WITHOUT the requested DAITA defense
// (the server did not grant it). Consumers must surface this loudly: the
// toggle being on does not mean the traffic is protected.
export function useDaitaUnavailable() {
  const tunnelState = useSelector((state) => state.connection.status);
  return isDaitaUnavailable(tunnelState);
}
