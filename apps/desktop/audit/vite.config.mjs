// Servidor da auditoria: a interface real, com src/api.ts e as APIs Tauri substituídas.
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const realApi = path.join(root, "src/api.ts");
const fakeApi = path.join(root, "audit/api.ts");
const stubs = path.join(root, "audit/tauri-stubs.ts");

export default defineConfig({
  root,
  plugins: [
    react(),
    {
      name: "audit-api",
      enforce: "pre",
      async resolveId(source, importer, opts) {
        if (/^@tauri-apps\/(api\/(webview|window)|plugin-dialog)$/.test(source)) return stubs;
        if (!importer || importer === fakeApi || !/(^|\/)api(\.ts)?$/.test(source)) return null;
        const r = await this.resolve(source, importer, { ...opts, skipSelf: true });
        return r && r.id === realApi ? fakeApi : null;
      },
    },
  ],
  server: { port: 1430, strictPort: true },
  logLevel: "warn",
});
