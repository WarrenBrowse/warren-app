import * as fs from 'fs';

export interface AtomicWriteFs {
  writeFileSync(file: string, data: string, options?: fs.WriteFileOptions): void;
  renameSync(from: string, to: string): void;
  rmSync(file: string, options?: fs.RmOptions): void;
}

/**
 * Replaces `filePath` with `data` in one step: the data goes to a temporary
 * file beside it, readable by the user alone, which a rename then puts in
 * place. A write cut short leaves the previous file whole.
 */
export function writeFileAtomic(filePath: string, data: string, fsImpl: AtomicWriteFs = fs) {
  const temporary = `${filePath}.tmp`;
  try {
    fsImpl.writeFileSync(temporary, data, { mode: 0o600 });
    fsImpl.renameSync(temporary, filePath);
  } catch (error) {
    fsImpl.rmSync(temporary, { force: true });
    throw error;
  }
}
