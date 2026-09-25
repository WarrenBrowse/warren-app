import React from 'react';

import { modeChangeConfirmation } from '../../../../../../shared/app-routing';
import { messages } from '../../../../../../shared/gettext';
import { useAppRouting } from '../../../../../features/app-routing/hooks';
import { Dialog } from '../../../../../lib/components/dialog';
import { useSplitTunnelingContext } from '../../SplitTunnelingContext';

// Asks once before a split mode change that leaves the device unprotected or
// replaces the other mode. The two app lists are kept either way.
export function ModeChangeDialog() {
  const { pendingSplitMode, confirmSplitMode, cancelSplitMode } = useSplitTunnelingContext();
  const { routing } = useAppRouting();

  const confirmation =
    pendingSplitMode === undefined
      ? undefined
      : modeChangeConfirmation(routing.splitMode, pendingSplitMode);

  const onOpenChange = React.useCallback(
    (open: boolean) => {
      if (!open) {
        cancelSplitMode();
      }
    },
    [cancelSplitMode],
  );

  return (
    <Dialog open={confirmation !== undefined} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Popup data-testid="mode-change-dialog">
          <Dialog.PopupContent>
            <Dialog.Icon
              icon="alert-circle"
              color={confirmation?.leavesDeviceUnprotected ? 'yellow' : 'white'}
            />
            <Dialog.TextGroup>
              {confirmation?.leavesDeviceUnprotected && (
                <Dialog.Text>
                  {messages.pgettext(
                    'split-tunneling-view',
                    'Only the apps you choose will use the VPN. The rest of this device will not be protected.',
                  )}
                </Dialog.Text>
              )}
              {confirmation?.replaces === 'exclude' && (
                <Dialog.Text>
                  {messages.pgettext(
                    'split-tunneling-view',
                    'This replaces Bypass VPN. Your list of apps there is kept.',
                  )}
                </Dialog.Text>
              )}
              {confirmation?.replaces === 'include-only' && (
                <Dialog.Text>
                  {messages.pgettext(
                    'split-tunneling-view',
                    'This replaces VPN only for, so every app uses the VPN again except the ones you bypass. Your list of apps there is kept.',
                  )}
                </Dialog.Text>
              )}
            </Dialog.TextGroup>
            <Dialog.ButtonGroup>
              <Dialog.Button variant="success" onClick={confirmSplitMode}>
                <Dialog.Button.Text>
                  {messages.pgettext('split-tunneling-view', 'Turn on')}
                </Dialog.Button.Text>
              </Dialog.Button>
              <Dialog.Button onClick={cancelSplitMode}>
                <Dialog.Button.Text>{messages.gettext('Cancel')}</Dialog.Button.Text>
              </Dialog.Button>
            </Dialog.ButtonGroup>
          </Dialog.PopupContent>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog>
  );
}
