import { Metadata, status as grpcStatus } from '@grpc/grpc-js';
import * as grpcTypes from 'management-interface/management-interface/grpc-types';

import {
  DEVICE_REVOKED_DETAILS,
  DeviceActionResult,
  DeviceFailure,
  DeviceListResult,
  IDeviceList,
  IDeviceRename,
} from '../shared/devices';
import { IpcMain } from '../shared/ipc-helpers';
import { ipcSchema } from '../shared/ipc-schema';

/** Where the daemon puts the machine-readable reason of a refused call. */
const REASON_TRAILER = 'grpc-status-details-bin';

/**
 * The verdict a failed device call stands for (`map_devices_error` in the
 * daemon). UNAUTHENTICATED is no wallet, unless its details say this device
 * was revoked.
 */
export function deviceFailureOfError(error: unknown): DeviceFailure {
  switch ((error as { code?: number } | undefined)?.code) {
    case grpcStatus.UNAUTHENTICATED:
      return reasonOf(error) === DEVICE_REVOKED_DETAILS ? 'revoked' : 'no-wallet';
    case grpcStatus.INVALID_ARGUMENT:
      return 'invalid';
    case grpcStatus.NOT_FOUND:
      return 'not-found';
    default:
      return 'unavailable';
  }
}

function reasonOf(error: unknown): string | undefined {
  const metadata = (error as { metadata?: unknown }).metadata;
  return metadata instanceof Metadata ? metadata.get(REASON_TRAILER)[0]?.toString() : undefined;
}

export function convertFromDeviceList(list: grpcTypes.DeviceList): IDeviceList {
  const devices = list.getDevicesList().map((device) => ({
    id: device.getId(),
    name: device.getName(),
    platform: device.getPlatform(),
    appVersion: device.getAppVersion(),
    registeredOn: device.getRegisteredOn(),
    lastSeenOn: device.getLastSeenOn(),
    thisDevice: device.getThisDevice(),
  }));
  return list.hasOlderClientSeenOn()
    ? { devices, olderClientSeenOn: list.getOlderClientSeenOn() }
    : { devices };
}

export interface DeviceRegistryDaemon {
  listDevices(): Promise<DeviceListResult>;
  removeDevice(id: string): Promise<DeviceActionResult>;
  renameDevice(rename: IDeviceRename): Promise<DeviceActionResult>;
}

/** Answers the renderer's device calls from the daemon. */
export function registerDeviceIpc(
  channel: IpcMain<typeof ipcSchema>,
  daemon: DeviceRegistryDaemon,
) {
  channel.devices.handleList(() => daemon.listDevices());
  channel.devices.handleRemove((id) => daemon.removeDevice(id));
  channel.devices.handleRename((rename) => daemon.renameDevice(rename));
}
