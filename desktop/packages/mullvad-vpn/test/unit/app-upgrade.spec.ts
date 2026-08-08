import { describe, expect, it, vi } from 'vitest';

// The module under test pulls in the gRPC client transitively, which reads
// Electron's `app.commandLine` at load time; only the decision logic is under
// test here, so stub the daemon-rpc module out.
vi.mock('../../src/main/daemon-rpc', () => ({
  DaemonRpc: class {},
  SubscriptionListener: class {},
}));

import { InstallerStartEffects, startVerifiedInstaller } from '../../src/main/app-upgrade';
import { IAppVersionInfo } from '../../src/shared/daemon-rpc-types';

function versionInfo(verifiedInstallerPath?: string): IAppVersionInfo {
  return {
    supported: true,
    suggestedUpgrade: {
      version: '1.1.8',
      changelog: [],
      verifiedInstallerPath,
    },
  };
}

function makeEffects(overrides: Partial<InstallerStartEffects> = {}): InstallerStartEffects {
  return {
    getVersionInfo: vi.fn().mockResolvedValue(versionInfo('/cache/warren-1.1.8.pkg')),
    restartUpgrade: vi.fn(),
    launchInstaller: vi.fn().mockResolvedValue(undefined),
    notifyStartFailed: vi.fn(),
    ...overrides,
  };
}

describe('startVerifiedInstaller', () => {
  it('launches the verified installer the fresh version info points at', async () => {
    const effects = makeEffects();
    await startVerifiedInstaller(effects);
    expect(effects.launchInstaller).toHaveBeenCalledWith('/cache/warren-1.1.8.pkg');
    expect(effects.restartUpgrade).not.toHaveBeenCalled();
    expect(effects.notifyStartFailed).not.toHaveBeenCalled();
  });

  it('restarts the upgrade when a newer release superseded the downloaded installer', async () => {
    // A fresh manifest that no longer carries a verified path means the
    // download belongs to an outdated version: launching it would install
    // the wrong release, and doing nothing wedges the update screen.
    const effects = makeEffects({
      getVersionInfo: vi.fn().mockResolvedValue(versionInfo(undefined)),
    });
    await startVerifiedInstaller(effects);
    expect(effects.restartUpgrade).toHaveBeenCalledOnce();
    expect(effects.launchInstaller).not.toHaveBeenCalled();
    expect(effects.notifyStartFailed).not.toHaveBeenCalled();
  });

  it('restarts the upgrade when no upgrade is suggested at all anymore', async () => {
    const effects = makeEffects({
      getVersionInfo: vi
        .fn()
        .mockResolvedValue({ supported: true, suggestedUpgrade: undefined } as IAppVersionInfo),
    });
    await startVerifiedInstaller(effects);
    expect(effects.restartUpgrade).toHaveBeenCalledOnce();
    expect(effects.launchInstaller).not.toHaveBeenCalled();
  });

  it('notifies the renderer when fetching version info fails, and never throws', async () => {
    // The silent variant of this failure left the renderer waiting on
    // "Starting installer..." forever.
    const effects = makeEffects({
      getVersionInfo: vi.fn().mockRejectedValue(new Error('gRPC down')),
    });
    await expect(startVerifiedInstaller(effects)).resolves.toBeUndefined();
    expect(effects.notifyStartFailed).toHaveBeenCalledOnce();
    expect(effects.launchInstaller).not.toHaveBeenCalled();
    expect(effects.restartUpgrade).not.toHaveBeenCalled();
  });

  it('notifies the renderer when launching the installer fails, and never throws', async () => {
    const effects = makeEffects({
      launchInstaller: vi.fn().mockRejectedValue(new Error('installer file vanished')),
    });
    await expect(startVerifiedInstaller(effects)).resolves.toBeUndefined();
    expect(effects.notifyStartFailed).toHaveBeenCalledOnce();
  });
});
