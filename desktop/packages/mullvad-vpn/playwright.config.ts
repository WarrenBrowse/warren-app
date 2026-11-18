import { PlaywrightTestConfig } from '@playwright/test';
const config: PlaywrightTestConfig = {
  testDir: './test/e2e',
  // One retry on CI, where a cold hosted runner can miss a timing; a spec that
  // only passes on its retry is still reported as flaky.
  retries: process.env.CI ? 1 : 0,
};

export default config;
