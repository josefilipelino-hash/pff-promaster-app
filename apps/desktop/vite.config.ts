/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Configuração recomendada pelo Tauri 2: porta fixa e sem limpar o ecrã do terminal.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "safari15",
    sourcemap: true,
  },
  test: {
    environment: "jsdom",
  },
});
