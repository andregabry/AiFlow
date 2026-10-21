import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 60000,
  use: {
    colorScheme: "dark",
    launchOptions: { args: ["--no-proxy-server"] },
    baseURL: "http://127.0.0.1:1421",
    viewport: { width: 1440, height: 1000 },
    reducedMotion: "reduce",
    screenshot: "only-on-failure",
  },
  webServer: {
    command:
      "npm run build && npx vite preview --host 127.0.0.1 --port 1421 --strictPort",
    url: "http://127.0.0.1:1421",
    reuseExistingServer: false,
    timeout: 120000,
  },
});
