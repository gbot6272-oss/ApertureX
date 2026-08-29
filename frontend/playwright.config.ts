import { defineConfig, devices } from "@playwright/test";

// Siehe DECISIONS.md ADR-0010: Diese E2E-Tests laufen gegen den
// Produktions-Build (`vite build` + `vite preview`) in normalem
// Chromium — nicht gegen die kompilierte native Tauri-App. Das ist
// derselbe JS/CSS-Bundle, der auch ins Tauri-Binary eingebettet wird
// (siehe `build.rs`), aber ohne echtes `apx://`-Protokoll oder echte
// IPC — dafür sorgt `e2e/tauri-mock.ts`.
const PORT = 4173;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: "list",

  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: "on-first-retry",
    launchOptions: {
      // In der Sandbox-Umgebung (und den meisten CI-Runnern) fehlt die
      // Berechtigung für Chromes eigene Sandbox-Isolation — headless
      // Chromium startet dort nur mit --no-sandbox.
      args: ["--no-sandbox"],
      // Diese Pfadangabe ist nur nötig, wenn die vorinstallierte
      // Chromium-Revision unter `$PLAYWRIGHT_BROWSERS_PATH` nicht zur
      // gerade gepinnten `@playwright/test`-Version passt (dann würde
      // Playwright versuchen, eine neue Revision herunterzuladen, was in
      // dieser Sandbox nicht möglich ist). Auf einer normalen
      // Entwicklungsmaschine mit `playwright install` lässt sich diese
      // Zeile entfernen; sie schadet dort aber auch nicht, solange der
      // Pfad existiert.
      executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH,
    },
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  webServer: {
    command: `pnpm build && pnpm preview -- --port ${PORT} --strictPort`,
    url: `http://127.0.0.1:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
