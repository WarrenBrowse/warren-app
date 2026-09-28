// Windows has no socket owner to stat, so the daemon's named pipe is trusted only when the native
// check reports an administrator owner. Any process can create a pipe under that name first, and
// the GUI would then hand it the wallet.
export function assertPipeAdminOwned(
  pipeIsAdminOwned: (pipePath: string) => boolean,
  pipePath: string,
): void {
  let adminOwned: boolean;
  try {
    adminOwned = pipeIsAdminOwned(pipePath);
  } catch (e) {
    if (e && typeof e === 'object' && 'message' in e) {
      throw new Error(`Failed to verify admin ownership of named pipe. ${e.message}`);
    } else {
      throw new Error('Failed to verify admin ownership of named pipe');
    }
  }
  if (!adminOwned) {
    throw new Error('Named pipe is not owned by an admin.');
  }
}
