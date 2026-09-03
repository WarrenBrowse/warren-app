// Types for the packaging config consumed from TypeScript (the unit test that
// pins the per-environment icons). The task scripts themselves stay plain CJS,
// so only the surface a TS caller touches is declared here.
export declare function newConfig(): {
  appId: string;
  productName: string;
  mac: { icon: string };
  linux: { icon: string };
  win: { icon: string };
  nsis: { installerSidebar: string };
  [key: string]: unknown;
};
export declare const WINDOWS_ASAR_UNPACK: string[];
export declare function packWin(...args: unknown[]): unknown;
export declare function packMac(...args: unknown[]): unknown;
export declare function packLinux(...args: unknown[]): unknown;
