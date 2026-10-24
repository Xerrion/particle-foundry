import { dirname, relative, resolve, sep } from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

function buildInput(mode: string): string | string[] {
  switch (mode) {
    case "svelte-smoke":
      return "index.html";
    case "engine-smoke":
      return ["tests/browser/engine-smoke.html", "benchmarks/gpu.html"];
    case "gpu-activity":
      return "gpu-activity.html";
    case "sentry-smoke":
      return "tests/browser/sentry-smoke.html";
    default:
      return ["index.html", "gpu.html"];
  }
}

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
      input: buildInput(mode),
    },
  },
  plugins: [tailwindcss(), svelte()],
}));
