import { sprintf } from 'sprintf-js';

import { messages } from '../../../../../../../../shared/gettext';

// Getters, never plain values: a value property would evaluate pgettext at
// module load, before `loadTranslations` has filled the catalogue, pinning
// the English msgid in every locale (the update screen once said "Download
// complete!" in the middle of a French UI). A getter runs at render time,
// after the locale is loaded. The `lazy-gettext/no-module-scope-gettext`
// lint rule enforces this for the whole renderer.
export const translations = {
  get downloadComplete() {
    // TRANSLATORS: Status text displayed below a progress bar when the download of an update is complete
    return messages.pgettext('app-upgrade-view', 'Download complete!');
  },
  get downloadFailed() {
    // TRANSLATORS: Status text displayed below a progress bar when the download of an update fails
    return messages.pgettext('app-upgrade-view', 'Download failed');
  },
  get downloadFewSecondsRemaining() {
    // TRANSLATORS: Status text displayed below a progress bar when the update is being downloaded
    // TRANSLATORS: with the estimated time of completion is within a few seconds.
    return messages.pgettext('app-upgrade-view', 'A few seconds remaining...');
  },
  get downloadPaused() {
    // TRANSLATORS: Status text displayed below a progress bar when the download of an update has been paused
    return messages.pgettext('app-upgrade-view', 'Download paused');
  },
  get downloadStarting() {
    // TRANSLATORS: Status text displayed below a progress bar when the download of an update is starting
    return messages.pgettext('app-upgrade-view', 'Starting download...');
  },
  getDownloadMinutesRemaining: (minutes: number) =>
    sprintf(
      // TRANSLATORS: Status text displayed below a progress bar when the update is being downloaded
      // TRANSLATORS: with the estimated time of completion represented in minutes.
      // TRANSLATORS: Available placeholders:
      // TRANSLATORS: %(minutes)s - Will be replaced with remaining minutes until download is complete
      messages.pgettext('app-upgrade-view', 'About %(minutes)s minutes remaining...'),
      {
        minutes,
      },
    ),
  getDownloadSecondsRemaining: (seconds: number) =>
    sprintf(
      // TRANSLATORS: Status text displayed below a progress bar when the update is being downloaded
      // TRANSLATORS: with the estimated time of completion represented in seconds.
      // TRANSLATORS: Available placeholders:
      // TRANSLATORS: %(second)s - Will be replaced with remaining seconds until download is complete
      messages.pgettext('app-upgrade-view', 'About %(seconds)s seconds remaining...'),
      {
        seconds,
      },
    ),
};
