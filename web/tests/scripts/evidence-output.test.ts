import { expect, test } from "bun:test";
import { lstat, mkdir, mkdtemp, realpath, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { createEvidenceDirectory } from "../../scripts/evidence-output";

test("evidence output stays inside a fresh validation run", async () => {
	const root = await mkdtemp(join(tmpdir(), "particle-evidence-"));
	try {
		await expect(
			createEvidenceDirectory(root, "artifacts/validation/../../escape", "browser"),
		).rejects.toThrow("Evidence directory must be");
		await expect(createEvidenceDirectory(root, "/tmp/escape", "browser")).rejects.toThrow(
			"Evidence directory must be",
		);
		const output = await createEvidenceDirectory(
			root,
			"artifacts/validation/browser/new-run",
			"browser",
		);
		expect(output).toBe(
			join(await realpath(root), "artifacts", "validation", "browser", "new-run"),
		);
		expect((await lstat(output)).isDirectory()).toBe(true);
		await expect(
			createEvidenceDirectory(root, "artifacts/validation/browser/new-run", "browser"),
		).rejects.toThrow();
	} finally {
		await rm(root, { recursive: true, force: true });
	}
});

test("evidence output rejects a linked category", async () => {
	const root = await mkdtemp(join(tmpdir(), "particle-evidence-"));
	const elsewhere = await mkdtemp(join(tmpdir(), "particle-evidence-outside-"));
	try {
		const validation = join(root, "artifacts", "validation");
		await mkdir(validation, { recursive: true });
		await symlink(elsewhere, join(validation, "browser"));
		await expect(
			createEvidenceDirectory(root, "artifacts/validation/browser/new-run", "browser"),
		).rejects.toThrow("symbolic link");
		await expect(lstat(join(elsewhere, "new-run"))).rejects.toThrow();
	} finally {
		await rm(root, { recursive: true, force: true });
		await rm(elsewhere, { recursive: true, force: true });
	}
});
