import { Action, Location } from 'history';

import { TransitionType } from '../renderer/lib/history';

export interface ICurrentAppVersionInfo {
  gui: string;
  daemon?: string;
  isConsistent: boolean;
  isBeta: boolean;
}

export interface IWindowShapeParameters {
  arrowPosition?: number;
}

export type SuppressOutdatedVersionOption = {
  type: 'suppress-outdated-version-warning';
};

export type ScrollToAnchorId =
  | 'daita-enable-setting'
  | 'multihop-setting'
  | 'custom-dns-settings'
  | 'allow-lan-setting'
  | 'lockdown-mode-setting'
  | 'dns-blocker-setting'
  | 'mtu-setting'
  | 'obfuscation-setting'
  | 'port-setting'
  | 'mss-fix-setting'
  | 'quantum-resistant-setting'
  // Anchors for the Warren toggles in VpnSettingsView.
  | 'warren-local-account-setting'
  // Anchor for the warren-api URL input.
  | 'warren-api-url-setting'
  // Anchors for the Warren multi-hop view (M4.H.C).
  | 'warren-multi-hop-setting'
  | 'warren-multi-hop-country-pickers'
  // Anchors for the Warren status display (reconnect_count + age,
  // obfuscation indicator).
  | 'warren-status-reconnect'
  | 'warren-obfuscation-indicator'
  // Anchor for the multi-exit auto-failover toggle (M5.B.2).
  | 'warren-failover-setting'
  // Anchor for the port-forwarding toggle row.
  | 'port-forwarding-setting'
  // Anchor for the port-forwarding advanced controls (protocol +
  // preferred port). Used when an external link (e.g., a "Configure
  // your port" CTA in a future onboarding step) wants to deep-link
  // directly to the form rather than the toggle.
  | 'port-forwarding-advanced';

export type ScrollToAnchorOption = {
  type: 'scroll-to-anchor';
  id: ScrollToAnchorId;
};

export type LocationStateOptions = SuppressOutdatedVersionOption | ScrollToAnchorOption;

export type IChangelog = Array<string>;

export interface LocationState {
  scrollPosition: [number, number];
  expandedSections: Record<string, boolean>;
  transition: TransitionType;
  options?: LocationStateOptions[];
}

export interface IHistoryObject {
  entries: Location<LocationState>[];
  index: number;
  lastAction: Action;
}

export type ScrollPositions = Record<string, [number, number]>;

export type DaemonStatus = 'start-requested' | 'running' | 'stopped';
