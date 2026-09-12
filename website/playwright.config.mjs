import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  testMatch: 'browser.spec.mjs',
  outputDir: '../artifacts/website-browser',
  reporter: 'list',
  use: { baseURL: 'http://127.0.0.1:4321', browserName: 'chromium', channel: process.env.PLAYWRIGHT_CHANNEL || undefined },
  webServer: {
    command: 'node tests/preview.mjs',
    url: 'http://127.0.0.1:4321/poqi/',
    reuseExistingServer: !process.env.CI,
  },
});
