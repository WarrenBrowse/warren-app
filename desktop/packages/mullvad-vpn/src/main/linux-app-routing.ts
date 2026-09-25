import path from 'path';

export type ExecutableLookup = {
  pathEnv: string;
  isExecutable: (candidate: string) => Promise<boolean>;
  realpath: (candidate: string) => Promise<string>;
};

// On Linux an app id is the program the kernel runs (`/proc/<pid>/exe`), so a
// desktop entry's command is resolved through PATH and every symlink. A shell
// script wrapper still resolves to the script, which the router never sees
// running: such an app gets no country until picked by its real binary.
export async function resolveLaunchCommand(
  argv: readonly string[],
  lookup: ExecutableLookup,
): Promise<string | undefined> {
  let program: string | undefined = argv[0];
  if (program === 'env') {
    program = argv.slice(1).find((argument) => !argument.includes('='));
  }
  if (program === undefined || program === '') {
    return undefined;
  }

  const candidates = path.isAbsolute(program)
    ? [program]
    : lookup.pathEnv
        .split(':')
        .filter((directory) => directory !== '')
        .map((directory) => path.join(directory, program));

  for (const candidate of candidates) {
    if (await lookup.isExecutable(candidate)) {
      return lookup.realpath(candidate);
    }
  }
  return undefined;
}
