// How many words of a freshly shown recovery phrase the user types back before
// the account is used. Three out of twelve is what a person who wrote the
// phrase down answers in seconds and someone who only ticked a box cannot.
export const BACKUP_CHECK_WORD_COUNT = 3;

/**
 * Draws the zero-based positions of the words to ask for, distinct and in
 * phrase order. `random` is `Math.random` outside tests.
 */
export function pickBackupCheckPositions(
  wordCount: number,
  random: () => number = Math.random,
): number[] {
  const wanted = Math.min(BACKUP_CHECK_WORD_COUNT, wordCount);
  const positions = new Set<number>();
  while (positions.size < wanted) {
    positions.add(Math.min(wordCount - 1, Math.floor(random() * wordCount)));
  }
  return [...positions].sort((a, b) => a - b);
}

export function backupCheckPasses(
  mnemonic: string,
  positions: number[],
  answers: string[],
): boolean {
  const words = mnemonic.trim().toLowerCase().split(/\s+/);
  return (
    positions.length > 0 &&
    positions.length === answers.length &&
    positions.every((position, index) => {
      const answer = answers[index].trim().toLowerCase();
      return answer !== '' && answer === words[position];
    })
  );
}
