import React from 'react';
import styled from 'styled-components';

import { strings } from '../../shared/constants';
import { messages } from '../../shared/gettext';
import log from '../../shared/logging';
import { Button } from '../lib/components';
import { FlexColumn } from '../lib/components/flex-column';
import { formatErrorDetails } from '../lib/error-details';
import { ErrorView } from './views';

interface IProps {
  children?: React.ReactNode;
}

interface IState {
  error?: Error;
  componentStack?: string;
}

const Email = styled.span({
  fontWeight: 900,
});

// The renderer log is out of reach for most users, so this view has to be
// enough on its own: name the failure and offer a way out that is not
// force-quitting the app.
const StyledErrorDetails = styled.pre({
  maxHeight: '140px',
  overflow: 'auto',
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
  fontSize: '11px',
  lineHeight: '15px',
  textAlign: 'left',
  color: 'rgba(255, 255, 255, 0.6)',
  backgroundColor: 'rgba(0, 0, 0, 0.2)',
  borderRadius: '4px',
  padding: '8px',
  userSelect: 'text',
});

export default class ErrorBoundary extends React.Component<IProps, IState> {
  public state: IState = {};

  public componentDidCatch(error: Error, info: React.ErrorInfo) {
    this.setState({ error, componentStack: info.componentStack ?? undefined });

    log.error(
      `The error boundary caught an error: ${error.message}\nError stack: ${
        error.stack || 'Not available'
      }\nComponent stack: ${info.componentStack}`,
    );
  }

  public render() {
    const { error, componentStack } = this.state;
    if (error) {
      const reachBackMessage: React.ReactNode[] =
        // TRANSLATORS: The message displayed to the user in case of critical error in the GUI
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(email)s - support email
        messages
          .pgettext('error-boundary-view', 'Something went wrong. Please contact us at %(email)s')
          .split('%(email)s', 2);
      void reachBackMessage.splice(1, 0, <Email>{strings.supportEmail}</Email>);

      return (
        <ErrorView
          settingsUnavailable
          footer={
            <FlexColumn gap="medium">
              <StyledErrorDetails>{formatErrorDetails(error, componentStack)}</StyledErrorDetails>
              <Button onClick={this.reload}>
                <Button.Text>
                  {
                    // TRANSLATORS: Button label on the fatal-error view that reloads the app
                    // TRANSLATORS: interface, as an alternative to force-quitting the app.
                    messages.pgettext('error-boundary-view', 'Reload the app')
                  }
                </Button.Text>
              </Button>
            </FlexColumn>
          }>
          {reachBackMessage}
        </ErrorView>
      );
    } else {
      return this.props.children;
    }
  }

  // A full renderer reload: the crash the boundary catches is a rendering
  // error, so remounting the UI and re-syncing state from the main process
  // is the same remedy as force-quitting and reopening the app, minus the
  // force-quit. The tunnel and the daemon are untouched. No setState first:
  // re-rendering the crashing child before the reload lands would just
  // re-throw.
  private reload = () => {
    window.location.reload();
  };
}
