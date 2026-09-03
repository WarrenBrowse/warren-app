// Compiled Warren product environment (prod | staging | beta) of this build.
//
// `WARREN_PRODUCT_ENV` is replaced by a string literal at build time by
// vite's `define` (main and renderer builds, see vite.config.ts), fed from
// the env var of the same name that also selects the Rust side
// (`warren-product-env` crate) and the packaging identity
// (tasks/distribution.cjs). Where the define is absent (unit tests run
// outside vite) the identifier is undefined and the build behaves as prod.
declare const WARREN_PRODUCT_ENV: string | undefined;

export type ProductEnvironment = 'prod' | 'staging' | 'beta';

function resolveProductEnvironment(): ProductEnvironment {
  const value = typeof WARREN_PRODUCT_ENV === 'string' ? WARREN_PRODUCT_ENV : 'prod';
  switch (value) {
    case 'prod':
    case 'staging':
    case 'beta':
      return value;
    default:
      throw new Error(`Invalid WARREN_PRODUCT_ENV baked into this build: ${value}`);
  }
}

export const productEnvironment: ProductEnvironment = resolveProductEnvironment();

// Build-time beta flag, written directly against the define (not via
// `productEnvironment`) so bundlers can constant-fold it: in a packaged
// build `WARREN_PRODUCT_ENV` is a string literal, the whole expression
// collapses to true/false, and beta-only UI branches drop out of prod
// bundles.
export const isBetaBuild: boolean =
  (typeof WARREN_PRODUCT_ENV === 'string' ? WARREN_PRODUCT_ENV : 'prod') === 'beta';

// Per-environment anchors, a copy of the Rust `warren-product-env` crate's
// table (the daemon side of the same values). The crate is the reference:
// its `tests/platform_lockstep.rs` reads this file and fails on drift, and
// `test/unit/product-env.spec.ts` replays the shared fixture from this side.
const productAnchorsByEnvironment = {
  prod: {
    // Trailing slash: consumers append relative paths (`${api}v1/...`).
    apiBaseUrl: 'https://api.warrenbrowse.com/',
    // Electron `productName`: namespaces userData/logs on every platform.
    displayName: 'Warren VPN',
    // Unix-style product dir, names the daemon RPC socket.
    unixProductDir: 'warren-vpn',
    // Deep-link URL scheme: per env so a beta and a prod install never
    // fight over OS-level scheme registration (lockstep with
    // tasks/distribution.cjs and the Android manifest placeholder).
    deepLinkScheme: 'warren',
  },
  staging: {
    apiBaseUrl: 'https://api.staging.warrenbrowse.com/',
    displayName: 'Warren VPN Staging',
    unixProductDir: 'warren-vpn-staging',
    deepLinkScheme: 'warren-staging',
  },
  beta: {
    apiBaseUrl: 'https://api.beta.warrenbrowse.com/',
    displayName: 'Warren VPN Beta',
    unixProductDir: 'warren-vpn-beta',
    deepLinkScheme: 'warren-beta',
  },
} as const;

export const productAnchors = productAnchorsByEnvironment[productEnvironment];
