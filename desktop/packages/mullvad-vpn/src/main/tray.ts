import { Tray } from 'electron';

import { productAnchors } from '../shared/constants/product-env';
import { TrayIcon } from './tray-icon';

function getInitialIcon() {
  if (process.platform === 'linux') {
    return new TrayIcon('lock-placeholder');
  }

  return new TrayIcon();
}

export function createTray() {
  const initialIcon = getInitialIcon();

  const tray = new Tray(initialIcon.toNativeImage());

  // The environment display name, not the prod one: a machine running both
  // products shows two lock icons, and the tooltip is what tells them apart.
  tray.setToolTip(productAnchors.displayName);

  // disable double click on tray icon since it causes weird delay
  tray.setIgnoreDoubleClickEvents(true);

  return tray;
}
