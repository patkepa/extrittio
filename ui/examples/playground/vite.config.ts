import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

const fromRoot = (path: string) =>
  fileURLToPath(new URL(`../../${path}`, import.meta.url));

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: [
      {
        find: "@extrittio/theme/theme.css",
        replacement: fromRoot("packages/theme/src/theme.css"),
      },
      {
        find: "@extrittio/app-shell/styles.css",
        replacement: fromRoot("examples/playground/src/app-shell-source.css"),
      },
      {
        find: "@extrittio/ui/styles.css",
        replacement: fromRoot("packages/ui/src/ui.css"),
      },
      {
        find: "@extrittio/app-shell",
        replacement: fromRoot("packages/app-shell/src/index.ts"),
      },
      {
        find: "@extrittio/interactions",
        replacement: fromRoot("packages/interactions/src/index.ts"),
      },
      {
        find: "@extrittio/navigation",
        replacement: fromRoot("packages/navigation/src/index.ts"),
      },
      {
        find: "@extrittio/theme",
        replacement: fromRoot("packages/theme/src/index.ts"),
      },
      {
        find: "@extrittio/ui",
        replacement: fromRoot("packages/ui/src/index.ts"),
      },
    ],
  },
});
