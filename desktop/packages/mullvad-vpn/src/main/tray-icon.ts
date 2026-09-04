import { nativeImage } from 'electron';
import path from 'path';

import { nonProdAssetSegments } from '../shared/constants/product-env';

// A non-prod build serves the whole menubar tree from a sibling directory whose
// coloured assets are drawn in another hue family, under file names identical
// to the prod ones, so the suffix matrix in tray-icon-controller.ts stays
// untouched. The directory name and the environment test are shared with every
// other non-prod asset tree (`nonProdAssetSegments`): staging is a non-prod
// install that has to be tellable from prod on the same machine, and there is
// no third palette.

export class TrayIcon {
  constructor(public fileName?: string) {}

  public get basePath() {
    const basePath = path.resolve(import.meta.dirname, 'assets/images/menubar-icons');

    return basePath;
  }

  public get extension() {
    const extension = process.platform === 'win32' ? 'ico' : 'png';

    return extension;
  }

  public get environmentDirectory(): string | undefined {
    return nonProdAssetSegments[0];
  }

  public get filePath() {
    if (this.fileName) {
      const environmentDirectory = this.environmentDirectory;
      const filePath = path.join(
        this.basePath,
        process.platform,
        ...(environmentDirectory ? [environmentDirectory] : []),
        `${this.fileName}.${this.extension}`,
      );

      return filePath;
    }

    return null;
  }

  public toNativeImage() {
    if (this.filePath) {
      return nativeImage.createFromPath(this.filePath);
    }

    return nativeImage.createEmpty();
  }
}
