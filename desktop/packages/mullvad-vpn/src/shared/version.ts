import { Url, urls } from './constants';

// The website has a single download page that detects the platform itself,
// so no per-platform or beta path is appended (those routes do not exist).
export function getDownloadUrl(_suggestedIsBeta: boolean): Url {
  return urls.download;
}
