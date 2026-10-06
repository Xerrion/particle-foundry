import type { ErrorEvent, StackFrame } from "@sentry/browser";

export interface SourceFrame extends StackFrame {
	source_link?: string;
}

export interface RustLocation {
	file: string;
	line: number;
	column: number;
}

export function attachSourceLink(frame: SourceFrame, file: string, revision?: string): void {
	if (!revision || !/^[a-f0-9]{40}$/.test(revision)) return;
	const path = file.split("/").map(encodeURIComponent).join("/");
	frame.source_link = `https://github.com/Xerrion/particle-foundry/blob/${revision}/${path}#L${frame.lineno}`;
}

/** Preserves the panic origin even when optional diagnostic assets cannot load. */
export function annotateRustOrigin(
	event: ErrorEvent,
	error: unknown,
	revision?: string,
): RustLocation | undefined {
	if (!(error instanceof Error) || error.name !== "RustPanic") return;
	let value: unknown;
	try {
		value = Reflect.get(error, "rustLocation");
	} catch {
		return;
	}
	if (typeof value !== "object" || value === null) return;
	const location = value as Record<string, unknown>;
	if (
		typeof location.file !== "string" ||
		!/^crates\/[a-zA-Z0-9_/-]+\/src\/[a-zA-Z0-9_/-]+\.rs$/.test(location.file) ||
		typeof location.line !== "number" ||
		!Number.isSafeInteger(location.line) ||
		location.line < 1 ||
		typeof location.column !== "number" ||
		!Number.isSafeInteger(location.column) ||
		location.column < 1
	)
		return;
	const origin = { file: `engine/${location.file}`, line: location.line, column: location.column };
	const exception = event.exception?.values?.find((value) => value.type === "RustPanic");
	if (!exception) return;
	exception.stacktrace ??= { frames: [] };
	exception.stacktrace.frames ??= [];
	if (
		!exception.stacktrace.frames.some(
			(frame) => frame.filename === origin.file && frame.lineno === origin.line,
		)
	) {
		const frame: SourceFrame = {
			filename: origin.file,
			lineno: origin.line,
			colno: origin.column,
			in_app: true,
			platform: "rust",
		};
		attachSourceLink(frame, origin.file, revision);
		exception.stacktrace.frames.push(frame);
	}
	exception.mechanism = { type: "rust-panic", handled: false };
	return origin;
}
