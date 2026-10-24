// Die SPA wird statisch gebaut und vom Rust-Server ausgeliefert (ADR 0039):
// adapter-static mit Fallback auf index.html (SPA-Modus, kein SSR).
// Siehe https://svelte.dev/docs/kit/single-page-apps
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
  },
};

export default config;
