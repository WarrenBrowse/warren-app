import { afterEach, describe, expect, it } from 'vitest';

import { parseChangelog, selectChangelog } from '../../src/main/changelog';
import { ChangelogBlock, ChangelogInline } from '../../src/shared/ipc-types';

const mockPlatform = (platform: string) => {
  Object.defineProperty(process, 'platform', { value: platform });
};

/** Flattens a block's inline runs back to plain text, for assertions. */
const text = (content: ChangelogInline[]): string => content.map((inline) => inline.value).join('');

const listItems = (block: ChangelogBlock): string[] => {
  if (block.type !== 'list') throw new Error(`expected a list block, got ${block.type}`);
  return block.items.map(text);
};

describe('Changelog parser', () => {
  const platform = process.platform;

  afterEach(() => {
    mockPlatform(platform);
  });

  describe('platform filtering', () => {
    // The same item set, tagged for different platforms. A tag may be written
    // with or without a space after the comma.
    const tagged = [
      '- Changelog item 1',
      '- [Windows] Changelog item 2',
      '- [macOS] Changelog item 3',
      '- [linux] Changelog item 4',
      '- [Windows, macOS] Changelog item 5',
      '- [Windows,linux] Changelog item 6',
      '- [Windows, macOS,linux] Changelog item 7',
    ].join('\n');

    it('keeps untagged items and Windows ones on Windows', () => {
      mockPlatform('win32');

      const [list] = parseChangelog(tagged);

      expect(listItems(list)).to.deep.equal([
        'Changelog item 1',
        'Changelog item 2',
        'Changelog item 5',
        'Changelog item 6',
        'Changelog item 7',
      ]);
    });

    it('keeps untagged items and macOS ones on macOS', () => {
      mockPlatform('darwin');

      const [list] = parseChangelog(tagged);

      expect(listItems(list)).to.deep.equal([
        'Changelog item 1',
        'Changelog item 3',
        'Changelog item 5',
        'Changelog item 7',
      ]);
    });

    it('keeps untagged items and Linux ones on Linux', () => {
      mockPlatform('linux');

      const [list] = parseChangelog(tagged);

      expect(listItems(list)).to.deep.equal([
        'Changelog item 1',
        'Changelog item 4',
        'Changelog item 6',
        'Changelog item 7',
      ]);
    });

    it('drops a list block whose every item was filtered out', () => {
      mockPlatform('darwin');

      expect(parseChangelog('- [Windows] Only for Windows')).to.deep.equal([]);
    });
  });

  describe('Markdown structure', () => {
    it('reads a heading as a heading block, not as a bullet', () => {
      // The regression this parser exists for: the release notes served in the
      // update manifest are Markdown, and the previous line-splitting parser
      // rendered '### Added' verbatim as a bullet.
      const [block] = parseChangelog('### Added');

      expect(block).to.deep.equal({
        type: 'heading',
        level: 3,
        content: [{ type: 'text', value: 'Added' }],
      });
    });

    it('joins a bullet wrapped over several lines into one item', () => {
      // CHANGELOG.md is hard-wrapped at 100 columns, so nearly every entry
      // spans several lines. Splitting on newlines turned each continuation
      // into its own bullet.
      const [list] = parseChangelog(
        ['- Tunnel all traffic through a QUIC transport,', '  with a TCP fallback carrier.'].join(
          '\n',
        ),
      );

      expect(listItems(list)).to.deep.equal([
        'Tunnel all traffic through a QUIC transport, with a TCP fallback carrier.',
      ]);
    });

    it('groups consecutive bullets into a single list block', () => {
      const blocks = parseChangelog(['- First', '- Second', '- Third'].join('\n'));

      expect(blocks).to.have.length(1);
      expect(listItems(blocks[0])).to.have.length(3);
    });

    it('starts a new list after a heading', () => {
      const blocks = parseChangelog(['### Added', '- One', '', '### Fixed', '- Two'].join('\n'));

      expect(blocks.map((block) => block.type)).to.deep.equal([
        'heading',
        'list',
        'heading',
        'list',
      ]);
    });

    it('reads free text as a paragraph', () => {
      const [block] = parseChangelog('First public beta release.');

      expect(block.type).to.equal('paragraph');
      expect(block.type === 'paragraph' && text(block.content)).to.equal(
        'First public beta release.',
      );
    });

    it('keeps a blank line as a paragraph boundary', () => {
      const blocks = parseChangelog(['First sentence.', '', 'Second sentence.'].join('\n'));

      expect(blocks).to.have.length(2);
      expect(blocks.every((block) => block.type === 'paragraph')).to.be.true;
    });
  });

  describe('inline formatting', () => {
    it('reads bold, code and links as their own runs', () => {
      const [list] = parseChangelog('- Use **bold**, `code` and [a link](https://example.com).');

      if (list.type !== 'list') throw new Error('expected a list');
      expect(list.items[0]).to.deep.equal([
        { type: 'text', value: 'Use ' },
        { type: 'strong', value: 'bold' },
        { type: 'text', value: ', ' },
        { type: 'code', value: 'code' },
        { type: 'text', value: ' and ' },
        { type: 'link', value: 'a link', href: 'https://example.com' },
        { type: 'text', value: '.' },
      ]);
    });

    it('leaves a link with a non-http target as plain text', () => {
      // The renderer turns a link run into an anchor handed to the OS browser,
      // so only http(s) may become one. CHANGELOG.md carries relative doc
      // paths that must never become clickable.
      const [list] = parseChangelog('- See [the docs](docs/relay-selector.md).');

      if (list.type !== 'list') throw new Error('expected a list');
      expect(list.items[0].some((inline) => inline.type === 'link')).to.be.false;
      expect(text(list.items[0])).to.equal('See the docs.');
    });
  });

  it('returns nothing for an empty changelog', () => {
    expect(parseChangelog('')).to.deep.equal([]);
    expect(parseChangelog('\n\n  \n')).to.deep.equal([]);
  });
});

describe('Changelog language selection', () => {
  const english = 'English notes';
  const translations: Array<[string, string]> = [
    ['fr', 'Notes en francais'],
    ['ro', 'Note in romana'],
    ['pt-BR', 'Notas brasileiras'],
    ['pt', 'Notas portuguesas'],
  ];

  it('picks the translation matching the app language', () => {
    expect(selectChangelog(english, translations, 'fr')).to.equal('Notes en francais');
    expect(selectChangelog(english, translations, 'ro')).to.equal('Note in romana');
  });

  it('falls back to the base language for a regional variant', () => {
    // The app reports locales such as `fr-FR`, and the notes are translated per
    // language, not per region.
    expect(selectChangelog(english, translations, 'fr-FR')).to.equal('Notes en francais');
    expect(selectChangelog(english, translations, 'fr_FR')).to.equal('Notes en francais');
  });

  it('prefers an exact regional translation over its base language', () => {
    expect(selectChangelog(english, translations, 'pt-BR')).to.equal('Notas brasileiras');
    expect(selectChangelog(english, translations, 'pt-PT')).to.equal('Notas portuguesas');
  });

  it('falls back to English for an untranslated language', () => {
    expect(selectChangelog(english, translations, 'de')).to.equal(english);
    expect(selectChangelog(english, [], 'fr')).to.equal(english);
  });

  it('falls back to English rather than showing a blank translation', () => {
    // A release published with an empty entry for a language must not blank the
    // release notes for everyone running in it.
    expect(selectChangelog(english, [['fr', '   ']], 'fr')).to.equal(english);
  });
});
