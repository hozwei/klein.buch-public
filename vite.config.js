import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

// Entwicklung mit `pnpm dev` (Plan W9): Die SPA kommt vom Vite-Dev-Server,
// API, Dateien, Healthcheck und `/legal/` gehen an den lokalen Rust-Server
// (`klein-buch-server`). Ziel über `KLEIN_BUCH_DEV_API`, Standard
// http://127.0.0.1:8080. Der Server prüft Host und Origin gegen seine
// öffentliche Adresse — für die Entwicklung mit
// `KLEIN_BUCH_PUBLIC_URL=http://127.0.0.1:5173` und
// `KLEIN_BUCH_INSECURE_HTTP=1` starten.
const api = process.env.KLEIN_BUCH_DEV_API || "http://127.0.0.1:8080";

// https://vite.dev/config/
export default defineConfig({
  plugins: [sveltekit()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": { target: api },
      "/files": { target: api },
      "/healthz": { target: api },
      "/legal": { target: api },
    },
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
