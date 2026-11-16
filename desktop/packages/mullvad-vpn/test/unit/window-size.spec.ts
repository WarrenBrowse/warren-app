import { describe, expect, it } from 'vitest';

import WindowController from '../../src/main/window-controller';

describe('the desktop window', () => {
  it('opens at the size the home screen is laid out for', () => {
    // Pinned on every platform: the Windows correction only applies unpinned.
    expect(WindowController.getContentSize(false)).toEqual({ width: 380, height: 620 });
  });
});
