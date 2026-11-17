import { describe, expect, it } from 'vitest';

import { openDialogOptions } from '../../src/main/open-dialog-options';

// The renderer asks for a file picker; the main process decides where it opens
// and what it may do. Only the fields the app's own pickers use pass through.
describe('the file picker options taken from the renderer', () => {
  it('keep what the pickers ask for, and start in the home directory', () => {
    expect(
      openDialogOptions(
        {
          properties: ['openFile'],
          buttonLabel: 'Import',
          filters: [{ name: 'Warren settings file', extensions: ['json'] }],
        },
        '/home/user',
      ),
    ).toEqual({
      defaultPath: '/home/user',
      properties: ['openFile'],
      buttonLabel: 'Import',
      filters: [{ name: 'Warren settings file', extensions: ['json'] }],
    });
  });

  it('drop any other field, the start directory included', () => {
    expect(
      openDialogOptions(
        {
          properties: ['openFile', 'openDirectory', 'showHiddenFiles'],
          defaultPath: '/etc',
          securityScopedBookmarks: true,
          message: 'Pick your keychain',
        } as never,
        '/home/user',
      ),
    ).toEqual({ defaultPath: '/home/user', properties: ['openFile'] });
  });
});
