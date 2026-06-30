import { useCallback, useState } from 'react';
import { sprintf } from 'sprintf-js';

import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { Button } from '../../../../../lib/components';
import { ListItemProps } from '../../../../../lib/components/list-item';
import { ModalAlert, ModalAlertType } from '../../../../Modal';
import { SettingsListItem } from '../../../../settings-list-item';

export type ResetPinnedExitKeysProps = Omit<ListItemProps, 'children'>;

// Settings entry that lets the user wipe the TOFU pin
// table. Useful in two scenarios:
//
//  * The user switched account / device and wants a fresh TOFU
//    baseline (the old pins reference exits the new identity may
//    never have visited).
//  * The user accidentally trusted a rotation they should not have
//    and wants to start over.
//
// Daemon-side this calls the gRPC `ResetPinnedExitKeys` RPC which
// returns the number of dropped entries. The success modal echoes
// that count so the user has feedback the action took effect.
export function ResetPinnedExitKeys(props: ResetPinnedExitKeysProps) {
  const { resetPinnedExitKeys } = useAppContext();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [resultCount, setResultCount] = useState<number | undefined>(undefined);
  const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);

  const openConfirm = useCallback(() => {
    setResultCount(undefined);
    setErrorMessage(undefined);
    setConfirmOpen(true);
  }, []);
  const closeConfirm = useCallback(() => {
    if (!busy) {
      setConfirmOpen(false);
    }
  }, [busy]);

  const handleConfirm = useCallback(async () => {
    setBusy(true);
    try {
      const count = await resetPinnedExitKeys();
      setResultCount(count);
    } catch (e) {
      const err = e as Error;
      setErrorMessage(err.message ?? String(err));
    } finally {
      setBusy(false);
    }
  }, [resetPinnedExitKeys]);

  const closeResult = useCallback(() => {
    setResultCount(undefined);
    setErrorMessage(undefined);
    setConfirmOpen(false);
  }, []);

  const resultModalOpen = resultCount !== undefined || errorMessage !== undefined;

  return (
    <>
      <SettingsListItem {...props}>
        <SettingsListItem.Item>
          <SettingsListItem.Item.Label>
            {
              // TRANSLATORS: Label for the action that clears every entry
              // TRANSLATORS: from the Warren TOFU pubkey pin table.
              messages.pgettext('vpn-settings-view', 'Reset pinned exit keys')
            }
          </SettingsListItem.Item.Label>
          <SettingsListItem.Item.ActionGroup>
            <Button variant="destructive" onClick={openConfirm}>
              <Button.Text>
                {
                  // TRANSLATORS: CTA opening the confirmation modal that
                  // TRANSLATORS: wipes the TOFU pin table.
                  messages.pgettext('vpn-settings-view', 'Reset')
                }
              </Button.Text>
            </Button>
          </SettingsListItem.Item.ActionGroup>
        </SettingsListItem.Item>
      </SettingsListItem>

      <ModalAlert
        isOpen={confirmOpen && !resultModalOpen}
        type={ModalAlertType.caution}
        title={messages.pgettext('vpn-settings-view', 'Reset pinned exit keys?')}
        message={[
          messages.pgettext(
            'vpn-settings-view',
            'Every Warren exit you previously connected to will be re-pinned on the next connection (Trust On First Use).',
          ),
          messages.pgettext(
            'vpn-settings-view',
            'Until the new pins are established the substitution-detection guard is briefly down: an attacker who replaced an exit between resets would not be flagged on the first reconnect.',
          ),
        ]}
        gridButtons={[
          <Button key="cancel" onClick={closeConfirm} disabled={busy}>
            <Button.Text>{messages.gettext('Cancel')}</Button.Text>
          </Button>,
          <Button key="confirm" variant="destructive" onClick={handleConfirm} disabled={busy}>
            <Button.Text>{messages.pgettext('vpn-settings-view', 'Reset all')}</Button.Text>
          </Button>,
        ]}
        close={closeConfirm}
      />

      <ModalAlert
        isOpen={resultModalOpen}
        type={errorMessage !== undefined ? ModalAlertType.failure : ModalAlertType.success}
        title={
          errorMessage !== undefined
            ? messages.pgettext('vpn-settings-view', 'Reset failed')
            : messages.pgettext('vpn-settings-view', 'Pin table cleared')
        }
        message={
          errorMessage !== undefined
            ? errorMessage
            : sprintf(
                // TRANSLATORS: %(count)s = number of pin entries dropped.
                messages.pgettext(
                  'vpn-settings-view',
                  'Dropped %(count)s pinned exit key entries.',
                ),
                { count: resultCount ?? 0 },
              )
        }
        buttons={[
          <Button key="ok" onClick={closeResult}>
            <Button.Text>{messages.gettext('OK')}</Button.Text>
          </Button>,
        ]}
        close={closeResult}
      />
    </>
  );
}
