import { defineConfig } from "vite";

export default defineConfig({
  build: {
    target: "esnext",
    rollupOptions: {
      output: {
        manualChunks: {
          blueprint: ["@blueprintjs/core", "@blueprintjs/icons"],
          charts: ["uplot"],
          query: ["@tanstack/react-query", "axios"],
        },
      },
    },
  },
  server: {
    proxy: {
      "/api": "http://localhost:8080",
    },
  },
});
