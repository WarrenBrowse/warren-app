import { useAppRouting } from '../../../../../../features/app-routing/hooks';
import { useSplitModeAvailability } from './use-split-mode-availability';

export function useCanEditSplitTunneling() {
  const { routing } = useAppRouting();
  const availability = useSplitModeAvailability();

  return routing.splitMode === 'exclude' && availability === 'available';
}
