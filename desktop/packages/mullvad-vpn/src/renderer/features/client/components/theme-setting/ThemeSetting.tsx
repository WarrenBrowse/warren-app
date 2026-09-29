import { useId } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import { ThemePreference } from '../../../../../shared/theme';
import { SettingsAccordion } from '../../../../components/settings-accordion';
import { SettingsListbox } from '../../../../components/settings-listbox';
import { AccordionProps } from '../../../../lib/components/accordion';
import { ListItem } from '../../../../lib/components/list-item';
import { spacings } from '../../../../lib/foundations';
import { useTheme } from '../../hooks';

export type ThemeSettingProps = Omit<AccordionProps, 'children'>;

const StyledAccordionTrigger = styled(SettingsAccordion.Header.AccordionTrigger)`
  display: grid;
  place-items: center;
  align-self: stretch;
  width: 48px;
`;

const StyledTrailing = styled.div`
  display: flex;
  align-items: center;
  align-self: stretch;
  gap: ${spacings.small};
`;

// Folded, with the current choice in the header: following the system is right
// for almost everyone, so the three options only cost space when overridden.
export function ThemeSetting(props: ThemeSettingProps) {
  const titleId = useId();
  const { theme, setTheme } = useTheme();

  // TRANSLATORS: Theme option: the screens follow the light or dark setting of the device.
  const system = messages.pgettext('user-interface-settings-view', 'System');
  // TRANSLATORS: Theme option: always the dark screens.
  const dark = messages.pgettext('user-interface-settings-view', 'Dark');
  // TRANSLATORS: Theme option: always the light (cream) screens.
  const light = messages.pgettext('user-interface-settings-view', 'Light');
  const labels: Record<ThemePreference, string> = { system, dark, light };

  return (
    <SettingsAccordion accordionId="theme-setting" titleId={titleId} {...props}>
      <SettingsAccordion.Container>
        <SettingsAccordion.Header>
          <SettingsAccordion.Header.Item>
            <SettingsAccordion.Header.Item.Title>
              {
                // TRANSLATORS: Title of the setting choosing between the light and dark screens.
                messages.pgettext('user-interface-settings-view', 'Theme')
              }
            </SettingsAccordion.Header.Item.Title>
            <StyledTrailing>
              <ListItem.Item.Text data-testid="theme-setting-current">
                {labels[theme]}
              </ListItem.Item.Text>
              <StyledAccordionTrigger>
                <SettingsAccordion.Header.Item.Chevron />
              </StyledAccordionTrigger>
            </StyledTrailing>
          </SettingsAccordion.Header.Item>
        </SettingsAccordion.Header>
        <SettingsAccordion.Content>
          <SettingsListbox<ThemePreference>
            value={theme}
            onValueChange={setTheme}
            labelId={titleId}>
            <SettingsListbox.Options>
              <SettingsListbox.Options.BaseOption value="system">
                {labels.system}
              </SettingsListbox.Options.BaseOption>
              <SettingsListbox.Options.BaseOption value="dark">
                {labels.dark}
              </SettingsListbox.Options.BaseOption>
              <SettingsListbox.Options.BaseOption value="light">
                {labels.light}
              </SettingsListbox.Options.BaseOption>
            </SettingsListbox.Options>
          </SettingsListbox>
        </SettingsAccordion.Content>
      </SettingsAccordion.Container>
    </SettingsAccordion>
  );
}
