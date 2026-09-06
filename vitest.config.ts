import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

// Vitest runs the React unit + frontend-integration layer. Playwright specs
// under tests/e2e are excluded (they use @playwright/test, not Vitest).
export default defineConfig({
  plugins: [react()],
  resolve: {
    // Vite 8 loads config files as ESM, where `__dirname` does not exist.
    // `import.meta.dirname` is the ESM equivalent (Node >= 20.11; CI runs 22).
    alias: { "@": path.resolve(import.meta.dirname, "./src") },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}", "tests/integration/**/*.test.{ts,tsx}"],
  },
});
