import { createHash } from "node:crypto";
import {
	copyFile,
	lstat,
	mkdir,
	mkdtemp,
	readdir,
	readFile,
	realpath,
	rename,
	rm,
	writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { createSentrySDK, type SentryOptions, type SourcemapUploadParams } from "sentry";
import { loadEnv } from "vite";

interface SourceMapClient {
	sourcemap: {
		inject(parameters: { directory: string }): Promise<unknown>;
		upload(parameters: SourcemapUploadParams): Promise<unknown>;
	};
}
type ClientFactory = (options: SentryOptions) => SourceMapClient;
interface ArtifactManifest {
	version: 1;
	release?: string;
	files: Record<string, string>;
}
export interface ReportingBuildResult {
	archivedScripts: number;
	archivedMaps: number;
	uploadedProjects: number;
	release?: string;
}
class ReportingBuildError extends Error {}
const scriptPattern = /\.(?:js|mjs|cjs)$/;
const mapPattern = /\.map(?:\.(?:gz|br))?$/;
const uuidPattern = /^[a-f\d]{8}-[a-f\d]{4}-[a-f\d]{4}-[a-f\d]{4}-[a-f\d]{12}$/i;

/** Match the production Vite build. Invoke this CLI with Bun's automatic env loading disabled. */
export function productionReportingEnvironment(directory = process.cwd()): NodeJS.ProcessEnv {
	return { ...process.env, ...loadEnv("production", directory, "VITE_") };
}

function uploadOptions(environment: NodeJS.ProcessEnv): SentryOptions[] {
	const token = environment.SENTRY_AUTH_TOKEN?.trim();
	if (!token) {
		throw new ReportingBuildError("Source map upload requires SENTRY_AUTH_TOKEN.");
	}
	const url = environment.SENTRY_URL?.trim();
	const org = environment.SENTRY_ORG?.trim();
	const project = environment.SENTRY_PROJECT?.trim();
	const simProject = environment.SENTRY_SIM_PROJECT?.trim();
	if (!url || !org || !project) {
		throw new ReportingBuildError(
			"Source map upload requires SENTRY_URL, SENTRY_ORG, and SENTRY_PROJECT.",
		);
	}
	try {
		const target = new URL(url);
		const slugs = [org, project, ...(simProject ? [simProject] : [])];
		if (
			!["http:", "https:"].includes(target.protocol) ||
			target.username ||
			target.password ||
			target.search ||
			target.hash ||
			slugs.some((slug) => !/^[a-z\d][a-z\d_-]*$/i.test(slug))
		)
			throw new Error();
	} catch {
		throw new ReportingBuildError(
			"Source map upload requires a valid instance URL and project slugs.",
		);
	}
	return [...new Set([project, ...(simProject ? [simProject] : [])])].map((slug) => ({
		token,
		url,
		org,
		project: slug,
	}));
}

async function listFiles(directory: string): Promise<string[]> {
	const files: string[] = [];
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const file = join(directory, entry.name);
		if (entry.isSymbolicLink()) throw new Error("Symbolic links are not reporting artifacts.");
		if (entry.isDirectory()) files.push(...(await listFiles(file)));
		else if (entry.isFile()) files.push(file);
	}
	return files.sort();
}

function contains(parent: string, child: string): boolean {
	const path = relative(parent, child);
	return path === "" || (!path.startsWith(`..${sep}`) && path !== ".." && !isAbsolute(path));
}

async function validateDirectories(site: string, archive: string): Promise<void> {
	await mkdir(dirname(archive), { recursive: true });
	const physicalSite = await realpath(site);
	const physicalArchive = join(
		await realpath(dirname(archive)),
		relative(dirname(archive), archive),
	);
	if (contains(physicalSite, physicalArchive) || contains(physicalArchive, physicalSite)) {
		throw new ReportingBuildError("Reporting artifacts must be outside the public site directory.");
	}
	try {
		if ((await lstat(archive)).isSymbolicLink()) throw new Error();
	} catch (error) {
		if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
			throw new ReportingBuildError("The reporting archive must be a private directory.");
		}
	}
}

async function withIsolatedSDK<T>(operation: () => Promise<T>): Promise<T> {
	const names = ["SENTRY_CONFIG_DIR", "SENTRY_CLI_NO_TELEMETRY", "DO_NOT_TRACK"] as const;
	const previous = names.map((name) => process.env[name]);
	const config = await mkdtemp(join(tmpdir(), "particle-reporting-sdk-"));
	process.env.SENTRY_CONFIG_DIR = config;
	process.env.SENTRY_CLI_NO_TELEMETRY = "1";
	process.env.DO_NOT_TRACK = "1";
	try {
		return await operation();
	} finally {
		for (const [index, name] of names.entries()) {
			if (previous[index] === undefined) delete process.env[name];
			else process.env[name] = previous[index];
		}
		await rm(config, { recursive: true, force: true });
	}
}

async function hashes(directory: string): Promise<Record<string, string>> {
	const files: Record<string, string> = {};
	for (const file of await listFiles(directory)) {
		files[relative(directory, file).split(sep).join("/")] = createHash("sha256")
			.update(await readFile(file))
			.digest("hex");
	}
	return files;
}

async function validatePairs(directory: string): Promise<number> {
	let scripts = 0;
	for (const file of await listFiles(directory)) {
		if (!scriptPattern.test(file)) continue;
		const code = await readFile(file, "utf8");
		const debugId = /(?:^|\n)\/\/# debugId=([^\r\n]+)/.exec(code)?.[1];
		const map = JSON.parse(await readFile(`${file}.map`, "utf8"));
		if (
			!debugId ||
			!uuidPattern.test(debugId) ||
			(map.debug_id ?? map.debugId) !== debugId ||
			map.version !== 3 ||
			!Array.isArray(map.sourcesContent) ||
			!map.sourcesContent.some((source: unknown) => typeof source === "string")
		)
			throw new Error();
		scripts++;
	}
	if (scripts === 0) throw new Error();
	return scripts;
}

async function readManifest(archive: string): Promise<ArtifactManifest> {
	const manifest: unknown = JSON.parse(await readFile(join(archive, "manifest.json"), "utf8"));
	if (
		typeof manifest !== "object" ||
		manifest === null ||
		!("version" in manifest) ||
		manifest.version !== 1 ||
		!("files" in manifest) ||
		typeof manifest.files !== "object" ||
		manifest.files === null ||
		Array.isArray(manifest.files) ||
		("release" in manifest && typeof manifest.release !== "string")
	)
		throw new Error();
	for (const [path, hash] of Object.entries(manifest.files)) {
		if (
			!path ||
			isAbsolute(path) ||
			path.split("/").some((part) => part === "..") ||
			path.includes("\\") ||
			typeof hash !== "string" ||
			!/^[a-f\d]{64}$/.test(hash)
		)
			throw new Error();
	}
	return manifest as ArtifactManifest;
}

/** Inject the final site output and preserve its exact scripts and maps outside the public site. */
export async function prepareReportingBuild(
	siteDirectory = "dist",
	archiveDirectory = "reporting-artifacts",
	environment: NodeJS.ProcessEnv = process.env,
	createClient: ClientFactory = createSentrySDK,
): Promise<ReportingBuildResult> {
	if (environment.SENTRY_AUTH_TOKEN?.trim()) uploadOptions(environment);
	const site = resolve(siteDirectory);
	const archive = resolve(archiveDirectory);
	await validateDirectories(site, archive);
	let archivedScripts: number;
	let archivedMaps = 0;
	const staging = await mkdtemp(join(dirname(archive), ".reporting-artifacts-"));
	try {
		await listFiles(site);
		await withIsolatedSDK(async () => {
			await createClient({ url: "http://127.0.0.1", cwd: site }).sourcemap.inject({
				directory: site,
			});
		});
		archivedScripts = await validatePairs(site);
		for (const file of await listFiles(site)) {
			if (!scriptPattern.test(file) && !mapPattern.test(file)) continue;
			const destination = join(staging, "files", relative(site, file));
			await mkdir(dirname(destination), { recursive: true });
			await copyFile(file, destination);
			if (mapPattern.test(file)) archivedMaps++;
		}
		const manifest: ArtifactManifest = {
			version: 1,
			release: environment.VITE_GLITCHTIP_RELEASE?.trim() || undefined,
			files: await hashes(join(staging, "files")),
		};
		await writeFile(join(staging, "manifest.json"), JSON.stringify(manifest, null, 2));
		await rm(archive, { recursive: true, force: true });
		await rename(staging, archive);
		for (const file of await listFiles(site)) {
			if (mapPattern.test(file)) await rm(file);
		}
	} catch {
		throw new ReportingBuildError("Source map preparation failed. Check the generated site maps.");
	} finally {
		await rm(staging, { recursive: true, force: true });
	}
	const uploadedProjects = environment.SENTRY_AUTH_TOKEN?.trim()
		? await uploadReportingArtifacts(archive, environment, createClient)
		: 0;
	return {
		archivedScripts,
		archivedMaps,
		uploadedProjects,
		release: environment.VITE_GLITCHTIP_RELEASE?.trim() || undefined,
	};
}

/** Upload an existing private archive without rebuilding the site or changing archived artifacts. */
export async function uploadReportingArtifacts(
	archiveDirectory = "reporting-artifacts",
	environment: NodeJS.ProcessEnv = process.env,
	createClient: ClientFactory = createSentrySDK,
): Promise<number> {
	const options = uploadOptions(environment);
	const archive = resolve(archiveDirectory);
	const temporary = await mkdtemp(join(tmpdir(), "particle-reporting-upload-"));
	try {
		const manifest = await readManifest(archive);
		const files = join(archive, "files");
		if (JSON.stringify(await hashes(files)) !== JSON.stringify(manifest.files)) throw new Error();
		await validatePairs(files);
		for (const file of await listFiles(files)) {
			const destination = join(temporary, relative(files, file));
			await mkdir(dirname(destination), { recursive: true });
			await copyFile(file, destination);
		}
		await withIsolatedSDK(async () => {
			for (const settings of options) {
				// SDK 0.45.0 drops bundle debug IDs with noRewrite. Existing matching IDs make injection a no-op.
				const result = await createClient({ ...settings, cwd: temporary }).sourcemap.upload({
					directory: temporary,
					release: manifest.release,
				});
				if (
					typeof result !== "object" ||
					result === null ||
					!("filesUploaded" in result) ||
					typeof result.filesUploaded !== "number" ||
					!Number.isInteger(result.filesUploaded) ||
					result.filesUploaded <= 0
				)
					throw new Error();
				if (JSON.stringify(await hashes(temporary)) !== JSON.stringify(manifest.files))
					throw new Error();
			}
		});
		return options.length;
	} catch {
		throw new ReportingBuildError(
			"Source map upload failed. Check the archive, instance settings, and build token.",
		);
	} finally {
		await rm(temporary, { recursive: true, force: true });
	}
}

if (import.meta.main) {
	try {
		const [operation = "prepare", first, second] = process.argv.slice(2);
		if (operation === "upload") {
			const projects = await uploadReportingArtifacts(first);
			console.log(`Reporting artifacts uploaded to ${projects} project(s).`);
		} else if (operation === "prepare") {
			const result = await prepareReportingBuild(first, second, productionReportingEnvironment());
			console.log(
				`Reporting artifacts prepared: ${result.archivedScripts} scripts and ${result.archivedMaps} maps. Uploaded projects: ${result.uploadedProjects}.`,
			);
		} else throw new ReportingBuildError("Use prepare or upload for reporting artifacts.");
	} catch (error) {
		console.error(
			error instanceof ReportingBuildError ? error.message : "Reporting artifact operation failed.",
		);
		process.exitCode = 1;
	}
}
