import { afterEach, expect, setDefaultTimeout, test } from "bun:test";
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { gunzipSync, inflateRawSync } from "node:zlib";
import { createSentrySDK, type SentryOptions } from "sentry";
import { SourceMapConsumer, SourceMapGenerator } from "source-map-js";
import { prepareReportingBuild, uploadReportingArtifacts } from "../../scripts/prepare-reporting";

setDefaultTimeout(15000);
const roots: string[] = [];
afterEach(async () => {
	await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function write(file: string, content: string): Promise<void> {
	await mkdir(dirname(file), { recursive: true });
	await writeFile(file, content);
}

async function fixture() {
	const root = await mkdtemp(join(tmpdir(), "particle-reporting-build-"));
	roots.push(root);
	const site = join(root, "dist");
	const archive = join(root, "private-maps");
	const script = join(site, "assets", "diagnostic.js");
	const source =
		"// Source fixture\nexport function diagnosticFailure(): never {\n  throw new Error('Synthetic diagnostic failure');\n}\n";
	const map = new SourceMapGenerator({ file: "diagnostic.js" });
	map.addMapping({
		generated: { line: 1, column: 0 },
		original: { line: 3, column: 2 },
		source: "../../src/diagnostic.ts",
		name: "diagnosticFailure",
	});
	map.setSourceContent("../../src/diagnostic.ts", source);
	await write(
		script,
		"export function diagnosticFailure(){throw new Error('Synthetic diagnostic failure')}\n//# sourceMappingURL=diagnostic.js.map\n",
	);
	await write(`${script}.map`, map.toString());
	await write(
		join(site, "styles.css.map"),
		JSON.stringify({ version: 3, sources: [], mappings: "" }),
	);
	await write(join(site, "styles.css.map.gz"), "compressed map fixture");
	return {
		root,
		site,
		archive,
		script,
		source,
		archivedScript: join(archive, "files", "assets", "diagnostic.js"),
	};
}

const uploadEnvironment = {
	SENTRY_AUTH_TOKEN: "synthetic-build-token",
	SENTRY_URL: "https://errors.example.invalid",
	SENTRY_ORG: "fixture",
	SENTRY_PROJECT: "particle-web",
	SENTRY_SIM_PROJECT: "particle-sim",
};

test("injects final JavaScript and archives matching maps with original source context", async () => {
	const build = await fixture();
	const result = await prepareReportingBuild(build.site, build.archive, {
		VITE_GLITCHTIP_RELEASE: "particle-foundry@fixture",
	});
	expect(result).toEqual({
		archivedScripts: 1,
		archivedMaps: 3,
		uploadedProjects: 0,
		release: "particle-foundry@fixture",
	});
	const code = await readFile(build.script, "utf8");
	expect(await readFile(build.archivedScript, "utf8")).toBe(code);
	const map = JSON.parse(await readFile(`${build.archivedScript}.map`, "utf8"));
	const debugId = map.debug_id ?? map.debugId;
	expect(debugId).toMatch(/^[a-f\d-]{36}$/i);
	expect(code).toContain(`//# debugId=${debugId}`);
	expect(code).toContain("_sentryDebugIds");
	expect(map.sourcesContent).toEqual([build.source]);
	const line = code.split("\n").findIndex((value) => value.includes("export function")) + 1;
	const origin = new SourceMapConsumer(map).originalPositionFor({ line, column: 0 });
	expect(origin).toEqual({
		source: "../../src/diagnostic.ts",
		line: 3,
		column: 2,
		name: "diagnosticFailure",
	});
	for (const file of [`${build.script}.map`, "styles.css.map", "styles.css.map.gz"]) {
		const path = file.startsWith(build.site) ? file : join(build.site, file);
		expect(await Bun.file(path).exists()).toBe(false);
	}
});

test("rejects an archive inside the public site before SDK execution", async () => {
	const build = await fixture();
	await expect(
		prepareReportingBuild(build.site, join(build.site, "maps"), {}, () => {
			throw new Error("SDK must not run");
		}),
	).rejects.toThrow("outside the public site");
	expect(await Bun.file(`${build.script}.map`).exists()).toBe(true);
});

test("upload-only retries preserve exact artifacts and use the archived release for both projects", async () => {
	const build = await fixture();
	await prepareReportingBuild(build.site, build.archive, {
		VITE_GLITCHTIP_RELEASE: "original-release",
	});
	const originalScript = await readFile(build.script, "utf8");
	const originalMap = await readFile(`${build.archivedScript}.map`, "utf8");
	const projects: string[] = [];
	const createClient = (options: SentryOptions) => ({
		sourcemap: {
			async inject() {
				throw new Error("Upload-only must not call inject");
			},
			async upload(parameters: { directory?: string; release?: string }) {
				projects.push(options.project ?? "");
				expect(options.token).toBe(uploadEnvironment.SENTRY_AUTH_TOKEN);
				expect(parameters.release).toBe("original-release");
				expect(parameters.directory).not.toBe(build.archive);
				expect(
					await readFile(join(parameters.directory ?? "", "assets/diagnostic.js"), "utf8"),
				).toBe(originalScript);
				expect(
					await readFile(join(parameters.directory ?? "", "assets/diagnostic.js.map"), "utf8"),
				).toBe(originalMap);
				return { filesUploaded: 1 };
			},
		},
	});
	for (let attempt = 0; attempt < 2; attempt++) {
		expect(
			await uploadReportingArtifacts(
				build.archive,
				{
					...uploadEnvironment,
					VITE_GLITCHTIP_RELEASE: "different-later-release",
				},
				createClient,
			),
		).toBe(2);
	}
	expect(projects).toEqual(["particle-web", "particle-sim", "particle-web", "particle-sim"]);
	expect(await readFile(build.script, "utf8")).toBe(originalScript);
	expect(await readFile(build.archivedScript, "utf8")).toBe(originalScript);
	expect(await readFile(`${build.archivedScript}.map`, "utf8")).toBe(originalMap);
	expect(await Bun.file(`${build.script}.map`).exists()).toBe(false);
});

test("requires explicit upload credentials and instance configuration", async () => {
	const build = await fixture();
	await expect(uploadReportingArtifacts(build.archive, {})).rejects.toThrow("SENTRY_AUTH_TOKEN");
	await expect(
		uploadReportingArtifacts(build.archive, { SENTRY_AUTH_TOKEN: "synthetic" }),
	).rejects.toThrow("SENTRY_URL, SENTRY_ORG, and SENTRY_PROJECT");
	for (const url of [
		"invalid",
		"file:///tmp/example",
		"https://user:synthetic@example.invalid",
		"https://example.invalid?token=synthetic",
	]) {
		await expect(
			uploadReportingArtifacts(build.archive, { ...uploadEnvironment, SENTRY_URL: url }),
		).rejects.toThrow("valid instance URL");
	}
});

test("a configured preparation upload failure stops the build and retains exact retry artifacts", async () => {
	const build = await fixture();
	const createClient = (options: SentryOptions) => {
		const client = createSentrySDK(options);
		return {
			sourcemap: {
				inject: client.sourcemap.inject,
				async upload() {
					throw new Error("synthetic-build-token private response");
				},
			},
		};
	};
	await expect(
		prepareReportingBuild(
			build.site,
			build.archive,
			{
				...uploadEnvironment,
				VITE_GLITCHTIP_RELEASE: "failed-upload-release",
			},
			createClient,
		),
	).rejects.toThrow("Source map upload failed");
	expect(await readFile(build.archivedScript, "utf8")).toBe(await readFile(build.script, "utf8"));
	expect(await Bun.file(`${build.script}.map`).exists()).toBe(false);
	expect(JSON.parse(await readFile(join(build.archive, "manifest.json"), "utf8")).release).toBe(
		"failed-upload-release",
	);
});

test.each([0, undefined, -1, 1.5])(
	"rejects an upload without a positive accepted file count: %j",
	async (filesUploaded) => {
		const build = await fixture();
		await prepareReportingBuild(build.site, build.archive, {});
		await expect(
			uploadReportingArtifacts(build.archive, uploadEnvironment, () => ({
				sourcemap: {
					async inject() {},
					async upload() {
						return { filesUploaded };
					},
				},
			})),
		).rejects.toThrow("Source map upload failed");
	},
);

test("rejects changed copies and hides SDK error details while retaining the retry archive", async () => {
	const build = await fixture();
	await prepareReportingBuild(build.site, build.archive, {});
	const before = await readFile(build.archivedScript, "utf8");
	for (const mutate of [false, true]) {
		await expect(
			uploadReportingArtifacts(build.archive, uploadEnvironment, () => ({
				sourcemap: {
					async inject() {},
					async upload(parameters) {
						if (!mutate) throw new Error("synthetic-build-token private API response");
						await write(join(parameters.directory ?? "", "assets/diagnostic.js"), "changed script");
						return { filesUploaded: 1 };
					},
				},
			})),
		).rejects.toThrow(
			"Source map upload failed. Check the archive, instance settings, and build token.",
		);
		expect(await readFile(build.archivedScript, "utf8")).toBe(before);
	}
	await write(build.archivedScript, "tampered archive");
	let created = false;
	await expect(
		uploadReportingArtifacts(build.archive, uploadEnvironment, () => {
			created = true;
			throw new Error();
		}),
	).rejects.toThrow("Source map upload failed");
	expect(created).toBe(false);
});

test.each([false, true])(
	"isolates SDK configuration and restores environment after injection failure = %j",
	async (fail) => {
		const build = await fixture();
		const previous = {
			config: process.env.SENTRY_CONFIG_DIR,
			telemetry: process.env.SENTRY_CLI_NO_TELEMETRY,
			tracking: process.env.DO_NOT_TRACK,
		};
		const originalConfig = join(build.root, "existing-config");
		await write(join(originalConfig, "cli.db"), "existing private state");
		process.env.SENTRY_CONFIG_DIR = originalConfig;
		let temporaryConfig = "";
		try {
			const operation = prepareReportingBuild(build.site, build.archive, {}, (options) => {
				expect(options.url).toBe("http://127.0.0.1");
				expect(options.token).toBeUndefined();
				temporaryConfig = process.env.SENTRY_CONFIG_DIR ?? "";
				expect(temporaryConfig).not.toBe(originalConfig);
				expect(process.env.SENTRY_CLI_NO_TELEMETRY).toBe("1");
				expect(process.env.DO_NOT_TRACK).toBe("1");
				const client = createSentrySDK(options);
				return {
					sourcemap: {
						async inject(parameters) {
							await access(temporaryConfig);
							if (fail) throw new Error("private injection detail");
							return client.sourcemap.inject(parameters);
						},
						async upload() {
							throw new Error("Unexpected upload");
						},
					},
				};
			});
			if (fail) await expect(operation).rejects.toThrow("Source map preparation failed");
			else await operation;
			expect(process.env.SENTRY_CONFIG_DIR).toBe(originalConfig);
			expect(process.env.SENTRY_CLI_NO_TELEMETRY).toBe(previous.telemetry);
			expect(process.env.DO_NOT_TRACK).toBe(previous.tracking);
			expect(await Bun.file(temporaryConfig).exists()).toBe(false);
			expect(await readFile(join(originalConfig, "cli.db"), "utf8")).toBe("existing private state");
		} finally {
			if (previous.config === undefined) delete process.env.SENTRY_CONFIG_DIR;
			else process.env.SENTRY_CONFIG_DIR = previous.config;
		}
	},
);

function bundleEntries(bundle: Buffer): Map<string, Buffer> {
	const entries = new Map<string, Buffer>();
	let offset = bundle.subarray(0, 4).toString() === "SYSB" ? 8 : 0;
	while (bundle.readUInt32LE(offset) === 0x04034b50) {
		const method = bundle.readUInt16LE(offset + 8);
		const size = bundle.readUInt32LE(offset + 18);
		const nameLength = bundle.readUInt16LE(offset + 26);
		const extraLength = bundle.readUInt16LE(offset + 28);
		const name = bundle.subarray(offset + 30, offset + 30 + nameLength).toString();
		const start = offset + 30 + nameLength + extraLength;
		const compressed = bundle.subarray(start, start + size);
		entries.set(name, method === 8 ? inflateRawSync(compressed) : compressed);
		offset = start + size;
	}
	return entries;
}

test("the real SDK uploads unchanged files with matching debug-ID bundle headers", async () => {
	const build = await fixture();
	await prepareReportingBuild(build.site, build.archive, {
		VITE_GLITCHTIP_RELEASE: "real-sdk-release",
	});
	const script = await readFile(build.archivedScript);
	const map = await readFile(`${build.archivedScript}.map`);
	const debugId = JSON.parse(map.toString()).debug_id;
	const chunks = new Map<string, Buffer>();
	const bundles: Buffer[] = [];
	let base = "";
	const server = Bun.serve({
		hostname: "127.0.0.1",
		port: 0,
		async fetch(request) {
			const path = new URL(request.url).pathname;
			if (path.endsWith("/chunk-upload/") && request.method === "GET") {
				return Response.json({
					url: `${base}/chunks`,
					chunkSize: 32 * 1024 * 1024,
					chunksPerRequest: 64,
					maxRequestSize: 32 * 1024 * 1024,
					hashAlgorithm: "sha1",
					concurrency: 1,
					compression: ["gzip"],
				});
			}
			if (path === "/chunks") {
				const form = await request.formData();
				const chunk = form.get("file_gzip");
				if (!(chunk instanceof File)) return new Response("Missing chunk", { status: 400 });
				chunks.set(chunk.name, gunzipSync(Buffer.from(await chunk.arrayBuffer())));
				return Response.json({});
			}
			if (path.endsWith("/artifactbundle/assemble/")) {
				const payload = (await request.json()) as { chunks: string[]; version: string };
				const missingChunks = payload.chunks.filter((hash) => !chunks.has(hash));
				if (missingChunks.length) return Response.json({ state: "not_found", missingChunks });
				bundles.push(
					Buffer.concat(payload.chunks.map((hash) => chunks.get(hash) ?? Buffer.alloc(0))),
				);
				return Response.json({ state: "ok" });
			}
			return Response.json({
				slug: "sdk-fixture",
				name: "SDK fixture",
				links: { regionUrl: base },
			});
		},
	});
	base = `http://127.0.0.1:${server.port}`;
	try {
		expect(
			await uploadReportingArtifacts(build.archive, {
				...uploadEnvironment,
				SENTRY_URL: base,
				SENTRY_ORG: "sdk-fixture",
				SENTRY_SIM_PROJECT: undefined,
				VITE_GLITCHTIP_RELEASE: "wrong-release",
			}),
		).toBe(1);
		expect(bundles).toHaveLength(1);
		const entries = bundleEntries(bundles[0]);
		const manifest = JSON.parse(entries.get("manifest.json")?.toString() ?? "");
		expect(manifest.release).toBe("real-sdk-release");
		const uploadedFiles = Object.entries(manifest.files) as [
			string,
			{ headers: Record<string, string> },
		][];
		expect(uploadedFiles).toHaveLength(2);
		for (const [, file] of uploadedFiles) expect(file.headers["debug-id"]).toBe(debugId);
		expect(entries.get("_/_/assets/diagnostic.js")).toEqual(script);
		expect(entries.get("_/_/assets/diagnostic.js.map")).toEqual(map);
		expect(await readFile(build.archivedScript)).toEqual(script);
		expect(await readFile(build.script)).toEqual(script);
	} finally {
		await server.stop(true);
	}
});
