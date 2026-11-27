import log from '../shared/logging';
import { loadOptionalModule } from './optional-module';

export interface OutsideClickForwarder {
  start(onOutsideClick: () => void): () => boolean;
}

interface ShowHideEmitter {
  on(event: 'show' | 'hide', listener: () => void): unknown;
}

// Hides the pinned menu bar window on a click outside it, which window blur
// alone misses on macOS (https://github.com/electron/electron/issues/8689).
//
// The forwarder is a native module and only an enhancement over blur: a
// binary that dyld refuses must cost that enhancement and nothing else. It
// once rejected ahead of the tray click handlers, and the icon stayed on
// screen doing nothing.
export async function installOutsideClickHide(
  window: ShowHideEmitter,
  hide: () => void,
  loadForwarder: () => Promise<OutsideClickForwarder>,
): Promise<boolean> {
  const forwarder = await loadOptionalModule('Outside-click forwarder', loadForwarder);
  if (!forwarder) {
    return false;
  }

  let stop: (() => boolean) | undefined;
  window.on('show', () => {
    try {
      stop = forwarder.start(hide);
    } catch (e) {
      stop = undefined;
      log.error(`Outside-click forwarder failed to start: ${(e as Error).message}`);
    }
  });
  window.on('hide', () => {
    stop?.();
    stop = undefined;
  });
  return true;
}
