import { useEffect, useState } from 'react';

import { messages } from '../../shared/gettext';
import { Button, Checkbox, Flex } from '../lib/components';
import { ModalAlert, ModalAlertType } from './Modal';

interface LogoutConfirmDialogProps {
  isOpen: boolean;
  close: () => void;
  onLogout: () => void;
  // Opens the recovery-phrase backup instead of logging out.
  onBackup: () => void;
}

/**
 * Logout is a true sign-out: the daemon erases the recovery phrase from this
 * device. So it waits for an explicit confirmation that the phrase is backed
 * up, otherwise the account (and its subscription) is unrecoverable. Shared by
 * the Account view and the device list, whose "Log out" is the same act.
 */
export function LogoutConfirmDialog({
  isOpen,
  close,
  onLogout,
  onBackup,
}: LogoutConfirmDialogProps) {
  const [backedUp, setBackedUp] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setBackedUp(false);
    }
  }, [isOpen]);

  return (
    <ModalAlert
      isOpen={isOpen}
      type={ModalAlertType.caution}
      title={messages.pgettext('account-view', 'Log out of this account?')}
      message={[
        messages.pgettext(
          'account-view',
          'Logging out erases this account from this device. There is no email or password to log back in, your recovery phrase is the ONLY way to restore it.',
        ),
        messages.pgettext(
          'account-view',
          'If you have not backed up your recovery phrase, your subscription will be lost permanently.',
        ),
      ]}
      gridButtons={[
        <Button key="backup" onClick={onBackup}>
          <Button.Text>
            {
              // TRANSLATORS: Button that opens the recovery-phrase backup view before logging out.
              messages.pgettext('account-view', 'Back up my phrase')
            }
          </Button.Text>
        </Button>,
        <Button key="logout" variant="destructive" disabled={!backedUp} onClick={onLogout}>
          <Button.Text>{messages.pgettext('account-view', 'Log out')}</Button.Text>
        </Button>,
      ]}
      close={close}>
      <Checkbox checked={backedUp} onCheckedChange={setBackedUp}>
        <Flex gap="small" alignItems="flex-start">
          <Checkbox.Trigger>
            <Checkbox.Input />
          </Checkbox.Trigger>
          <Checkbox.Label>
            {messages.pgettext(
              'account-view',
              'I have backed up my recovery phrase and understand this account will be removed from this device.',
            )}
          </Checkbox.Label>
        </Flex>
      </Checkbox>
    </ModalAlert>
  );
}
