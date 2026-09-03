import log from '../../shared/logging';

// Copy a value the user has to be able to carry out of the app.
//
// The clipboard write is the one place a bearer secret (a voucher code) leaves
// the renderer, so the failure path reports the outcome and nothing else: the
// rejection message can quote the value that was refused, and a log line is
// the last place that value belongs.
export async function copyToClipboard(
  value: string,
  clipboard: Clipboard = navigator.clipboard,
): Promise<boolean> {
  try {
    await clipboard.writeText(value);
    return true;
  } catch {
    log.error('Failed to copy to the clipboard');
    return false;
  }
}
