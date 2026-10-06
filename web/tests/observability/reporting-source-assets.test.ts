import { afterEach, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
	affectsReportingRevision,
	readReportingSources,
	reportingRevision,
} from "../../scripts/reporting-source-assets";

const roots: string[] = [];
afterEach(async () => {
	await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function fixture(): Promise<string> {
	const root = await mkdtemp(join(tmpdir(), "particle-reporting-assets-"));
	roots.push(root);
	return root;
}

async function source(root: string, file: string, content: string): Promise<void> {
	const path = join(root, file);
	await mkdir(join(path, ".."), { recursive: true });
	await writeFile(path, content);
}

test("source assets include only crate src files and preserve source line numbers", async () => {
	const root = await fixture();
	await source(
		root,
		"engine/crates/wasm/src/reporting.rs",
		"fn main() {\r\n    panic!();\r\n}\r\n",
	);
	await source(root, "engine/crates/wasm/examples/private.rs", "private example");
	await source(root, "engine/crates/wasm/tests/test.rs", "test source");
	await source(root, "engine/crates/wasm/src/.env", "private configuration");
	const assets = await readReportingSources(root);
	expect(assets.size).toBe(1);
	expect(
		JSON.parse(assets.get("reporting-sources/engine/crates/wasm/src/reporting.rs.json") ?? ""),
	).toEqual({
		file: "engine/crates/wasm/src/reporting.rs",
		lines: ["fn main() {", "    panic!();", "}", ""],
	});
});

test("source assets do not follow symlink files or directories", async () => {
	const root = await fixture();
	await source(root, "private/secret.rs", "private source");
	await mkdir(join(root, "engine/crates/wasm/src"), { recursive: true });
	await symlink(join(root, "private/secret.rs"), join(root, "engine/crates/wasm/src/secret.rs"));
	await symlink(join(root, "private"), join(root, "engine/crates/wasm/src/private"));
	expect((await readReportingSources(root)).size).toBe(0);
	await rm(join(root, "engine/crates/wasm/src"), { recursive: true });
	await symlink(join(root, "private"), join(root, "engine/crates/wasm/src"));
	expect((await readReportingSources(root)).size).toBe(0);
});

test("source assets reject oversized files", async () => {
	const root = await fixture();
	await source(root, "engine/crates/wasm/src/large.rs", "x".repeat(128 * 1024 + 1));
	await expect(readReportingSources(root)).rejects.toThrow("size limits");
});

test("revision changes cover authored build sources and exclude private or generated data", () => {
	for (const file of [
		"web/src/observability/reporting.ts",
		"web/scripts/reporting-source-assets.ts",
		"web/tests/browser/glitchtip-smoke.ts",
		"engine/crates/wasm/src/reporting.rs",
		"engine/crates/sim-gpu/shaders/render.wgsl",
		"web/package.json",
	]) {
		expect(affectsReportingRevision(file)).toBe(true);
	}
	for (const file of [
		"artifacts/reporting/error.json",
		"web/.env.local",
		"web/generated/wasm/particle_wasm.js",
		"engine/target/wasm32-unknown-unknown/release/particle_wasm.wasm",
		"docs/error-reporting.md",
	]) {
		expect(affectsReportingRevision(file)).toBe(false);
	}
});

test("revision distinguishes clean, tracked changes, untracked source and unrelated artifacts", async () => {
	const root = await fixture();
	const git = (args: string[]) =>
		execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
	git(["init"]);
	await source(root, "web/src/main.ts", "export const version = 1;\n");
	git(["add", "web/src/main.ts"]);
	git([
		"-c",
		"commit.gpgsign=false",
		"-c",
		"core.hooksPath=/dev/null",
		"-c",
		"user.name=Fixture",
		"-c",
		"user.email=fixture@example.invalid",
		"commit",
		"-m",
		"Fixture",
	]);
	const revision = git(["rev-parse", "HEAD"]).trim();
	expect(reportingRevision(root)).toBe(revision);
	await source(root, "artifacts/error.json", "{}");
	await source(root, "web/.env.local", "PRIVATE=value\n");
	expect(reportingRevision(root)).toBe(revision);
	await source(root, "engine/crates/wasm/src/reporting.rs", "fn reporting() {}\n");
	expect(reportingRevision(root)).toBe(`${revision}-dirty`);
	await rm(join(root, "engine"), { recursive: true });
	await source(root, "web/src/main.ts", "export const version = 2;\n");
	expect(reportingRevision(root)).toBe(`${revision}-dirty`);
});
