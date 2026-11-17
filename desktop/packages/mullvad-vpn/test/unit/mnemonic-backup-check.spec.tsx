import { renderToStaticMarkup } from 'react-dom/server';
import { ServerStyleSheet } from 'styled-components';
import { describe, expect, it, vi } from 'vitest';

// The renderer component library reads `window.env.platform` at top level.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import {
  backupCheckPasses,
  BackupCheckStep,
  pickBackupCheckPositions,
} from '../../src/renderer/components/warren-mnemonic';

const PHRASE = 'legal winner thank year wave sausage worth useful legal winner thank yellow'.split(
  ' ',
);

function sequence(values: number[]): () => number {
  let index = 0;
  return () => values[index++ % values.length];
}

function render(element: React.ReactElement): string {
  const sheet = new ServerStyleSheet();
  try {
    return renderToStaticMarkup(sheet.collectStyles(element));
  } finally {
    sheet.seal();
  }
}

// Ticking "I have written it down" proves nothing: the check asks for words
// only someone holding the phrase can give, before the account is used.
describe('the recovery phrase backup check', () => {
  it('asks for three distinct positions of the phrase, in order', () => {
    // 0.99 twice draws the last position twice: the duplicate is redrawn.
    const positions = pickBackupCheckPositions(12, sequence([0.99, 0.99, 0.0, 0.5]));

    expect(positions).toEqual([0, 6, 11]);
  });

  it('passes when every asked word is given, whatever the case and spacing', () => {
    expect(backupCheckPasses(PHRASE.join(' '), [0, 6, 11], ['Legal ', ' worth', 'YELLOW'])).toBe(
      true,
    );
  });

  it('fails when a word is wrong', () => {
    expect(backupCheckPasses(PHRASE.join(' '), [0, 6, 11], ['legal', 'worth', 'year'])).toBe(false);
  });

  it('fails when a word is missing', () => {
    expect(backupCheckPasses(PHRASE.join(' '), [0, 6, 11], ['legal', '', 'yellow'])).toBe(false);
  });

  it('names the asked words by their position, counted from one', () => {
    const html = render(
      <BackupCheckStep
        positions={[0, 6, 11]}
        answers={['', '', '']}
        onAnswerChange={() => {}}
        mismatch={false}
        onContinue={() => {}}
        onBack={() => {}}
      />,
    );

    expect(html).toContain('Word 1');
    expect(html).toContain('Word 7');
    expect(html).toContain('Word 12');
    expect(html).not.toContain('These words do not match');
  });

  it('says the words do not match after a wrong answer', () => {
    const html = render(
      <BackupCheckStep
        positions={[0, 6, 11]}
        answers={['legal', 'worth', 'year']}
        onAnswerChange={() => {}}
        mismatch={true}
        onContinue={() => {}}
        onBack={() => {}}
      />,
    );

    expect(html).toContain('These words do not match your recovery phrase.');
  });
});
