import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

const repoRoot = path.resolve(__dirname, "..");
const qefroJs = path.resolve(repoRoot, "packages/qefro-js/src");

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@qefro/js/styles.css": path.join(qefroJs, "styles.css"),
      "@qefro/js": path.join(qefroJs, "index.ts"),
      react: path.resolve(__dirname, "node_modules/react"),
      "react-dom": path.resolve(__dirname, "node_modules/react-dom"),
      "react-router-dom": path.resolve(__dirname, "node_modules/react-router-dom"),
      "@testing-library/react": path.resolve(__dirname, "node_modules/@testing-library/react"),
      "@testing-library/user-event": path.resolve(__dirname, "node_modules/@testing-library/user-event"),
      "@testing-library/jest-dom": path.resolve(__dirname, "node_modules/@testing-library/jest-dom"),
    },
    dedupe: ["react", "react-dom", "react-router-dom"],
  },
  server: {
    fs: {
      allow: [repoRoot],
    },
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8080",
      "/health": "http://127.0.0.1:8080",
      "/ready": "http://127.0.0.1:8080",
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test/setup.ts",
    include: ["src/**/*.test.{ts,tsx}", "../packages/qefro-js/src/**/*.test.{ts,tsx}"],
    server: {
      deps: {
        inline: ["@qefro/js"],
      },
    },
  },
});
