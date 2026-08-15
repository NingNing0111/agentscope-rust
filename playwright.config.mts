import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests/docs',
  testMatch: /.*\.spec\.ts/,
  fullyParallel: false,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: 'http://127.0.0.1:4173/agentscope-rust/',
    trace: 'on-first-retry'
  },
  webServer: {
    command: 'npm run docs:preview -- --port 4173',
    url: 'http://127.0.0.1:4173/agentscope-rust/',
    reuseExistingServer: !process.env.CI
  },
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'] } },
    { name: 'mobile', use: { ...devices['Pixel 7'] } }
  ]
})
