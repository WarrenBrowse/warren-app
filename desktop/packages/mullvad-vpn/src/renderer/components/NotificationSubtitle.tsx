import React from 'react';
import styled from 'styled-components';

import { InAppNotification, InAppNotificationSubtitle } from '../../shared/notifications';
import { Link } from '../lib/components/link';
import { surfaces } from '../lib/foundations';
import { ExternalLink } from './ExternalLink';
import { InternalLink } from './InternalLink';
import { NotificationSubtitleText } from './NotificationBanner';

export type NotificationSubtitleProps = Pick<InAppNotification, 'subtitle'>;

const StyledLink = styled(Link)``;

// The home banner's body, in the theme's secondary text; its links take the
// theme's main text so they read on the dark glass and on the paper one.
const StyledSubtitle = styled(NotificationSubtitleText)`
  && a,
  && a *,
  && button,
  && button * {
    color: ${surfaces.homeText};
  }
`;

const formatSubtitle = (subtitle: InAppNotificationSubtitle) => {
  if (subtitle.action) {
    switch (subtitle.action.type) {
      case 'navigate-internal':
        return (
          <InternalLink variant="labelTinySemiBold" {...subtitle.action.link}>
            <InternalLink.Text>{subtitle.content}</InternalLink.Text>
          </InternalLink>
        );
      case 'navigate-external':
        return (
          <ExternalLink variant="labelTinySemiBold" {...subtitle.action.link}>
            <ExternalLink.Text>{subtitle.content}</ExternalLink.Text>
            <ExternalLink.Icon icon="external" />
          </ExternalLink>
        );
      case 'run-function':
        return (
          <StyledLink forwardedAs="button" variant="labelTinySemiBold" {...subtitle.action.button}>
            <StyledLink.Text>{subtitle.content}</StyledLink.Text>
          </StyledLink>
        );
      default:
        break;
    }
  }

  return subtitle.content;
};

const getSubtitleKey = (subtitle: InAppNotificationSubtitle) => {
  if ('key' in subtitle) {
    return subtitle.key;
  }

  return subtitle.content;
};

export const NotificationSubtitle = ({ subtitle, ...props }: NotificationSubtitleProps) => {
  if (!subtitle) {
    return null;
  }

  if (!Array.isArray(subtitle)) {
    return <StyledSubtitle {...props}>{subtitle}</StyledSubtitle>;
  }

  return (
    <StyledSubtitle {...props}>
      {subtitle.map((subtitle, index, arr) => {
        const content = formatSubtitle(subtitle);
        const key = getSubtitleKey(subtitle);

        return (
          <React.Fragment key={key}>
            {content}
            {index !== arr.length - 1 && ' '}
          </React.Fragment>
        );
      })}
    </StyledSubtitle>
  );
};
