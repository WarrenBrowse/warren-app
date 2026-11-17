import type { OpenDialogOptions } from 'electron';

/**
 * The options of a file picker the renderer asked for, reduced to what the
 * app's own pickers use: one file, an optional filter and button label. The
 * picker always starts in the home directory.
 */
export function openDialogOptions(requested: OpenDialogOptions, home: string): OpenDialogOptions {
  const options: OpenDialogOptions = { defaultPath: home, properties: ['openFile'] };
  if (typeof requested.buttonLabel === 'string') {
    options.buttonLabel = requested.buttonLabel;
  }
  if (Array.isArray(requested.filters)) {
    options.filters = requested.filters.map(({ name, extensions }) => ({
      name: String(name),
      extensions: extensions.map(String),
    }));
  }
  return options;
}
