/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import fs from "fs";

const realRoot = fs
  .realpathSync(__dirname)
  .replace(/^[a-z]:/, (m) => m.toUpperCase())
  .replace(/\\/g, "/");

export default defineConfig({
  root: realRoot,
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true,
    fs: {
      strict: false,
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  test: {
    globals: true,
    environment: "jsdom",
  },
});
