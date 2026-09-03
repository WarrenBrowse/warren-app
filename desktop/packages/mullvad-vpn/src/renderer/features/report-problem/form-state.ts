import {
  FORUM_REPORT_MAX_DESCRIPTION_CHARS,
  FORUM_REPORT_MIN_DESCRIPTION_CHARS,
  ForumReportArea,
  ForumReportFrequency,
  ForumReportResult,
  IForumReportForm,
  isForumReportUploadTimeout,
} from '../../../shared/forum-report';
import { messages } from '../../../shared/gettext';

/**
 * The "Report a problem" form's state, kept as plain data so its rules are
 * unit-tested off Electron: what makes it sendable, what a field change
 * forgets, what each outcome says. The Android `ReportProblemViewModel` is the
 * reference; the two agree rule for rule.
 */
export interface ReportProblemFormState {
  area?: ForumReportArea;
  frequency?: ForumReportFrequency;
  whatHappened: string;
  steps: string;
  includeLogs: boolean;
  // The problem report collected for "View the logs", held by main under this
  // id; the send collects its own, fresh one.
  previewReportId?: string;
  collecting: boolean;
  collectFailed: boolean;
  sending: boolean;
  // The last outcome, rendered under the form.
  result?: ForumReportResult;
}

export const initialReportProblemFormState: ReportProblemFormState = {
  area: undefined,
  frequency: undefined,
  whatHappened: '',
  steps: '',
  includeLogs: true,
  previewReportId: undefined,
  collecting: false,
  collectFailed: false,
  sending: false,
  result: undefined,
};

/**
 * How far past the cap the fields still take keys: the counter then shows
 * the overrun to trim instead of the field silently refusing to type.
 */
export const REPORT_TEXT_OVERRUN_CHARS = 200;

export function clampReportText(text: string): string {
  return Array.from(text)
    .slice(0, FORUM_REPORT_MAX_DESCRIPTION_CHARS + REPORT_TEXT_OVERRUN_CHARS)
    .join('');
}

/** The description as the broker measures it: trimmed, in characters. */
export function descriptionChars(state: ReportProblemFormState): number {
  return Array.from(state.whatHappened.trim()).length;
}

export function canSendReport(state: ReportProblemFormState): boolean {
  const chars = descriptionChars(state);
  return (
    state.area !== undefined &&
    state.frequency !== undefined &&
    chars >= FORUM_REPORT_MIN_DESCRIPTION_CHARS &&
    chars <= FORUM_REPORT_MAX_DESCRIPTION_CHARS &&
    !state.sending &&
    !state.collecting &&
    state.result?.kind !== 'created'
  );
}

/** The form as main takes it; the caller checks [`canSendReport`] first. */
export function reportFormPayload(state: ReportProblemFormState): IForumReportForm {
  const steps = state.steps.trim();
  return {
    // Guarded by canSendReport; the fallbacks only satisfy the type.
    area: state.area ?? 'other',
    frequency: state.frequency ?? 'once',
    whatHappened: state.whatHappened.trim(),
    steps: steps.length > 0 ? steps : undefined,
    includeLogs: state.includeLogs,
  };
}

// Every field change forgets the last outcome: a notice about the previous
// attempt under a form that has since changed reads as being about this one.
export function setReportArea(
  state: ReportProblemFormState,
  area: ForumReportArea,
): ReportProblemFormState {
  return { ...state, area, result: undefined };
}

export function setReportFrequency(
  state: ReportProblemFormState,
  frequency: ForumReportFrequency,
): ReportProblemFormState {
  return { ...state, frequency, result: undefined };
}

export function setReportWhatHappened(
  state: ReportProblemFormState,
  text: string,
): ReportProblemFormState {
  return { ...state, whatHappened: clampReportText(text), result: undefined };
}

export function setReportSteps(
  state: ReportProblemFormState,
  text: string,
): ReportProblemFormState {
  return { ...state, steps: clampReportText(text), result: undefined };
}

/** Switching the logs off drops the previewed report; the caller discards it. */
export function setReportIncludeLogs(
  state: ReportProblemFormState,
  includeLogs: boolean,
): ReportProblemFormState {
  return {
    ...state,
    includeLogs,
    previewReportId: includeLogs ? state.previewReportId : undefined,
    result: undefined,
  };
}

export function beginReportCollect(state: ReportProblemFormState): ReportProblemFormState {
  return { ...state, collecting: true, collectFailed: false };
}

/** `reportId` undefined: the collection failed; the form stays as it is. */
export function settleReportCollect(
  state: ReportProblemFormState,
  reportId: string | undefined,
): ReportProblemFormState {
  return {
    ...state,
    collecting: false,
    collectFailed: reportId === undefined,
    previewReportId: reportId ?? state.previewReportId,
  };
}

export function beginReportSend(state: ReportProblemFormState): ReportProblemFormState {
  return { ...state, sending: true, result: undefined };
}

export function settleReportSend(
  state: ReportProblemFormState,
  result: ForumReportResult,
): ReportProblemFormState {
  return { ...state, sending: false, result };
}

/** What a created topic says about its logs. */
export function createdReportNotice(
  result: Extract<ForumReportResult, { kind: 'created' }>,
): string {
  switch (result.logs) {
    case 'attached':
      return messages.pgettext(
        'report-problem',
        'Your report is on the forum and your logs reached the support team.',
      );
    case 'partial':
      return messages.pgettext(
        'report-problem',
        'Your report is on the forum. The logs could only be delivered in part; the support team may ask for them again.',
      );
    case 'none':
      return messages.pgettext('report-problem', 'Your report is on the forum.');
  }
}

/** The action a refusal notice offers next to its words. */
export type ReportNoticeAction = 'open-help-form' | 'send-without-logs';

export interface ReportNotice {
  text: string;
  action?: ReportNoticeAction;
}

/**
 * The notice for a non-created outcome, the same words as the Android screen
 * so the forum's help reads the same for both. A created topic has its own
 * notice ([`createdReportNotice`]) and returns nothing here.
 */
export function noticeForForumReportResult(result: ForumReportResult): ReportNotice | undefined {
  switch (result.kind) {
    case 'created':
      return undefined;
    case 'subscription-required':
      return {
        text: messages.pgettext(
          'report-problem',
          'Forum access requires a Warren subscription. This wallet has never subscribed: use the help form on the website instead.',
        ),
        action: 'open-help-form',
      };
    case 'clock-skew':
      return {
        text: messages.pgettext(
          'report-problem',
          "This device's clock is off by more than a minute. Enable automatic date and time, then send again.",
        ),
      };
    case 'rate-limited':
      return {
        text: messages.pgettext(
          'report-problem',
          'Too many reports from this wallet for now. Please try again later.',
        ),
      };
    case 'too-large':
      return {
        text: messages.pgettext('report-problem', 'The logs are too large to send.'),
        action: 'send-without-logs',
      };
    case 'invalid':
      return {
        text: messages.pgettext(
          'report-problem',
          'Some fields could not be accepted. Check the description and try again.',
        ),
      };
    case 'server-error':
      return {
        text: messages.pgettext(
          'report-problem',
          'The forum could not take the report right now. Please try again later.',
        ),
      };
    case 'no-identity':
      return {
        text: messages.pgettext(
          'report-problem',
          'Your Warren account is not ready yet, so the report cannot be signed. Finish setting up the app, then send again.',
        ),
      };
    case 'failed':
      if (isForumReportUploadTimeout(result)) {
        return {
          text: messages.pgettext(
            'report-problem',
            'Sending the logs took too long on this connection.',
          ),
          action: 'send-without-logs',
        };
      }
      return {
        text: messages.pgettext(
          'report-problem',
          'The report could not be sent. Check the connection and try again.',
        ),
      };
  }
}
