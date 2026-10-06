import { resolve } from "node:path";
import { defineConfig } from "vite";

export default defineConfig(({ mode }) => ({
	base: ["engine-smoke", "glitchtip-smoke"].includes(mode) ? `/${mode}/` : "/",
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
