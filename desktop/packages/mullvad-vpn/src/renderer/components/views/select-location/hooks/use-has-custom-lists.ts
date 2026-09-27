import { listsWithLocations } from '../../../../features/custom-lists/utils';
import { useSelectLocationViewContext } from '../SelectLocationViewContext';

export function useHasCustomLists() {
  const { customListLocations } = useSelectLocationViewContext();

  // An empty list is not shown, so lists holding nothing leave no section.
  const hasCustomLists = listsWithLocations(customListLocations).length > 0;

  return hasCustomLists;
}
