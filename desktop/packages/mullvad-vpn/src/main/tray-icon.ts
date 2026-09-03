import { nativeImage } from 'electron';
import path from 'path';

import { productEnvironment } from '../shared/constants/product-env';

// A non-prod build serves the whole menubar tree from a sibling directory whose
// assets carry the beta pip, under file names identical to the prod ones, so
// the suffix matrix in tray-icon-controller.ts stays untouched.
//
// The test is `productEnvironment !== 'prod'`, not `isBetaBuild`: staging is a
// non-prod install that has to be tellable from prod on the same machine, and
// the packaging identity already hands staging the beta app icon (iconSuffix in
// tasks/distribution.cjs) for that same reason. There is no third artwork.
const BADGED_ENVIRONMENT_DIR = 'beta';

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
    return productEnvironment === 'prod' ? undefined : BADGED_ENVIRONMENT_DIR;
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
