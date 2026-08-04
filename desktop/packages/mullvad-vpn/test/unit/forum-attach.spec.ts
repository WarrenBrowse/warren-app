import { describe, expect, it, vi } from 'vitest';

import { DaemonRpc } from '../../src/main/daemon-rpc';
import {
  approveForumAttach,
  MAX_LOG_GZ_BYTES,
  parseForumAttachUrl,
  resolveApprovedReport,
} from '../../src/main/forum-attach';
import { findForumDeepLinkArg, PendingForumRequest } from '../../src/main/forum-login';
import { IForumAttachRequest } from '../../src/shared/forum-attach';

const sid = 'a'.repeat(32);
const good = `warren://attach-logs?sid=${sid}&topic=123&host=connect.warrenbrowse.com`;

describe('attach-logs deep link parsing', () => {
  it('accepts a well-formed allowlisted link', () => {
    expect(parseForumAttachUrl(good)).toEqual({
      sid,
      host: 'connect.warrenbrowse.com',
      topicId: 123,
    });
  });

  it('rejects a non-allowlisted host so a hostile link cannot redirect signed logs', () => {
    const evil = `warren://attach-logs?sid=${sid}&topic=123&host=evil.example.com`;
    expect(parseForumAttachUrl(evil)).toBeUndefined();
  });

  it('rejects a malformed sid (not 32 lowercase hex)', () => {
    expect(
      parseForumAttachUrl(
        'warren://attach-logs?sid=NOTHEX&topic=123&host=connect.warrenbrowse.com',
      ),
    ).toBeUndefined();
    expect(
      parseForumAttachUrl(
        `warren://attach-logs?sid=${'A'.repeat(32)}&topic=123&host=connect.warrenbrowse.com`,
      ),
    ).toBeUndefined();
  });

  it('accepts topic 0 (pre-topic session: logs sent while the report is being composed)', () => {
    expect(
      parseForumAttachUrl(`warren://attach-logs?sid=${sid}&topic=0&host=connect.warrenbrowse.com`),
    ).toEqual({
      sid,
      host: 'connect.warrenbrowse.com',
      topicId: 0,
    });
  });

  it('rejects a malformed topic (non-decimal, negative, or beyond safe bounds)', () => {
    for (const topic of ['', 'abc', '-1', '1.5', '0x10', '9007199254740993']) {
      expect(
        parseForumAttachUrl(
          `warren://attach-logs?sid=${sid}&topic=${topic}&host=connect.warrenbrowse.com`,
        ),
      ).toBeUndefined();
    }
  });

  it('rejects the wrong scheme or action', () => {
    expect(
      parseForumAttachUrl(`https://attach-logs?sid=${sid}&topic=1&host=connect.warrenbrowse.com`),
    ).toBeUndefined();
    expect(
      parseForumAttachUrl(`warren://forum-login?sid=${sid}&topic=1&host=connect.warrenbrowse.com`),
    ).toBeUndefined();
  });

  it('rejects a non-URL string without throwing', () => {
    expect(parseForumAttachUrl('not a url')).toBeUndefined();
  });
});

describe('forum deep link argv detection', () => {
  const login = `warren://forum-login?sid=${sid}&host=connect.warrenbrowse.com`;

  it('finds either deep link kind among process argv (Windows/Linux delivery)', () => {
    expect(findForumDeepLinkArg(['/path/to/app', '--flag', login])).toBe(login);
    expect(findForumDeepLinkArg(['/path/to/app', '--flag', good])).toBe(good);
    expect(findForumDeepLinkArg(['/path/to/app', '--flag'])).toBeUndefined();
  });
});

describe('pending forum request buffer', () => {
  const request: IForumAttachRequest = {
    sid,
    host: 'connect.warrenbrowse.com',
    topicId: 123,
    reportId: '123e4567-e89b-12d3-a456-426614174000',
  };

  it('keeps an unanswered request until cleared', () => {
    const pending = new PendingForumRequest<IForumAttachRequest>();
    pending.set(request, 1000);
    expect(pending.get(2000)).toEqual(request);
    pending.clear();
    expect(pending.get(2000)).toBeUndefined();
  });

  it('expires a request buffered longer than the server session TTL', () => {
    const pending = new PendingForumRequest<IForumAttachRequest>();
    pending.set(request, 1000);
    expect(pending.get(1000 + 10 * 60 * 1000 + 1)).toBeUndefined();
  });
});

describe('approveForumAttach validation', () => {
  const request: IForumAttachRequest = {
    sid,
    host: 'connect.warrenbrowse.com',
    topicId: 123,
    reportId: '123e4567-e89b-12d3-a456-426614174000',
  };
  const makeDaemon = () => {
    const signForumAttachLogs = vi.fn();
    return { rpc: { signForumAttachLogs } as unknown as DaemonRpc, signForumAttachLogs };
  };

  it('refuses a non-allowlisted host without touching the daemon', async () => {
    const { rpc, signForumAttachLogs } = makeDaemon();
    const result = await approveForumAttach(
      { ...request, host: 'evil.example.com' },
      rpc,
      new Uint8Array(8),
    );
    expect(result).toBe('error');
    expect(signForumAttachLogs).not.toHaveBeenCalled();
  });

  it('refuses a malformed sid without touching the daemon', async () => {
    const { rpc, signForumAttachLogs } = makeDaemon();
    const result = await approveForumAttach(
      { ...request, sid: 'A'.repeat(32) },
      rpc,
      new Uint8Array(8),
    );
    expect(result).toBe('error');
    expect(signForumAttachLogs).not.toHaveBeenCalled();
  });

  it('refuses an oversized gzip client-side instead of sending a doomed request', async () => {
    const { rpc, signForumAttachLogs } = makeDaemon();
    const result = await approveForumAttach(request, rpc, new Uint8Array(MAX_LOG_GZ_BYTES + 1));
    expect(result).toBe('too-large');
    expect(signForumAttachLogs).not.toHaveBeenCalled();
  });

  it('refuses an empty gzip without touching the daemon', async () => {
    const { rpc, signForumAttachLogs } = makeDaemon();
    const result = await approveForumAttach(request, rpc, new Uint8Array(0));
    expect(result).toBe('error');
    expect(signForumAttachLogs).not.toHaveBeenCalled();
  });

  it('accepts a pre-topic request (topicId 0) and asks the daemon to sign it', async () => {
    const { rpc, signForumAttachLogs } = makeDaemon();
    signForumAttachLogs.mockRejectedValue(new Error('no identity'));
    const logGz = new Uint8Array(8);
    const result = await approveForumAttach({ ...request, topicId: 0 }, rpc, logGz);
    expect(signForumAttachLogs).toHaveBeenCalledWith(sid, 0, logGz);
    expect(result).toBe('error');
  });

  it('refuses a negative topicId without touching the daemon', async () => {
    const { rpc, signForumAttachLogs } = makeDaemon();
    const result = await approveForumAttach({ ...request, topicId: -1 }, rpc, new Uint8Array(8));
    expect(result).toBe('error');
    expect(signForumAttachLogs).not.toHaveBeenCalled();
  });
});

describe('resolveApprovedReport', () => {
  it('reads the previewed report without collecting a new one', async () => {
    const collect = vi.fn(() => Promise.resolve('fresh-id'));
    const read = vi.fn((id: string) => Promise.resolve(Buffer.from(`report:${id}`)));
    const report = await resolveApprovedReport('previewed-id', collect, read);
    expect(report.bytes.toString()).toBe('report:previewed-id');
    expect(report.reportId).toBe('previewed-id');
    expect(collect).not.toHaveBeenCalled();
  });

  it('collects a fresh report when the deep-link collection had failed', async () => {
    const collect = vi.fn(() => Promise.resolve('fresh-id'));
    const read = vi.fn((id: string) => Promise.resolve(Buffer.from(`report:${id}`)));
    const report = await resolveApprovedReport(undefined, collect, read);
    expect(report.bytes.toString()).toBe('report:fresh-id');
    expect(report.reportId).toBe('fresh-id');
    expect(collect).toHaveBeenCalledOnce();
  });

  it('re-collects when the previewed report file has vanished', async () => {
    const collect = vi.fn(() => Promise.resolve('fresh-id'));
    const read = vi.fn((id: string) =>
      id === 'previewed-id'
        ? Promise.reject(new Error('ENOENT'))
        : Promise.resolve(Buffer.from(`report:${id}`)),
    );
    const report = await resolveApprovedReport('previewed-id', collect, read);
    expect(report.bytes.toString()).toBe('report:fresh-id');
    expect(report.reportId).toBe('fresh-id');
    expect(collect).toHaveBeenCalledOnce();
  });

  it('propagates a failed retry so approve surfaces an error', async () => {
    const collect = vi.fn(() => Promise.reject(new Error('collector missing')));
    const read = vi.fn();
    await expect(resolveApprovedReport(undefined, collect, read)).rejects.toThrow();
    expect(read).not.toHaveBeenCalled();
  });
});
