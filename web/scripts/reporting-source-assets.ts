import { execFileSync } from "node:child_process";
import { lstat, readdir, readFile } from "node:fs/promises";
import { join, relative, resolve, sep } from "node:path";
import type { Plugin } from "vite";

const assetPrefix = "reporting-sources/";
const rustSourcePath = /^engine\/crates\/[\w-]+\/src\/(?:[\w-]+\/)*[\w-]+\.rs$/;
const maxSourceBytes = 128 * 1024;
const maxTotalBytes = 4 * 1024 * 1024;
const maxSourceFiles = 512;

export function affectsReportingRevision(file: string): boolean {
	return (
		/^web\/(?:src|scripts|tests\/browser)\/.+\.(?:ts|css|html)$/.test(file) ||
		/^web\/(?:vite\.config\.ts|(?:index|gpu)\.html|package\.json|bun\.lock)$/.test(file) ||
		/^engine\/(?:Cargo\.(?:toml|lock)|rust-toolchain\.toml)$/.test(file) ||
		/^engine\/crates\/[^/]+\/(?:Cargo\.toml|(?:src|shaders)\/.+\.(?:rs|wgsl))$/.test(file)
	);
}

export function reportingRevision(root: string): string {
	try {
		const git = (args: string[]) =>
			execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
		const revision = git(["rev-parse", "HEAD"]).trim();
		const changed = [
			...git(["diff", "--name-only", "HEAD", "-z"]).split("\0"),
			...git(["ls-files", "--others", "--exclude-standard", "-z"]).split("\0"),
		];
		return `${revision}${changed.some(affectsReportingRevision) ? "-dirty" : ""}`;
	} catch {
		// Docker builds omit Git metadata. Only accept an explicit immutable revision.
		const revision = process.env.VITE_GLITCHTIP_REVISION;
		return revision && /^[a-f0-9]{40}$/.test(revision) ? revision : "unknown";
	}
}

export async function readReportingSources(root: string): Promise<Map<string, string>> {
	const assets = new Map<string, string>();
	let totalBytes = 0;
	async function visit(directory: string): Promise<void> {
		for (const entry of await readdir(directory, { withFileTypes: true })) {
			const path = join(directory, entry.name);
			if (entry.isDirectory()) {
				await visit(path);
			} else if (entry.isFile()) {
				const file = relative(root, path).split(sep).join("/");
				if (!rustSourcePath.test(file)) continue;
				const metadata = await lstat(path);
				if (!metadata.isFile()) continue;
				if (metadata.size > maxSourceBytes) {
					throw new Error("Rust reporting source assets exceed the configured size limits");
				}
				const source = await readFile(path);
				totalBytes += source.byteLength;
				if (
					source.byteLength > maxSourceBytes ||
					totalBytes > maxTotalBytes ||
					assets.size >= maxSourceFiles
				) {
					throw new Error("Rust reporting source assets exceed the configured size limits");
				}
				assets.set(
					`${assetPrefix}${file}.json`,
					JSON.stringify({ file, lines: source.toString("utf8").split(/\r?\n/) }),
				);
			}
		}
	}
	const crates = resolve(root, "engine/crates");
	for (const entry of await readdir(crates, { withFileTypes: true })) {
		if (!entry.isDirectory()) continue;
		const source = join(crates, entry.name, "src");
		try {
			if (!(await lstat(source)).isDirectory()) continue;
			await visit(source);
		} catch (error) {
			if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error;
		}
	}
	return assets;
}

export function reportingSourceAssets(root: string): Plugin {
	let base = "/";
	return {
		name: "particle-foundry-reporting-sources",
		config() {
			return { define: { __PF_BUILD_REVISION__: JSON.stringify(reportingRevision(root)) } };
		},
		configResolved(config) {
			base = config.base;
		},
		async generateBundle() {
			for (const [fileName, source] of await readReportingSources(root)) {
				this.emitFile({ type: "asset", fileName, source });
			}
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const url = request.url?.split("?")[0];
				if (!url?.startsWith(`${base}${assetPrefix}`) || !url.endsWith(".json")) return next();
				const fileName = url.slice(base.length);
				if (!rustSourcePath.test(fileName.slice(assetPrefix.length, -5))) return next();
				// Requests select exact collected assets. They never become filesystem paths.
				void readReportingSources(root)
					.then((assets) => {
						const source = assets.get(fileName);
						if (source === undefined) return next();
						response.setHeader("Content-Type", "application/json; charset=utf-8");
						response.setHeader("Cache-Control", "no-store");
						response.end(source);
					})
					.catch(next);
			});
		},
	};
}
