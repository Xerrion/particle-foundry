import { dirname, relative, resolve, sep } from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig(({ mode }) => ({
  define: { __PF_VERIFY_LIVE__: false },
  base: ["engine-smoke", "sentry-smoke", "gpu-activity"].includes(mode)
    ? `/${mode}/`
    : "/",
  resolve: {
    alias: [
      {
        find: "$lib",
        replacement: resolve(import.meta.dirname, "src/lib"),
      },
      ...(mode === "sentry-smoke"
        ? [
            {
              find: "../../generated/wasm/particle_wasm",
              replacement: resolve(
                import.meta.dirname,
                "generated/wasm-sentry-smoke/particle_wasm.js",
              ),
            },
          ]
        : []),
    ],
  },
  build: {
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        sourcemapPathTransform: (source, mapPath) =>
          relative(
            resolve(import.meta.dirname, ".."),
            resolve(dirname(mapPath), source),
          )
            .split(sep)
            .join("/"),
      },
      input:
        mode === "svelte-smoke"
          ? "index.html"
          : mode === "engine-smoke"
            ? ["tests/browser/engine-smoke.html", "benchmarks/gpu.html"]
            : mode === "gpu-activity"
              ? "gpu-activity.html"
              : mode === "sentry-smoke"
                ? "tests/browser/sentry-smoke.html"
                : ["index.html", "gpu.html"],
    },
  },
  plugins: [tailwindcss(), svelte()],
}));
