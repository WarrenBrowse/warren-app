import { useSelector } from '../../../../../../redux/store';

// Reads `Settings::warren_nat_pmp.enabled` from the redux store
// (pushed by the daemon on every settings refresh). Used by the
// Settings list item to display "On" / "Off" without entering the
// dedicated view. Mirrors the `WarrenMultiHopListItem` pattern.
export const useIsOn = () => {
  return useSelector((state) => state.settings.warrenNatPmp.enabled);
};
