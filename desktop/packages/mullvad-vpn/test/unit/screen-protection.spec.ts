import { beforeEach, describe, expect, it } from 'vitest';

import { ScreenProtection } from '../../src/main/screen-protection';

describe('keeping the window out of screen captures', () => {
  let applied: boolean[];
  let protection: ScreenProtection;

  beforeEach(() => {
    applied = [];
    protection = new ScreenProtection((protect) => applied.push(protect));
  });

  it('protects while a recovery phrase is on screen', () => {
    protection.mnemonicShown();
    protection.mnemonicHidden();

    expect(applied).toEqual([true, false]);
  });

  it('stays protected while a sign-in code is still up when the phrase leaves', () => {
    protection.setForumCode(true);
    protection.mnemonicShown();
    protection.mnemonicHidden();

    expect(applied.at(-1)).toBe(true);
  });

  it('stays protected while another screen still shows the phrase', () => {
    protection.mnemonicShown();
    protection.mnemonicShown();
    protection.mnemonicHidden();

    expect(applied.at(-1)).toBe(true);
  });

  it('protects a recreated window only for what its new screens show again', () => {
    protection.mnemonicShown();
    protection.setForumCode(true);
    protection.windowRecreated();
    protection.setForumCode(false);

    expect(applied.at(-1)).toBe(false);
  });
});
