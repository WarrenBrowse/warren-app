import { app } from 'electron';
import * as fs from 'fs';
import * as path from 'path';

import { IGuiSettingsState, SYSTEM_PREFERRED_LOCALE_KEY } from '../shared/gui-settings-state';
import log from '../shared/logging';

const settingsSchema: Record<keyof IGuiSettingsState, string> = {
  preferredLocale: 'string',
  autoConnect: 'boolean',
  enableSystemNotifications: 'boolean',
  forumNotifications: 'boolean',
  monochromaticIcon: 'boolean',
  startMinimized: 'boolean',
  unpinnedWindow: 'boolean',
  browsedForSplitTunnelingApplications: 'Array<string>',
  changelogDisplayedForVersion: 'string',
  updateDismissedForVersion: 'string',
  animateMap: 'boolean',
  onboardingCompletedUnix: 'number',
  backupPending: 'boolean',
  pendingPurchases: 'Array<string>',
};

const defaultSettings: IGuiSettingsState = {
  preferredLocale: SYSTEM_PREFERRED_LOCALE_KEY,
  autoConnect: false,
  enableSystemNotifications: true,
  forumNotifications: true,
  monochromaticIcon: false,
  startMinimized: false,
  unpinnedWindow: process.platform !== 'win32' && process.platform !== 'darwin',
  browsedForSplitTunnelingApplications: [],
  changelogDisplayedForVersion: '',
  updateDismissedForVersion: '',
  animateMap: true,
  backupPending: false,
  pendingPurchases: [],
};

export default class GuiSettings {
  public onChange?: (newState: IGuiSettingsState, oldState: IGuiSettingsState) => void;

  private stateValue: IGuiSettingsState = { ...defaultSettings };

  get state(): IGuiSettingsState {
    return this.stateValue;
  }

  set preferredLocale(newValue: string) {
    this.changeStateAndNotify({ ...this.stateValue, preferredLocale: newValue });
  }

  get preferredLocale(): string {
    return this.stateValue.preferredLocale;
  }

  set enableSystemNotifications(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, enableSystemNotifications: newValue });
  }

  get enableSystemNotifications(): boolean {
    return this.stateValue.enableSystemNotifications;
  }

  // Community-forum banner and tray dot (see gui-settings-state.ts). Absent
  // in a settings file written before the setting existed, and on there:
  // a user who already has a forum account wants their replies.
  set forumNotifications(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, forumNotifications: newValue });
  }

  get forumNotifications(): boolean {
    return this.stateValue.forumNotifications ?? true;
  }

  set autoConnect(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, autoConnect: newValue });
  }

  get autoConnect(): boolean {
    return this.stateValue.autoConnect;
  }

  set monochromaticIcon(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, monochromaticIcon: newValue });
  }

  get monochromaticIcon(): boolean {
    return this.stateValue.monochromaticIcon;
  }

  set startMinimized(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, startMinimized: newValue });
  }

  get startMinimized(): boolean {
    return this.stateValue.startMinimized;
  }

  set unpinnedWindow(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, unpinnedWindow: newValue });
  }

  get unpinnedWindow(): boolean {
    return this.stateValue.unpinnedWindow;
  }

  public addBrowsedForSplitTunnelingApplications(newApp: string) {
    this.changeStateAndNotify({
      ...this.stateValue,
      browsedForSplitTunnelingApplications: [...this.browsedForSplitTunnelingApplications, newApp],
    });
  }

  public deleteBrowsedForSplitTunnelingApplications(path: string) {
    this.changeStateAndNotify({
      ...this.stateValue,
      browsedForSplitTunnelingApplications: this.browsedForSplitTunnelingApplications.filter(
        (application) => application !== path,
      ),
    });
  }

  get browsedForSplitTunnelingApplications(): Array<string> {
    return this.stateValue.browsedForSplitTunnelingApplications;
  }

  set changelogDisplayedForVersion(newValue: string | undefined) {
    this.changeStateAndNotify({ ...this.stateValue, changelogDisplayedForVersion: newValue ?? '' });
  }

  get changelogDisplayedForVersion(): string | undefined {
    return this.stateValue.changelogDisplayedForVersion === ''
      ? undefined
      : this.stateValue.changelogDisplayedForVersion;
  }

  set updateDismissedForVersion(newValue: string | undefined) {
    this.changeStateAndNotify({ ...this.stateValue, updateDismissedForVersion: newValue ?? '' });
  }

  get updateDismissedForVersion(): string | undefined {
    return this.stateValue.updateDismissedForVersion === ''
      ? undefined
      : this.stateValue.updateDismissedForVersion;
  }

  set animateMap(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, animateMap: newValue });
  }

  get animateMap(): boolean {
    return this.stateValue.animateMap;
  }

  // Onboarding-completion timestamp. `undefined` clears the
  // flag so the wizard re-runs on next boot (Settings "Replay
  // onboarding" CTA). The renderer-side AppRouter consults
  // `onboardingCompletedUnix` via `getNavigationBase` and redirects
  // to `RoutePath.onboardingWelcome` when it is unset.
  set onboardingCompletedUnix(newValue: number | undefined) {
    this.changeStateAndNotify({ ...this.stateValue, onboardingCompletedUnix: newValue });
  }

  get onboardingCompletedUnix(): number | undefined {
    return this.stateValue.onboardingCompletedUnix;
  }

  // Persisted backup gate. Survives a GUI restart so a freshly minted,
  // un-backed-up identity cannot land on the main view (see the field
  // doc in gui-settings-state.ts).
  set backupPending(newValue: boolean) {
    this.changeStateAndNotify({ ...this.stateValue, backupPending: newValue });
  }

  get backupPending(): boolean {
    return this.stateValue.backupPending ?? false;
  }

  // Pending app-initiated purchases (see gui-settings-state.ts).
  set pendingPurchases(newValue: Array<string>) {
    this.changeStateAndNotify({ ...this.stateValue, pendingPurchases: newValue });
  }

  get pendingPurchases(): Array<string> {
    return this.stateValue.pendingPurchases ?? [];
  }

  public load() {
    try {
      const settingsFile = this.filePath();
      const contents = fs.readFileSync(settingsFile, 'utf8');
      const rawJson = JSON.parse(contents);

      this.stateValue = {
        ...defaultSettings,
        ...this.validateSettings(rawJson),
      };
    } catch (e) {
      const error = e as Error & { code?: string };
      // Read settings if the file exists, otherwise write the default settings to it.
      if (error.code === 'ENOENT') {
        log.verbose('Creating gui-settings file and writing the default settings to it');
        this.store();
      } else {
        log.error(`Failed to read GUI settings file: ${error}`);
      }
    }
  }

  public store() {
    try {
      const settingsFile = this.filePath();

      fs.writeFileSync(settingsFile, JSON.stringify(this.stateValue));
    } catch (error) {
      log.error(`Failed to write GUI settings file: ${error}`);
    }
  }

  private validateSettings(settings: Record<string, unknown>) {
    Object.entries(settingsSchema).forEach(([key, expectedType]) => {
      if (key in settings) {
        if (/^Array<.*>/.test(expectedType)) {
          const value = settings[key];
          if (!Array.isArray(value)) {
            throw new Error(`Expected ${key} to be array but wasn't`);
          } else {
            const expectedInnerType = expectedType.replace(/^Array</, '').replace(/>$/, '');
            const innerTypes: string[] = value.map((value) => typeof value);
            if (
              innerTypes.length > 0 &&
              (innerTypes.some((value) => value !== innerTypes[0]) ||
                innerTypes[0] !== expectedInnerType)
            ) {
              throw new Error(`Expected ${key} to contain ${expectedInnerType}s`);
            }
          }
        } else {
          const actualType = typeof settings[key];
          if (actualType !== expectedType) {
            throw new Error(`Expected ${key} to be of type ${expectedType} but was ${actualType}`);
          }
        }
      }
    });

    return settings as Partial<IGuiSettingsState>;
  }

  private filePath() {
    return path.join(app.getPath('userData'), 'gui_settings.json');
  }

  private changeStateAndNotify(newState: IGuiSettingsState) {
    const oldState = this.stateValue;
    this.stateValue = newState;

    this.store();

    if (this.onChange) {
      this.onChange({ ...newState }, oldState);
    }
  }
}
