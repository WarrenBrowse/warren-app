import { useSelector } from '../../../../../../redux/store';

// Reads `Settings::warren_multi_hop.enabled` from the redux store
// (pushed by the daemon on every settings refresh). Used by the
// Settings list item to display "On" / "Off" without entering the
// dedicated view.
export const useIsOn = () => {
  return useSelector((state) => state.settings.warrenMultiHop.enabled);
};
