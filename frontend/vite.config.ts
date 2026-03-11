import { defineConfig } from "vite";

export default defineConfig({
  build: {
    target: "esnext",
  },
  server: {
    proxy: {
      "/api": "http://localhost:8080",
    },
  },
});
