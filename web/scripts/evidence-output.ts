import { lstat, mkdir, realpath } from "node:fs/promises";
import { basename, join } from "node:path";

const EVIDENCE_PATH =
	/^artifacts\/validation\/([A-Za-z0-9][A-Za-z0-9_-]*)\/([A-Za-z0-9][A-Za-z0-9_-]*)$/;

/** Create a fresh evidence directory inside this checkout's validation tree. */
export async function createEvidenceDirectory(
	projectRoot: string,
	requested: string | undefined,
	defaultGroup: string,
): Promise<string> {
	const candidate = requested ?? `artifacts/validation/${defaultGroup}/${Date.now()}`;
	const match = EVIDENCE_PATH.exec(candidate);
	if (!match) {
		throw new Error("Evidence directory must be artifacts/validation/<group>/<new-run-name>");
	}
	const category = basename(match[1] ?? "");
	const run = basename(match[2] ?? "");
	const root = await realpath(projectRoot);
	const artifacts = join(root, "artifacts");
	await mkdir(artifacts, { recursive: true });
	if ((await realpath(artifacts)) !== artifacts) {
		throw new Error("Artifacts root cannot follow a link outside this checkout");
	}
	const validation = join(artifacts, "validation");
	await mkdir(validation, { recursive: true });
	if ((await realpath(validation)) !== validation) {
		throw new Error("Evidence root cannot follow a link outside this checkout");
	}
	const parent = join(validation, category);
	await mkdir(parent, { recursive: true });
	const parentInfo = await lstat(parent);
	if (
		!parentInfo.isDirectory() ||
		parentInfo.isSymbolicLink() ||
		(await realpath(parent)) !== parent
	) {
		throw new Error("Evidence group cannot be a symbolic link");
	}
	const output = join(parent, run);
	await mkdir(output); // A prior run must never be overwritten.
	return output;
}
