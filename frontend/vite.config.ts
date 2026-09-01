/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Siehe https://v2.tauri.app/start/frontend/vite/ — diese Einstellungen
// sind von Tauri vorgeschrieben, damit `pnpm tauri dev` zuverlässig
// funktioniert (fester Port, damit das Rust-Backend das Frontend findet;
// clearScreen: false, damit Rust-Fehler in der Konsole sichtbar bleiben).
export default defineConfig({
  plugins: [react(), tailwindcss()],

  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      // Das Rust-Backend liegt außerhalb von frontend/ (crates/apx-app) —
      // trotzdem vorsichtshalber ausschließen, falls das Cargo-Target-
      // Verzeichnis je hierher verlinkt wird.
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    // E2E-Tests (Playwright, siehe frontend/e2e/) laufen über ein
    // eigenes Kommando, nicht über Vitest.
    exclude: ["e2e/**", "node_modules/**"],
  },
});
