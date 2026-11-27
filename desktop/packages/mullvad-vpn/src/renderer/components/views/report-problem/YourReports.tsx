import { useCallback, useMemo, useState } from 'react';
import styled from 'styled-components';

import { formatDate } from '../../../../shared/account-expiry';
import { Url, urls } from '../../../../shared/constants';
import {
  ForumReportLogsResult,
  IForumReportRecord,
  sanitizeForumReportRecords,
} from '../../../../shared/forum-report';
import { messages } from '../../../../shared/gettext';
import { useAppContext } from '../../../context';
import { noticeForReportLogsResult } from '../../../features/report-problem/form-state';
import { Button, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { colors, Radius, spacings } from '../../../lib/foundations';
import { useSelector } from '../../../redux/store';

const StyledSection = styled.section`
  display: flex;
  flex-direction: column;
  gap: ${spacings.small};
  padding: ${spacings.medium};
  border-top: 1px solid ${colors.whiteAlpha20};
`;

const StyledRow = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${spacings.tiny};
  padding: ${spacings.small};
  background-color: ${colors.darkerBlue50};
  border-radius: ${Radius.radius4};
`;

const StyledActions = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: ${spacings.small};
`;

const StyledOutcome = styled.p<{ $ok: boolean }>`
  margin: 0;
  color: ${({ $ok }) => ($ok ? colors.green : colors.red)};
  font-size: 13px;
  line-height: 19px;
`;

/**
 * The reports this app filed, newest first, each with its topic and a way to
 * send fresh logs to it: what the forum banner tells a reporter to look for
 * ("Report a problem, then your report, then Send new logs"). Hidden until
 * there is one.
 */
export function YourReports() {
  const stored = useSelector((state) => state.settings.guiSettings.forumReports);
  const reports = useMemo(() => sanitizeForumReportRecords(stored), [stored]);
  if (reports.length === 0) {
    return null;
  }
  return (
    <StyledSection>
      <Text variant="titleMedium">
        {
          // TRANSLATORS: Heading of the list of reports this app has filed.
          messages.pgettext('report-problem', 'Your reports')
        }
      </Text>
      {reports.map((report) => (
        <ReportRow key={report.topicId} report={report} />
      ))}
    </StyledSection>
  );
}

function ReportRow({ report }: { report: IForumReportRecord }) {
  const { openUrl } = useAppContext();
  const locale = useSelector((state) => state.userInterface.locale);
  const [sending, setSending] = useState(false);
  const [outcome, setOutcome] = useState<ForumReportLogsResult | undefined>(undefined);

  const handleOpen = useCallback(
    () => openUrl(`${urls.forum}t/${report.topicId}` as Url),
    [openUrl, report.topicId],
  );
  const handleSendLogs = useCallback(async () => {
    setSending(true);
    setOutcome(undefined);
    const result = await window.ipc.forumReport.sendLogs(report.topicId);
    setSending(false);
    setOutcome(result);
  }, [report.topicId]);

  const notice = outcome === undefined ? undefined : noticeForReportLogsResult(outcome);

  return (
    <StyledRow>
      <Text variant="bodySmallSemibold">{report.summary}</Text>
      <Text variant="labelTiny" color="whiteOnDarkBlue60">
        {formatDate(report.createdAt, locale)}
      </Text>
      <StyledActions>
        <Button type="button" width="fit" onClick={handleOpen}>
          <Button.Text>
            {
              // TRANSLATORS: Button that opens the created forum topic.
              messages.pgettext('report-problem', 'Open the topic')
            }
          </Button.Text>
        </Button>
        <Button type="button" width="fit" disabled={sending} onClick={handleSendLogs}>
          <Button.Text>
            {sending
              ? // TRANSLATORS: Label of the "Send new logs" button while it works.
                messages.pgettext('report-problem', 'Sending the logs, please wait.')
              : // TRANSLATORS: Button that sends fresh logs to a report already filed.
                messages.pgettext('report-problem', 'Send new logs')}
          </Button.Text>
        </Button>
      </StyledActions>
      {notice && (
        <FlexColumn gap="tiny">
          <StyledOutcome $ok={notice.ok} role={notice.ok ? 'status' : 'alert'}>
            {notice.text}
          </StyledOutcome>
        </FlexColumn>
      )}
    </StyledRow>
  );
}
