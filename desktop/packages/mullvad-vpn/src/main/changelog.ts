import fs from 'fs';
import path from 'path';

import { ChangelogBlock, ChangelogInline, IChangelog } from '../shared/ipc-types';
import log from '../shared/logging';

// Reads and parses the changelog file.
export function readChangelog(): IChangelog {
  try {
    const changelogPath = path.join(import.meta.dirname, '..', 'changes.txt');
    const contents = fs.readFileSync(changelogPath).toString();
    return parseChangelog(contents);
  } catch (e) {
    const error = e as Error;
    log.error('Failed to read changelog.txt', error.message);
    return [];
  }
}

// Resolved once at startup, like the gettext catalogs, because the release
// notes reach the conversion layer far from where the locale is detected.
let appLocale = 'en';

export function setChangelogLocale(locale: string) {
  appLocale = locale;
}

/**
 * Resolves the release notes offered by an update into renderable blocks, in
 * the app's own language when the release was published with a translation.
 */
export function parseSuggestedUpgradeChangelog(
  english: string,
  translations: Array<[string, string]>,
): IChangelog {
  return parseChangelog(selectChangelog(english, translations, appLocale));
}

/**
 * Picks the release notes to show, given the ones the update manifest carries
 * per language and the language the app runs in.
 *
 * English is the fallback for anything not translated, and for a translation
 * that came through empty: blank notes read as "this release changed nothing",
 * which is worse than notes in another language.
 */
export function selectChangelog(
  english: string,
  translations: Array<[string, string]>,
  locale: string,
): string {
  const byTag = new Map(translations.map(([tag, text]) => [tag.toLowerCase(), text]));
  const normalized = locale.replace('_', '-').toLowerCase();
  // Regional first: pt-BR and pt are genuinely different translations, while
  // fr-FR is served by fr.
  const candidates = [normalized, normalized.split('-')[0]];

  for (const candidate of candidates) {
    const translated = byTag.get(candidate);
    if (translated !== undefined && translated.trim() !== '') {
      return translated;
    }
  }

  return english;
}

const HEADING = /^(#{1,6})\s+(.+)$/;
const BULLET = /^[-*]\s+(.+)$/;
const PLATFORM_TAG = /^\[.*?\]/;
const INLINE = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)]+)\)/g;

/**
 * Parses the release notes into renderable blocks.
 *
 * The notes are Markdown: the update manifest carries the CHANGELOG section of
 * the offered version verbatim, hard-wrapped at 100 columns. Splitting it on
 * newlines showed every `###` heading and every `-` marker literally, and broke
 * each wrapped entry into separate bullets, so resolving that structure before
 * it reaches the renderer is the whole point of this parser.
 */
export function parseChangelog(changelog: string): IChangelog {
  const blocks: ChangelogBlock[] = [];
  let paragraphLines: string[] = [];
  let listItems: string[] = [];

  const flushParagraph = () => {
    if (paragraphLines.length === 0) return;
    const text = keepForPlatform(paragraphLines.join(' '));
    paragraphLines = [];
    if (text !== undefined) {
      blocks.push({ type: 'paragraph', content: parseInline(text) });
    }
  };

  const flushList = () => {
    if (listItems.length === 0) return;
    const items = listItems
      .map(keepForPlatform)
      .filter((item): item is string => item !== undefined)
      .map(parseInline);
    listItems = [];
    // Every entry of a platform-scoped list can be filtered out, and an empty
    // list would still render its bullet frame.
    if (items.length > 0) {
      blocks.push({ type: 'list', items });
    }
  };

  const flush = () => {
    flushParagraph();
    flushList();
  };

  for (const line of changelog.split('\n')) {
    const trimmed = line.trim();

    if (trimmed === '') {
      flush();
      continue;
    }

    const heading = HEADING.exec(trimmed);
    if (heading) {
      flush();
      blocks.push({
        type: 'heading',
        level: heading[1].length,
        content: parseInline(heading[2].trim()),
      });
      continue;
    }

    const bullet = BULLET.exec(trimmed);
    if (bullet) {
      flushParagraph();
      listItems.push(bullet[1].trim());
      continue;
    }

    // An indented line continues whatever precedes it: CHANGELOG.md wraps its
    // entries at 100 columns and indents the continuation by two spaces.
    const isContinuation = line !== trimmed;
    if (isContinuation && listItems.length > 0) {
      listItems[listItems.length - 1] += ` ${trimmed}`;
      continue;
    }

    flushList();
    paragraphLines.push(trimmed);
  }

  flush();

  return blocks;
}

/**
 * Applies the `[Windows, macOS]` scoping prefix, returning the text without it,
 * or `undefined` when the entry is not meant for the running platform.
 */
function keepForPlatform(text: string): string | undefined {
  const platforms = text
    .match(PLATFORM_TAG)
    ?.flatMap((match) => match.slice(1, -1).split(','))
    .map((platform) => platform.trim());

  if (!platforms || isPlatform(platforms)) {
    return text.replace(PLATFORM_TAG, '').trim();
  }
  return undefined;
}

/** Resolves Markdown emphasis, inline code and links into typed runs. */
function parseInline(text: string): ChangelogInline[] {
  const runs: ChangelogInline[] = [];
  const pushText = (value: string) => {
    if (value === '') return;
    const last = runs[runs.length - 1];
    if (last?.type === 'text') {
      last.value += value;
    } else {
      runs.push({ type: 'text', value });
    }
  };

  let index = 0;
  INLINE.lastIndex = 0;
  let match = INLINE.exec(text);
  while (match !== null) {
    pushText(text.slice(index, match.index));

    const [whole, strong, code, linkLabel, href] = match;
    if (strong !== undefined) {
      runs.push({ type: 'strong', value: strong });
    } else if (code !== undefined) {
      runs.push({ type: 'code', value: code });
    } else if (isExternalUrl(href)) {
      runs.push({ type: 'link', value: linkLabel, href });
    } else {
      // A relative documentation path (CHANGELOG.md carries a few) must not
      // become an anchor: the renderer hands an anchor's target to the OS
      // browser, so only an absolute web URL may become one.
      pushText(linkLabel);
    }

    index = match.index + whole.length;
    match = INLINE.exec(text);
  }
  pushText(text.slice(index));

  return runs;
}

function isExternalUrl(href: string): boolean {
  try {
    const { protocol } = new URL(href);
    return protocol === 'https:' || protocol === 'http:';
  } catch {
    return false;
  }
}

// Checks if an OS name corresponds to the current platform.
function isPlatform(platformNames: Array<string>): boolean {
  const platforms = platformNames.map((platformName) => {
    switch (platformName.toLowerCase()) {
      case 'windows':
        return 'win32';
      case 'macos':
        return 'darwin';
      case 'linux':
        return 'linux';
      default:
        return platformName;
    }
  });

  return platforms.includes(process.platform);
}
