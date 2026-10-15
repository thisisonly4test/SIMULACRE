import { defineConfig, devices } from "@playwright/test";

// Two deployments under test, with an offline face source:
// - "desktop"/"mobile": the real Rust server and a throw-away vault;
// - "browser-*": the static build (as on Cloudflare): engine in WebAssembly,
//   vault in IndexedDB, served by tests/e2e/static-server.mjs.
const launch = process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {};

export default defineConfig({
  testDir: "tests/e2e",
  timeout: 60_000,
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: { baseURL: "http://127.0.0.1:7988", trace: "retain-on-failure", launchOptions: launch },
  webServer: [
    { command: "node tests/e2e/mock-faces.mjs 7989", url: "http://127.0.0.1:7989/health", reuseExistingServer: false, timeout: 10_000 },
    { command: "node tests/e2e/static-server.mjs 7990", url: "http://127.0.0.1:7990/health", reuseExistingServer: false, timeout: 10_000 },
    {
      command: "rm -rf .e2e-data && cargo run -q -p simulacre-server -- --addr 127.0.0.1:7988 --data .e2e-data",
      url: "http://127.0.0.1:7988/api/health",
      env: { SIMULACRE_FACE_URL: "http://127.0.0.1:7989/face.png" },
      reuseExistingServer: false,
      timeout: 600_000,
    },
  ],
  projects: [
    { name: "desktop", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 }, launchOptions: launch }, testIgnore: /mobile\.spec/ },
    { name: "mobile", use: { ...devices["Pixel 7"], launchOptions: launch }, testMatch: /mobile\.spec/ },
    { name: "browser-desktop", use: { ...devices["Desktop Chrome"], baseURL: "http://127.0.0.1:7990", viewport: { width: 1440, height: 900 }, launchOptions: launch }, testIgnore: /mobile\.spec/ },
    { name: "browser-mobile", use: { ...devices["Pixel 7"], baseURL: "http://127.0.0.1:7990", launchOptions: launch }, testMatch: /mobile\.spec/ },
  ],
});
