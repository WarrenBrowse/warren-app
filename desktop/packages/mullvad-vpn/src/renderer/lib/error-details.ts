// The fatal-error view must show the user something actionable to report,
// not just "something went wrong": the renderer log is out of reach for most
// users, so this is often the only place the failure ever becomes visible.
const MAX_STACK_LINES = 12;

export function formatErrorDetails(
  error: { name: string; message: string },
  componentStack?: string | null,
): string {
  const heading = `${error.name}: ${error.message}`;

  const frames = (componentStack ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  if (frames.length === 0) {
    return heading;
  }

  const shown =
    frames.length > MAX_STACK_LINES ? [...frames.slice(0, MAX_STACK_LINES - 1), '...'] : frames;
  return [heading, ...shown].join('\n');
}
