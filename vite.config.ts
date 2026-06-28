import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: {
    rolldownOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes("/node_modules/")) return;
          if (id.includes("/node_modules/pixi.js/")) {
            if (id.includes("/lib/app/")) return "vendor-pixi-app";
            if (id.includes("/lib/assets/")) return "vendor-pixi-assets";
            if (id.includes("/lib/rendering/")) return "vendor-pixi-rendering";
            if (id.includes("/lib/scene/graphics/")) return "vendor-pixi-graphics";
            if (id.includes("/lib/scene/text/")) return "vendor-pixi-text";
            if (id.includes("/lib/scene/sprite") || id.includes("/lib/texture/")) return "vendor-pixi-sprites";
            return "vendor-pixi-core";
          }
          if (id.includes("/node_modules/svelte/")) return "vendor-svelte";
          if (id.includes("/node_modules/@lucide/svelte/")) return "vendor-icons";
          return "vendor";
        },
      },
    },
  },
  server: {
    host: "127.0.0.1",
    port: 49320,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
