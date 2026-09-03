import { ExternalLinkProps } from '../../renderer/components/ExternalLink';
import { InternalLinkProps } from '../../renderer/components/InternalLink';
import { ButtonProps } from '../../renderer/lib/components';
import { RoutePath } from '../../shared/routes';
import { Url } from '../constants';

export type SystemNotificationAction =
  | {
      type: 'navigate-internal';
      link: {
        to: RoutePath;
        text?: string;
      };
    }
  | {
      type: 'navigate-external';
      link: {
        to: Url;
        text?: string;
        withAuth?: boolean;
      };
    };

export interface InAppNotificationTroubleshootInfo {
  details: string;
  steps: string[];
  buttons?: Array<InAppNotificationTroubleshootButton>;
}

export interface InAppNotificationTroubleshootButton {
  label: string;
  action: () => void;
  variant?: 'primary' | 'success' | 'destructive';
}

export type InAppNotificationAction =
  | {
      type: 'troubleshoot-dialog';
      troubleshoot: InAppNotificationTroubleshootInfo;
    }
  | {
      type: 'close';
      close: () => void;
    }
  | {
      type: 'navigate-internal';
      link: Pick<InternalLinkProps, 'to' | 'onClick' | 'aria-label'>;
    }
  | {
      type: 'navigate-external';
      link: Pick<ExternalLinkProps, 'to' | 'onClick' | 'aria-label' | 'withAuth'>;
    }
  | {
      type: 'run-function';
      button: Pick<ButtonProps, 'onClick' | 'aria-label'>;
    }
  | {
      // The banner text is unbounded (an operator-authored notice), so it is
      // clamped to a few lines and the full text moves to a scrollable modal.
      // Carrying the content here rather than reusing the subtitle keeps the
      // modal independent of how the subtitle happens to be rendered.
      type: 'expand-text';
      expand: { title: string; content: string };
      // The expand control lives under the text, so the action column is free
      // for a close when the banner may be put away.
      dismiss?: () => void;
    }
  | {
      // A launch announcement is a card rather than a one-line banner: it
      // brings its own headline, an optional call to action, and a voucher
      // code the reader has to be able to copy. The whole payload travels
      // here so the providers stay pure data and the card owns its layout.
      type: 'announcement-card';
      announcement: InAppAnnouncementCard;
    };

export interface InAppAnnouncementCard {
  id: string;
  body: string;
  // Verbatim, in the grouping the operator published: a code is transcribed
  // by hand as often as it is copied, so it is never regrouped here.
  voucherCode: string | null;
  // `null` when the announcement carries no call to action, and when its url
  // did not survive the render-time https check: the text still reaches the
  // reader, the link never becomes clickable.
  cta: { label: string; url: Url } | null;
  // An announcement is an event, so the reader can put it away for good. A
  // notice carries no equivalent: it is a live operator statement and clears
  // from the same signal that raised it.
  dismiss: () => void;
}

export type InAppNotificationIndicatorType = 'success' | 'warning' | 'error';

export enum SystemNotificationSeverityType {
  info = 0,
  low,
  medium,
  high,
}

export enum SystemNotificationCategory {
  tunnelState,
  expiry,
  newVersion,
  inconsistentVersion,
  // Auto-renewal lifecycle (warren-core doc 65): its own category so device-event
  // handling that clears `expiry` notifications leaves these alone.
  renewal,
  // Community-forum activity. Its own category so a fresh count replaces the
  // previous banner instead of stacking one per refresh.
  forumActivity,
}

interface NotificationProvider {
  mayDisplay(): boolean;
}

export interface SystemNotification {
  message: string;
  severity: SystemNotificationSeverityType;
  category: SystemNotificationCategory;
  throttle?: boolean;
  presentOnce?: { value: boolean; name: string };
  suppressInDevelopment?: boolean;
  action?: SystemNotificationAction;
}

export interface InAppNotification {
  indicator?: InAppNotificationIndicatorType;
  action?: InAppNotificationAction;
  title: string;
  subtitle?: string | React.ReactElement | InAppNotificationSubtitle[];
}

export type InAppNotificationSubtitleString = {
  content: string;
};

export type InAppNotificationSubtitleElement = {
  content: React.ReactElement;
  key: string;
};

export type InAppNotificationSubtitle = (
  | InAppNotificationSubtitleString
  | InAppNotificationSubtitleElement
) & {
  action?: InAppNotificationAction;
};

export interface SystemNotificationProvider extends NotificationProvider {
  getSystemNotification(): SystemNotification | undefined;
}

export interface InAppNotificationProvider extends NotificationProvider {
  getInAppNotification(): InAppNotification | undefined;
}
