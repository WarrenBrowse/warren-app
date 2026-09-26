import React from 'react';

import { NatPmpRule } from '../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../shared/gettext';
import {
  ProbeResult,
  TORRENT_CLIENT_KINDS,
  TorrentClientKind,
  torrentClientLabel,
  TorrentClientRuleRef,
  torrentClientUsesUsername,
} from '../../../../../shared/torrent-client';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { spacings } from '../../../../lib/foundations';
import { usePortForwarding, useTorrentClient } from '../../hooks';
import { protocolLabel, rulePort } from '../../mapping';
import {
  torrentClientConfigErrorLine,
  torrentClientProbeLine,
  torrentClientStatusLine,
} from '../../torrent-client';
import {
  StyledActions,
  StyledButton,
  StyledField,
  StyledInput,
  StyledSelect,
} from './TorrentClientSetting.styles';

/** The address of the client's web interface, as a fresh install serves it.
 * Shown as a placeholder rather than prefilled: a prefilled address that
 * nothing answers on reads as a configured client that is broken. */
function placeholderUrl(kind: TorrentClientKind | 'none'): string {
  switch (kind) {
    case 'transmission':
      return 'http://127.0.0.1:9091';
    case 'deluge':
      return 'http://127.0.0.1:8112';
    default:
      return 'http://127.0.0.1:8080';
  }
}

function ruleKey(rule: TorrentClientRuleRef | NatPmpRule): string {
  return `${rule.protocol}-${rule.internalPort}`;
}

function savedPlaceholder(): string {
  // TRANSLATORS: Placeholder of the password field, shown when a password is
  // TRANSLATORS: already stored and the field is empty.
  return messages.pgettext('port-forwarding-view', 'Saved');
}

/**
 * Hands the forwarded public port to the user's torrent client.
 *
 * Rendered under the notification toggle when port forwarding is on. The
 * section is deliberately a plain form: a client, an address, credentials,
 * and the two actions that let the user see it working right away instead of
 * waiting for the exit to move a grant.
 */
export function TorrentClientSetting() {
  const { config, status, save, test, applyNow } = useTorrentClient();
  const { rules } = usePortForwarding();

  const [url, setUrl] = React.useState(config.url);
  const [username, setUsername] = React.useState(config.username);
  const [password, setPassword] = React.useState('');
  const [message, setMessage] = React.useState<string | undefined>(undefined);
  const [busy, setBusy] = React.useState(false);

  // Re-sync when the stored settings change underneath the form, which is
  // what a save answers with.
  React.useEffect(() => {
    setUrl(config.url);
    setUsername(config.username);
  }, [config.url, config.username]);

  const persist = React.useCallback(
    async (patch: {
      kind?: TorrentClientKind | 'none';
      url?: string;
      username?: string;
      password?: string;
      rule?: TorrentClientRuleRef;
    }) => {
      const refusal = await save({
        kind: patch.kind ?? config.kind,
        url: patch.url ?? url,
        username: patch.username ?? username,
        // Absent unless the user typed one, so a save that only changes the
        // address keeps the stored password rather than clearing it.
        password: patch.password,
        rule: patch.rule ?? config.rule,
      });
      setMessage(refusal === 'encryption-unavailable' ? torrentClientConfigErrorLine() : undefined);
      return refusal;
    },
    [save, config.kind, config.rule, url, username],
  );

  const handleKindChange = React.useCallback(
    (event: React.ChangeEvent<HTMLSelectElement>) => {
      const kind = event.target.value as TorrentClientKind | 'none';
      void persist({ kind });
    },
    [persist],
  );

  const handleUrlChange = React.useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    setUrl(event.target.value);
  }, []);

  const handleUrlBlur = React.useCallback(() => {
    if (url !== config.url) {
      void persist({ url });
    }
  }, [persist, url, config.url]);

  const handleUsernameChange = React.useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    setUsername(event.target.value);
  }, []);

  const handleUsernameBlur = React.useCallback(() => {
    if (username !== config.username) {
      void persist({ username });
    }
  }, [persist, username, config.username]);

  const handlePasswordChange = React.useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    setPassword(event.target.value);
  }, []);

  // The password leaves the renderer once, when the field is done with, never
  // on a keystroke. The field is cleared only once the main process says the
  // password was stored: an emptied field over nothing saved is the one state
  // the user cannot tell apart from a saved password.
  const handlePasswordBlur = React.useCallback(() => {
    if (password === '') {
      return;
    }
    void persist({ password }).then((refusal) => {
      if (refusal === undefined) {
        setPassword('');
      }
    });
  }, [persist, password]);

  const handleRuleChange = React.useCallback(
    (event: React.ChangeEvent<HTMLSelectElement>) => {
      const chosen = rules.find((rule) => ruleKey(rule) === event.target.value);
      if (chosen !== undefined) {
        void persist({
          rule: { internalPort: chosen.internalPort, protocol: chosen.protocol },
        });
      }
    },
    [persist, rules],
  );

  const handleTest = React.useCallback(async () => {
    setBusy(true);
    const result = await test();
    setBusy(false);
    setMessage(
      'kind' in result
        ? torrentClientStatusLine({ state: 'error', error: result, at: Date.now() }, config)
        : torrentClientProbeLine(result as ProbeResult, config),
    );
  }, [test, config]);

  const handleApply = React.useCallback(async () => {
    setBusy(true);
    setMessage(undefined);
    await applyNow();
    setBusy(false);
  }, [applyNow]);

  // The same fallback the controller applies: a recorded rule the user has
  // since deleted is not in the list, and a select whose value matches no
  // option shows an empty box next to a feature that is in fact running on
  // the first rule.
  const linkedRule =
    rules.find(
      (candidate) => config.rule !== undefined && ruleKey(candidate) === ruleKey(config.rule),
    ) ?? rules[0];
  const configured = config.kind !== 'none';
  const line =
    message ?? (status === undefined ? undefined : torrentClientStatusLine(status, config));
  const working = busy || status?.state === 'pushing';

  return (
    <FlexColumn gap="small">
      <Text variant="titleMedium">
        {
          // TRANSLATORS: Title of the section that hands the forwarded port to
          // TRANSLATORS: a torrent client.
          messages.pgettext('port-forwarding-view', 'Torrent client')
        }
      </Text>

      <SettingsListItem>
        <SettingsListItem.Item>
          <FlexColumn gap="medium" style={{ width: '100%', paddingInlineEnd: spacings.medium }}>
            <StyledField>
              <Text variant="labelTiny" color="whiteAlpha60">
                {messages.pgettext('port-forwarding-view', 'Client')}
              </Text>
              <StyledSelect
                value={config.kind}
                onChange={handleKindChange}
                aria-label={messages.pgettext('port-forwarding-view', 'Client')}>
                <option value="none">{messages.pgettext('port-forwarding-view', 'None')}</option>
                {TORRENT_CLIENT_KINDS.map((kind) => (
                  <option key={kind} value={kind}>
                    {torrentClientLabel(kind)}
                  </option>
                ))}
              </StyledSelect>
            </StyledField>

            {configured ? (
              <>
                <StyledField>
                  <Text variant="labelTiny" color="whiteAlpha60">
                    {messages.pgettext('port-forwarding-view', 'Web interface address')}
                  </Text>
                  <StyledInput
                    type="text"
                    value={url}
                    onChange={handleUrlChange}
                    onBlur={handleUrlBlur}
                    placeholder={placeholderUrl(config.kind)}
                    aria-label={messages.pgettext('port-forwarding-view', 'Web interface address')}
                  />
                </StyledField>

                {torrentClientUsesUsername(config.kind) ? (
                  <StyledField>
                    <Text variant="labelTiny" color="whiteAlpha60">
                      {messages.pgettext('port-forwarding-view', 'Username')}
                    </Text>
                    <StyledInput
                      type="text"
                      value={username}
                      onChange={handleUsernameChange}
                      onBlur={handleUsernameBlur}
                      aria-label={messages.pgettext('port-forwarding-view', 'Username')}
                    />
                  </StyledField>
                ) : null}

                <StyledField>
                  <Text variant="labelTiny" color="whiteAlpha60">
                    {messages.pgettext('port-forwarding-view', 'Password')}
                  </Text>
                  <StyledInput
                    type="password"
                    value={password}
                    onChange={handlePasswordChange}
                    onBlur={handlePasswordBlur}
                    placeholder={config.hasPassword ? savedPlaceholder() : ''}
                    aria-label={messages.pgettext('port-forwarding-view', 'Password')}
                  />
                </StyledField>

                {rules.length > 1 ? (
                  <StyledField>
                    <Text variant="labelTiny" color="whiteAlpha60">
                      {messages.pgettext('port-forwarding-view', 'Rule')}
                    </Text>
                    <StyledSelect
                      value={ruleKey(linkedRule)}
                      onChange={handleRuleChange}
                      aria-label={messages.pgettext('port-forwarding-view', 'Rule')}>
                      {rules.map((rule) => (
                        <option key={ruleKey(rule)} value={ruleKey(rule)}>
                          {`${rulePort(rule)} ${protocolLabel(rule.protocol)}`}
                        </option>
                      ))}
                    </StyledSelect>
                  </StyledField>
                ) : null}

                <StyledActions>
                  <StyledButton
                    type="button"
                    $disabled={working}
                    disabled={working}
                    onClick={handleTest}>
                    {messages.pgettext('port-forwarding-view', 'Test connection')}
                  </StyledButton>
                  <StyledButton
                    type="button"
                    $disabled={working}
                    disabled={working}
                    onClick={handleApply}>
                    {messages.pgettext('port-forwarding-view', 'Apply now')}
                  </StyledButton>
                </StyledActions>
              </>
            ) : null}
          </FlexColumn>
        </SettingsListItem.Item>
      </SettingsListItem>

      {line === undefined ? null : (
        <Text
          variant="labelTiny"
          color={status?.state === 'error' || status?.state === 'held' ? 'red' : 'whiteAlpha60'}>
          {line}
        </Text>
      )}

      <Text variant="labelTiny" color="whiteAlpha60">
        {
          // TRANSLATORS: Description of the torrent client section.
          messages.pgettext(
            'port-forwarding-view',
            'Warren can enter the public port into your torrent client and update it whenever it changes. Enable the client’s web interface first.',
          )
        }
      </Text>
    </FlexColumn>
  );
}
