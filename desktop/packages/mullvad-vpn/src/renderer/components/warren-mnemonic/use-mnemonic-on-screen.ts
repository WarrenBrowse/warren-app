import { useEffect } from 'react';

/**
 * Tells main that the recovery phrase is visible for as long as `visible`
 * holds and the calling component is mounted, so the window stays out of
 * screenshots and screen sharing meanwhile.
 */
export function useMnemonicOnScreen(visible: boolean) {
  useEffect(() => {
    if (!visible) {
      return;
    }
    window.ipc.app.mnemonicOnScreen(true);
    return () => window.ipc.app.mnemonicOnScreen(false);
  }, [visible]);
}
