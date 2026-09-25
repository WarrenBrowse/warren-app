import argvSplit from 'argv-split';
import child_process from 'child_process';
import fs from 'fs/promises';
import path from 'path';

import {
  ILinuxSplitTunnelingApplication,
  ISplitTunnelingApplication,
} from '../shared/application-types';
import { messages } from '../shared/gettext';
import { LaunchApplicationResult } from '../shared/ipc-schema';
import { Scheduler } from '../shared/scheduler';
import { ExecutableLookup, resolveLaunchTarget } from './linux-app-routing';
import {
  DesktopEntry,
  findIconPath,
  getDesktopEntries,
  getImageDataUrl,
  readDesktopEntry,
  shouldShowApplication,
} from './linux-desktop-entry';

const PROBLEMATIC_APPLICATIONS = {
  launchingInExistingProcess: [
    'brave-browser-stable',
    'chromium-browser',
    'firefox',
    'firefox-esr',
    'google-chrome-stable',
    'mate-terminal',
    'opera',
    'xfce4-terminal',
  ],
  launchingElsewhere: ['gnome-terminal'],
};

// `warren-exclude` runs a program outside the tunnel, `warren-include` runs it
// as one of the only programs inside it (include-only mode).
export type LinuxLauncher = 'warren-exclude' | 'warren-include';

// Launches an application. The application parameter could be a path the an executable or .desktop
// file or an object representing an application
export async function launchApplication(
  app: ILinuxSplitTunnelingApplication | string,
  launcher: LinuxLauncher = 'warren-exclude',
): Promise<LaunchApplicationResult> {
  let excludeArguments: string[];
  try {
    excludeArguments = await getLaunchCommand(app);
  } catch (e) {
    const error = e as Error;
    return { error: error.message };
  }

  return new Promise((resolve, _reject) => {
    const scheduler = new Scheduler();
    const proc = child_process.spawn(launcher, excludeArguments, { detached: true });

    // If the process exits within 200 milliseconds the user is notified that it failed to launch.
    scheduler.schedule(() => {
      proc.removeAllListeners();
      resolve({ success: true });
    }, 200);

    proc.stderr.on('data', (data) => {
      if (data.includes('Failed to launch the process') && data.includes('ENOENT')) {
        scheduler.cancel();
        proc.removeAllListeners();
        resolve({
          error:
            // TRANSLATORS: This error message is shown if the user tries to launch an app that
            // TRANSLATORS: doesn't exist.
            messages.pgettext('split-tunneling-view', 'Please try again or send a problem report.'),
        });
      }
    });
    proc.once('exit', (code) => {
      scheduler.cancel();
      proc.removeAllListeners();

      if (code === 1) {
        resolve({
          error:
            // TRANSLATORS: This error message is shown if an application fails during startup.
            messages.pgettext('split-tunneling-view', 'Please try again or send a problem report.'),
        });
      } else {
        resolve({ success: true });
      }
    });
  });
}

// Takes the same argument as launchApplication and returns the command to run
async function getLaunchCommand(app: ILinuxSplitTunnelingApplication | string): Promise<string[]> {
  if (typeof app === 'object') {
    return formatExec(app.exec);
  } else if (path.extname(app) === '.desktop') {
    const entry = await readDesktopEntry(app);
    if (entry.exec !== undefined) {
      return formatExec(entry.exec);
    } else {
      throw new Error(
        // TRANSLATORS: This error message is shown if the user tries to launch a Linux desktop
        // TRANSLATORS: entry file that doesn't contain the required 'Exec' value.
        messages.pgettext('split-tunneling-view', 'Please send a problem report.'),
      );
    }
  } else {
    return [app];
  }
}

// Removes placeholder arguments and separates command into list of strings
function formatExec(exec: string) {
  return argvSplit(exec).filter((argument: string) => !/%[cdDfFikmnNuUv]/.test(argument));
}

export async function getApplications(locale: string): Promise<ILinuxSplitTunnelingApplication[]> {
  const desktopEntryPaths = await getDesktopEntries();
  const desktopEntries: DesktopEntry[] = [];

  for (const entryPath of desktopEntryPaths) {
    try {
      desktopEntries.push(await readDesktopEntry(entryPath, locale));
    } catch {
      // no-op
    }
  }

  const applications = desktopEntries
    .filter(shouldShowApplication)
    .map(addApplicationWarnings)
    .map(replaceIconNameWithDataUrl);

  return Promise.all(applications);
}

async function replaceIconNameWithDataUrl(
  app: ILinuxSplitTunnelingApplication,
): Promise<ILinuxSplitTunnelingApplication> {
  try {
    // Either the app has no icon or it's already an absolute path.
    if (app.icon === undefined) {
      return app;
    }

    const iconPath = path.isAbsolute(app.icon) ? app.icon : await findIconPath(app.icon);
    if (iconPath === undefined) {
      return app;
    }

    return { ...app, icon: await getImageDataUrl(iconPath) };
  } catch {
    return app;
  }
}

function addApplicationWarnings(
  application: ILinuxSplitTunnelingApplication,
): ILinuxSplitTunnelingApplication {
  const binaryBasename = path.basename(application.exec!.split(' ')[0]);
  if (PROBLEMATIC_APPLICATIONS.launchingInExistingProcess.includes(binaryBasename)) {
    return {
      ...application,
      warning: 'launches-in-existing-process',
    };
  } else if (PROBLEMATIC_APPLICATIONS.launchingElsewhere.includes(binaryBasename)) {
    return {
      ...application,
      warning: 'launches-elsewhere',
    };
  } else {
    return application;
  }
}

const executableLookup: ExecutableLookup = {
  pathEnv: process.env.PATH ?? '',
  isExecutable: async (candidate) => {
    try {
      await fs.access(candidate, fs.constants.X_OK);
      return (await fs.stat(candidate)).isFile();
    } catch {
      return false;
    }
  },
  realpath: (candidate) => fs.realpath(candidate),
  readScript: async (candidate) => {
    try {
      const file = await fs.open(candidate, 'r');
      try {
        const buffer = Buffer.alloc(64 * 1024);
        const { bytesRead } = await file.read(buffer, 0, buffer.length, 0);
        const text = buffer.subarray(0, bytesRead).toString('utf8');
        return text.startsWith('#!') ? text : undefined;
      } finally {
        await file.close();
      }
    } catch {
      return undefined;
    }
  },
};

// The desktop apps keyed by the program they run, which is what a per-app
// country names on Linux. An app whose program cannot be named is kept, keyed
// by its desktop entry and marked with the reason, so its row can say why it
// takes no country. An app whose program cannot be found is left out.
export async function getPathBasedApplications(
  locale: string,
): Promise<ISplitTunnelingApplication[]> {
  const applications: ISplitTunnelingApplication[] = [];
  for (const application of await getApplications(locale)) {
    const target = await resolveLaunchTarget(formatExec(application.exec), executableLookup);
    if (target === undefined) {
      continue;
    }
    const entry: ISplitTunnelingApplication =
      target.kind === 'program'
        ? {
            absolutepath: target.path,
            name: application.name,
            icon: application.icon,
            deletable: false,
          }
        : {
            absolutepath: application.absolutepath,
            name: application.name,
            icon: application.icon,
            deletable: false,
            routingLimitation: target.reason,
          };
    if (!applications.some((known) => known.absolutepath === entry.absolutepath)) {
      applications.push(entry);
    }
  }
  return applications;
}

// A program picked with the file dialog: a desktop entry resolves to the
// program it runs, anything else to the file it links to.
export async function resolveExecutablePath(selected: string): Promise<string> {
  const argv = await getLaunchCommand(selected);
  const target = await resolveLaunchTarget(argv, executableLookup);
  return target?.kind === 'program' ? target.path : selected;
}
