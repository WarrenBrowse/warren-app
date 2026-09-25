import { describe, expect, it } from 'vitest';

import { resolveLaunchCommand } from '../../src/main/linux-app-routing';

// A fake filesystem: the executables that exist, and the symlinks between them.
function fakeFs(executables: string[], links: Record<string, string> = {}) {
  return {
    pathEnv: '/usr/local/bin:/usr/bin',
    isExecutable: (candidate: string) => Promise.resolve(executables.includes(candidate)),
    realpath: (candidate: string) => Promise.resolve(links[candidate] ?? candidate),
  };
}

describe('resolveLaunchCommand, which turns a desktop entry command into an app id', () => {
  it('finds a bare program name on PATH, in PATH order', async () => {
    const fs = fakeFs(['/usr/bin/firefox', '/usr/local/bin/firefox']);

    await expect(resolveLaunchCommand(['firefox', '--new-window'], fs)).resolves.toBe(
      '/usr/local/bin/firefox',
    );
  });

  it('follows symlinks, since the router sees the program the kernel runs', async () => {
    const fs = fakeFs(['/usr/bin/chromium'], { '/usr/bin/chromium': '/usr/lib/chromium/chromium' });

    await expect(resolveLaunchCommand(['chromium'], fs)).resolves.toBe(
      '/usr/lib/chromium/chromium',
    );
  });

  it('keeps an absolute command', async () => {
    const fs = fakeFs(['/opt/slack/slack']);

    await expect(resolveLaunchCommand(['/opt/slack/slack', '-s'], fs)).resolves.toBe(
      '/opt/slack/slack',
    );
  });

  it('skips an env prefix and its variable assignments', async () => {
    const fs = fakeFs(['/usr/bin/steam']);

    await expect(resolveLaunchCommand(['env', 'GDK_BACKEND=x11', 'steam'], fs)).resolves.toBe(
      '/usr/bin/steam',
    );
  });

  it('answers nothing for a program that is not installed', async () => {
    const fs = fakeFs([]);

    await expect(resolveLaunchCommand(['missing'], fs)).resolves.toBeUndefined();
  });

  it('answers nothing for an empty command', async () => {
    await expect(resolveLaunchCommand([], fakeFs([]))).resolves.toBeUndefined();
  });
});
