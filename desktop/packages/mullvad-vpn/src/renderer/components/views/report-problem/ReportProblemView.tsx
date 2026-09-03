import React, { useCallback, useEffect, useRef, useState } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { Url, urls } from '../../../../shared/constants';
import {
  FORUM_REPORT_AREAS,
  FORUM_REPORT_FREQUENCIES,
  FORUM_REPORT_MIN_DESCRIPTION_CHARS,
  ForumReportArea,
  ForumReportFrequency,
} from '../../../../shared/forum-report';
import { messages } from '../../../../shared/gettext';
import { useAppContext } from '../../../context';
import {
  beginReportCollect,
  beginReportSend,
  canSendReport,
  createdReportNotice,
  descriptionChars,
  initialReportProblemFormState,
  noticeForForumReportResult,
  reportFormPayload,
  ReportProblemFormState,
  setReportArea,
  setReportFrequency,
  setReportIncludeLogs,
  setReportSteps,
  setReportWhatHappened,
  settleReportCollect,
  settleReportSend,
} from '../../../features/report-problem/form-state';
import { Button, Text } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { Listbox } from '../../../lib/components/listbox';
import { Switch } from '../../../lib/components/switch';
import { View } from '../../../lib/components/view';
import { colors, Radius, spacings } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { SettingsListItem } from '../../settings-list-item';

const StyledForm = styled.form`
  display: flex;
  flex-direction: column;
  gap: ${spacings.medium};
  padding: 0 ${spacings.medium} ${spacings.medium};
`;

const StyledField = styled.label`
  display: flex;
  flex-direction: column;
  gap: ${spacings.tiny};
  color: ${colors.white};
  font-size: 13px;
  line-height: 19px;
`;

const StyledTextarea = styled.textarea`
  width: 100%;
  padding: ${spacings.small};
  background-color: ${colors.darkerBlue50};
  border: 1px solid ${colors.whiteAlpha20};
  border-radius: ${Radius.radius4};
  color: ${colors.white};
  font-family: inherit;
  font-size: 13px;
  line-height: 1.5;
  resize: vertical;

  &::placeholder {
    color: ${colors.whiteAlpha40};
  }

  &:focus-visible {
    outline: 2px solid ${colors.white};
    outline-offset: 2px;
  }

  &:disabled {
    color: ${colors.whiteAlpha60};
  }
`;

// Readable on the dark view: the default dim text is not.
const StyledRefusal = styled.p`
  margin: 0;
  color: ${colors.red};
  font-size: 13px;
  line-height: 19px;
`;

const StyledCreated = styled.p`
  margin: 0;
  color: ${colors.green};
  font-size: 13px;
  line-height: 19px;
`;

function areaLabel(area: ForumReportArea): string {
  switch (area) {
    case 'browsing':
      // TRANSLATORS: A choice of where the reported problem happens.
      return messages.pgettext(
        'report-problem',
        'Browsing (pages not loading, slow, broken sites)',
      );
    case 'connection':
      // TRANSLATORS: A choice of where the reported problem happens.
      return messages.pgettext('report-problem', 'Connection (cannot connect, drops, no internet)');
    case 'wallet':
      // TRANSLATORS: A choice of where the reported problem happens.
      return messages.pgettext('report-problem', 'Wallet, payment or subscription');
    case 'install':
      // TRANSLATORS: A choice of where the reported problem happens.
      return messages.pgettext('report-problem', 'Installing or updating Warren');
    case 'other':
      // TRANSLATORS: A choice of where the reported problem happens.
      return messages.pgettext('report-problem', 'Something else');
  }
}

function frequencyLabel(frequency: ForumReportFrequency): string {
  switch (frequency) {
    case 'always':
      // TRANSLATORS: A choice of how often the reported problem happens.
      return messages.pgettext('report-problem', 'Every time');
    case 'sometimes':
      // TRANSLATORS: A choice of how often the reported problem happens.
      return messages.pgettext('report-problem', 'Sometimes');
    case 'once':
      // TRANSLATORS: A choice of how often the reported problem happens.
      return messages.pgettext('report-problem', 'It happened once');
  }
}

/**
 * The in-app bug report: the forum's "Report a bug" form, filed with the
 * wallet signature and the redacted logs through the connect broker, so a
 * user who cannot complete the browser sign-in can still be heard. The
 * description is public under the anonymous forum name; the logs go privately
 * to the support team, and can be read here before they leave the machine.
 * The Android "Report a problem" screen is the reference; the rules live in
 * `features/report-problem/form-state.ts`.
 */
export function ReportProblemView() {
  const { pop } = useHistory();
  const { openUrl } = useAppContext();
  const [state, setState] = useState<ReportProblemFormState>(initialReportProblemFormState);
  const whatHappenedId = React.useId();
  const stepsId = React.useId();
  const includeLogsId = React.useId();
  const includeLogsDescriptionId = React.useId();

  // A previewed report outliving the form would sit in the OS temp dir.
  const previewRef = useRef<string | undefined>(undefined);
  useEffect(() => {
    previewRef.current = state.previewReportId;
  }, [state.previewReportId]);
  useEffect(
    () => () => {
      if (previewRef.current !== undefined) {
        void window.ipc.forumReport.discard(previewRef.current);
      }
    },
    [],
  );

  const handleArea = useCallback(
    (area: ForumReportArea) => setState((current) => setReportArea(current, area)),
    [],
  );
  const handleFrequency = useCallback(
    (frequency: ForumReportFrequency) =>
      setState((current) => setReportFrequency(current, frequency)),
    [],
  );
  const handleWhatHappened = useCallback(
    (event: React.ChangeEvent<HTMLTextAreaElement>) =>
      setState((current) => setReportWhatHappened(current, event.target.value)),
    [],
  );
  const handleSteps = useCallback(
    (event: React.ChangeEvent<HTMLTextAreaElement>) =>
      setState((current) => setReportSteps(current, event.target.value)),
    [],
  );

  const handleIncludeLogs = useCallback(
    (includeLogs: boolean) => {
      if (!includeLogs && state.previewReportId !== undefined) {
        void window.ipc.forumReport.discard(state.previewReportId);
      }
      setState((current) => setReportIncludeLogs(current, includeLogs));
    },
    [state.previewReportId],
  );

  // Collects a report so the person can read it (nothing leaves the machine
  // for it) and opens it the way the attach prompt does. A fresh collection
  // each time, the previous one discarded once the new one exists.
  const handleViewLogs = useCallback(async () => {
    const previous = state.previewReportId;
    setState((current) => beginReportCollect(current));
    const reportId = await window.ipc.forumReport.collect();
    setState((current) => settleReportCollect(current, reportId));
    if (reportId !== undefined) {
      if (previous !== undefined) {
        void window.ipc.forumReport.discard(previous);
      }
      void window.ipc.problemReport.viewLog(reportId);
    }
  }, [state.previewReportId]);

  const send = useCallback(async (from: ReportProblemFormState) => {
    if (!canSendReport(from)) {
      return;
    }
    const payload = reportFormPayload(from);
    setState((current) => beginReportSend(current));
    const result = await window.ipc.forumReport.send(payload);
    setState((current) => settleReportSend(current, result));
  }, []);

  const handleSubmit = useCallback(
    (event: React.FormEvent) => {
      event.preventDefault();
      void send(state);
    },
    [send, state],
  );

  // After a size refusal or the upload deadline: the same report, logs off.
  const handleSendWithoutLogs = useCallback(() => {
    if (state.previewReportId !== undefined) {
      void window.ipc.forumReport.discard(state.previewReportId);
    }
    const next = setReportIncludeLogs(state, false);
    setState(next);
    void send(next);
  }, [send, state]);

  const handleOpenHelpForm = useCallback(() => openUrl(urls.help), [openUrl]);

  const chars = descriptionChars(state);
  const result = state.result;
  // Main kept the URL only when it points at the forum origin the app vouches
  // for, which is what the Url type spells.
  const topicUrl = result?.kind === 'created' ? result.topicUrl : undefined;
  const handleOpenTopic = useCallback(() => {
    if (topicUrl !== undefined) {
      void openUrl(topicUrl as Url);
    }
  }, [openUrl, topicUrl]);
  const notice = result === undefined ? undefined : noticeForForumReportResult(result);
  const busy = state.sending || state.collecting;

  // TRANSLATORS: Title of the in-app bug report view.
  const title = messages.pgettext('report-problem', 'Report a problem');

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader title={title} />
          <NavigationScrollbars>
            <View.Content>
              <StyledForm onSubmit={handleSubmit}>
                <Text variant="bodySmall" color="whiteOnDarkBlue60">
                  {
                    // TRANSLATORS: Explains where the report and the logs go.
                    messages.pgettext(
                      'report-problem',
                      'Your description is posted on the community forum under your anonymous forum name. The technical logs, if you include them, go privately to the support team and never appear publicly.',
                    )
                  }
                </Text>

                <Listbox value={state.area} onValueChange={handleArea}>
                  <Listbox.Header>
                    <Listbox.Header.Item>
                      <Listbox.Header.Item.Label>
                        {
                          // TRANSLATORS: Label of the choice of where the problem happens.
                          messages.pgettext('report-problem', 'Where does the problem happen?')
                        }
                      </Listbox.Header.Item.Label>
                    </Listbox.Header.Item>
                  </Listbox.Header>
                  <Listbox.Options>
                    {FORUM_REPORT_AREAS.map((area) => (
                      <Listbox.Options.Option key={area} value={area} disabled={state.sending}>
                        <Listbox.Options.Option.Trigger>
                          <Listbox.Options.Option.Item>
                            <Listbox.Options.Option.Item.Label>
                              {areaLabel(area)}
                            </Listbox.Options.Option.Item.Label>
                          </Listbox.Options.Option.Item>
                        </Listbox.Options.Option.Trigger>
                      </Listbox.Options.Option>
                    ))}
                  </Listbox.Options>
                </Listbox>

                <StyledField htmlFor={whatHappenedId}>
                  {
                    // TRANSLATORS: Label of the description field.
                    messages.pgettext('report-problem', 'What happened?')
                  }
                  <StyledTextarea
                    id={whatHappenedId}
                    value={state.whatHappened}
                    onChange={handleWhatHappened}
                    rows={5}
                    disabled={state.sending}
                    placeholder={
                      // TRANSLATORS: Placeholder of the description field.
                      messages.pgettext(
                        'report-problem',
                        'What did you see, and what did you expect instead? Copy any error message. Never include your wallet address.',
                      )
                    }
                  />
                  <Text variant="labelTiny" color="whiteOnDarkBlue60">
                    {sprintf(
                      // TRANSLATORS: Character counter under the description field.
                      // TRANSLATORS: Available placeholders:
                      // TRANSLATORS: %(count)d - characters typed so far
                      // TRANSLATORS: %(min)d - the shortest description the forum accepts
                      messages.pgettext(
                        'report-problem',
                        '%(count)d characters (at least %(min)d)',
                      ),
                      { count: chars, min: FORUM_REPORT_MIN_DESCRIPTION_CHARS },
                    )}
                  </Text>
                </StyledField>

                <StyledField htmlFor={stepsId}>
                  {
                    // TRANSLATORS: Label of the optional steps-to-reproduce field.
                    messages.pgettext('report-problem', 'How can we make it happen too? (optional)')
                  }
                  <StyledTextarea
                    id={stepsId}
                    value={state.steps}
                    onChange={handleSteps}
                    rows={3}
                    disabled={state.sending}
                    placeholder={
                      // TRANSLATORS: Placeholder of the steps-to-reproduce field.
                      messages.pgettext(
                        'report-problem',
                        'What did you do just before? Step by step if you can.',
                      )
                    }
                  />
                </StyledField>

                <Listbox value={state.frequency} onValueChange={handleFrequency}>
                  <Listbox.Header>
                    <Listbox.Header.Item>
                      <Listbox.Header.Item.Label>
                        {
                          // TRANSLATORS: Label of the choice of how often the problem happens.
                          messages.pgettext('report-problem', 'How often does it happen?')
                        }
                      </Listbox.Header.Item.Label>
                    </Listbox.Header.Item>
                  </Listbox.Header>
                  <Listbox.Options>
                    {FORUM_REPORT_FREQUENCIES.map((frequency) => (
                      <Listbox.Options.Option
                        key={frequency}
                        value={frequency}
                        disabled={state.sending}>
                        <Listbox.Options.Option.Trigger>
                          <Listbox.Options.Option.Item>
                            <Listbox.Options.Option.Item.Label>
                              {frequencyLabel(frequency)}
                            </Listbox.Options.Option.Item.Label>
                          </Listbox.Options.Option.Item>
                        </Listbox.Options.Option.Trigger>
                      </Listbox.Options.Option>
                    ))}
                  </Listbox.Options>
                </Listbox>

                <FlexColumn gap="small">
                  <SettingsListItem labelId={includeLogsId}>
                    <SettingsListItem.Item>
                      <Switch
                        inputId={includeLogsId}
                        descriptionId={includeLogsDescriptionId}
                        checked={state.includeLogs}
                        onCheckedChange={handleIncludeLogs}
                        disabled={state.sending}>
                        <Switch.Label>
                          {
                            // TRANSLATORS: Label of the switch that attaches the logs.
                            messages.pgettext('report-problem', 'Include technical logs')
                          }
                        </Switch.Label>
                        <SettingsListItem.Item.ActionGroup>
                          <Switch.Input />
                        </SettingsListItem.Item.ActionGroup>
                      </Switch>
                    </SettingsListItem.Item>
                    <SettingsListItem.Footer>
                      <SettingsListItem.Footer.Text id={includeLogsDescriptionId}>
                        {
                          // TRANSLATORS: Description of the switch that attaches the logs.
                          messages.pgettext(
                            'report-problem',
                            'Anonymized logs from this app, visible to the support team only.',
                          )
                        }
                      </SettingsListItem.Footer.Text>
                    </SettingsListItem.Footer>
                  </SettingsListItem>
                  {state.includeLogs && (
                    <FlexColumn gap="tiny">
                      <Button type="button" disabled={busy} onClick={handleViewLogs} width="fit">
                        <Button.Text>
                          {
                            // TRANSLATORS: Button that opens the logs about to be sent.
                            messages.pgettext('report-problem', 'View the logs')
                          }
                        </Button.Text>
                      </Button>
                      <Text variant="labelTiny" color="whiteOnDarkBlue60" role="status">
                        {state.collecting
                          ? // TRANSLATORS: Shown while the logs are being collected.
                            messages.pgettext(
                              'report-problem',
                              'Preparing the report, please wait.',
                            )
                          : ''}
                      </Text>
                      {state.collectFailed && (
                        <StyledRefusal role="alert">
                          {
                            // TRANSLATORS: Shown when the logs could not be collected.
                            messages.pgettext(
                              'report-problem',
                              'The logs could not be collected. You can still send the description.',
                            )
                          }
                        </StyledRefusal>
                      )}
                    </FlexColumn>
                  )}
                </FlexColumn>

                {result?.kind === 'created' && (
                  <FlexColumn gap="tiny" role="status">
                    <StyledCreated>{createdReportNotice(result)}</StyledCreated>
                    {result.identity && (
                      <Text variant="labelTiny" color="whiteOnDarkBlue60">
                        {sprintf(
                          // TRANSLATORS: Names the anonymous forum handle the report was posted under.
                          // TRANSLATORS: Available placeholders:
                          // TRANSLATORS: %(handle)s - the forum name
                          messages.pgettext('report-problem', 'Posted as %(handle)s.'),
                          { handle: result.identity.handle },
                        )}
                      </Text>
                    )}
                    {topicUrl !== undefined && (
                      <Button type="button" width="fit" onClick={handleOpenTopic}>
                        <Button.Text>
                          {
                            // TRANSLATORS: Button that opens the created forum topic.
                            messages.pgettext('report-problem', 'Open the topic')
                          }
                        </Button.Text>
                      </Button>
                    )}
                  </FlexColumn>
                )}

                {notice && (
                  <FlexColumn gap="tiny">
                    <StyledRefusal role="alert" aria-live="assertive">
                      {notice.text}
                    </StyledRefusal>
                    {notice.action === 'open-help-form' && (
                      <Button type="button" width="fit" onClick={handleOpenHelpForm}>
                        <Button.Text>
                          {
                            // TRANSLATORS: Button that opens the website help form.
                            messages.pgettext('report-problem', 'Open the help form')
                          }
                        </Button.Text>
                      </Button>
                    )}
                    {notice.action === 'send-without-logs' && (
                      <Button
                        type="button"
                        width="fit"
                        disabled={busy}
                        onClick={handleSendWithoutLogs}>
                        <Button.Text>
                          {
                            // TRANSLATORS: Button that resends the report with the logs left out.
                            messages.pgettext('report-problem', 'Send without the logs')
                          }
                        </Button.Text>
                      </Button>
                    )}
                  </FlexColumn>
                )}

                <Button type="submit" variant="success" disabled={!canSendReport(state)}>
                  <Button.Text>
                    {state.sending
                      ? // TRANSLATORS: Send button label while the report is being sent.
                        messages.pgettext('report-problem', 'Sending, please wait.')
                      : // TRANSLATORS: Send button label.
                        messages.pgettext('report-problem', 'Send the report')}
                  </Button.Text>
                </Button>
              </StyledForm>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
