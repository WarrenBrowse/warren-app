import { execFile } from 'child_process';
import { randomUUID } from 'crypto';
import { app, shell } from 'electron';
import * as path from 'path';

import log from '../shared/logging';
import { IpcMainEventChannel } from './ipc-event-channel';
import { resolveBin } from './proc';

export function registerIpcListeners() {
  IpcMainEventChannel.problemReport.handleViewLog((savedReportId) => {
    const problemReportPath = getProblemReportPath(savedReportId);
    if (process.platform === 'linux') {
      // As of this upstream PR[1] the underlying C implementation for
      // shell.openPath no longer waits for the process to exit, which
      // means that the callback in the C code will never be called.
      //
      // That callback is what eventually causes the promise returned
      // by shell.openPath to resolve, and as it is never being called,
      // the promise will never be resolved.
      //
      // Because of that, we just invoke shell.openPath and return a
      // promise resolved with an empty string, the same signature as
      // returned from shell.openPath.
      //
      // [1] https://github.com/electron/electron/pull/48079
      void shell.openPath(problemReportPath);
      return Promise.resolve('');
    }

    return shell.openPath(problemReportPath);
  });
}

// Also used by the forum attach-logs flow (forum-attach.ts): the report is
// collected when the deep link arrives so the consent prompt can show it.
export function collectLogs(toRedact?: string): Promise<string> {
  const id = randomUUID();
  const reportPath = getProblemReportPath(id);
  const executable = resolveBin('warren-problem-report');
  const args = ['collect', '--output', reportPath];
  if (toRedact) {
    args.push('--redact', toRedact);
  }

  return new Promise((resolve, reject) => {
    execFile(executable, args, { windowsHide: true }, (error, stdout, stderr) => {
      if (error) {
        log.error(
          `Failed to collect a problem report.
            Stdout: ${stdout.toString()}
            Stderr: ${stderr.toString()}`,
        );
        reject(error.message);
      } else {
        log.verbose(`Problem report was written to ${reportPath}`);
        resolve(id);
      }
    });
  });
}

// The id is always a `randomUUID()` the main process issued (see
// `collectLogs`), but it crosses the IPC boundary from the renderer
// before being joined into a filesystem path. Validate it against the
// canonical UUID shape so a malicious/buggy renderer cannot inject path
// traversal (e.g. `../../etc/passwd`) into the report path.
const UUID_REGEX = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function getProblemReportPath(id: string): string {
  if (!UUID_REGEX.test(id)) {
    throw new Error('Invalid problem report id');
  }
  return path.join(app.getPath('temp'), `${id}.log`);
}
