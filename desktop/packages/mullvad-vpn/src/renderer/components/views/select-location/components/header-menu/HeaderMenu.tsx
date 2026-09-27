import React from 'react';

import { messages } from '../../../../../../shared/gettext';
import { CreateCustomListDialog } from '../../../../../features/custom-lists/components';
import { DisableRecentsDialog } from '../../../../../features/locations/components';
import { useRecents } from '../../../../../features/locations/hooks';
import { Menu, type MenuProps } from '../../../../../lib/components/menu';

export type HeaderMenuProps = MenuProps;

export function HeaderMenu({ onOpenChange, ...props }: HeaderMenuProps) {
  const { hasRecents, setEnabledRecents } = useRecents();

  const [disableRecentsDialogOpen, setDisableRecentsDialogOpen] = React.useState(false);
  const [createCustomListDialogOpen, setCreateCustomListDialogOpen] = React.useState(false);

  const openCreateCustomListDialog = React.useCallback(() => {
    setCreateCustomListDialogOpen(true);
    onOpenChange?.(false);
  }, [onOpenChange]);

  const openDisableRecentsDialog = React.useCallback(() => {
    setDisableRecentsDialogOpen(true);
    onOpenChange?.(false);
  }, [onOpenChange]);

  const enableRecents = React.useCallback(async () => {
    await setEnabledRecents(true);
    onOpenChange?.(false);
  }, [onOpenChange, setEnabledRecents]);

  return (
    <>
      {/* No "Filters" entry: the upstream filter view (ownership/provider) has no
          meaning on the Warren network, where every exit is a Warren node. The
          view itself stays in the code in case filters come back one day. */}
      <Menu onOpenChange={onOpenChange} {...props}>
        <Menu.Popup>
          <Menu.Option>
            <Menu.Option.Trigger onClick={openCreateCustomListDialog}>
              <Menu.Option.Item>
                <Menu.Option.Item.Icon icon="add" />
                <Menu.Option.Item.Label>
                  {
                    // TRANSLATORS: Menu option that opens a dialog to create a new custom list of locations.
                    messages.pgettext('select-location-view', 'New custom list')
                  }
                </Menu.Option.Item.Label>
              </Menu.Option.Item>
            </Menu.Option.Trigger>
          </Menu.Option>
          <Menu.Option>
            <Menu.Option.Trigger onClick={hasRecents ? openDisableRecentsDialog : enableRecents}>
              <Menu.Option.Item>
                <Menu.Option.Item.Icon icon="history-remove" />
                <Menu.Option.Item.Label>
                  {hasRecents
                    ? // TRANSLATORS: Used in button to disable showing list of recent locations.
                      messages.pgettext('select-location-view', 'Disable recents')
                    : // TRANSLATORS: Used in button to enable showing list of recent locations.
                      messages.pgettext('select-location-view', 'Enable recents')}
                </Menu.Option.Item.Label>
              </Menu.Option.Item>
            </Menu.Option.Trigger>
          </Menu.Option>
        </Menu.Popup>
      </Menu>
      <CreateCustomListDialog
        open={createCustomListDialogOpen}
        onOpenChange={setCreateCustomListDialogOpen}
      />
      <DisableRecentsDialog
        open={disableRecentsDialogOpen}
        onOpenChange={setDisableRecentsDialogOpen}
      />
    </>
  );
}
