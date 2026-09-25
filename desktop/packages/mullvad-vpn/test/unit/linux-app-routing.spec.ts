import { describe, expect, it } from 'vitest';

import { resolveLaunchTarget } from '../../src/main/linux-app-routing';

// A fake filesystem: the executables that exist, the symlinks between them,
// and the text of those that are scripts.
function fakeFs(
  executables: string[],
  links: Record<string, string> = {},
  scripts: Record<string, string> = {},
) {
  return {
    pathEnv: '/usr/local/bin:/usr/bin',
    isExecutable: (candidate: string) => Promise.resolve(executables.includes(candidate)),
    realpath: (candidate: string) => Promise.resolve(links[candidate] ?? candidate),
    readScript: (candidate: string) => Promise.resolve(scripts[candidate]),
  };
}

const program = (path: string) => ({ kind: 'program', path });

describe('resolveLaunchTarget, which turns a desktop entry command into an app id', () => {
  it('finds a bare program name on PATH, in PATH order', async () => {
    const fs = fakeFs(['/usr/bin/firefox', '/usr/local/bin/firefox']);

    await expect(resolveLaunchTarget(['firefox', '--new-window'], fs)).resolves.toEqual(
      program('/usr/local/bin/firefox'),
    );
  });

  it('follows symlinks, since the router sees the program the kernel runs', async () => {
    const fs = fakeFs(['/usr/bin/chromium'], { '/usr/bin/chromium': '/usr/lib/chromium/chromium' });

    await expect(resolveLaunchTarget(['chromium'], fs)).resolves.toEqual(
      program('/usr/lib/chromium/chromium'),
    );
  });

  it('keeps an absolute command', async () => {
    const fs = fakeFs(['/opt/slack/slack']);

    await expect(resolveLaunchTarget(['/opt/slack/slack', '-s'], fs)).resolves.toEqual(
      program('/opt/slack/slack'),
    );
  });

  it('skips an env prefix and its variable assignments', async () => {
    const fs = fakeFs(['/usr/bin/steam']);

    await expect(resolveLaunchTarget(['env', 'GDK_BACKEND=x11', 'steam'], fs)).resolves.toEqual(
      program('/usr/bin/steam'),
    );
  });

  it('answers nothing for a program that is not installed', async () => {
    const fs = fakeFs([]);

    await expect(resolveLaunchTarget(['missing'], fs)).resolves.toBeUndefined();
  });

  it('answers nothing for an empty command', async () => {
    await expect(resolveLaunchTarget([], fakeFs([]))).resolves.toBeUndefined();
  });

  it('follows a shell wrapper to the program it execs with its arguments', async () => {
    const fs = fakeFs(
      ['/usr/bin/signal-desktop', '/opt/Signal/signal-desktop'],
      {},
      {
        '/usr/bin/signal-desktop':
          '#!/bin/sh\n# Launcher\nexport LC_ALL=C.UTF-8\nexec /opt/Signal/signal-desktop "$@"\n',
      },
    );

    await expect(resolveLaunchTarget(['signal-desktop', '%U'], fs)).resolves.toEqual(
      program('/opt/Signal/signal-desktop'),
    );
  });

  it('reads a quoted program and an exec -a name in a wrapper', async () => {
    const fs = fakeFs(
      ['/usr/bin/code', '/usr/share/code/code'],
      {},
      { '/usr/bin/code': '#!/usr/bin/env sh\nexec -a "$0" "/usr/share/code/code" "$@"\n' },
    );

    await expect(resolveLaunchTarget(['code'], fs)).resolves.toEqual(
      program('/usr/share/code/code'),
    );
  });

  it('names a wrapper whose program it cannot read a script', async () => {
    const fs = fakeFs(
      ['/usr/bin/firefox'],
      {},
      {
        '/usr/bin/firefox':
          '#!/bin/sh\nMOZ_LIBDIR=/usr/lib/firefox\nexec $MOZ_LIBDIR/firefox "$@"\n',
      },
    );

    await expect(resolveLaunchTarget(['firefox'], fs)).resolves.toEqual({
      kind: 'unsupported',
      path: '/usr/bin/firefox',
      reason: 'script',
    });
  });

  it('names a wrapper that execs itself a script rather than looping', async () => {
    const fs = fakeFs(
      ['/usr/bin/loop'],
      {},
      { '/usr/bin/loop': '#!/bin/sh\nexec /usr/bin/loop "$@"\n' },
    );

    await expect(resolveLaunchTarget(['loop'], fs)).resolves.toEqual({
      kind: 'unsupported',
      path: '/usr/bin/loop',
      reason: 'script',
    });
  });

  it('recognises a Flatpak app, launched directly or through its exported wrapper', async () => {
    const fs = fakeFs(
      ['/usr/bin/flatpak', '/var/lib/flatpak/exports/bin/org.gimp.GIMP'],
      {},
      {
        '/var/lib/flatpak/exports/bin/org.gimp.GIMP':
          '#!/bin/sh\nexec /usr/bin/flatpak run --branch=stable --arch=x86_64 org.gimp.GIMP "$@"\n',
      },
    );

    await expect(
      resolveLaunchTarget(['/usr/bin/flatpak', 'run', 'org.gimp.GIMP'], fs),
    ).resolves.toEqual({ kind: 'unsupported', path: '/usr/bin/flatpak', reason: 'flatpak' });
    await expect(
      resolveLaunchTarget(['/var/lib/flatpak/exports/bin/org.gimp.GIMP'], fs),
    ).resolves.toEqual({
      kind: 'unsupported',
      path: '/var/lib/flatpak/exports/bin/org.gimp.GIMP',
      reason: 'flatpak',
    });
  });

  it('recognises a Snap app by the snap launcher its command links to', async () => {
    const fs = fakeFs(['/snap/bin/spotify'], { '/snap/bin/spotify': '/usr/bin/snap' });

    await expect(resolveLaunchTarget(['/snap/bin/spotify'], fs)).resolves.toEqual({
      kind: 'unsupported',
      path: '/snap/bin/spotify',
      reason: 'snap',
    });
  });
});
