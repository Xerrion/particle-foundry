import { afterEach, expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { build } from "vite";
import { productionReportingEnvironment } from "../../scripts/prepare-reporting";

const roots: string[] = [];
const originalRelease = process.env.VITE_GLITCHTIP_RELEASE;
afterEach(async () => {
	if (originalRelease === undefined) delete process.env.VITE_GLITCHTIP_RELEASE;
	else process.env.VITE_GLITCHTIP_RELEASE = originalRelease;
	await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function fixture(): Promise<string> {
	const root = await mkdtemp(join(tmpdir(), "particle-reporting-env-"));
	roots.push(root);
	delete process.env.VITE_GLITCHTIP_RELEASE;
	for (const [file, release] of [
		[".env", "base-release"],
		[".env.local", "local-release"],
		[".env.production", "production-release"],
		[".env.production.local", "production-local-release"],
	])
		await writeFile(join(root, file), `VITE_GLITCHTIP_RELEASE=${release}\n`);
	return root;
}

test("production preparation uses Vite mode-file precedence", async () => {
	const root = await fixture();
	expect(productionReportingEnvironment(root).VITE_GLITCHTIP_RELEASE).toBe(
		"production-local-release",
	);
	await rm(join(root, ".env.production.local"));
	expect(productionReportingEnvironment(root).VITE_GLITCHTIP_RELEASE).toBe("production-release");
	await rm(join(root, ".env.production"));
	expect(productionReportingEnvironment(root).VITE_GLITCHTIP_RELEASE).toBe("local-release");
	await rm(join(root, ".env.local"));
	expect(productionReportingEnvironment(root).VITE_GLITCHTIP_RELEASE).toBe("base-release");
	process.env.VITE_GLITCHTIP_RELEASE = "explicit-process-release";
	expect(productionReportingEnvironment(root).VITE_GLITCHTIP_RELEASE).toBe(
		"explicit-process-release",
	);
});

test.each([undefined, "explicit-process-release"])(
	"the CLI archives the release compiled by the production Vite build with process override %j",
	async (processRelease) => {
		const root = await fixture();
		if (processRelease) process.env.VITE_GLITCHTIP_RELEASE = processRelease;
		await mkdir(join(root, "src"));
		await writeFile(
			join(root, "src/entry.ts"),
			"export const compiledRelease = import.meta.env.VITE_GLITCHTIP_RELEASE;\n",
		);
		await build({
			root,
			configFile: false,
			logLevel: "silent",
			build: {
				sourcemap: true,
				minify: false,
				lib: { entry: join(root, "src/entry.ts"), formats: ["es"], fileName: "entry" },
			},
		});
		const environment: NodeJS.ProcessEnv = { ...process.env, SENTRY_AUTH_TOKEN: undefined };
		if (!processRelease) delete environment.VITE_GLITCHTIP_RELEASE;
		const child = Bun.spawn(
			[
				process.execPath,
				"--no-env-file",
				resolve(import.meta.dir, "../../scripts/prepare-reporting.ts"),
				"prepare",
				"dist",
				"private-artifacts",
			],
			{ cwd: root, env: environment, stdout: "pipe", stderr: "pipe" },
		);
		const [exitCode, errors] = await Promise.all([child.exited, new Response(child.stderr).text()]);
		expect(errors).toBe("");
		expect(exitCode).toBe(0);
		const release = processRelease ?? "production-local-release";
		const manifest = JSON.parse(
			await readFile(join(root, "private-artifacts/manifest.json"), "utf8"),
		);
		expect(manifest.release).toBe(release);
		const finalScript = await readFile(join(root, "dist/entry.mjs"), "utf8");
		expect(finalScript).toContain(JSON.stringify(release));
		expect(finalScript).toContain("_sentryDebugIds");
	},
);
