import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { buildCountryOptions, CountryOption, sameExitChoice } from '../../../../shared/app-routing';
import { ExitChoice } from '../../../../shared/daemon-rpc-types';
import { messages, relayLocations as relayLocationsCatalog } from '../../../../shared/gettext';
import { smallText, tinyText } from '../../../components/common-styles';
import SearchBar from '../../../components/SearchBar';
import { Icon, IconButton } from '../../../lib/components';
import { Dialog } from '../../../lib/components/dialog';
import { colors, spacings } from '../../../lib/foundations';
import { useSelector } from '../../../redux/store';
import { CountryFlag } from './CountryFlag';

const StyledList = styled.ul({
  listStyle: 'none',
  margin: 0,
  padding: 0,
  // The picker opens over a 598 px window: the list scrolls, the search field
  // and the buttons stay put.
  maxHeight: '280px',
  overflowY: 'auto',
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
});

const StyledRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  borderRadius: '8px',
  backgroundColor: colors.blue40,
});

const StyledOption = styled.button<{ $indent?: boolean }>((props) => ({
  ...smallText,
  flex: 1,
  minWidth: 0,
  display: 'flex',
  alignItems: 'center',
  gap: spacings.small,
  paddingBlock: '8px',
  paddingInlineStart: props.$indent ? '38px' : spacings.small,
  paddingInlineEnd: spacings.small,
  border: 'none',
  borderRadius: '8px',
  background: 'transparent',
  color: colors.white,
  textAlign: 'start',
  cursor: 'default',
  '&&:hover': {
    backgroundColor: colors.blue60,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '-2px',
  },
}));

const StyledOptionName = styled.span({
  flex: 1,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
});

const StyledTag = styled.span({
  ...tinyText,
  color: colors.whiteAlpha60,
  flexShrink: 0,
});

const StyledEmpty = styled.p({
  ...tinyText,
  fontWeight: 400,
  margin: `${spacings.small} 0`,
  textAlign: 'center',
  color: colors.whiteAlpha60,
});

export type CountryPickerDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  applicationName: string;
  current?: ExitChoice;
  exitsInUse: ExitChoice[];
  onSelect: (exit: ExitChoice) => void;
  onRemove?: () => void;
};

// Picks the country (and optionally the city) one app leaves from. It only
// reports the choice: it never touches the main connection's location.
export function CountryPickerDialog({
  open,
  onOpenChange,
  applicationName,
  current,
  exitsInUse,
  onSelect,
  onRemove,
}: CountryPickerDialogProps) {
  const relayLocations = useSelector((state) => state.settings.relayLocations);
  const [searchTerm, setSearchTerm] = React.useState('');
  // Opens on the app's own country.
  const [expanded, setExpanded] = React.useState<ReadonlySet<string>>(
    () => new Set<string>(current ? [current.country] : []),
  );

  const translate = React.useCallback((name: string) => relayLocationsCatalog.gettext(name), []);
  const options = React.useMemo(
    () => buildCountryOptions(relayLocations, searchTerm, translate),
    [relayLocations, searchTerm, translate],
  );

  const close = React.useCallback(() => onOpenChange(false), [onOpenChange]);
  const toggleCountry = React.useCallback(
    (country: string) =>
      setExpanded((value) => {
        const next = new Set(value);
        if (!next.delete(country)) {
          next.add(country);
        }
        return next;
      }),
    [],
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Popup data-testid="country-picker">
          <Dialog.PopupContent>
            <Dialog.Title>
              {sprintf(
                // TRANSLATORS: Title of the dialog choosing the country one app
                // TRANSLATORS: leaves from. Available placeholders:
                // TRANSLATORS: %(application)s - the app's name
                messages.pgettext('split-tunneling-view', 'Country for %(application)s'),
                { application: applicationName },
              )}
            </Dialog.Title>
            <SearchBar searchTerm={searchTerm} onSearch={setSearchTerm} />
            {options.length === 0 ? (
              <StyledEmpty>{messages.gettext('Try a different search.')}</StyledEmpty>
            ) : (
              <StyledList aria-label={messages.pgettext('split-tunneling-view', 'Countries')}>
                {options.map((option) => (
                  <CountryOptionItem
                    key={option.country}
                    option={option}
                    current={current}
                    exitsInUse={exitsInUse}
                    // A search that only matched cities shows them straight away.
                    expanded={expanded.has(option.country) || searchTerm !== ''}
                    onToggle={toggleCountry}
                    onSelect={onSelect}
                  />
                ))}
              </StyledList>
            )}
            <Dialog.ButtonGroup>
              {onRemove && (
                <Dialog.Button variant="destructive" onClick={onRemove}>
                  <Dialog.Button.Text>
                    {messages.pgettext('split-tunneling-view', 'Remove country')}
                  </Dialog.Button.Text>
                </Dialog.Button>
              )}
              <Dialog.Button onClick={close}>
                <Dialog.Button.Text>{messages.gettext('Cancel')}</Dialog.Button.Text>
              </Dialog.Button>
            </Dialog.ButtonGroup>
          </Dialog.PopupContent>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog>
  );
}

type CountryOptionItemProps = {
  option: CountryOption;
  current?: ExitChoice;
  exitsInUse: ExitChoice[];
  expanded: boolean;
  onToggle: (country: string) => void;
  onSelect: (exit: ExitChoice) => void;
};

function CountryOptionItem({
  option,
  current,
  exitsInUse,
  expanded,
  onToggle,
  onSelect,
}: CountryOptionItemProps) {
  const countryExit: ExitChoice = { country: option.country };
  const showCities = option.cities.length > 1;
  const citiesId = React.useId();
  const toggle = React.useCallback(() => onToggle(option.country), [onToggle, option.country]);

  return (
    <li>
      <StyledRow>
        <ExitOptionButton
          exit={countryExit}
          label={option.name}
          flag
          current={current}
          exitsInUse={exitsInUse}
          onSelect={onSelect}
        />
        {showCities && (
          <IconButton
            variant="secondary"
            aria-expanded={expanded}
            aria-controls={citiesId}
            aria-label={sprintf(
              // TRANSLATORS: Accessibility label of the button listing the
              // TRANSLATORS: cities of a country. Available placeholders:
              // TRANSLATORS: %(country)s - the country's name
              messages.pgettext('split-tunneling-view', 'Cities in %(country)s'),
              { country: option.name },
            )}
            onClick={toggle}>
            <IconButton.Icon icon={expanded ? 'chevron-up' : 'chevron-down'} />
          </IconButton>
        )}
      </StyledRow>
      {showCities && expanded && (
        <StyledList id={citiesId} as="ul">
          {option.cities.map((city) => (
            <li key={city.code}>
              <StyledRow>
                <ExitOptionButton
                  exit={{ country: option.country, city: city.code }}
                  label={city.name}
                  indent
                  current={current}
                  exitsInUse={exitsInUse}
                  onSelect={onSelect}
                />
              </StyledRow>
            </li>
          ))}
        </StyledList>
      )}
    </li>
  );
}

type ExitOptionButtonProps = {
  exit: ExitChoice;
  label: string;
  flag?: boolean;
  indent?: boolean;
  current?: ExitChoice;
  exitsInUse: ExitChoice[];
  onSelect: (exit: ExitChoice) => void;
};

function ExitOptionButton({
  exit,
  label,
  flag,
  indent,
  current,
  exitsInUse,
  onSelect,
}: ExitOptionButtonProps) {
  const selected = current !== undefined && sameExitChoice(current, exit);
  const inUse = exitsInUse.some((used) => sameExitChoice(used, exit));

  const onClick = React.useCallback(() => onSelect(exit), [exit, onSelect]);

  return (
    <StyledOption type="button" $indent={indent} aria-pressed={selected} onClick={onClick}>
      {flag && <CountryFlag country={exit.country} />}
      <StyledOptionName>{label}</StyledOptionName>
      {inUse && !selected && (
        <StyledTag>{messages.pgettext('split-tunneling-view', 'In use')}</StyledTag>
      )}
      {selected && <Icon icon="checkmark" size="small" color="green" />}
    </StyledOption>
  );
}
