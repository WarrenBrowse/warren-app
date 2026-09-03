import { WarrenAnnouncement } from '../shared/daemon-rpc-types';

// The destination of an announcement's call to action, ready for the system
// browser, or `undefined` when nothing may be opened.
//
// It cannot go through the app's own link allowlist (`shared/constants/urls`):
// that list is Warren's own pages, spelled out in full, and an operator writes
// the announcement's URL. Every call to action whose destination was not
// already one of those ten paths therefore opened nothing at all, with no
// error and no feedback on the card.
//
// What admits the URL instead is the snapshot the daemon published: the
// announcements are verified against the pinned server key before they reach
// the main process, so a URL that is one of their `cta.url` is a URL the
// operator signed. `https` only, as the daemon and the card already require.
//
// The two sides are compared as parsed URLs, not as strings: the renderer hands
// over the href of the URL it parsed, which normalises what the operator wrote
// (`https://warren.ro` becomes `https://warren.ro/`).
export function announcementUrlToOpen(
  url: string,
  announcements: WarrenAnnouncement[] | undefined,
): string | undefined {
  const requested = httpsHref(url);
  if (requested === undefined) {
    return undefined;
  }
  const offered = (announcements ?? []).some(
    (announcement) => announcement.cta !== null && httpsHref(announcement.cta.url) === requested,
  );
  return offered ? requested : undefined;
}

function httpsHref(url: string): string | undefined {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return undefined;
  }
  return parsed.protocol === 'https:' ? parsed.href : undefined;
}
