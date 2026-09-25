import React from 'react';

import { Flex } from '../../../../../lib/components';
import { Switch } from '../../../../../lib/components/switch';
import { HeaderSubTitle } from '../../../../SettingsHeader';

export type TabHeaderProps = {
  label: string;
  description?: React.ReactNode;
  // Omitted for a tab that has no mode to switch (Linux Bypass VPN is
  // launch based).
  checked?: boolean;
  disabled?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  children?: React.ReactNode;
};

// The switch row that opens each App routing tab, and the line saying what the
// tab does.
export function TabHeader({
  label,
  description,
  checked,
  disabled,
  onCheckedChange,
  children,
}: TabHeaderProps) {
  const descriptionId = React.useId();
  const inputId = React.useId();

  return (
    <Flex flexDirection="column" gap="small" margin={{ horizontal: 'medium', bottom: 'medium' }}>
      {onCheckedChange !== undefined && (
        <Switch
          inputId={inputId}
          descriptionId={description ? descriptionId : undefined}
          checked={checked}
          disabled={disabled}
          onCheckedChange={onCheckedChange}>
          <Flex justifyContent="space-between" alignItems="center">
            <Switch.Label>{label}</Switch.Label>
            <Switch.Input />
          </Flex>
        </Switch>
      )}
      {description && <HeaderSubTitle id={descriptionId}>{description}</HeaderSubTitle>}
      {children}
    </Flex>
  );
}
