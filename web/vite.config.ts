import { defineConfig } from "vite";

export default defineConfig(({ mode }) => ({
	base: mode === "engine-smoke" ? "/engine-smoke/" : "/",
	build: {
		outDir: "dist",
		emptyOutDir: true,
		rollupOptions:
			mode === "engine-smoke"
				? { input: "tests/browser/engine-smoke.html" }
				: { input: ["index.html", "gpu.html"] },
	},
}));
