import { useCallback } from 'react';
import styled from 'styled-components';

import {
  deviceActivityLabel,
  devicePlatformLine,
  displayedDeviceName,
  IDevice,
  IDeviceList,
  olderClientSeenLabel,
  sortDevices,
} from '../../../shared/devices';
import { messages } from '../../../shared/gettext';
import { Button, Flex, IconButton, Text } from '../../lib/components';
import { FlexColumn } from '../../lib/components/flex-column';
import { ListItem } from '../../lib/components/list-item';
import { colors, Radius, spacings } from '../../lib/foundations';

export type DeviceListProps = {
  list: IDeviceList;
  // The clock and the language the activity days are read against.
  now: Date;
  locale: string;
  // The device whose request is in flight: its row is disabled meanwhile.
  busyId?: string;
  onDisconnect: (device: IDevice) => void;
  onLogOut: (device: IDevice) => void;
  onRename: (device: IDevice) => void;
};

const StyledRowContent = styled(FlexColumn)`
  padding-block: ${spacings.small};
  min-width: 0;
`;

const StyledName = styled(Text)`
  overflow-wrap: anywhere;
`;

const StyledBadge = styled(Text)`
  flex-shrink: 0;
  padding: 2px ${spacings.tiny};
  border-radius: ${Radius.radius4};
  background-color: ${colors.green};
`;

const StyledActions = styled(Flex)`
  padding-inline-end: ${spacings.medium};
`;

function DeviceRow({
  device,
  now,
  locale,
  busy,
  onDisconnect,
  onLogOut,
  onRename,
}: { device: IDevice; busy: boolean } & Omit<DeviceListProps, 'list' | 'busyId'>) {
  const name = displayedDeviceName(device.name);
  const rename = useCallback(() => onRename(device), [onRename, device]);
  const disconnect = useCallback(() => onDisconnect(device), [onDisconnect, device]);
  const logOut = useCallback(() => onLogOut(device), [onLogOut, device]);
  return (
    <ListItem disabled={busy} data-testid="device-row">
      <ListItem.Item>
        <StyledRowContent gap="tiny">
          <Flex gap="small" alignItems="center">
            <StyledName variant="bodySmallSemibold">{name}</StyledName>
            {device.thisDevice && (
              <StyledBadge variant="labelTinySemiBold" color="white">
                {
                  // TRANSLATORS: Badge on the row of the device the app runs on, in
                  // TRANSLATORS: the list of the account's devices.
                  messages.pgettext('device-management', 'This device')
                }
              </StyledBadge>
            )}
          </Flex>
          <Text variant="labelTiny" color="whiteAlpha60">
            {devicePlatformLine(device)}
          </Text>
          <Text variant="labelTiny" color="whiteAlpha60">
            {deviceActivityLabel(device.lastSeenOn, now, locale)}
          </Text>
        </StyledRowContent>
        <StyledActions gap="small" alignItems="center">
          <IconButton
            variant="secondary"
            disabled={busy}
            onClick={rename}
            aria-label={messages.pgettext('device-management', 'Rename device')}>
            <IconButton.Icon icon="edit-circle" />
          </IconButton>
          {device.thisDevice ? (
            <Button width="fit" variant="destructive" disabled={busy} onClick={logOut}>
              <Button.Text>{messages.pgettext('account-view', 'Log out')}</Button.Text>
            </Button>
          ) : (
            <Button width="fit" variant="destructive" disabled={busy} onClick={disconnect}>
              <Button.Text>
                {
                  // TRANSLATORS: Button that signs another device of the account out.
                  messages.pgettext('device-management', 'Disconnect')
                }
              </Button.Text>
            </Button>
          )}
        </StyledActions>
      </ListItem.Item>
    </ListItem>
  );
}

// A device too old to register: it cannot be named or disconnected, only
// pointed at, since a registered device already took its place.
function OlderClientRow({ day, locale }: { day: string; locale: string }) {
  return (
    <ListItem data-testid="older-client-row">
      <ListItem.Item>
        <StyledRowContent gap="tiny">
          <StyledName variant="bodySmallSemibold">
            {
              // TRANSLATORS: Title of the row, in the list of the account's devices,
              // TRANSLATORS: for a device that runs an old version of the app.
              messages.pgettext('device-management', 'An older version of Warren')
            }
          </StyledName>
          <Text variant="labelTiny" color="whiteAlpha60">
            {messages.pgettext(
              'device-management',
              'A device of this account runs a version of Warren older than 1.1.36. It no longer blocks your other devices. Update or uninstall it.',
            )}
          </Text>
          <Text variant="labelTiny" color="whiteAlpha60">
            {olderClientSeenLabel(day, locale)}
          </Text>
        </StyledRowContent>
      </ListItem.Item>
    </ListItem>
  );
}

/** The account's devices, this one first, then the older-client row when there is one. */
export function DeviceList({ list, busyId, ...props }: DeviceListProps) {
  return (
    <FlexColumn gap="small">
      {sortDevices(list.devices).map((device) => (
        <DeviceRow key={device.id} device={device} busy={busyId === device.id} {...props} />
      ))}
      {list.olderClientSeenOn !== undefined && (
        <OlderClientRow day={list.olderClientSeenOn} locale={props.locale} />
      )}
    </FlexColumn>
  );
}
