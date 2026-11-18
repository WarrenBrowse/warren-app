import { messages } from '../../../../../../../shared/gettext';

// A missing status snapshot reads as on, as in the anti-censorship view: the
// obfuscation cannot be turned off, and boot must not flash an alarming state.
export function getAntiCensorshipLabel(obfuscationActive: boolean | undefined): string {
  return (obfuscationActive ?? true) ? messages.gettext('On') : messages.gettext('Off');
}
