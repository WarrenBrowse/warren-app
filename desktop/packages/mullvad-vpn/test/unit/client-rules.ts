/* eslint-disable @typescript-eslint/naming-convention */
// The property names below are the fixture's own spelling, shared with the
// Rust and Kotlin readers of the same files, so they stay snake_case here.
import fs from 'fs';
import path from 'path';
import { vi } from 'vitest';

/**
 * The cross-platform client-rule fixtures (`fixtures/client-rules/README.md`
 * at the repository root), read by this suite the way the Rust and JVM suites
 * read them: one file, several readers, no copy.
 */
const fixturesDir = path.resolve(__dirname, '../../../../../fixtures/client-rules');

export function loadClientRules<T>(name: string): T {
  return JSON.parse(fs.readFileSync(path.join(fixturesDir, name), 'utf8')) as T;
}

/** A case the desktop reader must leave alone: a divergence a later lot closes. */
export function skippedOnDesktop(fixtureCase: { skip?: string[] }): boolean {
  return fixtureCase.skip?.includes('desktop') ?? false;
}

export interface AcceptedLogin {
  sid: string;
  host: string;
  cross_device: boolean;
}

export interface AcceptedAttach {
  sid: string;
  host: string;
  topic_id: number;
}

export interface LinkCase<Accepted> {
  name: string;
  url: string | null;
  expected_scheme: string;
  expect: { accepted?: Accepted; rejected?: string };
  skip?: string[];
}

export interface SignInCodeCase {
  name: string;
  typed: string;
  expect: string | null;
  skip?: string[];
}

export interface ForumLinkFixture {
  schemes: Record<string, string>;
  allowed_hosts: string[];
  pending_ttl_secs: { login: number; attach: number };
  login_cases: LinkCase<AcceptedLogin>[];
  attach_cases: LinkCase<AcceptedAttach>[];
  sign_in_code_cases: SignInCodeCase[];
  sign_in_code_cross_device: boolean;
}

export interface OutcomeCase {
  name: string;
  status: number;
  body: string;
  expect: {
    kind: string;
    handle?: string;
    notify_slot?: number | null;
    reason?: string;
    // The report cases: the created topic and what the broker did with the logs.
    topic_id?: number;
    topic_url?: string | null;
    logs?: string;
  };
  envelope: string;
  skip?: string[];
}

export interface ForumOutcomesFixture {
  login: { terminal_kinds: string[]; cases: OutcomeCase[] };
  report: { cases: OutcomeCase[] };
}

export interface ProductEnvRow {
  name: string;
  api_url: string;
  api_host: string;
  desktop_update_url: string;
  display_name: string;
  unix_product_dir: string;
  application_id: string;
  deep_link_scheme: string;
  connect_host: string;
  forum_public_url: string;
}

export interface ProductEnvFixture {
  environments: Record<string, ProductEnvRow>;
}

/**
 * Loads `modulePath` compiled for `productEnv`. The product environment is a
 * build-time define that vitest leaves undefined, and `product-env.ts` reads
 * the bare identifier, so a global of that name stands in for the define for
 * the duration of one fresh module load. Everything the module imported is
 * re-evaluated too, which is what makes a scheme fixed at module load
 * (`FORUM_DEEP_LINK_SCHEME`) follow the environment.
 */
export async function importForProductEnv<T>(productEnv: string, modulePath: string): Promise<T> {
  const globals = globalThis as Record<string, unknown>;
  globals.WARREN_PRODUCT_ENV = productEnv;
  try {
    vi.resetModules();
    return (await import(/* @vite-ignore */ modulePath)) as T;
  } finally {
    globals.WARREN_PRODUCT_ENV = undefined;
    vi.resetModules();
  }
}
