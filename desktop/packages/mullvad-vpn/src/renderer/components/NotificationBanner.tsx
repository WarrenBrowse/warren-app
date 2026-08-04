import { motion } from 'motion/react';
import React, { useLayoutEffect, useRef, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../shared/gettext';
import { InAppNotificationIndicatorType } from '../../shared/notifications/notification';
import { IconButton } from '../lib/components';
import { colors } from '../lib/foundations';
import { useExclusiveTask } from '../lib/hooks/use-exclusive-task';
import { tinyText } from './common-styles';

const NOTIFICATION_AREA_ID = 'notification-area';

export const NotificationTitle = styled.span(tinyText, {
  color: colors.white,
});

export const NotificationSubtitleText = styled.span(tinyText, {
  color: colors.whiteAlpha60,
});

interface INotificationSubtitleProps {
  children?: React.ReactNode;
}

export function NotificationSubtitle(props: INotificationSubtitleProps) {
  return React.Children.count(props.children) > 0 ? <NotificationSubtitleText {...props} /> : null;
}

// How many lines of an unbounded banner text are shown before it is clamped.
// Three keeps the banner from swallowing the connect view, which a long
// operator notice otherwise does.
const CLAMPED_SUBTITLE_LINES = 3;

const ClampedSubtitleText = styled(NotificationSubtitleText)<{ $clickable: boolean }>((props) => ({
  display: '-webkit-box',
  WebkitBoxOrient: 'vertical',
  WebkitLineClamp: CLAMPED_SUBTITLE_LINES,
  overflow: 'hidden',
  cursor: props.$clickable ? 'pointer' : 'auto',
}));

const ExpandHint = styled.button(tinyText, {
  alignSelf: 'flex-start',
  marginTop: '2px',
  padding: 0,
  border: 'none',
  background: 'none',
  color: colors.white,
  textDecoration: 'underline',
  cursor: 'pointer',
});

interface INotificationExpandableSubtitleProps {
  text: string;
  // Label of the affordance shown only when the text is actually clamped.
  expandLabel: string;
  onExpand: () => void;
}

// Subtitle for a banner whose text has no length bound. It renders clamped and
// says so: the affordance appears only when the text really overflows, which is
// measured rather than guessed from a character count, because the banner width
// changes with the window.
export function NotificationExpandableSubtitle(props: INotificationExpandableSubtitleProps) {
  const textRef = useRef<HTMLSpanElement>(null);
  const [truncated, setTruncated] = useState(false);

  useLayoutEffect(() => {
    const element = textRef.current;
    if (!element) {
      return;
    }
    // A one-pixel slack absorbs sub-pixel line-height rounding, which would
    // otherwise report a clamp on text that fits exactly.
    const measure = () => setTruncated(element.scrollHeight - element.clientHeight > 1);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [props.text]);

  return (
    <>
      <ClampedSubtitleText
        ref={textRef}
        $clickable={truncated}
        onClick={truncated ? props.onExpand : undefined}>
        {props.text}
      </ClampedSubtitleText>
      {truncated && (
        <ExpandHint onClick={props.onExpand} aria-describedby={NOTIFICATION_AREA_ID}>
          {props.expandLabel}
        </ExpandHint>
      )}
    </>
  );
}

interface NotificationActionProps {
  onClick: () => Promise<void>;
}

export function NotificationOpenLinkAction(props: NotificationActionProps) {
  const [onClick] = useExclusiveTask(props.onClick);
  return (
    <IconButton
      size="small"
      variant="secondary"
      onClick={onClick}
      aria-describedby={NOTIFICATION_AREA_ID}
      aria-label={messages.gettext('Open URL')}>
      <IconButton.Icon icon="external" />
    </IconButton>
  );
}

export function NotificationTroubleshootDialogAction(props: NotificationActionProps) {
  return (
    <IconButton
      size="small"
      variant="secondary"
      aria-describedby={NOTIFICATION_AREA_ID}
      aria-label={messages.gettext('Troubleshoot')}
      onClick={props.onClick}>
      <IconButton.Icon icon="info-circle" />
    </IconButton>
  );
}

export function NotificationCloseAction(props: NotificationActionProps) {
  return (
    <IconButton
      aria-describedby={NOTIFICATION_AREA_ID}
      variant="secondary"
      aria-label={messages.pgettext('accessibility', 'Close notification')}
      onClick={props.onClick}
      size="small">
      <IconButton.Icon icon="cross-circle" />
    </IconButton>
  );
}

export const NotificationContent = styled.div.attrs({ id: NOTIFICATION_AREA_ID })({
  display: 'flex',
  flexDirection: 'column',
  flex: 1,
  paddingRight: '4px',
});

export const NotificationActions = styled.div({
  display: 'flex',
  flex: 0,
  flexDirection: 'column',
  justifyContent: 'center',
});

interface INotificationIndicatorProps {
  $type?: InAppNotificationIndicatorType;
}

const notificationIndicatorTypeColorMap = {
  success: colors.green,
  warning: colors.yellow,
  error: colors.red,
};

export const NotificationIndicator = styled.div<INotificationIndicatorProps>((props) => ({
  width: '10px',
  height: '10px',
  borderRadius: '5px',
  marginTop: '4px',
  marginRight: '8px',
  backgroundColor: props.$type
    ? notificationIndicatorTypeColorMap[props.$type]
    : colors.transparent,
}));

// Floats as a glass card over the scenery, matching the connection panel, rather
// than a solid bar. A green top accent echoes the mockup's update banner.
const Collapsible = styled(motion.div)({
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'flex-start',
  translateY: '0%',
  // Offset toward the right rather than spanning the full width (per the art
  // direction), leaving the left of the scene open.
  margin: '12px 16px 0 auto',
  maxWidth: '300px',
  borderRadius: '14px',
  border: `1px solid ${colors.whiteAlpha20}`,
  borderTop: `2px solid ${colors.green}`,
  backgroundColor: colors.blackAlpha60,
  backdropFilter: 'blur(10px)',
  boxShadow: '0 8px 24px rgba(0, 0, 0, 0.35)',
  overflow: 'hidden',
});

const Content = styled.section({
  display: 'flex',
  flexDirection: 'row',
  padding: '10px 12px 10px 16px',
  height: 'fit-content',
});

interface INotificationBannerProps {
  children?: React.ReactNode; // Array<NotificationContent | NotificationActions>,
  className?: string;
  animateIn: boolean;
}

export function NotificationBanner({ className, children, animateIn }: INotificationBannerProps) {
  const translateYInitial = animateIn ? '-100%' : '0%';

  return (
    <Collapsible
      animate={{ translateY: '0%' }}
      className={className}
      exit={{ translateY: '-100%' }}
      initial={{ translateY: translateYInitial }}
      transition={{ duration: 0.25 }}>
      <Content>{children}</Content>
    </Collapsible>
  );
}
