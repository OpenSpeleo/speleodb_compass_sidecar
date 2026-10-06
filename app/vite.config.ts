import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

// Runtime asset URLs include public/. Vite's default publicDir would remove it.
function existingStaticAssets(): Plugin {
  return {
    name: "sidecar-static-assets",
    configureServer(server) {
      server.middlewares.use("/about.html", (_request, response) => {
        response.setHeader("Content-Type", "text/html");
        response.end(readFileSync(resolve(import.meta.dirname, "about.html")));
      });
    },
    generateBundle() {
      this.emitFile({
        type: "asset",
        fileName: "about.html",
        source: readFileSync(resolve(import.meta.dirname, "about.html")),
      });
      for (const file of readdirSync(resolve(import.meta.dirname, "public"), {
        recursive: true,
        withFileTypes: true,
      })) {
        if (!file.isFile()) continue;
        const fullPath = resolve(file.parentPath, file.name);
        const fileName = fullPath
          .slice(resolve(import.meta.dirname).length + 1)
          .replaceAll("\\", "/");
        this.emitFile({
          type: "asset",
          fileName,
          source: readFileSync(fullPath),
        });
      }
    },
  };
}

export default defineConfig({
  plugins: [react(), existingStaticAssets()],
  publicDir: false,
  clearScreen: false,
  define: {
    __SIDECAR_API_BASE_URL__: JSON.stringify(
      process.env.SIDECAR_UI_PROFILE === "release"
        ? "https://www.speleodb.org"
        : "https://stage.speleodb.org",
    ),
  },
  server: {
    host: "localhost",
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    outDir: "dist",
    target: ["chrome105", "safari15"],
    sourcemap: false,
  },
});
