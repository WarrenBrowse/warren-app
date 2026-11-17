/**
 * Keeps the window out of screenshots and screen sharing while it shows a
 * secret: a forum sign-in code, or the recovery phrase (on the backup, login
 * and import screens). Several of them can be up at once, so the window is
 * released only when none is left. Electron implements the protection on macOS
 * and Windows only; on Linux the call does nothing.
 */
export class ScreenProtection {
  private forumCode = false;
  private mnemonicScreens = 0;

  public constructor(private readonly apply: (protect: boolean) => void) {}

  public setForumCode(on: boolean): void {
    this.forumCode = on;
    this.update();
  }

  public mnemonicShown(): void {
    this.mnemonicScreens += 1;
    this.update();
  }

  public mnemonicHidden(): void {
    this.mnemonicScreens = Math.max(0, this.mnemonicScreens - 1);
    this.update();
  }

  /**
   * A recreated window starts unprotected, and the screens of the old one are
   * gone without saying so: the new screens announce the phrase again.
   */
  public windowRecreated(): void {
    this.mnemonicScreens = 0;
    this.update();
  }

  private update(): void {
    this.apply(this.forumCode || this.mnemonicScreens > 0);
  }
}
