import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import react from '@vitejs/plugin-react';
import { startup } from 'vite-plugin-electron';
import electron from 'vite-plugin-electron/simple';
// `vitest/config` rather than `vite`: same function, plus the `test` field
// below. It has no effect on a build.
import { defineConfig } from 'vitest/config';

import { treeKillSync } from './vite-utils';

// Resolve the Warren product version from the single source of truth: the
// `mullvad-version` binary, which reads dist-assets/desktop-product-version.txt
// and appends a `-dev-<hash>` suffix when HEAD is not on a release tag. The
// daemon derives its own version from the exact same binary, so injecting this
// string into the main process keeps the GUI version consistent with the daemon
// in both dev (`1.0.0-dev-<hash>`) and release (`1.0.0`) builds. Without this,
// `app.getVersion()` falls back to the hardcoded `0.0.0` in package.json during
// development. Falls back to `0.0.0` if cargo is unavailable.
function resolveProductVersion(): string {
  try {
    return execFileSync('cargo', ['run', '-q', '--bin', 'mullvad-version'], {
      encoding: 'utf-8',
    }).trim();
  } catch {
    return '0.0.0';
  }
}

const PRODUCT_VERSION = resolveProductVersion();

// Compiled Warren product environment (prod | staging | beta), selected by
// the same WARREN_PRODUCT_ENV env var that drives the Rust build and the
// packaging identity (tasks/distribution.cjs). Injected as a define into
// both the main and renderer builds; src/shared/constants/product-env.ts
// resolves it (and treats a missing define as prod).
function resolveProductEnv(): string {
  const value = process.env.WARREN_PRODUCT_ENV || 'prod';
  if (!['prod', 'staging', 'beta'].includes(value)) {
    throw new Error(`WARREN_PRODUCT_ENV must be prod|staging|beta, got: ${value}`);
  }
  return value;
}

const PRODUCT_ENV = resolveProductEnv();

// NOTE: We have to monkey patch the exit handler to override the default
// behavior for how to kill the electron app. We use a custom variant of the
// vite-plugin-electron's treeKillSync function to target only the electron
// application's process and its children and not the current behavior where
// the current process' children is targeted. This is because the current
// process spawns two processes, the electron app and esbuild.
//
// The default behavior of vite-plugin-electron when the electron app needs to
// restart is to kill both the electron app and the esbuild processes, however
// after that only the electron app gets respawned, leaving the esbuild process
// permanently dead after the first time the electron app has restarted.
//
// This should be fixed upstream but until then this is an okay workaround.
// As this is a hack I didn't bother fixing the types for process.electronApp
// correctly, hence the ts-ignore below.
startup.exit = async () => {
  // eslint-disable-next-line @typescript-eslint/ban-ts-comment
  // @ts-ignore
  const electronApp = process.electronApp;
  if (electronApp) {
    await new Promise((resolve) => {
      electronApp.removeAllListeners();
      electronApp.once('exit', resolve);
      treeKillSync(electronApp.pid);
    });
  }
};

const MAIN = process.env.NODE_ENV === 'test' ? 'test/e2e/setup/main.ts' : 'src/main/index.ts';
const OUT_DIR = 'build';

const viteConfig = defineConfig({
  define: {
    global: 'window',
    WARREN_PRODUCT_ENV: JSON.stringify(PRODUCT_ENV),
    process: {
      platform: process.platform,
      env: {
        NODE_ENV: process.env.NODE_ENV,
      },
    },
  },
  mode: process.env.NODE_ENV,
  build: {
    outDir: OUT_DIR,
  },
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
  test: {
    // Unit tests never drive a real Electron: see the stub for why resolving
    // the real module throws in a fresh checkout and in CI.
    alias: {
      electron: fileURLToPath(new URL('./test/unit/electron-stub.cjs', import.meta.url)),
    },
  },
  plugins: [
    electron({
      main: {
        entry: MAIN,
        async onstart({ startup }) {
          // NOTE: vite-plugin-electron automatically adds --no-sandbox to its
          // command line arguments when spawning electron. From a security
          // standpoint this is not a good default so we omit it to allow
          // us setting it programmatically in the main process.
          //
          // Another consequence of the default --no-sandbox being added was
          // that it caused a crash when the devtools opened if the sandbox
          // had not been enabled again. However, after the default --no-sandbox
          // was omitted we can open the devtools regardless of whether the
          // sandbox is enabled or not.
          await startup(['.', ...process.argv.slice(3)]);
        },
        vite: {
          // We define process.env.NODE_ENV here in order for vite to statically
          // replace the references in the production build with the string value.
          // WARREN_GUI_VERSION carries the product version (see
          // resolveProductVersion above) so the main process reports the same
          // version as the daemon in every build mode.
          define: {
            'process.env.NODE_ENV': `"${process.env.NODE_ENV}"`,
            WARREN_GUI_VERSION: JSON.stringify(PRODUCT_VERSION),
            WARREN_PRODUCT_ENV: JSON.stringify(PRODUCT_ENV),
          },
          build: {
            outDir: OUT_DIR,
            commonjsOptions: {
              include: [
                // Packages in workspace which exports common js
                /management-interface/,
                /nseventforwarder/,
                /windows-utils/,
                // External dependencies which exports common js
                /node_modules/,
              ],
            },
            rollupOptions: {
              output: {
                // We have to specify main.js here as otherwise it would
                // inherit the name from the entry file, i.e. index
                entryFileNames: 'main.js',
              },
              external: [
                // Packages in workspace which can not be bundled
                'windows-utils',
                'nseventforwarder',
                // External dependencies
                '@grpc/grpc-js',
                'google-protobuf',
                'simple-plist',
              ],
            },
          },
          // Dependencies which can be transformed to e.g. become smaller or more efficient
          optimizeDeps: {
            include: ['management-interface', 'nseventforwarder'],
          },
        },
      },
      preload: {
        input: 'src/renderer/preload.ts',
        vite: {
          build: {
            outDir: OUT_DIR,
            rollupOptions: {
              output: {
                // We have to keep the preload script as an CJS module as otherwise we
                // would not be able to disable the Chromium sandbox at runtime using
                // the --no-sandbox flag.
                //
                // For more information see the Preload section in:
                // https://github.com/electron/electron/blob/v39.2.6/docs/tutorial/esm.md
                entryFileNames: 'preload.cjs',
              },
            },
          },
        },
      },
    }),
    react(),
  ],
});

export default viteConfig;
