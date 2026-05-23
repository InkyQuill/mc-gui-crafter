import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 49320,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
