import React from 'react';

import { messages } from '../../../../shared/gettext';
import { SettingsListItem } from '../../../components/settings-list-item';
import { ListItemProps } from '../../../lib/components/list-item';
import { Switch } from '../../../lib/components/switch';
import { useTextField } from '../../../lib/components/text-field';
import { useWarrenCustomExit } from '../hooks/use-warren-custom-exit';

// Advanced "custom exit" override for the VPN settings page. Deliberately
// collapsed: only the enable switch shows until the user turns it on, so
// the feature stays out of the way for everyone who does not need it. The
// fields are pushed to the daemon on blur, which validates them and
// reconnects; an unparseable value keeps the tunnel on the roster.

export type WarrenCustomExitSettingProps = Omit<ListItemProps, 'children'>;

// Accept empty (= not yet filled) so the field is not flagged red before
// the user types. The daemon does the authoritative parse.
function endpointIsValid(value: string): boolean {
  if (value === '') {
    return true;
  }
  // `host:port` or bracketed `[v6]:port`; permissive on the host part.
  return /^\[?[^\s\]]+\]?:\d{1,5}$/.test(value.trim());
}

function pubkeyIsValid(value: string): boolean {
  if (value === '') {
    return true;
  }
  return /^[0-9a-fA-F]{64}$/.test(value.trim());
}

function exitIdIsValid(value: string): boolean {
  if (value === '') {
    return true;
  }
  return /^[0-9a-fA-F]{32}$/.test(value.trim());
}

function coverDomainIsValid(value: string): boolean {
  if (value === '') {
    return true;
  }
  return !/\s/.test(value);
}

interface CustomExitFieldProps {
  label: string;
  placeholder: string;
  initialValue: string;
  validate: (value: string) => boolean;
  onCommit: (value: string) => void | Promise<void>;
  description?: string;
}

// One labelled text field that commits on blur / submit when valid and
// dirty, and resets to the stored value when left in an invalid state.
// Mirrors `WarrenApiUrlSetting`.
function CustomExitField({
  label,
  placeholder,
  initialValue,
  validate,
  onCommit,
  description,
}: CustomExitFieldProps) {
  const inputRef = React.useRef<HTMLInputElement>(null);
  const labelId = React.useId();
  const descriptionId = React.useId();

  const { value, handleOnValueChange, invalid, dirty, blur, reset } = useTextField({
    inputRef,
    defaultValue: initialValue,
    validate,
  });

  const handleBlur = React.useCallback(async () => {
    if (!invalid && dirty) {
      await onCommit(value);
    } else if (invalid) {
      reset();
    }
  }, [dirty, invalid, onCommit, reset, value]);

  const handleSubmit = React.useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (!invalid) {
        await onCommit(value);
        blur();
      }
    },
    [blur, invalid, onCommit, value],
  );

  return (
    <SettingsListItem aria-labelledby={labelId} position="solo">
      <SettingsListItem.Item>
        <SettingsListItem.Item.Label id={labelId}>{label}</SettingsListItem.Item.Label>
        <SettingsListItem.Item.ActionGroup>
          <SettingsListItem.Item.TextField
            value={value}
            onValueChange={handleOnValueChange}
            onSubmit={handleSubmit}
            invalid={invalid}>
            <SettingsListItem.Item.TextField.Input
              ref={inputRef}
              placeholder={placeholder}
              aria-labelledby={labelId}
              aria-describedby={descriptionId}
              onBlur={handleBlur}
            />
          </SettingsListItem.Item.TextField>
        </SettingsListItem.Item.ActionGroup>
      </SettingsListItem.Item>
      {description ? (
        <SettingsListItem.Footer>
          <SettingsListItem.Footer.Text id={descriptionId}>
            {description}
          </SettingsListItem.Footer.Text>
        </SettingsListItem.Footer>
      ) : null}
    </SettingsListItem>
  );
}

export function WarrenCustomExitSetting(props: WarrenCustomExitSettingProps) {
  const {
    warrenCustomExit,
    setEnabled,
    setEndpoint,
    setPubkeyHex,
    setX25519MultihopPubkeyHex,
    setExitIdHex,
    setCoverDomain,
  } = useWarrenCustomExit();
  const switchLabelId = React.useId();

  return (
    <>
      <SettingsListItem position="solo" {...props}>
        <SettingsListItem.Item>
          <Switch
            checked={warrenCustomExit.enabled}
            onCheckedChange={setEnabled}
            aria-labelledby={switchLabelId}>
            <Switch.Label id={switchLabelId}>
              {messages.pgettext('vpn-settings-view', 'Custom exit (advanced)')}
            </Switch.Label>
            <SettingsListItem.Item.ActionGroup>
              <Switch.Input />
            </SettingsListItem.Item.ActionGroup>
          </Switch>
        </SettingsListItem.Item>
        <SettingsListItem.Footer>
          <SettingsListItem.Footer.Text>
            {messages.pgettext(
              'vpn-settings-view',
              'Connect to a single self-hosted exit you enter manually, bypassing the verified server list. Only use an exit you operate or fully trust: it sees your unencrypted traffic.',
            )}
          </SettingsListItem.Footer.Text>
        </SettingsListItem.Footer>
      </SettingsListItem>

      {warrenCustomExit.enabled ? (
        <>
          <CustomExitField
            label={messages.pgettext('vpn-settings-view', 'Endpoint')}
            placeholder="203.0.113.5:443"
            initialValue={warrenCustomExit.endpoint}
            validate={endpointIsValid}
            onCommit={setEndpoint}
            description={messages.pgettext(
              'vpn-settings-view',
              'Exit address as host:port. Bracket IPv6, for example [2001:db8::1]:443.',
            )}
          />
          <CustomExitField
            label={messages.pgettext('vpn-settings-view', 'Public key')}
            placeholder="64 hex characters"
            initialValue={warrenCustomExit.pubkeyHex}
            validate={pubkeyIsValid}
            onCommit={setPubkeyHex}
            description={messages.pgettext(
              'vpn-settings-view',
              "The exit's Ed25519 public key, 64 hexadecimal characters.",
            )}
          />
          <CustomExitField
            label={messages.pgettext('vpn-settings-view', 'X25519 multi-hop key')}
            placeholder="64 hex characters"
            initialValue={warrenCustomExit.x25519MultihopPubkeyHex}
            validate={pubkeyIsValid}
            onCommit={setX25519MultihopPubkeyHex}
            description={messages.pgettext(
              'vpn-settings-view',
              "The exit's X25519 multi-hop key, 64 hexadecimal characters.",
            )}
          />
          <CustomExitField
            label={messages.pgettext('vpn-settings-view', 'Exit ID')}
            placeholder="32 hex characters"
            initialValue={warrenCustomExit.exitIdHex}
            validate={exitIdIsValid}
            onCommit={setExitIdHex}
            description={messages.pgettext(
              'vpn-settings-view',
              "The exit's routing id, 32 hexadecimal characters.",
            )}
          />
          <CustomExitField
            label={messages.pgettext('vpn-settings-view', 'Cover domain (optional)')}
            placeholder="cdn.example.com"
            initialValue={warrenCustomExit.coverDomain ?? ''}
            validate={coverDomainIsValid}
            onCommit={setCoverDomain}
            description={messages.pgettext(
              'vpn-settings-view',
              'Set only when the exit runs a public TLS certificate (X.509 mode). Leave empty for the default raw-key mode.',
            )}
          />
        </>
      ) : null}
    </>
  );
}
