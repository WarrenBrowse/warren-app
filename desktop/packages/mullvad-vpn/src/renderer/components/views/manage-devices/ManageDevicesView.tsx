import React, { useCallback, useEffect, useRef, useState } from 'react';
import { sprintf } from 'sprintf-js';

import {
  DEVICE_NAME_MAX_CHARS,
  displayedDeviceName,
  IDevice,
  IDeviceList,
  isValidDeviceName,
} from '../../../../shared/devices';
import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Flex, Spinner, Text } from '../../../lib/components';
import { EmptyState } from '../../../lib/components/empty-state';
import { FlexColumn } from '../../../lib/components/flex-column';
import { TextField } from '../../../lib/components/text-field';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { AppNavigationHeader } from '../..';
import { DeviceList } from '../../device-list';
import { BackAction } from '../../keyboard-navigation';
import { LogoutConfirmDialog } from '../../LogoutConfirmDialog';
import { ModalAlert, ModalAlertType } from '../../Modal';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';

type ListState = { type: 'loading' } | { type: 'listed'; list: IDeviceList } | { type: 'error' };

type Pending =
  | { kind: 'disconnect'; device: IDevice }
  | { kind: 'logout'; device: IDevice }
  | { kind: 'rename'; device: IDevice };

type Failure = 'disconnect' | 'logout' | 'rename';

function failureMessage(failure: Failure): string {
  switch (failure) {
    case 'disconnect':
      // TRANSLATORS: Shown when disconnecting another device of the account failed.
      return messages.pgettext('device-management', 'Could not disconnect the device. Try again.');
    case 'logout':
      return messages.pgettext('account-view', 'Something went wrong while logging out.');
    case 'rename':
      // TRANSLATORS: Shown when renaming a device of the account failed.
      return messages.pgettext('device-management', 'Could not rename the device. Try again.');
  }
}

function RenameDialog({
  device,
  close,
  save,
}: {
  device: IDevice | undefined;
  close: () => void;
  save: (name: string) => void;
}) {
  const [name, setName] = useState('');

  useEffect(() => {
    if (device) {
      setName(displayedDeviceName(device.name));
    }
  }, [device]);

  const submit = useCallback(() => {
    if (isValidDeviceName(name)) {
      save(name.trim());
    }
  }, [name, save]);

  const submitOnEnter = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === 'Enter') {
        submit();
      }
    },
    [submit],
  );

  return (
    <ModalAlert
      isOpen={device !== undefined}
      title={
        // TRANSLATORS: Title of the dialog that renames a device of the account.
        messages.pgettext('device-management', 'Rename device')
      }
      buttons={[
        <Button key="save" disabled={!isValidDeviceName(name)} onClick={submit}>
          <Button.Text>{messages.gettext('Save')}</Button.Text>
        </Button>,
        <Button key="cancel" onClick={close}>
          <Button.Text>{messages.gettext('Cancel')}</Button.Text>
        </Button>,
      ]}
      close={close}>
      <TextField value={name} onValueChange={setName}>
        <TextField.Input
          autoFocus
          maxLength={DEVICE_NAME_MAX_CHARS}
          spellCheck={false}
          aria-label={messages.pgettext('device-management', 'Rename device')}
          onKeyDown={submitOnEnter}
        />
      </TextField>
    </ModalAlert>
  );
}

/**
 * The account's devices (warren-core doc 115): another device is disconnected
 * by revoking it, this one is logged out the same way, and the registry keeps
 * the names the account gives them.
 */
export function ManageDevicesView() {
  const history = useHistory();
  const { listDevices, removeDevice, renameDevice } = useAppContext();
  const locale = useSelector((state) => state.userInterface.locale);
  const [state, setState] = useState<ListState>({ type: 'loading' });
  const [pending, setPending] = useState<Pending>();
  const [busyId, setBusyId] = useState<string>();
  const [failure, setFailure] = useState<Failure>();
  const mounted = useRef(true);

  const load = useCallback(async () => {
    const result = await listDevices().catch(() => ({ type: 'error' }) as const);
    if (!mounted.current) {
      return;
    }
    setState(result.type === 'listed' ? result : { type: 'error' });
  }, [listDevices]);

  useEffect(() => {
    mounted.current = true;
    void load();
    return () => {
      mounted.current = false;
    };
  }, [load]);

  const retry = useCallback(() => {
    setState({ type: 'loading' });
    void load();
  }, [load]);

  const closePending = useCallback(() => setPending(undefined), []);
  const closeFailure = useCallback(() => setFailure(undefined), []);

  // `not-found` means the device is already gone: the list is read again
  // either way.
  const act = useCallback(
    async (device: IDevice, kind: Failure, request: () => ReturnType<typeof removeDevice>) => {
      setPending(undefined);
      setBusyId(device.id);
      const result = await request().catch(
        () => ({ type: 'error', reason: 'unavailable' }) as const,
      );
      if (!mounted.current) {
        return;
      }
      setBusyId(undefined);
      if (result.type === 'error' && result.reason !== 'not-found') {
        setFailure(kind);
      }
      // A logout that went through ends on the login view, by the device event.
      if (kind !== 'logout' || result.type === 'error') {
        await load();
      }
    },
    [load],
  );

  const confirmDisconnect = useCallback(() => {
    if (pending?.kind === 'disconnect') {
      void act(pending.device, 'disconnect', () => removeDevice(pending.device.id));
    }
  }, [act, pending, removeDevice]);

  const confirmLogout = useCallback(() => {
    if (pending?.kind === 'logout') {
      void act(pending.device, 'logout', () => removeDevice(pending.device.id));
    }
  }, [act, pending, removeDevice]);

  const saveName = useCallback(
    (name: string) => {
      if (pending?.kind === 'rename') {
        const { id } = pending.device;
        void act(pending.device, 'rename', () => renameDevice({ id, name }));
      }
    },
    [act, pending, renameDevice],
  );

  const askDisconnect = useCallback(
    (device: IDevice) => setPending({ kind: 'disconnect', device }),
    [],
  );
  const askLogout = useCallback((device: IDevice) => setPending({ kind: 'logout', device }), []);
  const askRename = useCallback((device: IDevice) => setPending({ kind: 'rename', device }), []);

  const goBackupPhrase = useCallback(() => {
    setPending(undefined);
    history.push(RoutePath.keys);
  }, [history]);

  // TRANSLATORS: Title of the view that lists the devices of the account.
  const title = messages.pgettext('device-management', 'Devices');

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={history.pop}>
        <NavigationContainer>
          <AppNavigationHeader title={title} />
          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                <FlexColumn gap="small">
                  <Text variant="titleBig">{title}</Text>
                  <Text variant="labelTiny" color="whiteAlpha60">
                    {
                      // TRANSLATORS: Explanation at the top of the list of the account's devices.
                      messages.pgettext(
                        'device-management',
                        'Devices signed in to this account. Disconnecting one signs it out: it needs the recovery phrase to sign in again.',
                      )
                    }
                  </Text>
                </FlexColumn>
                {state.type === 'loading' && (
                  <Flex justifyContent="center">
                    <Spinner size="big" />
                  </Flex>
                )}
                {state.type === 'error' && (
                  <EmptyState variant="error" alignSelf="stretch">
                    <EmptyState.StatusIcon />
                    <EmptyState.TextContainer>
                      <EmptyState.Title>
                        {
                          // TRANSLATORS: Shown when the list of the account's devices could not be loaded.
                          messages.pgettext(
                            'device-management',
                            'Could not load your devices. Try again.',
                          )
                        }
                      </EmptyState.Title>
                    </EmptyState.TextContainer>
                    <EmptyState.Button onClick={retry}>
                      <EmptyState.Button.Text>
                        {
                          // TRANSLATORS: Button text to retry fetching devices.
                          messages.pgettext('device-management', 'Try again')
                        }
                      </EmptyState.Button.Text>
                    </EmptyState.Button>
                  </EmptyState>
                )}
                {state.type === 'listed' && (
                  <DeviceList
                    list={state.list}
                    now={new Date()}
                    locale={locale}
                    busyId={busyId}
                    onDisconnect={askDisconnect}
                    onLogOut={askLogout}
                    onRename={askRename}
                  />
                )}
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>

      <ModalAlert
        isOpen={pending?.kind === 'disconnect'}
        type={ModalAlertType.caution}
        title={
          pending?.kind === 'disconnect'
            ? sprintf(
                // TRANSLATORS: Title of the dialog that confirms disconnecting another device.
                // TRANSLATORS: Available placeholder:
                // TRANSLATORS: %(name)s - the name of the device
                messages.pgettext('device-management', 'Disconnect %(name)s?'),
                { name: displayedDeviceName(pending.device.name) },
              )
            : undefined
        }
        message={messages.pgettext(
          'device-management',
          'It will be signed out within 10 minutes and will need the recovery phrase to sign in again. For a lost or stolen device, move to a new account instead: a device that keeps the recovery phrase can sign in again.',
        )}
        buttons={[
          <Button key="disconnect" variant="destructive" onClick={confirmDisconnect}>
            <Button.Text>{messages.pgettext('device-management', 'Disconnect')}</Button.Text>
          </Button>,
          <Button key="cancel" onClick={closePending}>
            <Button.Text>{messages.gettext('Cancel')}</Button.Text>
          </Button>,
        ]}
        close={closePending}
      />

      <LogoutConfirmDialog
        isOpen={pending?.kind === 'logout'}
        close={closePending}
        onLogout={confirmLogout}
        onBackup={goBackupPhrase}
      />

      <RenameDialog
        device={pending?.kind === 'rename' ? pending.device : undefined}
        close={closePending}
        save={saveName}
      />

      <ModalAlert
        isOpen={failure !== undefined}
        type={ModalAlertType.failure}
        message={failure !== undefined ? failureMessage(failure) : undefined}
        buttons={[
          <Button key="close" onClick={closeFailure}>
            <Button.Text>{messages.gettext('Close')}</Button.Text>
          </Button>,
        ]}
        close={closeFailure}
      />
    </View>
  );
}
