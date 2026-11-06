import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

// https://vite.dev/config/
export default defineConfig({
  // GitHub Pages serves this project from https://<user>.github.io/hermeneus/.
  base: process.env.GITHUB_ACTIONS ? "/hermeneus/" : "/",
  plugins: [vue(), wasm()],
  build: {
    target: "esnext",
  },
});
