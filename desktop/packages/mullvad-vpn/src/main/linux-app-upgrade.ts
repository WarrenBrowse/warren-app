import { spawn } from 'child_process';
import fs from 'fs';
import path from 'path';

import { productAnchors } from '../shared/constants/product-env';

// In-app upgrade on Linux. The GUI cannot install a package (that takes root),
// so it asks the daemon, which verified the download, to hand it to the
// package manager. The upgrade runs detached from both: the package's scripts
// stop the daemon and close this GUI halfway through. Two things follow from
// that. The outcome is read from the status file the job writes, never from a
// process of ours. And the app is brought back by a small shell script started
// before the upgrade, which outlives the GUI it replaces.

export type UpgradeStatus = { state: 'running' } | { state: 'exited'; code: number };

/** Parse the status file written by the daemon ("running") and the job ("exit <code>"). */
export function parseUpgradeStatus(contents: string | undefined): UpgradeStatus | undefined {
  const line = contents?.trim();
  if (line === 'running') {
    return { state: 'running' };
  }
  const match = line?.match(/^exit (\d+)$/);
  return match ? { state: 'exited', code: Number(match[1]) } : undefined;
}

/** Effects of [`runLinuxUpgrade`], injected so the decisions are testable. */
export interface LinuxUpgradeEffects {
  /** Ask the daemon to start the upgrade job; resolves to its status file. */
  installUpgrade(): Promise<string>;
  spawnRelauncher(statusPath: string): void;
  readStatus(statusPath: string): Promise<string | undefined>;
  sleep(ms: number): Promise<void>;
  notifyStarted(): void;
  /** The package manager ran and failed: the app is still the old version. */
  notifyInstallFailed(): void;
  /** No upgrade job could be started at all. */
  notifyStartFailed(): void;
}

const POLL_INTERVAL_MS = 1000;
// An apt waiting out an unattended upgrade holds the dpkg lock for up to ten
// minutes before it even starts (the job's lock timeout), so give it longer.
const POLL_LIMIT = 30 * 60;

/** Start the upgrade and follow it until it ends or this process is closed by it. Never throws. */
export async function runLinuxUpgrade(effects: LinuxUpgradeEffects): Promise<void> {
  let statusPath: string;
  try {
    statusPath = await effects.installUpgrade();
  } catch {
    effects.notifyStartFailed();
    return;
  }

  effects.spawnRelauncher(statusPath);
  effects.notifyStarted();

  for (let i = 0; i < POLL_LIMIT; i++) {
    const status = parseUpgradeStatus(await effects.readStatus(statusPath));
    if (status?.state === 'exited') {
      if (status.code !== 0) {
        effects.notifyInstallFailed();
      }
      return;
    }
    if (status === undefined) {
      return;
    }
    await effects.sleep(POLL_INTERVAL_MS);
  }
}

/**
 * Waits for the upgrade job to finish, then starts the app again unless the
 * old one is still running (the upgrade failed before closing it, and that
 * GUI shows the failure). Arguments: status file, GUI pid, launcher, and the
 * seconds to let the package scripts settle before deciding.
 */
export const RELAUNCHER_SCRIPT = `status="$1"; gui="$2"; launcher="$3"; settle="$4"
i=0
while [ "$i" -lt ${POLL_LIMIT} ]; do
  case "$(cat "$status" 2>/dev/null)" in
    running*) ;;
    *) break ;;
  esac
  sleep 1
  i=$((i + 1))
done
sleep "$settle"
if kill -0 "$gui" 2>/dev/null; then exit 0; fi
exec "$launcher"
`;

/**
 * The script the desktop entry runs, which picks the sandbox flag the kernel
 * allows. Packaging renames it to the package name and the Electron binary
 * beside it to warren-gui (tasks/distribution.cjs, the Linux afterPack).
 */
function launcherPath(): string {
  const launcher = path.join(path.dirname(process.execPath), productAnchors.unixProductDir);
  return fs.existsSync(launcher) ? launcher : process.execPath;
}

export function spawnRelauncher(statusPath: string): void {
  // `detached` puts it in a session of its own, and its name is not the GUI's,
  // so neither the pkill of the package scripts nor our exit takes it down.
  const child = spawn(
    '/bin/sh',
    [
      '-c',
      RELAUNCHER_SCRIPT,
      'warren-relauncher',
      statusPath,
      String(process.pid),
      launcherPath(),
      '2',
    ],
    { detached: true, stdio: 'ignore' },
  );
  child.unref();
}

export async function readStatus(statusPath: string): Promise<string | undefined> {
  try {
    return await fs.promises.readFile(statusPath, 'utf8');
  } catch {
    return undefined;
  }
}
