import { Metadata, status as grpcStatus } from '@grpc/grpc-js';
import * as grpcTypes from 'management-interface/management-interface/grpc-types';
import { describe, expect, it, vi } from 'vitest';

// The gRPC client reads one command-line switch at load; nothing else of
// Electron is reached.
vi.mock('electron', () => ({ app: { commandLine: { hasSwitch: () => false } } }));

import { DaemonRpc } from '../../src/main/daemon-rpc';
import { deviceFailureOfError, registerDeviceIpc } from '../../src/main/devices';
import { DeviceActionResult, DeviceListResult } from '../../src/shared/devices';
import { createIpcMain, createIpcRenderer } from '../../src/shared/ipc-helpers';
import { ipcSchema } from '../../src/shared/ipc-schema';

function rpcError(code: grpcStatus, reason?: string) {
  const metadata = new Metadata();
  if (reason !== undefined) {
    metadata.set('grpc-status-details-bin', Buffer.from(reason));
  }
  return Object.assign(new Error('refused'), { code, metadata });
}

type Callback = (error: Error | null, response?: unknown) => void;

/** A DaemonRpc whose gRPC client answers from `client`, as if connected. */
function daemonWith(client: Record<string, (request: unknown, callback: Callback) => void>) {
  const rpc = new DaemonRpc();
  Object.assign(rpc, { client, isConnectedValue: true });
  return rpc;
}

function grpcDevice(id: string, name: string, thisDevice: boolean): grpcTypes.Device {
  const device = new grpcTypes.Device();
  device.setId(id);
  device.setName(name);
  device.setPlatform('macos');
  device.setAppVersion('1.1.56');
  device.setRegisteredOn('2026-10-01');
  device.setLastSeenOn('2026-10-28');
  device.setThisDevice(thisDevice);
  return device;
}

describe('the device list over gRPC', () => {
  it('carries every field of each device and the older-client day', async () => {
    const list = new grpcTypes.DeviceList();
    list.setDevicesList([grpcDevice('a', 'Blue fox', true), grpcDevice('b', 'Red owl', false)]);
    list.setOlderClientSeenOn('2026-10-27');
    const rpc = daemonWith({ listDevices: (_request, callback) => callback(null, list) });

    const result = await rpc.listDevices();

    expect(result).to.deep.equal({
      type: 'listed',
      list: {
        devices: [
          {
            id: 'a',
            name: 'Blue fox',
            platform: 'macos',
            appVersion: '1.1.56',
            registeredOn: '2026-10-01',
            lastSeenOn: '2026-10-28',
            thisDevice: true,
          },
          {
            id: 'b',
            name: 'Red owl',
            platform: 'macos',
            appVersion: '1.1.56',
            registeredOn: '2026-10-01',
            lastSeenOn: '2026-10-28',
            thisDevice: false,
          },
        ],
        olderClientSeenOn: '2026-10-27',
      },
    });
  });

  it('leaves the older-client day out when the daemon sends none', async () => {
    const rpc = daemonWith({
      listDevices: (_request, callback) => callback(null, new grpcTypes.DeviceList()),
    });

    expect(await rpc.listDevices()).to.deep.equal({ type: 'listed', list: { devices: [] } });
  });

  it('answers a refused list with the reason, never a throw', async () => {
    const rpc = daemonWith({
      listDevices: (_request, callback) => callback(rpcError(grpcStatus.UNAVAILABLE)),
    });

    expect(await rpc.listDevices()).to.deep.equal({ type: 'error', reason: 'unavailable' });
  });

  it('revokes the device it names', async () => {
    const removeDevice = vi.fn((_request: unknown, callback: Callback) => callback(null));
    const rpc = daemonWith({ removeDevice });

    expect(await rpc.removeDevice('b')).to.deep.equal({ type: 'done' });
    const request = removeDevice.mock.calls[0][0] as grpcTypes.DeviceRemoval;
    expect(request.getDeviceId()).to.equal('b');
  });

  it('renames the device it names to the name given', async () => {
    const renameDevice = vi.fn((_request: unknown, callback: Callback) => callback(null));
    const rpc = daemonWith({ renameDevice });

    expect(await rpc.renameDevice({ id: 'b', name: 'Kitchen laptop' })).to.deep.equal({
      type: 'done',
    });
    const request = renameDevice.mock.calls[0][0] as grpcTypes.DeviceRename;
    expect(request.getDeviceId()).to.equal('b');
    expect(request.getName()).to.equal('Kitchen laptop');
  });

  it('answers a refused rename with the reason', async () => {
    const rpc = daemonWith({
      renameDevice: (_request, callback) => callback(rpcError(grpcStatus.INVALID_ARGUMENT)),
    });

    expect(await rpc.renameDevice({ id: 'b', name: '' })).to.deep.equal({
      type: 'error',
      reason: 'invalid',
    });
  });
});

describe('deviceFailureOfError', () => {
  it('tells a revoked device from no wallet by the status details', () => {
    expect(deviceFailureOfError(rpcError(grpcStatus.UNAUTHENTICATED, 'device_revoked'))).toBe(
      'revoked',
    );
    expect(deviceFailureOfError(rpcError(grpcStatus.UNAUTHENTICATED))).toBe('no-wallet');
  });

  it('reads each other status the daemon answers', () => {
    expect(deviceFailureOfError(rpcError(grpcStatus.INVALID_ARGUMENT))).toBe('invalid');
    expect(deviceFailureOfError(rpcError(grpcStatus.NOT_FOUND))).toBe('not-found');
    expect(deviceFailureOfError(rpcError(grpcStatus.UNAVAILABLE))).toBe('unavailable');
    expect(deviceFailureOfError(new Error('no daemon'))).toBe('unavailable');
  });
});

/** An ipcMain and an ipcRenderer joined by `invoke`, as Electron joins them. */
function linkedIpc() {
  const handlers = new Map<string, (event: unknown, arg: unknown) => unknown>();
  const ipcMain = {
    handle: (channel: string, handler: (event: unknown, arg: unknown) => unknown) =>
      handlers.set(channel, handler),
    on: () => undefined,
  };
  const ipcRenderer = {
    invoke: (channel: string, arg: unknown) => {
      const handler = handlers.get(channel);
      return handler
        ? Promise.resolve(handler({}, arg))
        : Promise.reject(new Error(`no handler for ${channel}`));
    },
    on: () => undefined,
  };
  return { ipcMain, ipcRenderer };
}

describe('the device IPC between renderer and main', () => {
  function wire() {
    const listed: DeviceListResult = { type: 'listed', list: { devices: [] } };
    const done: DeviceActionResult = { type: 'done' };
    const daemon = {
      listDevices: vi.fn(() => Promise.resolve(listed)),
      removeDevice: vi.fn((_id: string) => Promise.resolve(done)),
      renameDevice: vi.fn((_rename: { id: string; name: string }) => Promise.resolve(done)),
    };
    const { ipcMain, ipcRenderer } = linkedIpc();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    registerDeviceIpc(createIpcMain(ipcSchema, ipcMain as any, undefined), daemon);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const renderer = createIpcRenderer(ipcSchema, ipcRenderer as any);
    return { daemon, renderer, listed, done };
  }

  it('lists the devices through the daemon', async () => {
    const { daemon, renderer, listed } = wire();

    expect(await renderer.devices.list()).to.deep.equal(listed);
    expect(daemon.listDevices).toHaveBeenCalledOnce();
  });

  it('hands the id to revoke to the daemon', async () => {
    const { daemon, renderer, done } = wire();

    expect(await renderer.devices.remove('b')).to.deep.equal(done);
    expect(daemon.removeDevice).toHaveBeenCalledWith('b');
  });

  it('hands the id and the new name to the daemon', async () => {
    const { daemon, renderer, done } = wire();

    expect(await renderer.devices.rename({ id: 'b', name: 'Kitchen laptop' })).to.deep.equal(done);
    expect(daemon.renameDevice).toHaveBeenCalledWith({ id: 'b', name: 'Kitchen laptop' });
  });
});
