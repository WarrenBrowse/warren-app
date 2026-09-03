import { status as grpcStatus } from '@grpc/grpc-js';
import { afterEach, describe, expect, it, vi } from 'vitest';

const forumFetch = vi.hoisted(() => vi.fn());
vi.mock('../../src/main/forum-login', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../src/main/forum-login')>()),
  getForumSession: () => ({ fetch: forumFetch }),
}));

import { DaemonRpc, ForumLoginSignature } from '../../src/main/daemon-rpc';
import { MAX_LOG_GZ_BYTES } from '../../src/main/forum-attach';
import {
  buildForumReportJson,
  desktopOsVersion,
  ForumReportFacts,
  forumReportPlatform,
  forumReportResultForResponse,
  sendForumReport,
  uploadDeadlineMs,
} from '../../src/main/forum-report';
import { ForumReportResult, IForumReportForm } from '../../src/shared/forum-report';
import { ForumOutcomesFixture, loadClientRules, skippedOnDesktop } from './client-rules';

const facts: ForumReportFacts = {
  platform: 'android',
  appVersion: '1.2.3',
  osVersion: 'Android 15 (API 35)',
  locale: 'en',
};

const form: IForumReportForm = {
  area: 'connection',
  frequency: 'sometimes',
  whatHappened: 'Cannot connect after the update, the app stays on Connecting.',
  steps: 'Open the app\nTap Connect',
  includeLogs: true,
};

describe('the report body fields', () => {
  it('are the connect contract fields the shared vector pins, under their wire names', () => {
    // `fields` of the `report_with_log` request in
    // `vectors/forum_login_v1.json`: the same object the Android app assembles
    // and the daemon signs, so the desktop cannot file a report the broker
    // reads differently.
    expect(JSON.parse(buildForumReportJson(form, facts))).toEqual({
      app_version: '1.2.3',
      area: 'connection',
      frequency: 'sometimes',
      locale: 'en',
      os_version: 'Android 15 (API 35)',
      platform: 'android',
      steps: 'Open the app\nTap Connect',
      what_happened: 'Cannot connect after the update, the app stays on Connecting.',
    });
  });

  it('never carry a log field of their own, which the daemon adds', () => {
    expect(JSON.parse(buildForumReportJson(form, facts))).not.toHaveProperty('log_gz_b64');
  });

  it('omit blank steps instead of sending an empty field, and trim the texts', () => {
    const fields = JSON.parse(
      buildForumReportJson({ ...form, steps: '  \n', whatHappened: '  padded  ' }, facts),
    );
    expect(fields).not.toHaveProperty('steps');
    expect(fields.what_happened).toBe('padded');
    expect(
      JSON.parse(buildForumReportJson({ ...form, steps: undefined }, facts)),
    ).not.toHaveProperty('steps');
  });

  it('clip the machine facts to the broker cap and keep the locale to its shape', () => {
    const fields = JSON.parse(
      buildForumReportJson(form, {
        ...facts,
        appVersion: 'v'.repeat(100),
        osVersion: 'o'.repeat(100),
        locale: 'zh_Hans-CN.UTF-8@collation',
      }),
    );
    expect(fields.app_version).toHaveLength(80);
    expect(fields.os_version).toHaveLength(80);
    // Letters and dashes only, at most 10 of them: the broker refuses the
    // whole report (422) for a locale outside that shape.
    expect(fields.locale).toBe('zhHans-CNU');
  });

  it('drop a locale too short to be one rather than fail the report on it', () => {
    expect(JSON.parse(buildForumReportJson(form, { ...facts, locale: '' }))).not.toHaveProperty(
      'locale',
    );
    expect(JSON.parse(buildForumReportJson(form, { ...facts, locale: '1' }))).not.toHaveProperty(
      'locale',
    );
  });

  it('spell the device line per operating system, within the fact cap', () => {
    expect(desktopOsVersion('darwin', '25.6.0', 'arm64')).toBe('macOS (Darwin 25.6.0, arm64)');
    expect(desktopOsVersion('win32', '10.0.22631', 'x64')).toBe('Windows 10.0.22631 (x64)');
    expect(desktopOsVersion('linux', '6.8.0-45-generic', 'x64')).toBe(
      'Linux 6.8.0-45-generic (x64)',
    );
  });

  it('name the desktop platforms by the forum tags and nothing else', () => {
    expect(forumReportPlatform('darwin')).toBe('macos');
    expect(forumReportPlatform('win32')).toBe('windows');
    expect(forumReportPlatform('linux')).toBe('linux');
    expect(forumReportPlatform('freebsd')).toBeUndefined();
  });
});

describe('the upload deadline', () => {
  it('is 20 s plus 10 s per MiB of body started, as on Android', () => {
    // `warren_jni::forum::upload_deadline`: the mint's 15 s killed a report
    // with a few MiB of logs on a slow uplink after the data was spent.
    const mib = 1024 * 1024;
    expect(uploadDeadlineMs(0)).toBe(20_000);
    expect(uploadDeadlineMs(1)).toBe(30_000);
    expect(uploadDeadlineMs(mib)).toBe(30_000);
    expect(uploadDeadlineMs(mib + 1)).toBe(40_000);
    expect(uploadDeadlineMs(MAX_LOG_GZ_BYTES)).toBe(140_000);
  });
});

describe('the report outcome table', () => {
  const fixture = loadClientRules<ForumOutcomesFixture>('forum_outcomes.json');

  it('classes every pinned broker answer as the shared fixture says', () => {
    for (const fixtureCase of fixture.report.cases) {
      if (skippedOnDesktop(fixtureCase)) {
        continue;
      }
      const result = forumReportResultForResponse(fixtureCase.status, fixtureCase.body);
      const expected = fixtureCase.expect;
      expect(result.kind, fixtureCase.name).toBe(expected.kind);
      if (result.kind === 'created') {
        expect(result.topicId, fixtureCase.name).toBe(expected.topic_id);
        expect(result.topicUrl, fixtureCase.name).toBe(expected.topic_url ?? undefined);
        expect(result.logs, fixtureCase.name).toBe(expected.logs);
        expect(result.identity?.handle, fixtureCase.name).toBe(expected.handle);
        if (expected.handle !== undefined) {
          expect(result.identity?.notifySlot, fixtureCase.name).toBe(expected.notify_slot ?? null);
        }
      }
      if (result.kind === 'failed') {
        expect(result.reason, fixtureCase.name).toBe(expected.reason);
      }
    }
  });
});

describe('sending a report', () => {
  const signature: ForumLoginSignature = {
    pubkeySs58: 'wbCTo7no13eynB3yV6tZadBhFxd5h1gyqTic8ksbWbtXw45gX',
    signatureHex: 'ab'.repeat(64),
    timestamp: 1_800_000_000,
    nonceHex: '00'.repeat(16),
    body: '{"area":"connection","log_gz_b64":"H4sI"}',
  };
  const makeDaemon = () => {
    const signForumReport = vi.fn().mockResolvedValue(signature);
    return { rpc: { signForumReport } as unknown as DaemonRpc, signForumReport };
  };
  const grpcError = (code: grpcStatus, message = '') => Object.assign(new Error(message), { code });
  const answer = (status: number, body: string) =>
    forumFetch.mockResolvedValue({ status, ok: status < 300, text: () => Promise.resolve(body) });

  afterEach(() => {
    forumFetch.mockReset();
    vi.useRealTimers();
  });

  it('refuses a gzip over the cap before the daemon or the network see it', async () => {
    const { rpc, signForumReport } = makeDaemon();
    const result = await sendForumReport(form, new Uint8Array(MAX_LOG_GZ_BYTES + 1), facts, rpc);
    expect(result).toEqual({ kind: 'too-large' });
    expect(signForumReport).not.toHaveBeenCalled();
    expect(forumFetch).not.toHaveBeenCalled();
  });

  it('posts the signed body verbatim with the four headers to the allowlisted broker', async () => {
    const { rpc, signForumReport } = makeDaemon();
    answer(
      201,
      '{"status":"created","topic_id":4242,"topic_url":"https://forum.warrenbrowse.com/t/4242","logs":"attached","handle":"jugop-lobab-virar","notify_slot":1}',
    );
    const logGz = new Uint8Array([0x1f, 0x8b, 0x08]);
    const result = await sendForumReport(form, logGz, facts, rpc);
    expect(result).toEqual({
      kind: 'created',
      topicId: 4242,
      topicUrl: 'https://forum.warrenbrowse.com/t/4242',
      logs: 'attached',
      identity: { handle: 'jugop-lobab-virar', notifySlot: 1 },
    });
    expect(signForumReport).toHaveBeenCalledWith(buildForumReportJson(form, facts), logGz);
    const [url, init] = forumFetch.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://connect.warrenbrowse.com/v1/forum/report');
    expect(init.method).toBe('POST');
    expect(init.body).toBe(signature.body);
    expect(init.headers).toEqual({
      'Content-Type': 'application/json',
      'X-Warren-PubKey': signature.pubkeySs58,
      'X-Warren-Sig': signature.signatureHex,
      'X-Warren-Timestamp': '1800000000',
      'X-Warren-Nonce': signature.nonceHex,
    });
    expect(init.signal).toBeInstanceOf(AbortSignal);
  });

  it('hands the daemon an empty gzip for a report without logs', async () => {
    const { rpc, signForumReport } = makeDaemon();
    answer(201, '{"status":"created","topic_id":7,"logs":"none"}');
    const result = await sendForumReport({ ...form, includeLogs: false }, undefined, facts, rpc);
    expect(result).toEqual({ kind: 'created', topicId: 7, logs: 'none' });
    expect(signForumReport).toHaveBeenCalledWith(expect.any(String), new Uint8Array(0));
  });

  it('names a daemon without an identity instead of a generic failure', async () => {
    const { rpc } = makeDaemon();
    (rpc.signForumReport as ReturnType<typeof vi.fn>).mockRejectedValue(
      grpcError(grpcStatus.FAILED_PRECONDITION, 'no Warren identity bootstrapped'),
    );
    expect(await sendForumReport(form, undefined, facts, rpc)).toEqual({ kind: 'no-identity' });
    expect(forumFetch).not.toHaveBeenCalled();
  });

  it('maps a daemon refusal of the fields to invalid and any other daemon error to build', async () => {
    const { rpc } = makeDaemon();
    (rpc.signForumReport as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      grpcError(grpcStatus.INVALID_ARGUMENT, 'report_json must be a JSON object'),
    );
    expect(await sendForumReport(form, undefined, facts, rpc)).toEqual({ kind: 'invalid' });
    (rpc.signForumReport as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      grpcError(grpcStatus.UNAVAILABLE, 'daemon gone'),
    );
    expect(await sendForumReport(form, undefined, facts, rpc)).toEqual({
      kind: 'failed',
      reason: 'build',
    });
  });

  it('classes the broker answers through the shared table', async () => {
    const { rpc } = makeDaemon();
    answer(403, '');
    expect(await sendForumReport(form, undefined, facts, rpc)).toEqual({
      kind: 'subscription-required',
    });
    answer(413, '');
    expect(await sendForumReport(form, new Uint8Array(3), facts, rpc)).toEqual({
      kind: 'too-large',
    });
    answer(502, '');
    expect(await sendForumReport(form, undefined, facts, rpc)).toEqual({ kind: 'server-error' });
  });

  it('classes a network failure as transport', async () => {
    const { rpc } = makeDaemon();
    forumFetch.mockRejectedValue(new Error('ECONNRESET'));
    expect(await sendForumReport(form, new Uint8Array(3), facts, rpc)).toEqual({
      kind: 'failed',
      reason: 'transport',
    });
  });

  const abortsOnSignal = () =>
    forumFetch.mockImplementation(
      (_url: string, init: RequestInit) =>
        new Promise((_resolve, reject) => {
          init.signal?.addEventListener('abort', () =>
            reject(Object.assign(new Error('aborted'), { name: 'AbortError' })),
          );
        }),
    );

  it('runs a deadline sized to the signed body and, with logs attached, names the upload', async () => {
    // A deadline run out WITH logs is the uplink, not the network: the form
    // offers the resend without them on this reason.
    vi.useFakeTimers();
    const { rpc } = makeDaemon();
    abortsOnSignal();
    const pending = sendForumReport(form, new Uint8Array(3), facts, rpc);
    await vi.advanceTimersByTimeAsync(uploadDeadlineMs(Buffer.byteLength(signature.body)) - 1);
    let settled: ForumReportResult | undefined;
    void pending.then((result) => (settled = result));
    await vi.advanceTimersByTimeAsync(0);
    expect(settled).toBeUndefined();
    await vi.advanceTimersByTimeAsync(1);
    expect(await pending).toEqual({ kind: 'failed', reason: 'upload-timeout' });
  });

  it('classes the same deadline without logs as a plain transport failure', async () => {
    vi.useFakeTimers();
    const { rpc } = makeDaemon();
    abortsOnSignal();
    const pending = sendForumReport({ ...form, includeLogs: false }, undefined, facts, rpc);
    await vi.advanceTimersByTimeAsync(uploadDeadlineMs(Buffer.byteLength(signature.body)));
    expect(await pending).toEqual({ kind: 'failed', reason: 'transport' });
  });
});
