import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue(), wasm()],
  build: {
    target: "esnext",
  },
});
