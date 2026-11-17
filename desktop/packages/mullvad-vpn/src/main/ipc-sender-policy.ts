import { pathToFileURL } from 'url';

export interface AppPageLocation {
  // The `index.html` the main window loads in a packaged or production build.
  indexPath: string;
  // `VITE_DEV_SERVER_URL` in a development build, undefined otherwise.
  devServerUrl: string | undefined;
}

/**
 * Whether an IPC message comes from the app's own page: its `index.html`, on
 * any route, or the development server of a development build. Anything else
 * (another file, a web page, a frame with no URL) is refused.
 */
export function appPageSenderPolicy({ indexPath, devServerUrl }: AppPageLocation) {
  const appPage = pathToFileURL(indexPath).href;
  const devOrigin = devServerUrl ? new URL(devServerUrl).origin : undefined;

  return (senderUrl: string | undefined): boolean => {
    if (senderUrl === undefined) {
      return false;
    }
    if (senderUrl.split('#')[0] === appPage) {
      return true;
    }
    try {
      return devOrigin !== undefined && new URL(senderUrl).origin === devOrigin;
    } catch {
      return false;
    }
  };
}
