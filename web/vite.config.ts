import { resolve } from "node:path";
import { defineConfig } from "vite";
import { reportingSourceAssets } from "./scripts/reporting-source-assets.ts";

export default defineConfig(({ mode }) => ({
	base: ["engine-smoke", "glitchtip-smoke"].includes(mode) ? `/${mode}/` : "/",
	plugins: [reportingSourceAssets(resolve(import.meta.dirname, ".."))],
	resolve: {
		alias:
			mode === "glitchtip-smoke"
				? [
						{
							find: "../../generated/wasm/particle_wasm",
							replacement: resolve(
								import.meta.dirname,
								"generated/wasm-glitchtip-smoke/particle_wasm.js",
							),
						},
					]
				: [],
	},
	build: {
		sourcemap: true,
		outDir: "dist",
		emptyOutDir: true,
		rollupOptions:
			mode === "engine-smoke"
				? { input: "tests/browser/engine-smoke.html" }
				: mode === "glitchtip-smoke"
					? { input: "tests/browser/glitchtip-smoke.html" }
					: { input: ["index.html", "gpu.html"] },
	},
}));
