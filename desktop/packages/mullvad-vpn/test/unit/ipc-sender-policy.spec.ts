import { pathToFileURL } from 'url';
import { describe, expect, it } from 'vitest';

import { appPageSenderPolicy } from '../../src/main/ipc-sender-policy';
import { invoke, send, setIpcSenderPolicy } from '../../src/shared/ipc-helpers';

const INDEX = '/opt/Warren VPN/resources/app.asar/build/index.html';
const APP_URL = pathToFileURL(INDEX).href;

type Listener = (event: { senderFrame: { url: string } | null }, arg: unknown) => unknown;

function fakeIpcMain() {
  const handlers = new Map<string, Listener>();
  return {
    handlers,
    handle: (channel: string, listener: Listener) => handlers.set(channel, listener),
    on: (channel: string, listener: Listener) => handlers.set(channel, listener),
  };
}

const frame = (url: string) => ({ senderFrame: { url } });

// The main process acts on what the renderer asks: launch a program, read a
// file, open a dialog. Only the app's own page may ask, so a frame that is
// not it (a page that got loaded by mistake, an injected frame) is refused.
describe('the IPC sender policy', () => {
  const policy = appPageSenderPolicy({ indexPath: INDEX, devServerUrl: undefined });

  it('accepts the app page, on any of its routes', () => {
    expect(policy(APP_URL)).toBe(true);
    expect(policy(`${APP_URL}#/settings/app-info`)).toBe(true);
  });

  it('refuses any other page', () => {
    expect(policy('file:///tmp/index.html')).toBe(false);
    expect(policy('https://example.com/')).toBe(false);
    expect(policy(undefined)).toBe(false);
  });

  it('accepts the development server in a development build only', () => {
    const dev = appPageSenderPolicy({ indexPath: INDEX, devServerUrl: 'http://localhost:5173/' });

    expect(dev('http://localhost:5173/#/main')).toBe(true);
    expect(policy('http://localhost:5173/#/main')).toBe(false);
  });
});

describe('IPC handlers under the sender policy', () => {
  it('answer the app page and refuse any other sender without running', async () => {
    setIpcSenderPolicy((url) => url === APP_URL);
    try {
      const ipcMain = fakeIpcMain();
      let runs = 0;
      invoke<string, string>().receive(
        'app-test',
        ipcMain as never,
      )((arg) => {
        runs++;
        return Promise.resolve(`got ${arg}`);
      });
      const listener = ipcMain.handlers.get('app-test')!;

      await expect(listener(frame(APP_URL), 'x')).resolves.toEqual({
        type: 'success',
        value: 'got x',
      });
      await expect(listener(frame('https://example.com/'), 'x')).resolves.toEqual({
        type: 'error',
        message: 'IPC refused',
      });
      expect(runs).toBe(1);
    } finally {
      setIpcSenderPolicy(() => true);
    }
  });

  it('drop a message sent by any other page', () => {
    setIpcSenderPolicy((url) => url === APP_URL);
    try {
      const ipcMain = fakeIpcMain();
      const received: unknown[] = [];
      send<string>().receive('app-test-send', ipcMain as never)((value) => received.push(value));
      const listener = ipcMain.handlers.get('app-test-send')!;

      listener(frame('https://example.com/'), 'ignored');
      listener(frame(APP_URL), 'kept');

      expect(received).toEqual(['kept']);
    } finally {
      setIpcSenderPolicy(() => true);
    }
  });
});
