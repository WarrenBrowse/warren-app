import React from 'react';

import log from '../../../../shared/logging';
import {
  isTorrentClientConfigError,
  ProbeResult,
  TorrentClientConfigUpdate,
  TorrentClientError,
} from '../../../../shared/torrent-client';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

/**
 * The torrent client settings and the state of the last write into it.
 *
 * The password is not here and never will be: the main process keeps it
 * sealed and answers only whether one is stored, which is all the form needs
 * to show a saved password as saved.
 */
export function useTorrentClient() {
  const config = useSelector((state) => state.settings.torrentClient);
  const status = useSelector((state) => state.settings.torrentClientStatus);
  const { setTorrentClientConfig, testTorrentClientConnection, applyTorrentClientPort } =
    useAppContext();

  /**
   * Answers the refusal when there is one, and `undefined` only when the
   * settings really were stored.
   *
   * A call that never reached the main process answers `failed` rather than
   * `undefined`: the form clears the password field on a successful save, and
   * showing a cleared field for a password that was never kept is the one
   * outcome the user cannot recover from on their own.
   */
  const save = React.useCallback(
    async (
      update: TorrentClientConfigUpdate,
    ): Promise<'encryption-unavailable' | 'failed' | undefined> => {
      try {
        const result = await setTorrentClientConfig(update);
        return isTorrentClientConfigError(result) ? result.error : undefined;
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not save the torrent client settings', message);
        return 'failed';
      }
    },
    [setTorrentClientConfig],
  );

  const test = React.useCallback(async (): Promise<ProbeResult | TorrentClientError> => {
    try {
      return await testTorrentClientConnection();
    } catch (error) {
      const message = error instanceof Error ? error.message : '';
      log.error('Could not reach the torrent client', message);
      return { kind: 'unreachable' };
    }
  }, [testTorrentClientConnection]);

  const applyNow = React.useCallback(async () => {
    try {
      await applyTorrentClientPort();
    } catch (error) {
      const message = error instanceof Error ? error.message : '';
      log.error('Could not write the port into the torrent client', message);
    }
  }, [applyTorrentClientPort]);

  return { config, status, save, test, applyNow };
}
