import React, { useCallback } from 'react';
import { sprintf } from 'sprintf-js';

import { productEnvironment } from '../../../../../../shared/constants/product-env';
import { messages } from '../../../../../../shared/gettext';
import log from '../../../../../../shared/logging';
import { removeNonNumericCharacters } from '../../../../../../shared/string-helpers';
import { useAppContext } from '../../../../../context';
import { ListItemProps } from '../../../../../lib/components/list-item';
import { useTextField } from '../../../../../lib/components/text-field';
import { useSelector } from '../../../../../redux/store';
import { SettingsListItem } from '../../../../settings-list-item';

const MIN_MAX_RATE_MBPS = 1;
const MAX_MAX_RATE_MBPS = 10_000;
const BPS_PER_MBPS = 1_000_000;

export type MaxRateSettingProps = Omit<ListItemProps, 'children'>;

function maxRateIsValid(mbps: string): boolean {
  const parsed = mbps ? parseInt(mbps, 10) : undefined;
  return parsed === undefined || (parsed >= MIN_MAX_RATE_MBPS && parsed <= MAX_MAX_RATE_MBPS);
}

// The user setting is server-agnostic display-side: entered in Mbps,
// persisted in bits per second (the daemon's unit).
export function MaxRateSetting(props: MaxRateSettingProps) {
  if (productEnvironment === 'beta') {
    return <BetaMaxRateRow {...props} />;
  }
  return <EditableMaxRateRow {...props} />;
}

// Beta builds: the cap is network-imposed and server-enforced, so the
// row is read-only. It surfaces the live cap from the daemon's
// network-info feed so the shown figure follows an ops-side change.
function BetaMaxRateRow(props: MaxRateSettingProps) {
  const labelId = React.useId();
  const descriptionId = React.useId();
  const networkInfo = useSelector((state) => state.settings.warrenStatus?.networkInfo);
  const capMbps = networkInfo?.defaultRateBps
    ? Math.round(networkInfo.defaultRateBps / BPS_PER_MBPS)
    : undefined;

  return (
    <SettingsListItem
      anchorId="max-rate-setting"
      aria-labelledby={labelId}
      position="solo"
      disabled
      {...props}>
      <SettingsListItem.Item>
        <SettingsListItem.Item.Label id={labelId}>
          {
            // TRANSLATORS: The title for the bandwidth limit setting.
            messages.pgettext('vpn-settings-view', 'Max bandwidth')
          }
        </SettingsListItem.Item.Label>
        <SettingsListItem.Item.Text>
          {capMbps !== undefined
            ? sprintf(
                // TRANSLATORS: Value shown for the bandwidth limit in beta builds.
                // TRANSLATORS: Available placeholders:
                // TRANSLATORS: %(mbps)d - the bandwidth cap in Mbps
                messages.pgettext('vpn-settings-view', '%(mbps)d Mbps'),
                { mbps: capMbps },
              )
            : // TRANSLATORS: Shown when the beta bandwidth cap is not known yet.
              messages.pgettext('vpn-settings-view', 'Managed')}
        </SettingsListItem.Item.Text>
      </SettingsListItem.Item>
      <SettingsListItem.Footer>
        <SettingsListItem.Footer.Text id={descriptionId}>
          {
            // TRANSLATORS: Hint below the bandwidth limit row in beta builds.
            messages.pgettext(
              'vpn-settings-view',
              'Bandwidth is limited by the free beta network and enforced by the servers. This setting is unavailable during the beta.',
            )
          }
        </SettingsListItem.Footer.Text>
      </SettingsListItem.Footer>
    </SettingsListItem>
  );
}

function EditableMaxRateRow(props: MaxRateSettingProps) {
  const { setWarrenMaxRateBps: setWarrenMaxRateBpsImpl } = useAppContext();
  const maxRateBps = useSelector((state) => state.settings.warrenMaxRateBps);

  const inputRef = React.useRef<HTMLInputElement>(null);
  const labelId = React.useId();
  const descriptionId = React.useId();

  const setMaxRate = useCallback(
    async (bps?: number) => {
      try {
        await setWarrenMaxRateBpsImpl(bps);
      } catch (e) {
        const error = e as Error;
        log.error('Failed to update max rate value', error.message);
      }
    },
    [setWarrenMaxRateBpsImpl],
  );

  const onSubmit = useCallback(
    async (value: string) => {
      if (maxRateIsValid(value)) {
        const mbps = value === '' ? undefined : parseInt(value, 10);
        await setMaxRate(mbps === undefined ? undefined : mbps * BPS_PER_MBPS);
      }
    },
    [setMaxRate],
  );

  const { value, handleOnValueChange, invalid, dirty, blur, reset } = useTextField({
    inputRef,
    defaultValue: maxRateBps ? Math.round(maxRateBps / BPS_PER_MBPS).toString() : '',
    format: removeNonNumericCharacters,
    validate: maxRateIsValid,
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
      anchorId="max-rate-setting"
      aria-labelledby={labelId}
      position="solo"
      {...props}>
      <SettingsListItem.Item>
        <SettingsListItem.Item.Label id={labelId}>
          {
            // TRANSLATORS: The title for the bandwidth limit setting.
            messages.pgettext('vpn-settings-view', 'Max bandwidth')
          }
        </SettingsListItem.Item.Label>
        <SettingsListItem.Item.ActionGroup>
          <SettingsListItem.Item.TextField
            value={value}
            onValueChange={handleOnValueChange}
            onSubmit={handleSubmit}
            invalid={invalid}>
            <SettingsListItem.Item.TextField.Input
              ref={inputRef}
              placeholder={
                // TRANSLATORS: Placeholder shown when no bandwidth limit is set.
                messages.pgettext('vpn-settings-view', 'Unlimited')
              }
              width="small"
              inputMode="numeric"
              maxLength={5}
              aria-labelledby={labelId}
              aria-describedby={descriptionId}
              onBlur={handleBlur}
            />
          </SettingsListItem.Item.TextField>
        </SettingsListItem.Item.ActionGroup>
      </SettingsListItem.Item>
      <SettingsListItem.Footer>
        <SettingsListItem.Footer.Text id={descriptionId}>
          {sprintf(
            // TRANSLATORS: The hint displayed below the bandwidth limit input field.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(max)d - the maximum accepted value in Mbps
            // TRANSLATORS: %(min)d - the minimum accepted value in Mbps
            messages.pgettext(
              'vpn-settings-view',
              'Limit the VPN bandwidth in Mbps, applied to upload and download separately. Valid range: %(min)d - %(max)d. Leave empty for no limit. Applies immediately.',
            ),
            { min: MIN_MAX_RATE_MBPS, max: MAX_MAX_RATE_MBPS },
          )}
        </SettingsListItem.Footer.Text>
      </SettingsListItem.Footer>
    </SettingsListItem>
  );
}
