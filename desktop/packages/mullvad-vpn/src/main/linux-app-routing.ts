import path from 'path';

export type ExecutableLookup = {
  pathEnv: string;
  isExecutable: (candidate: string) => Promise<boolean>;
  realpath: (candidate: string) => Promise<string>;
  // The text of a file that starts with `#!`, undefined for any other file.
  readScript: (candidate: string) => Promise<string | undefined>;
};

// Why an app cannot be given a country: the kernel never runs a program the
// router could name. A Flatpak or Snap app runs from a sandbox whose paths
// change with every update, and a script runs its interpreter.
export type RoutingLimitation = 'flatpak' | 'snap' | 'script';

export type LaunchTarget =
  | { kind: 'program'; path: string }
  | { kind: 'unsupported'; path: string; reason: RoutingLimitation };

// How many wrappers deep a command is followed.
const MAX_WRAPPERS = 4;

// On Linux an app id is the program the kernel runs (`/proc/<pid>/exe`), so a
// desktop entry's command is resolved through PATH, every symlink and the
// shell wrappers that end by exec'ing a fixed program with their arguments.
// Undefined when the command names nothing installed.
export async function resolveLaunchTarget(
  argv: readonly string[],
  lookup: ExecutableLookup,
): Promise<LaunchTarget | undefined> {
  let program: string | undefined = argv[0];
  if (program === 'env') {
    program = argv.slice(1).find((argument) => !argument.includes('='));
  }
  if (program === undefined || program === '') {
    return undefined;
  }
  return resolveProgram(program, lookup, []);
}

async function resolveProgram(
  program: string,
  lookup: ExecutableLookup,
  followed: string[],
): Promise<LaunchTarget | undefined> {
  const found = await findExecutable(program, lookup);
  if (found === undefined) {
    return undefined;
  }
  const real = await lookup.realpath(found);
  const entry = followed[0] ?? found;
  if (path.basename(real) === 'flatpak') {
    return { kind: 'unsupported', path: entry, reason: 'flatpak' };
  }
  if (path.basename(real) === 'snap' || real.startsWith('/snap/')) {
    return { kind: 'unsupported', path: entry, reason: 'snap' };
  }
  const script = await lookup.readScript(real);
  if (script === undefined) {
    return { kind: 'program', path: real };
  }
  const next = execedProgram(script);
  if (next === undefined || followed.length >= MAX_WRAPPERS) {
    return { kind: 'unsupported', path: entry, reason: 'script' };
  }
  const target = await resolveProgram(next, lookup, [...followed, real]);
  return target ?? { kind: 'unsupported', path: entry, reason: 'script' };
}

async function findExecutable(
  program: string,
  lookup: ExecutableLookup,
): Promise<string | undefined> {
  const candidates = path.isAbsolute(program)
    ? [program]
    : lookup.pathEnv
        .split(':')
        .filter((directory) => directory !== '')
        .map((directory) => path.join(directory, program));
  for (const candidate of candidates) {
    if (await lookup.isExecutable(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

// `exec [-a NAME] PROGRAM ... "$@"`, the last line of a wrapper that hands its
// arguments to a fixed program. A program spelled with a variable is not read.
const EXEC_LINE =
  /^\s*exec\s+(?:-a\s+(?:"[^"]*"|'[^']*'|\S+)\s+)?(?:"([^"$`]+)"|'([^']+)'|([^\s"'$`;&|]+))(?:\s|$)/;

function execedProgram(script: string): string | undefined {
  for (const line of script.split('\n').reverse()) {
    if (!/"\$(?:@|\{@\})"/.test(line)) {
      continue;
    }
    const match = EXEC_LINE.exec(line);
    if (match) {
      return match[1] ?? match[2] ?? match[3];
    }
  }
  return undefined;
}
