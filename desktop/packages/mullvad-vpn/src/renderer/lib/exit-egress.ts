import { useSelector } from '../redux/store';

/// Live view of the daemon's exit-egress verdict (doc 62 item 5): true
/// while the in-tunnel probe reports the exit not forwarding. No
/// renderer-side debounce: the daemon already debounces (N consecutive
/// probe failures before the verdict, one success to clear), so both
/// edges apply immediately. Shared by the banner, the connection
/// status and the backdrop so they flip together.
export function useExitEgressDead(): boolean {
  return useSelector((state) => state.settings.warrenStatus?.exitEgressDead ?? false);
}
