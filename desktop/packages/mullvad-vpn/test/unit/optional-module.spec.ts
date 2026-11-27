import { describe, expect, it } from 'vitest';

import { loadOptionalModule } from '../../src/main/optional-module';

// Native modules (windows-utils, nseventforwarder) can be refused by the
// platform loader. Startup awaited them before the daemon connection and the
// tray existed, so a refused binary left a process with nothing to click.
describe('loadOptionalModule', () => {
  it('hands back the module when it loads', async () => {
    const module = { start: () => true };

    await expect(loadOptionalModule('forwarder', () => Promise.resolve(module))).resolves.toBe(
      module,
    );
  });

  it('resolves to undefined instead of rejecting when the module cannot be loaded', async () => {
    await expect(
      loadOptionalModule('forwarder', () => Promise.reject(new Error('dlopen refused'))),
    ).resolves.toBeUndefined();
  });
});
