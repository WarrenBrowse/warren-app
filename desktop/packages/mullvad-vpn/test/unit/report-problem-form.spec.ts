import { describe, expect, it } from 'vitest';

import {
  beginReportCollect,
  beginReportSend,
  canSendReport,
  clampReportText,
  createdReportNotice,
  descriptionChars,
  initialReportProblemFormState,
  noticeForForumReportResult,
  REPORT_TEXT_OVERRUN_CHARS,
  reportFormPayload,
  ReportProblemFormState,
  setReportArea,
  setReportFrequency,
  setReportIncludeLogs,
  setReportSteps,
  setReportWhatHappened,
  settleReportCollect,
  settleReportSend,
} from '../../src/renderer/features/report-problem/form-state';
import {
  FORUM_REPORT_MAX_DESCRIPTION_CHARS,
  FORUM_REPORT_MIN_DESCRIPTION_CHARS,
  ForumReportResult,
} from '../../src/shared/forum-report';

const description = 'The sign-in button does nothing at all, twice.';

const filled: ReportProblemFormState = setReportWhatHappened(
  setReportFrequency(setReportArea(initialReportProblemFormState, 'other'), 'always'),
  description,
);

const created: ForumReportResult = {
  kind: 'created',
  topicId: 4242,
  topicUrl: 'https://forum.warrenbrowse.com/t/4242',
  logs: 'attached',
};

describe('the report form gate', () => {
  it('starts with logs included and nothing else filled in', () => {
    expect(initialReportProblemFormState.includeLogs).toBe(true);
    expect(initialReportProblemFormState.area).toBeUndefined();
    expect(initialReportProblemFormState.frequency).toBeUndefined();
    expect(canSendReport(initialReportProblemFormState)).toBe(false);
  });

  it('sends only once the area, the frequency and a long enough description are there', () => {
    expect(canSendReport(filled)).toBe(true);
    expect(canSendReport({ ...filled, area: undefined })).toBe(false);
    expect(canSendReport({ ...filled, frequency: undefined })).toBe(false);
    expect(
      canSendReport(
        setReportWhatHappened(filled, 'x'.repeat(FORUM_REPORT_MIN_DESCRIPTION_CHARS - 1)),
      ),
    ).toBe(false);
    expect(
      canSendReport(setReportWhatHappened(filled, 'x'.repeat(FORUM_REPORT_MIN_DESCRIPTION_CHARS))),
    ).toBe(true);
  });

  it('counts the description trimmed, the way the broker measures it', () => {
    expect(descriptionChars(setReportWhatHappened(filled, '   short   '))).toBe(5);
    expect(
      canSendReport(
        setReportWhatHappened(
          filled,
          `   ${'x'.repeat(FORUM_REPORT_MIN_DESCRIPTION_CHARS - 1)}   `,
        ),
      ),
    ).toBe(false);
  });

  it('refuses a description over the broker cap', () => {
    expect(
      canSendReport(setReportWhatHappened(filled, 'x'.repeat(FORUM_REPORT_MAX_DESCRIPTION_CHARS))),
    ).toBe(true);
    expect(
      canSendReport(
        setReportWhatHappened(filled, 'x'.repeat(FORUM_REPORT_MAX_DESCRIPTION_CHARS + 1)),
      ),
    ).toBe(false);
  });

  it('keeps taking keys a little past the cap so the counter shows what to trim', () => {
    // A field that stops typing at the cap reads as broken; the counter
    // showing 4,120 against 4,000 reads as too long.
    const over = 'x'.repeat(FORUM_REPORT_MAX_DESCRIPTION_CHARS + REPORT_TEXT_OVERRUN_CHARS + 50);
    expect(clampReportText(over)).toHaveLength(
      FORUM_REPORT_MAX_DESCRIPTION_CHARS + REPORT_TEXT_OVERRUN_CHARS,
    );
    expect(setReportSteps(filled, over).steps).toHaveLength(
      FORUM_REPORT_MAX_DESCRIPTION_CHARS + REPORT_TEXT_OVERRUN_CHARS,
    );
  });

  it('is closed while a send or a collection is in flight, and once the topic exists', () => {
    expect(canSendReport(beginReportSend(filled))).toBe(false);
    expect(canSendReport(beginReportCollect(filled))).toBe(false);
    expect(canSendReport(settleReportSend(beginReportSend(filled), created))).toBe(false);
    expect(canSendReport(settleReportSend(beginReportSend(filled), { kind: 'rate-limited' }))).toBe(
      true,
    );
  });
});

describe('the payload handed to main', () => {
  it('is the trimmed form with blank steps left out', () => {
    expect(reportFormPayload(setReportSteps(filled, '  \n '))).toEqual({
      area: 'other',
      frequency: 'always',
      whatHappened: description,
      steps: undefined,
      includeLogs: true,
    });
    expect(reportFormPayload(setReportSteps(filled, ' Open the app ')).steps).toBe('Open the app');
  });

  it('carries the logs switch', () => {
    expect(reportFormPayload(setReportIncludeLogs(filled, false)).includeLogs).toBe(false);
  });
});

describe('the form transitions', () => {
  it('forget the last outcome when any field changes', () => {
    const refused = settleReportSend(beginReportSend(filled), { kind: 'rate-limited' });
    expect(refused.result).toEqual({ kind: 'rate-limited' });
    expect(setReportArea(refused, 'wallet').result).toBeUndefined();
    expect(setReportFrequency(refused, 'once').result).toBeUndefined();
    expect(setReportWhatHappened(refused, description).result).toBeUndefined();
    expect(setReportSteps(refused, 'steps').result).toBeUndefined();
    expect(setReportIncludeLogs(refused, false).result).toBeUndefined();
  });

  it('drop the previewed report when the logs are switched off', () => {
    const previewed = settleReportCollect(beginReportCollect(filled), 'report-1');
    expect(previewed.previewReportId).toBe('report-1');
    expect(setReportIncludeLogs(previewed, false).previewReportId).toBeUndefined();
    expect(setReportIncludeLogs(previewed, true).previewReportId).toBe('report-1');
  });

  it('name a failed collection without losing the form', () => {
    const failed = settleReportCollect(beginReportCollect(filled), undefined);
    expect(failed.collecting).toBe(false);
    expect(failed.collectFailed).toBe(true);
    expect(failed.whatHappened).toBe(description);
    expect(beginReportCollect(failed).collectFailed).toBe(false);
  });

  it('settle a send with its outcome and reopen the form', () => {
    const sent = settleReportSend(beginReportSend(filled), created);
    expect(sent.sending).toBe(false);
    expect(sent.result).toEqual(created);
  });
});

describe('the outcome notices', () => {
  it('say what the logs did once the topic exists', () => {
    expect(createdReportNotice({ ...created, logs: 'attached' })).toMatch(/logs reached/);
    expect(createdReportNotice({ ...created, logs: 'partial' })).toMatch(/in part/);
    expect(createdReportNotice({ ...created, logs: 'none' })).toBe('Your report is on the forum.');
  });

  it('offer the help form to a wallet that never subscribed', () => {
    expect(noticeForForumReportResult({ kind: 'subscription-required' })?.action).toBe(
      'open-help-form',
    );
  });

  it('offer the resend without logs on a size refusal and on the upload deadline', () => {
    expect(noticeForForumReportResult({ kind: 'too-large' })?.action).toBe('send-without-logs');
    expect(noticeForForumReportResult({ kind: 'failed', reason: 'upload-timeout' })?.action).toBe(
      'send-without-logs',
    );
    expect(noticeForForumReportResult({ kind: 'failed', reason: 'transport' })?.action).toBe(
      undefined,
    );
  });

  it('give every refusal its own words and the created topic none', () => {
    const kinds: ForumReportResult[] = [
      { kind: 'subscription-required' },
      { kind: 'clock-skew' },
      { kind: 'rate-limited' },
      { kind: 'too-large' },
      { kind: 'invalid' },
      { kind: 'server-error' },
      { kind: 'no-identity' },
      { kind: 'failed', reason: 'upload-timeout' },
      { kind: 'failed', reason: 'transport' },
    ];
    const texts = new Set(kinds.map((result) => noticeForForumReportResult(result)?.text));
    expect(texts.has(undefined)).toBe(false);
    expect(texts.size).toBe(kinds.length);
    expect(noticeForForumReportResult(created)).toBeUndefined();
  });
});
