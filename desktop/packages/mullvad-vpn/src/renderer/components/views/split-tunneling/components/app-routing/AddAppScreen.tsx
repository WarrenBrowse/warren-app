import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { appRules, sameAppId } from '../../../../../../shared/app-routing';
import { type ISplitTunnelingApplication } from '../../../../../../shared/application-types';
import { messages } from '../../../../../../shared/gettext';
import log from '../../../../../../shared/logging';
import { useAppContext } from '../../../../../context';
import { useAppRouting } from '../../../../../features/app-routing/hooks';
import { Flex, IconButton, Spinner } from '../../../../../lib/components';
import { colors } from '../../../../../lib/foundations';
import SearchBar from '../../../../SearchBar';
import { useFilePicker } from '../../hooks/use-file-picker';
import { useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { includesSearchTerm } from '../../utils';
import { getFilePickerOptionsForPlatform } from '../split-tunneling-settings/utils';
import { AppAvatar } from './AppAvatar';
import { ScreenFrame } from './ScreenFrame';
import {
  StyledHelp,
  StyledRowButton,
  StyledRowName,
  StyledScreenBody,
  StyledScreenButton,
  StyledScreenTitle,
} from './styles';

const StyledItem = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: '4px',
  flexShrink: 0,
});

const StyledEmpty = styled(StyledHelp)({
  textAlign: 'center',
  padding: '12px 0',
});

function FolderGlyph() {
  return (
    <svg
      width="36"
      height="36"
      viewBox="0 0 24 24"
      fill="none"
      stroke={colors.whiteAlpha60}
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      style={{ flexShrink: 0, padding: '5px' }}>
      <path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
    </svg>
  );
}

function basename(filePath: string) {
  return filePath.split(/[\\/]/).pop() || filePath;
}

type AppItemProps = {
  application: ISplitTunnelingApplication;
  onPick: (application: ISplitTunnelingApplication) => void;
  onForget: (application: ISplitTunnelingApplication) => void;
};

function AppItem({ application, onPick, onForget }: AppItemProps) {
  const pick = React.useCallback(() => onPick(application), [application, onPick]);
  const forget = React.useCallback(() => onForget(application), [application, onForget]);
  return (
    <StyledItem>
      <StyledRowButton type="button" onClick={pick}>
        <AppAvatar name={application.name} icon={application.icon} />
        <StyledRowName>{application.name}</StyledRowName>
      </StyledRowButton>
      {application.deletable && (
        <IconButton
          variant="secondary"
          aria-label={sprintf(
            // TRANSLATORS: Accessibility label of the button removing an app
            // TRANSLATORS: the user added by hand from the list of apps.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(application)s - the app's name
            messages.pgettext('split-tunneling-view', 'Remove %(application)s from the list'),
            { application: application.name },
          )}
          onClick={forget}>
          <IconButton.Icon icon="trash" />
        </IconButton>
      )}
    </StyledItem>
  );
}

// "Add an app": the apps without a rule. Picking one opens its route; no
// rule exists until a route other than the default is chosen there.
export function AddAppScreen() {
  const { routing, platform } = useAppRouting();
  const { forgetManuallyAddedSplitTunnelingApplication, resolveAppRoutingApplication } =
    useAppContext();
  const { catalog, reloadCatalog, setBrowsing, showList, showRoute } = useSplitTunnelingContext();
  const [searchTerm, setSearchTerm] = React.useState('');

  const addable = React.useMemo(() => {
    const ruled = appRules(routing, platform).map((rule) => rule.app);
    return (catalog ?? [])
      .filter(
        (application) =>
          !ruled.some((app) => sameAppId(app, application.absolutepath, platform)) &&
          includesSearchTerm(application, searchTerm),
      )
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [catalog, platform, routing, searchTerm]);

  const pick = React.useCallback(
    (application: ISplitTunnelingApplication) =>
      showRoute({ id: application.absolutepath, name: application.name, application }),
    [showRoute],
  );

  const forget = React.useCallback(
    async (application: ISplitTunnelingApplication) => {
      await forgetManuallyAddedSplitTunnelingApplication(application);
      await reloadCatalog();
    },
    [forgetManuallyAddedSplitTunnelingApplication, reloadCatalog],
  );

  const pickFile = useFilePicker(
    messages.pgettext('split-tunneling-view', 'Choose'),
    setBrowsing,
    async (filePath: string) => {
      // Planned under the id the daemon will store (a shortcut's target, a
      // desktop entry's program), so the rules the plan reads are that app's.
      let id = filePath;
      try {
        id = await resolveAppRoutingApplication(filePath);
      } catch {
        log.error('Could not resolve a picked program');
      }
      showRoute({ id, name: basename(filePath), application: filePath });
      await reloadCatalog();
    },
    getFilePickerOptionsForPlatform(),
  );

  return (
    <ScreenFrame testId="add-app-screen" onClose={showList}>
      <StyledScreenTitle>
        {messages.pgettext('split-tunneling-view', 'Add an app')}
      </StyledScreenTitle>
      <SearchBar searchTerm={searchTerm} onSearch={setSearchTerm} />
      <StyledScreenBody>
        {catalog === undefined ? (
          <Flex justifyContent="center" margin={{ top: 'large' }}>
            <Spinner size="big" />
          </Flex>
        ) : (
          <>
            {addable.map((application) => (
              <AppItem
                key={application.absolutepath}
                application={application}
                onPick={pick}
                onForget={forget}
              />
            ))}
            {addable.length === 0 && searchTerm !== '' && (
              <StyledEmpty>{messages.gettext('Try a different search.')}</StyledEmpty>
            )}
          </>
        )}
        <StyledRowButton type="button" onClick={pickFile}>
          <FolderGlyph />
          <StyledRowName>
            {
              // TRANSLATORS: Button label for browsing applications with split tunneling.
              messages.pgettext('split-tunneling-view', 'Find another app')
            }
          </StyledRowName>
        </StyledRowButton>
      </StyledScreenBody>
      <StyledScreenButton type="button" onClick={showList}>
        {messages.gettext('Cancel')}
      </StyledScreenButton>
    </ScreenFrame>
  );
}
