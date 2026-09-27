import React from 'react';

/**
 * The open state of a row's lists menu, shared by its button and by a right
 * click anywhere on the row.
 */
export function useListsMenu() {
  const [open, setOpen] = React.useState(false);
  const triggerRef = React.useRef<HTMLButtonElement>(null);
  const onContextMenu = React.useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    setOpen(true);
  }, []);
  const toggle = React.useCallback(() => setOpen((previous) => !previous), []);
  return { open, setOpen, toggle, triggerRef, onContextMenu };
}

export type ListsMenuState = ReturnType<typeof useListsMenu>;
