import fs from 'fs/promises';
import path from 'path';

import { NatPmpMapping, NatPmpProto, NatPmpStatus } from '../shared/daemon-rpc-types';

/** Where the main process keeps the public ports it last saw, next to the GUI
 * settings, so a GUI started after the daemon got its port can tell a port
 * that moved from one it already announced. */
export const SEEN_PORTS_FILE_NAME = 'port-forwarding-seen.json';

interface SeenPort {
  internalPort: number;
  protocol: NatPmpProto;
  port: number;
}

function isPort(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value > 0 && value <= 65535;
}

function isSeenPort(value: unknown): value is SeenPort {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  return (
    isPort(candidate.internalPort) &&
    isPort(candidate.port) &&
    Object.values(NatPmpProto).includes(candidate.protocol as NatPmpProto)
  );
}

/** The granted ports of `status`, as the file stores them. */
export function serializeSeenPorts(status: NatPmpStatus): string {
  const ports: SeenPort[] = status.mappings.flatMap((mapping) =>
    mapping.status.state === 'mapped'
      ? [
          {
            internalPort: mapping.internalPort,
            protocol: mapping.protocol,
            port: mapping.status.externalPort,
          },
        ]
      : [],
  );
  return JSON.stringify({ ports });
}

/** The ports a file holds, or `undefined` for a file that is missing, damaged
 * or not ours: the app then announces the current port once, which is the
 * whole cost of losing the file. */
export function parseSeenPorts(content: string): NatPmpStatus | undefined {
  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch {
    return undefined;
  }
  const ports = (parsed as { ports?: unknown } | null)?.ports;
  if (!Array.isArray(ports) || !ports.every(isSeenPort)) {
    return undefined;
  }
  return {
    mappings: ports.map(
      (seen): NatPmpMapping => ({
        internalPort: seen.internalPort,
        protocol: seen.protocol,
        status: {
          state: 'mapped',
          externalPort: seen.port,
          lifetimeGrantedSecs: 0,
          windowResetSecs: 0,
        },
      }),
    ),
  };
}

export async function readSeenPorts(directory: string): Promise<NatPmpStatus | undefined> {
  try {
    return parseSeenPorts(await fs.readFile(path.join(directory, SEEN_PORTS_FILE_NAME), 'utf8'));
  } catch {
    return undefined;
  }
}

/** Written next to the target and renamed over it, so a crash mid-write
 * leaves the previous file rather than a damaged one. */
export async function writeSeenPorts(directory: string, status: NatPmpStatus): Promise<void> {
  const target = path.join(directory, SEEN_PORTS_FILE_NAME);
  const temporary = `${target}.tmp`;
  await fs.writeFile(temporary, serializeSeenPorts(status), 'utf8');
  await fs.rename(temporary, target);
}
