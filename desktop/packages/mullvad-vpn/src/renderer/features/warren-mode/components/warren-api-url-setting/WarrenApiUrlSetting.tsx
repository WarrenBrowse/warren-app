import React, { useCallback } from 'react';

import { messages } from '../../../../../shared/gettext';
import log from '../../../../../shared/logging';
import { SettingsListItem } from '../../../../components/settings-list-item';
import { useAppContext } from '../../../../context';
import { ListItemProps } from '../../../../lib/components/list-item';
import { useTextField } from '../../../../lib/components/text-field';
import { useSelector } from '../../../../redux/store';

// Entry de la page VPN settings pour `Settings::warren_api_url`.
// Pattern aligné avec `MtuSetting` (TextField + onBlur submit).
// Empty string → unset côté daemon (= fallback Mullvad upstream
// backend). Restart daemon requis pour appliquer (cf.
// `warren_remote_config::resolve` côté Rust).

export type WarrenApiUrlSettingProps = Omit<ListItemProps, 'children'>;

function urlIsValid(url: string): boolean {
  // Empty = unset = valide.
  if (url === '') {
    return true;
  }
  // Format minimal accepté : `http(s)://host[:port][/path]`. Pas de
  // validation hostname stricte (le daemon le fera au boot via reqwest).
  // Refuse les URL avec espace ou trailing slash (= cohérent avec convention).
  if (/\s/.test(url)) {
    return false;
  }
  return /^https?:\/\/[^\s/]+/.test(url);
}

export function WarrenApiUrlSetting(props: WarrenApiUrlSettingProps) {
  const { setWarrenApiUrl: setWarrenApiUrlImpl } = useAppContext();
  const warrenApiUrl = useSelector((state) => state.settings.warrenApiUrl);

  const inputRef = React.useRef<HTMLInputElement>(null);
  const labelId = React.useId();
  const descriptionId = React.useId();

  const setUrl = useCallback(
    async (url: string) => {
      try {
        await setWarrenApiUrlImpl(url);
      } catch (e) {
        const error = e as Error;
        log.error('Failed to update warren api URL', error.message);
      }
    },
    [setWarrenApiUrlImpl],
  );

  const onSubmit = useCallback(
    async (value: string) => {
      if (urlIsValid(value)) {
        await setUrl(value);
      }
    },
    [setUrl],
  );

  const { value, handleOnValueChange, invalid, dirty, blur, reset } = useTextField({
    inputRef,
    defaultValue: warrenApiUrl ?? '',
    validate: urlIsValid,
  });

  const handleBlur = React.useCallback(async () => {
    if (!invalid && dirty) {
      await onSubmit(value);
    }
    if (invalid) {
      reset();
    }
  }, [dirty, invalid, onSubmit, reset, value]);

  const handleSubmit = React.useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (!invalid) {
        await onSubmit(value);
        blur();
      }
    },
    [blur, invalid, onSubmit, value],
  );

  return (
    <SettingsListItem
      anchorId="warren-api-url-setting"
      aria-labelledby={labelId}
      position="solo"
      {...props}>
      <SettingsListItem.Item>
        <SettingsListItem.Item.Label id={labelId}>
          {messages.pgettext('vpn-settings-view', 'Warren api URL')}
        </SettingsListItem.Item.Label>
        <SettingsListItem.Item.ActionGroup>
          <SettingsListItem.Item.TextField
            value={value}
            onValueChange={handleOnValueChange}
            onSubmit={handleSubmit}
            invalid={invalid}>
            <SettingsListItem.Item.TextField.Input
              ref={inputRef}
              placeholder="https://api.warrenbrowse.com"
              aria-labelledby={labelId}
              aria-describedby={descriptionId}
              onBlur={handleBlur}
            />
          </SettingsListItem.Item.TextField>
        </SettingsListItem.Item.ActionGroup>
      </SettingsListItem.Item>
      <SettingsListItem.Footer>
        <SettingsListItem.Footer.Text id={descriptionId}>
          {messages.pgettext(
            'vpn-settings-view',
            'URL of the warren-api server. Format http(s)://host:port. Empty = unset (fallback to Mullvad upstream). Restart the daemon for the change to take effect.',
          )}
        </SettingsListItem.Footer.Text>
      </SettingsListItem.Footer>
    </SettingsListItem>
  );
}
