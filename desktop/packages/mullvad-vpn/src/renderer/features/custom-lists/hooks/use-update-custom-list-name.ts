import React from 'react';

import { useGetCustomListById } from './use-get-custom-list-by-id';
import { useUpdateCustomList } from './use-update-custom-list';

export function useUpdateCustomListName() {
  const getCustomListById = useGetCustomListById();
  const updateCustomList = useUpdateCustomList();

  const updateCustomListName = React.useCallback(
    async (listId: string, name: string) => {
      const customList = getCustomListById(listId);
      if (!customList) {
        return {
          success: false,
        };
      }
      const updatedCustomList = { ...customList, name };

      const result = await updateCustomList(updatedCustomList);
      return result;
    },

    [getCustomListById, updateCustomList],
  );

  return updateCustomListName;
}
